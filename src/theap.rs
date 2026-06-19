//! Mechanical translation of the early `src/theap.c` helper slice.

#![allow(non_camel_case_types)]

use core::{ffi::c_void, mem::size_of, ptr};

use crate::arena::_mi_arenas_collect;
use crate::atomic::{mi_lock_acquire, mi_lock_release};
use crate::bits::{MI_INTPTR_BITS, MI_SIZE_BITS, mi_clz, mi_ctz};
use crate::core_types::{
    BlockVisitCallback, BlockVisitCallbackArg, BorrowedHeap, Errno, HeapPtr, PagePtr, PageQueuePtr,
    ThreadHeapLocal, ThreadHeapPtr, ThreadHeapState, TldPtr,
};
use crate::internal::{
    _mi_align_up, _mi_arenas_alloc, _mi_arenas_free, _mi_deferred_free, _mi_divide_up,
    _mi_is_heap_main, _mi_memcpy_aligned, _mi_meta_free, _mi_meta_zalloc, _mi_subproc,
    _mi_theap_guarded_init, _mi_theap_heap, _mi_theap_options_init, mi_memid_t, mi_page_all_free,
    mi_page_area, mi_page_block_size, mi_page_block_size_typed, mi_page_heap, mi_page_start,
    mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::mimalloc::mi_free;
use crate::options::_mi_error_message_fmt;
use crate::page::{
    _mi_page_abandon, _mi_page_free, _mi_page_free_collect, _mi_page_is_valid,
    _mi_theap_collect_retired, mi_full_page_queue_is_valid, mi_huge_page_queue_is_valid,
    mi_normal_page_queue_is_valid, mi_page_is_abandoned, mi_page_theap,
};
use crate::prim::{__mi_theap_empty, _mi_theap_default};
use crate::random::{_mi_random_init, _mi_random_next, _mi_random_split};
use crate::stats::{__mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_stats_merge_into};
use crate::types::{
    MI_ARENA_MIN_OBJ_SIZE, MI_BIN_COUNT, MI_BIN_FULL, MI_BIN_HUGE, MI_SMALL_PAGE_SIZE,
    mi_block_visit_fun, mi_heap_area_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_theap_t,
    mi_tld_t,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum mi_collect_t {
    MI_NORMAL = 0,
    MI_FORCE = 1,
    MI_ABANDON = 2,
}

pub const MI_MAX_BLOCKS: usize = MI_SMALL_PAGE_SIZE / core::mem::size_of::<*const c_void>();

/// # Safety
///
/// The callback receives allocator-owned thread-heap, queue, and page pointers
/// that must remain valid for the duration of the visit.
pub type theap_page_visitor_fun = unsafe extern "C" fn(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool;

#[derive(Clone, Copy)]
struct TheapPageVisitCallback(theap_page_visitor_fun);

impl TheapPageVisitCallback {
    #[inline(always)]
    fn new(visitor: Option<theap_page_visitor_fun>) -> Option<Self> {
        visitor.map(Self)
    }

    #[inline(always)]
    unsafe fn invoke(
        self,
        theap: ThreadHeapPtr,
        queue: PageQueuePtr,
        page: PagePtr,
        state: TheapPageVisitState,
    ) -> bool {
        unsafe {
            (self.0)(
                theap.as_ptr(),
                queue.as_ptr(),
                page.as_ptr(),
                state.arg1,
                state.arg2,
            )
        }
    }
}

#[derive(Clone, Copy)]
struct TheapPageVisitState {
    visitor: Option<TheapPageVisitCallback>,
    arg1: *mut c_void,
    arg2: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct mi_theap_area_ex_s {
    pub area: mi_heap_area_t,
    pub page: *mut mi_page_t,
}

pub type mi_theap_area_ex_t = mi_theap_area_ex_s;

/// # Safety
///
/// The callback receives allocator-owned thread-heap and area pointers that
/// must remain valid for the duration of the visit.
pub type mi_theap_area_visit_fun = unsafe extern "C" fn(
    theap: *const mi_theap_t,
    area: *const mi_theap_area_ex_t,
    arg: *mut c_void,
) -> bool;

#[derive(Clone, Copy)]
struct TheapAreaVisitCallback(mi_theap_area_visit_fun);

impl TheapAreaVisitCallback {
    #[inline(always)]
    unsafe fn from_raw_slot(vfun: *mut c_void) -> Self {
        unsafe { Self(*vfun.cast::<mi_theap_area_visit_fun>()) }
    }

    #[inline(always)]
    unsafe fn invoke(
        self,
        theap: ThreadHeapPtr,
        area: *const mi_theap_area_ex_t,
        arg: *mut c_void,
    ) -> bool {
        unsafe { (self.0)(theap.as_ptr().cast_const(), area, arg) }
    }
}

#[repr(C)]
struct mi_visit_blocks_args_s {
    visit_blocks: bool,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
}

type mi_visit_blocks_args_t = mi_visit_blocks_args_s;

/// # Safety
///
/// `area` must point to writable heap-area storage and `page` must point to a
/// valid page owned by this allocator.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_area_init(
    area: *mut mi_heap_area_t,
    page: *mut mi_page_t,
) {
    let page = unsafe { PagePtr::new_unchecked(page) };
    let heap = unsafe { HeapPtr::new_unchecked(mi_page_heap(page.as_ptr())) };
    unsafe { mi_heap_area_init_typed(area, heap, page) };
}

unsafe fn mi_heap_area_init_typed(area: *mut mi_heap_area_t, heap: HeapPtr, page: PagePtr) {
    unsafe {
        crate::MI_UNUSED!(heap);
        let page = page.as_ptr();
        let full_bsize =
            crate::core_types::FullBlockSize::from_block_size(mi_page_block_size_typed(page));
        let bsize = full_bsize.get();
        let ubsize = mi_page_usable_block_size(page);
        let reserved = crate::page::mi_page_reserved_count(page);
        let capacity = crate::page::mi_page_capacity_count(page);
        let used = crate::page::mi_page_used_count(page);
        (*area).reserved = reserved.as_usize() * bsize;
        (*area).committed = capacity.as_usize() * bsize;
        (*area).blocks = mi_page_start(page).cast::<c_void>();
        (*area).used = used.as_usize();
        (*area).block_size = ubsize.get();
        (*area).full_block_size = full_bsize.get();
        (*area).reserved1 = page.cast::<c_void>();
    }
}

unsafe fn mi_get_fast_divisor(divisor: usize, magic: *mut u64, shift: *mut usize) {
    unsafe {
        crate::mi_assert_internal!(divisor > 0 && u32::try_from(divisor).is_ok());
        *shift = MI_SIZE_BITS - mi_clz(divisor - 1);
        *magic = (((1u64 << 32) * ((1u64 << *shift) - divisor as u64)) / divisor as u64) + 1;
    }
}

/// # Safety
///
/// `magic` and `shift` must come from `mi_get_fast_divisor` for the divisor
/// used to derive `n`'s quotient.
#[inline]
pub unsafe fn mi_fast_divide(n: usize, magic: u64, shift: usize) -> usize {
    crate::mi_assert_internal!(u32::try_from(n).is_ok());
    let hi = ((n as u64) * magic) >> 32;
    ((hi + n as u64) >> shift) as usize
}

/// # Safety
///
/// `area` and `page` must describe the same valid allocator page. `visitor`
/// must be valid to call for each live block in the page.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_area_visit_blocks(
    area: *const mi_heap_area_t,
    page: *mut mi_page_t,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
) -> bool {
    let page = unsafe { PagePtr::new_unchecked(page) };
    if unsafe { mi_page_is_abandoned(page.as_ptr()) } {
        return true;
    }
    let theap = unsafe { ThreadHeapPtr::new_unchecked(mi_page_theap(page.as_ptr())) };
    unsafe { mi_theap_area_visit_blocks_typed(area, theap, page, visitor, arg) }
}

unsafe fn mi_theap_area_visit_blocks_typed(
    area: *const mi_heap_area_t,
    theap: ThreadHeapPtr,
    page: PagePtr,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
) -> bool {
    unsafe {
        crate::mi_assert_internal!(!area.is_null());
        if area.is_null() {
            return true;
        }
        let page = page.as_ptr();
        crate::mi_assert_internal!(mi_page_theap(page) == theap.as_ptr());

        _mi_page_free_collect(page, true);
        crate::mi_assert_internal!((*page).local_free.is_null());
        let used = crate::page::mi_page_used_count(page);
        if used.get() == 0 {
            return true;
        }

        let mut psize = 0usize;
        let pstart = mi_page_area(page, &mut psize);
        let heap = HeapPtr::new_unchecked(mi_page_heap(page));
        let bsize = mi_page_block_size(page);
        let ubsize = mi_page_usable_block_size(page);

        let capacity = crate::page::mi_page_capacity_count(page);
        if capacity.get() == 1 {
            crate::mi_assert_internal!(used.get() == 1 && (*page).free.is_null());
            return visitor.invoke(heap, area, pstart.cast::<c_void>(), ubsize.get(), arg);
        }
        crate::mi_assert_internal!(u32::try_from(bsize).is_ok());

        if used.get() == capacity.get() {
            let mut block = pstart;
            for _ in 0..capacity.as_usize() {
                if !visitor.invoke(heap, area, block.cast::<c_void>(), ubsize.get(), arg) {
                    return false;
                }
                block = block.add(bsize);
            }
            return true;
        }

        let mut free_map = [0usize; MI_MAX_BLOCKS / MI_INTPTR_BITS];
        let bmapsize = _mi_divide_up(capacity.as_usize(), MI_INTPTR_BITS);
        if !capacity.as_usize().is_multiple_of(MI_INTPTR_BITS) {
            let shift = capacity.as_usize() % MI_INTPTR_BITS;
            free_map[bmapsize - 1] = usize::MAX << shift;
        }

        let mut magic = 0u64;
        let mut shift = 0usize;
        mi_get_fast_divisor(bsize, &mut magic, &mut shift);

        let mut free_count = 0usize;
        let mut free = (*page).free;
        while !free.is_null() {
            free_count += 1;
            let block = free;
            crate::mi_assert_internal!(
                block.cast::<u8>() >= pstart && block.cast::<u8>() < pstart.add(psize)
            );
            let offset = block.cast::<u8>() as usize - pstart as usize;
            crate::mi_assert_internal!(offset.is_multiple_of(bsize));
            crate::mi_assert_internal!(u32::try_from(offset).is_ok());
            let blockidx = mi_fast_divide(offset, magic, shift);
            crate::mi_assert_internal!(blockidx == offset / bsize);
            crate::mi_assert_internal!(blockidx < MI_MAX_BLOCKS);
            let bitidx = blockidx / MI_INTPTR_BITS;
            let bit = blockidx - (bitidx * MI_INTPTR_BITS);
            free_map[bitidx] |= 1usize << bit;
            free = crate::internal::mi_block_next(page, block);
        }
        crate::mi_assert_internal!(capacity.as_usize() == free_count + used.as_usize());

        let mut block = pstart;
        for mask in free_map.iter().take(bmapsize) {
            if *mask == 0 {
                for _ in 0..MI_INTPTR_BITS {
                    if !visitor.invoke(heap, area, block.cast::<c_void>(), ubsize.get(), arg) {
                        return false;
                    }
                    block = block.add(bsize);
                }
            } else {
                let mut used_mask = !*mask;
                while used_mask != 0 {
                    let bitidx = mi_ctz(used_mask);
                    if !visitor.invoke(
                        heap,
                        area,
                        block.add(bitidx * bsize).cast::<c_void>(),
                        ubsize.get(),
                        arg,
                    ) {
                        return false;
                    }
                    used_mask &= used_mask - 1;
                }
                block = block.add(bsize * MI_INTPTR_BITS);
            }
        }
        true
    }
}

#[used]
static MI_THEAP_AREA_VISIT_BLOCKS_KEEP: unsafe extern "C" fn(
    *const mi_heap_area_t,
    *mut mi_page_t,
    BlockVisitCallback,
    BlockVisitCallbackArg,
) -> bool = _mi_theap_area_visit_blocks;

unsafe extern "C" fn mi_theap_visit_areas_page(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    vfun: *mut c_void,
    arg: *mut c_void,
) -> bool {
    unsafe {
        crate::MI_UNUSED!(pq);
        let theap = ThreadHeapPtr::new_unchecked(theap);
        let page = PagePtr::new_unchecked(page);
        let visitor = TheapAreaVisitCallback::from_raw_slot(vfun);
        mi_theap_visit_areas_page_typed(theap, page, visitor, arg)
    }
}

unsafe fn mi_theap_visit_areas_page_typed(
    theap: ThreadHeapPtr,
    page: PagePtr,
    visitor: TheapAreaVisitCallback,
    arg: *mut c_void,
) -> bool {
    unsafe {
        crate::mi_assert_internal!(mi_page_theap(page.as_ptr()) == theap.as_ptr());
        let page = page.as_ptr();
        let mut xarea = mi_theap_area_ex_t {
            page,
            ..mi_theap_area_ex_t::default()
        };
        _mi_heap_area_init(ptr::addr_of_mut!(xarea.area), page);
        visitor.invoke(theap, ptr::addr_of!(xarea), arg)
    }
}

unsafe fn mi_theap_visit_areas(
    theap: *const mi_theap_t,
    visitor: Option<mi_theap_area_visit_fun>,
    arg: *mut c_void,
) -> bool {
    unsafe {
        let Some(visitor) = visitor else {
            return false;
        };
        let mut visitor = visitor;
        mi_theap_visit_pages(
            theap.cast_mut(),
            Some(mi_theap_visit_areas_page),
            true,
            ptr::addr_of_mut!(visitor).cast(),
            arg,
        )
    }
}

#[inline(never)]
unsafe extern "C" fn mi_theap_area_visitor(
    theap: *const mi_theap_t,
    xarea: *const mi_theap_area_ex_t,
    arg: *mut c_void,
) -> bool {
    unsafe {
        let args = arg.cast::<mi_visit_blocks_args_t>();
        if !(*args).visitor.invoke(
            HeapPtr::new_unchecked(_mi_theap_heap(theap.cast_mut())),
            ptr::addr_of!((*xarea).area),
            ptr::null_mut(),
            (*xarea).area.block_size,
            (*args).arg,
        ) {
            return false;
        }
        if (*args).visit_blocks {
            _mi_theap_area_visit_blocks(
                ptr::addr_of!((*xarea).area),
                (*xarea).page,
                (*args).visitor,
                (*args).arg,
            )
        } else {
            true
        }
    }
}

#[used]
static MI_THEAP_AREA_VISITOR_KEEP: mi_theap_area_visit_fun = mi_theap_area_visitor;

/// # Safety
///
/// `theap`, when non-null, must point to a valid thread heap. `visitor`, when
/// present, must be valid for every visited heap area and block.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_visit_blocks(
    theap: *const mi_theap_t,
    visit_blocks: bool,
    visitor: Option<mi_block_visit_fun>,
    arg: *mut c_void,
) -> bool {
    unsafe {
        let Some(visitor) = visitor else {
            return false;
        };
        let mut args = mi_visit_blocks_args_t {
            visit_blocks,
            visitor: BlockVisitCallback::new(Some(visitor)),
            arg: BlockVisitCallbackArg::from_raw(arg),
        };
        mi_theap_visit_areas(
            theap,
            Some(mi_theap_area_visitor),
            ptr::addr_of_mut!(args).cast::<c_void>(),
        )
    }
}

/// # Safety
///
/// `theap` must point to a valid initialized thread heap. `fn_page`, when
/// present, must be valid for every page visited from that heap.
pub unsafe fn mi_theap_visit_pages(
    theap: *mut mi_theap_t,
    fn_page: Option<theap_page_visitor_fun>,
    include_full: bool,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    unsafe {
        let Some(theap) = ThreadHeapPtr::new(theap) else {
            return false;
        };
        let state = TheapPageVisitState {
            visitor: TheapPageVisitCallback::new(fn_page),
            arg1,
            arg2,
        };
        mi_theap_visit_pages_typed(theap, include_full, state)
    }
}

unsafe fn mi_theap_visit_pages_typed(
    theap: ThreadHeapPtr,
    include_full: bool,
    state: TheapPageVisitState,
) -> bool {
    unsafe {
        let theap_ptr = theap.as_ptr();
        if (*theap_ptr).page_count == 0 {
            return false;
        }
        let mut count = 0usize;
        let total = (*theap_ptr).page_count;
        let max_bin = if include_full {
            MI_BIN_FULL
        } else {
            MI_BIN_FULL - 1
        };

        for i in 0..=max_bin {
            let pq = core::ptr::addr_of_mut!((*theap_ptr).pages[i]);
            let queue = PageQueuePtr::new_unchecked(pq);
            let mut page = queue.first();
            while let Some(current) = page {
                let current_ptr = current.as_ptr();
                let next = PagePtr::new((*current_ptr).next);
                crate::mi_assert_internal!(mi_page_theap(current_ptr) == theap_ptr);
                if crate::internal::MI_DEBUG > 1 {
                    count += 1;
                }
                if let Some(visitor) = state.visitor
                    && !visitor.invoke(theap, queue, current, state)
                {
                    return false;
                }
                page = next;
            }
        }

        crate::mi_assert_internal!(
            !include_full || crate::internal::MI_DEBUG <= 1 || count == total
        );
        true
    }
}

/// # Safety
///
/// `theap`, `pq`, and `page` must be valid allocator objects from the same
/// thread heap.
pub unsafe extern "C" fn mi_theap_page_is_valid(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    unsafe {
        crate::MI_UNUSED!(arg1);
        crate::MI_UNUSED!(arg2);
        crate::MI_UNUSED!(pq);
        let theap = ThreadHeapPtr::new_unchecked(theap);
        let page = PagePtr::new_unchecked(page);
        mi_theap_page_is_valid_typed(theap, page)
    }
}

unsafe fn mi_theap_page_is_valid_typed(theap: ThreadHeapPtr, page: PagePtr) -> bool {
    unsafe {
        let page = page.as_ptr();
        crate::mi_assert_internal!(mi_page_theap(page) == theap.as_ptr());
        crate::mi_assert_expensive!(_mi_page_is_valid(page));
        true
    }
}

/// # Safety
///
/// `theap`, `pq`, and `page` must be valid allocator objects from the same
/// thread heap. `arg_collect` must point to a valid `mi_collect_t` value.
pub unsafe extern "C" fn mi_theap_page_collect(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg_collect: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    unsafe {
        crate::MI_UNUSED!(arg2);
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        let queue = PageQueuePtr::new_unchecked(pq);
        let page = PagePtr::new_unchecked(page);
        let collect = *arg_collect.cast::<mi_collect_t>();
        mi_theap_page_collect_typed(theap, queue, page, collect)
    }
}

unsafe fn mi_theap_page_collect_typed(
    theap: ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
    collect: mi_collect_t,
) -> bool {
    unsafe {
        crate::mi_assert_internal!(mi_theap_page_is_valid_typed(theap.ptr(), page));

        let local_page = crate::core_types::LocalPage::new_unchecked(page);
        let page = local_page.page().as_ptr();
        let pq = queue.as_ptr();
        _mi_page_free_collect(page, collect as i32 >= mi_collect_t::MI_FORCE as i32);

        if mi_page_all_free(page) {
            if crate::page::mi_page_is_in_full(page) {
                let full_page = crate::core_types::FullPage::new_unchecked(local_page.page());
                crate::mi_assert_internal!(full_page.mark_all_free().is_some());
            } else {
                let active_page = crate::core_types::ActivePage::new_unchecked(local_page.page());
                crate::mi_assert_internal!(active_page.mark_all_free().is_some());
            }
            if collect as i32 >= mi_collect_t::MI_FORCE as i32
                || crate::page::mi_page_retire_expire(page).is_zero()
            {
                _mi_page_free(page, pq);
            }
        } else if collect == mi_collect_t::MI_ABANDON {
            let _page_for_abandon = if crate::page::mi_page_is_in_full(page) {
                crate::core_types::FullPage::new_unchecked(local_page.page()).for_abandon()
            } else {
                crate::core_types::ActivePage::new_unchecked(local_page.page()).for_abandon()
            };
            _mi_page_abandon(page, pq);
        }

        true
    }
}

/// # Safety
///
/// `theap` must point to an initialized thread heap.
pub unsafe fn mi_theap_is_valid(theap: *mut mi_theap_t) -> bool {
    unsafe {
        let Some(theap) = ThreadHeapPtr::new(theap) else {
            crate::mi_assert_internal!(false);
            return false;
        };
        mi_theap_is_valid_typed(theap)
    }
}

unsafe fn mi_theap_is_valid_typed(theap: ThreadHeapPtr) -> bool {
    unsafe {
        let state = TheapPageVisitState {
            visitor: TheapPageVisitCallback::new(Some(mi_theap_page_is_valid)),
            arg1: core::ptr::null_mut(),
            arg2: core::ptr::null_mut(),
        };
        mi_theap_visit_pages_typed(theap, true, state);
        let theap_wrapper = ThreadHeapLocal::new_unchecked(theap);
        let theap = theap.as_ptr();
        for bin in 0..MI_BIN_COUNT {
            let queue = PageQueuePtr::new_unchecked(core::ptr::addr_of_mut!((*theap).pages[bin]));
            let is_valid = if bin == MI_BIN_FULL {
                mi_full_page_queue_is_valid(
                    theap_wrapper,
                    crate::core_types::FullPageQueue::new_unchecked(queue),
                )
            } else if bin == MI_BIN_HUGE {
                mi_huge_page_queue_is_valid(
                    theap_wrapper,
                    crate::core_types::HugePageQueue::new_unchecked(queue),
                )
            } else {
                mi_normal_page_queue_is_valid(
                    theap_wrapper,
                    crate::core_types::NormalPageQueue::new_unchecked(queue),
                )
            };
            crate::mi_assert_internal!(is_valid);
        }
        true
    }
}

/// # Safety
///
/// `theap` must point to an initialized thread heap whose backing heap remains
/// valid for stats merging.
pub unsafe fn mi_theap_merge_stats(theap: *mut mi_theap_t) {
    unsafe {
        let Some(theap) = ThreadHeapPtr::new(theap) else {
            crate::mi_assert_internal!(false);
            return;
        };
        mi_theap_merge_stats_typed(theap);
    }
}

unsafe fn mi_theap_merge_stats_typed(theap: ThreadHeapPtr) {
    unsafe {
        let theap_raw = theap.as_ptr();
        crate::mi_assert_internal!(mi_theap_is_initialized(theap_raw));
        let heap = HeapPtr::new_unchecked(_mi_theap_heap(theap_raw).cast::<mi_heap_t>());
        _mi_stats_merge_into(heap.stats(), theap.stats());
    }
}

unsafe fn mi_theap_collect_ex(theap: *mut mi_theap_t, collect: mi_collect_t) {
    unsafe {
        if theap.is_null() || !mi_theap_is_initialized(theap) {
            return;
        }
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        mi_theap_collect_ex_typed(theap, collect);
    }
}

unsafe fn mi_theap_collect_ex_typed(theap: ThreadHeapLocal, mut collect: mi_collect_t) {
    unsafe {
        let theap_ptr = theap.ptr();
        let theap = theap_ptr.as_ptr();
        crate::mi_assert_expensive!(mi_theap_is_valid(theap));

        let force = collect as i32 >= mi_collect_t::MI_FORCE as i32;
        _mi_deferred_free(theap, force);
        _mi_theap_collect_retired(theap, force);
        let state = TheapPageVisitState {
            visitor: TheapPageVisitCallback::new(Some(mi_theap_page_collect)),
            arg1: core::ptr::addr_of_mut!(collect).cast(),
            arg2: core::ptr::null_mut(),
        };
        mi_theap_visit_pages_typed(theap_ptr, collect != mi_collect_t::MI_NORMAL, state);
        _mi_arenas_collect(
            collect == mi_collect_t::MI_FORCE,
            collect as i32 >= mi_collect_t::MI_FORCE as i32,
            (*theap).tld,
        );
        mi_theap_merge_stats_typed(theap_ptr);
    }
}

/// # Safety
///
/// `theap` must be null or point to an initialized thread heap that can be
/// collected and abandoned.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_collect_abandon(theap: *mut mi_theap_t) {
    unsafe {
        if theap.is_null() || !mi_theap_is_initialized(theap) {
            return;
        }
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        mi_theap_collect_abandon_typed(theap);
    }
}

unsafe fn mi_theap_collect_abandon_typed(theap: ThreadHeapLocal) {
    unsafe {
        mi_theap_collect_ex_typed(theap, mi_collect_t::MI_ABANDON);
    }
}

/// # Safety
///
/// `theap` must be null or point to an initialized thread heap owned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_collect(theap: *mut mi_theap_t, force: bool) {
    unsafe {
        if theap.is_null() || !mi_theap_is_initialized(theap) {
            return;
        }
        let collect = if force {
            mi_collect_t::MI_FORCE
        } else {
            mi_collect_t::MI_NORMAL
        };
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        mi_theap_collect_ex_typed(theap, collect);
    }
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect(force: bool) {
    unsafe {
        mi_theap_collect(_mi_theap_default(), force);
    }
}

/// # Safety
///
/// `heap` must be null or point to a heap managed by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_collect(heap: *mut mi_heap_t, force: bool) {
    unsafe {
        mi_heap_collect_typed(BorrowedHeap::from_lookup_raw(heap), force);
    }
}

#[inline]
unsafe fn mi_heap_collect_typed(heap: Option<BorrowedHeap>, force: bool) {
    unsafe {
        let Some(theap) = crate::heap::mi_heap_theap_typed(heap.map(BorrowedHeap::ptr)) else {
            return;
        };
        if !mi_theap_is_initialized(theap.as_ptr()) {
            return;
        }
        let collect = if force {
            mi_collect_t::MI_FORCE
        } else {
            mi_collect_t::MI_NORMAL
        };
        mi_theap_collect_ex_typed(ThreadHeapLocal::new_unchecked(theap), collect);
    }
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_get_default() -> *mut mi_theap_t {
    unsafe {
        let mut theap = _mi_theap_default();
        if !mi_theap_is_initialized(theap) {
            crate::init::mi_thread_init();
            theap = _mi_theap_default();
            crate::mi_assert_internal!(mi_theap_is_initialized(theap));
        }
        theap
    }
}

/// # Safety
///
/// `theap`, when non-null, must point to a valid initialized thread heap whose
/// lifetime covers subsequent default-thread-heap use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_set_default(theap: *mut mi_theap_t) -> *mut mi_theap_t {
    unsafe {
        let old = mi_theap_get_default();
        if !theap.is_null() {
            let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
            mi_theap_set_default_typed(theap);
        }
        old
    }
}

unsafe fn mi_theap_set_default_typed(theap: ThreadHeapLocal) {
    unsafe {
        crate::init::_mi_theap_default_set(theap.ptr().as_ptr());
    }
}

/// # Safety
///
/// `theap`, `heap`, and `tld` must point to valid writable allocator metadata
/// objects that are not concurrently initialized elsewhere.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_init(
    theap: *mut mi_theap_t,
    heap: *mut mi_heap_t,
    tld: *mut mi_tld_t,
) {
    crate::mi_assert_internal!(!theap.is_null());
    crate::mi_assert_internal!(!heap.is_null());
    crate::mi_assert_internal!(!tld.is_null());

    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    let Some(tld) = TldPtr::new(tld) else {
        return;
    };

    // SAFETY: raw pointers were validated into typed metadata wrappers above.
    unsafe { _mi_theap_init_typed(theap, heap, tld) };
}

#[inline]
unsafe fn _mi_theap_init_typed(theap: ThreadHeapPtr, heap: HeapPtr, tld: TldPtr) {
    let theap_raw = theap.as_ptr();
    let heap_raw = heap.as_ptr();
    let tld_raw = tld.as_ptr();
    unsafe {
        let memid = (*theap_raw).memid;
        _mi_memcpy_aligned(
            theap_raw.cast(),
            __mi_theap_empty().cast_const().cast(),
            size_of::<mi_theap_t>(),
        );
        (*theap_raw).memid = memid;
        theap.refcount_cell().store_seq_cst(1);
        theap.set_tld(Some(tld));
        theap.set_heap(Some(heap));
        crate::mi_assert_internal!(
            (*theap_raw).stats.size == size_of::<crate::stats::mi_stats_t>()
        );

        _mi_theap_options_init(theap_raw);
        if (*tld_raw).is_in_threadpool && (*theap_raw).page_full_retain > 0 {
            (*theap_raw).page_full_retain /= 4;
        }

        mi_lock_acquire(ptr::addr_of_mut!((*tld_raw).theaps_lock));
        let head: *mut mi_theap_t = (*tld_raw).theaps;
        (*theap_raw).tprev = ptr::null_mut();
        (*theap_raw).tnext = head;
        if !head.is_null() {
            (*head).tprev = theap_raw;
        }
        (*tld_raw).theaps = theap_raw;
        mi_lock_release(ptr::addr_of_mut!((*tld_raw).theaps_lock));

        if head.is_null() {
            #[cfg(any(target_os = "macos", windows))]
            crate::internal::_mi_random_init_weak(ptr::addr_of_mut!((*theap_raw).random));
            #[cfg(not(any(target_os = "macos", windows)))]
            _mi_random_init(ptr::addr_of_mut!((*theap_raw).random));
        } else {
            _mi_random_split(
                ptr::addr_of_mut!((*head).random),
                ptr::addr_of_mut!((*theap_raw).random),
            );
        }
        (*theap_raw).cookie = _mi_random_next(ptr::addr_of_mut!((*theap_raw).random)) | 1;
        _mi_theap_guarded_init(theap_raw);
        __mi_stat_increase_mt(&mut (*_mi_subproc()).stats.theaps, 1);

        mi_lock_acquire(ptr::addr_of_mut!((*heap_raw).theaps_lock));
        let head = (*heap_raw).theaps;
        (*theap_raw).hprev = ptr::null_mut();
        (*theap_raw).hnext = head;
        if !head.is_null() {
            (*head).hprev = theap_raw;
        }
        (*heap_raw).theaps = theap_raw;
        mi_lock_release(ptr::addr_of_mut!((*heap_raw).theaps_lock));
    }
}

/// # Safety
///
/// `heap` and `tld` must point to valid allocator metadata objects used to own
/// the new thread heap.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_create(
    heap: *mut mi_heap_t,
    tld: *mut mi_tld_t,
) -> *mut mi_theap_t {
    crate::mi_assert_internal!(!tld.is_null());
    crate::mi_assert_internal!(!heap.is_null());

    let Some(heap) = HeapPtr::new(heap) else {
        return ptr::null_mut();
    };
    let Some(tld) = TldPtr::new(tld) else {
        return ptr::null_mut();
    };

    unsafe { _mi_theap_create_typed(heap, tld) }
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

#[inline]
unsafe fn _mi_theap_create_typed(heap: HeapPtr, tld: TldPtr) -> Option<ThreadHeapPtr> {
    unsafe {
        let heap_raw = heap.as_ptr();
        let tld_raw = tld.as_ptr();

        let mut memid = mi_memid_t::default();
        let theap = if (*heap_raw).exclusive_arena.is_null() {
            _mi_meta_zalloc(size_of::<mi_theap_t>(), ptr::addr_of_mut!(memid)).cast::<mi_theap_t>()
        } else {
            let size = _mi_align_up(size_of::<mi_theap_t>(), MI_ARENA_MIN_OBJ_SIZE);
            let theap = _mi_arenas_alloc(
                heap_raw,
                size,
                true,
                true,
                (*heap_raw).exclusive_arena,
                (*tld_raw).thread_seq,
                crate::core_types::NumaNode::new((*tld_raw).numa_node).get(),
                ptr::addr_of_mut!(memid),
            )
            .cast::<mi_theap_t>();
            if !theap.is_null() {
                let allocated_size = match crate::core_types::MemId::view(memid) {
                    crate::core_types::MemIdView::Arena(a) => {
                        a.range.count_usize() * crate::arena::MI_ARENA_SLICE_SIZE
                    }
                    crate::core_types::MemIdView::Os(os) => os.size.get(),
                    crate::core_types::MemIdView::External(external) => external.size.get(),
                    _ => 0,
                };
                crate::mi_assert_internal!(allocated_size >= size);
            }
            theap
        };

        if theap.is_null() {
            _mi_error_message_fmt(
                Errno::ENOMEM,
                format_args!("unable to allocate theap meta-data\n"),
            );
            return None;
        }

        (*theap).memid = memid;
        let theap = ThreadHeapPtr::new_unchecked(theap);
        _mi_theap_init_typed(theap, heap, tld);
        Some(theap)
    }
}

/// # Safety
///
/// `theap` must point to a valid initialized thread heap.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_random_next(theap: *mut mi_theap_t) -> usize {
    unsafe {
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        _mi_theap_random_next_typed(theap)
    }
}

#[inline(always)]
unsafe fn _mi_theap_random_next_typed(theap: ThreadHeapLocal) -> usize {
    unsafe {
        let theap = theap.ptr().as_ptr();
        _mi_random_next(ptr::addr_of_mut!((*theap).random))
    }
}

#[inline]
fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::None(_)
        | crate::core_types::MemIdView::External(_)
        | crate::core_types::MemIdView::Static(_) => false,
        crate::core_types::MemIdView::Unknown(code) => {
            code.0 > crate::core_types::MemKindCode::STATIC.0
        }
        _ => true,
    }
}

#[inline(always)]
unsafe fn mi_theap_free_mem(theap: *mut mi_theap_t) {
    unsafe {
        let Some(theap) = ThreadHeapPtr::new(theap) else {
            return;
        };
        let theap = ThreadHeapLocal::new_unchecked(theap);
        let raw = theap.ptr().as_ptr();
        let memid = (*raw).memid;
        let memid_view = crate::core_types::MemId::view(memid);
        let size = crate::core_types::ReservedBytes::new(size_of::<mi_theap_t>());
        mi_theap_free_mem_typed(theap, memid, memid_view, size);
    }
}

#[inline(always)]
unsafe fn mi_theap_free_mem_typed(
    theap: ThreadHeapLocal,
    raw_memid: mi_memid_t,
    memid: crate::core_types::MemIdView,
    size: crate::core_types::ReservedBytes,
) {
    unsafe {
        let raw = theap.ptr().as_ptr();
        __mi_stat_decrease_mt(&mut (*_mi_subproc()).stats.theaps, 1);
        match memid {
            crate::core_types::MemIdView::HeapMain(_) => {
                let theap = raw;
                crate::mi_assert_internal!(_mi_is_heap_main(_mi_theap_heap(theap).cast_const()));
                mi_free(theap.cast());
            }
            crate::core_types::MemIdView::Meta(_) => {
                _mi_meta_free(raw.cast(), size.get(), raw_memid);
            }
            crate::core_types::MemIdView::Arena(arena_memid) => {
                mi_theap_free_arena_mem(theap, raw_memid, arena_memid.range, size);
            }
            _ => {
                _mi_arenas_free(raw.cast(), size.get(), raw_memid);
            }
        }
    }
}

#[inline(always)]
unsafe fn mi_theap_free_arena_mem(
    theap: ThreadHeapLocal,
    memid: mi_memid_t,
    _range: crate::core_types::SliceRange,
    size: crate::core_types::ReservedBytes,
) {
    unsafe {
        let raw = theap.ptr().as_ptr();
        let alloc_size =
            crate::core_types::ReservedBytes::new(_mi_align_up(size.get(), MI_ARENA_MIN_OBJ_SIZE));
        _mi_arenas_free(raw.cast(), alloc_size.get(), memid);
    }
}

/// # Safety
///
/// `theap` must be null or point to a valid thread heap metadata object.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_incref(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    unsafe { _mi_theap_incref_typed(theap) };
}

#[inline(always)]
unsafe fn _mi_theap_incref_typed(theap: ThreadHeapPtr) {
    unsafe {
        if mi_memid_is_refcounted((*theap.as_ptr()).memid) {
            theap.refcount_cell().incref_acq_rel();
        }
    }
}

#[inline(always)]
unsafe fn mi_theap_decref_inline(theap: ThreadHeapPtr) {
    unsafe {
        if mi_memid_is_refcounted((*theap.as_ptr()).memid)
            && theap.refcount_cell().decref_acq_rel() == 1
        {
            mi_theap_free_mem(theap.as_ptr());
        }
    }
}

/// # Safety
///
/// `theap` must be null or point to a valid thread heap metadata object.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_decref(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    unsafe { mi_theap_decref_inline(theap) };
}

/// # Safety
///
/// `theap` must point to a valid thread heap. Lock flags must accurately reflect
/// whether the caller already holds the corresponding heap and TLD locks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_free(
    theap: *mut mi_theap_t,
    acquire_heap_theaps_lock: bool,
    acquire_tld_theaps_lock: bool,
) -> bool {
    crate::mi_assert!(!theap.is_null());
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return true;
    };
    unsafe { _mi_theap_free_typed(theap, acquire_heap_theaps_lock, acquire_tld_theaps_lock) }
}

#[inline]
unsafe fn _mi_theap_free_typed(
    theap: ThreadHeapPtr,
    acquire_heap_theaps_lock: bool,
    acquire_tld_theaps_lock: bool,
) -> bool {
    unsafe {
        let theap = match theap.state() {
            ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => {
                theap.ptr().as_ptr()
            }
            ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return false,
        };

        let heap = crate::core_types::AtomicPtrField(ptr::addr_of_mut!((*theap).heap))
            .exchange_acq_rel(ptr::null_mut());
        if heap.is_null() {
            return false;
        }

        _mi_stats_merge_into(
            ptr::addr_of_mut!((*heap).stats),
            ptr::addr_of_mut!((*theap).stats),
        );

        if acquire_heap_theaps_lock {
            mi_lock_acquire(ptr::addr_of_mut!((*heap).theaps_lock));
        }
        if !(*theap).hnext.is_null() {
            (*(*theap).hnext).hprev = (*theap).hprev;
        }
        if !(*theap).hprev.is_null() {
            (*(*theap).hprev).hnext = (*theap).hnext;
        } else {
            crate::mi_assert_internal!((*heap).theaps == theap);
            (*heap).theaps = (*theap).hnext;
        }
        (*theap).hnext = ptr::null_mut();
        (*theap).hprev = ptr::null_mut();
        if acquire_heap_theaps_lock {
            mi_lock_release(ptr::addr_of_mut!((*heap).theaps_lock));
        }

        if acquire_tld_theaps_lock {
            mi_lock_acquire(ptr::addr_of_mut!((*(*theap).tld).theaps_lock));
        }
        if !(*theap).tnext.is_null() {
            (*(*theap).tnext).tprev = (*theap).tprev;
        }
        if !(*theap).tprev.is_null() {
            (*(*theap).tprev).tnext = (*theap).tnext;
        } else {
            crate::mi_assert_internal!((*(*theap).tld).theaps == theap);
            (*(*theap).tld).theaps = (*theap).tnext;
        }
        (*theap).tnext = ptr::null_mut();
        (*theap).tprev = ptr::null_mut();
        if acquire_tld_theaps_lock {
            mi_lock_release(ptr::addr_of_mut!((*(*theap).tld).theaps_lock));
        }

        (*theap).tld = ptr::null_mut();
        mi_theap_decref_inline(ThreadHeapPtr::new_unchecked(theap));
        true
    }
}

/// # Safety
///
/// `theap` must point to a valid initialized thread heap with no concurrent
/// users. `acquire_tld_theaps_lock` must reflect whether the caller already
/// holds the TLD thread-heap list lock.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_delete(
    theap: *mut mi_theap_t,
    acquire_tld_theaps_lock: bool,
) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    unsafe { _mi_theap_delete_typed(theap, acquire_tld_theaps_lock) };
}

#[inline]
unsafe fn _mi_theap_delete_typed(theap: ThreadHeapPtr, acquire_tld_theaps_lock: bool) {
    unsafe {
        let theap_raw = match theap.state() {
            ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => {
                theap.ptr().as_ptr()
            }
            ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return,
        };

        crate::mi_assert!(!theap_raw.is_null());
        crate::mi_assert!(mi_theap_is_initialized(theap_raw));
        crate::mi_assert_expensive!(mi_theap_is_valid(theap_raw));
        if !mi_theap_is_initialized(theap_raw) {
            return;
        }

        _mi_theap_collect_abandon(theap_raw);
        crate::mi_assert_internal!((*theap_raw).page_count == 0);
        _mi_theap_free(theap_raw, true, acquire_tld_theaps_lock);
    }
}

#[used]
static MI_THEAP_CREATE_KEEP: unsafe extern "C" fn(
    *mut mi_heap_t,
    *mut mi_tld_t,
) -> *mut mi_theap_t = _mi_theap_create;

#[used]
static MI_THEAP_DELETE_KEEP: unsafe extern "C" fn(*mut mi_theap_t, bool) = _mi_theap_delete;
