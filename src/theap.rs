//! Mechanical translation of the early `src/theap.c` helper slice.

#![allow(non_camel_case_types, unsafe_op_in_unsafe_fn)]

use core::{ffi::c_void, mem::size_of, ptr};

use crate::arena::_mi_arenas_collect;
use crate::atomic::{mi_lock_acquire, mi_lock_release};
use crate::bits::{MI_INTPTR_BITS, MI_SIZE_BITS, mi_clz, mi_ctz};
use crate::internal::{
    _mi_align_up, _mi_arenas_alloc, _mi_arenas_free, _mi_deferred_free, _mi_divide_up,
    _mi_error_message, _mi_is_heap_main, _mi_memcpy_aligned, _mi_meta_free, _mi_meta_zalloc,
    _mi_subproc, _mi_theap_guarded_init, _mi_theap_heap, _mi_theap_options_init, mi_memid_t,
    mi_page_all_free, mi_page_area, mi_page_block_size, mi_page_heap, mi_page_start,
    mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::mimalloc::mi_free;
use crate::page::{
    _mi_page_abandon, _mi_page_free, _mi_page_free_collect, _mi_page_is_valid,
    _mi_page_queue_is_valid, _mi_theap_collect_retired, mi_page_theap,
};
use crate::prim::{__mi_theap_empty, _mi_theap_default};
use crate::random::{_mi_random_init, _mi_random_next, _mi_random_split};
use crate::stats::{__mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_stats_merge_into};
use crate::types::{
    ENOMEM, MI_ARENA_MIN_OBJ_SIZE, MI_BIN_COUNT, MI_BIN_FULL, MI_SMALL_PAGE_SIZE,
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

#[repr(C)]
pub struct mi_visit_blocks_args_s {
    pub visit_blocks: bool,
    pub visitor: mi_block_visit_fun,
    pub arg: *mut c_void,
}

pub type mi_visit_blocks_args_t = mi_visit_blocks_args_s;

/// # Safety
///
/// `area` must point to writable heap-area storage and `page` must point to a
/// valid page owned by this allocator.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_area_init(
    area: *mut mi_heap_area_t,
    page: *mut mi_page_t,
) {
    let bsize = mi_page_block_size(page);
    let ubsize = mi_page_usable_block_size(page);
    (*area).reserved = (*page).reserved as usize * bsize;
    (*area).committed = (*page).capacity as usize * bsize;
    (*area).blocks = mi_page_start(page).cast::<c_void>();
    (*area).used = (*page).used as usize;
    (*area).block_size = ubsize.get();
    (*area).full_block_size = bsize;
    (*area).reserved1 = page.cast::<c_void>();
}

unsafe fn mi_get_fast_divisor(divisor: usize, magic: *mut u64, shift: *mut usize) {
    crate::mi_assert_internal!(divisor > 0 && u32::try_from(divisor).is_ok());
    *shift = MI_SIZE_BITS - mi_clz(divisor - 1);
    *magic = (((1u64 << 32) * ((1u64 << *shift) - divisor as u64)) / divisor as u64) + 1;
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
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    crate::mi_assert_internal!(!area.is_null());
    if area.is_null() {
        return true;
    }
    crate::mi_assert_internal!(!page.is_null());
    if page.is_null() {
        return true;
    }

    _mi_page_free_collect(page, true);
    crate::mi_assert_internal!((*page).local_free.is_null());
    if (*page).used == 0 {
        return true;
    }

    let mut psize = 0usize;
    let pstart = mi_page_area(page, &mut psize);
    let heap = mi_page_heap(page);
    let bsize = mi_page_block_size(page);
    let ubsize = mi_page_usable_block_size(page);

    if (*page).capacity == 1 {
        crate::mi_assert_internal!((*page).used == 1 && (*page).free.is_null());
        return visitor(heap, area, pstart.cast::<c_void>(), ubsize.get(), arg);
    }
    crate::mi_assert_internal!(u32::try_from(bsize).is_ok());

    if (*page).used == (*page).capacity {
        let mut block = pstart;
        for _ in 0..(*page).capacity as usize {
            if !visitor(heap, area, block.cast::<c_void>(), ubsize.get(), arg) {
                return false;
            }
            block = block.add(bsize);
        }
        return true;
    }

    let mut free_map = [0usize; MI_MAX_BLOCKS / MI_INTPTR_BITS];
    let bmapsize = _mi_divide_up((*page).capacity as usize, MI_INTPTR_BITS);
    if !((*page).capacity as usize).is_multiple_of(MI_INTPTR_BITS) {
        let shift = (*page).capacity as usize % MI_INTPTR_BITS;
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
    crate::mi_assert_internal!((*page).capacity as usize == free_count + (*page).used as usize);

    let mut block = pstart;
    for mask in free_map.iter().take(bmapsize) {
        if *mask == 0 {
            for _ in 0..MI_INTPTR_BITS {
                if !visitor(heap, area, block.cast::<c_void>(), ubsize.get(), arg) {
                    return false;
                }
                block = block.add(bsize);
            }
        } else {
            let mut used_mask = !*mask;
            while used_mask != 0 {
                let bitidx = mi_ctz(used_mask);
                if !visitor(
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

#[used]
static MI_THEAP_AREA_VISIT_BLOCKS_KEEP: unsafe extern "C" fn(
    *const mi_heap_area_t,
    *mut mi_page_t,
    mi_block_visit_fun,
    *mut c_void,
) -> bool = _mi_theap_area_visit_blocks;

unsafe extern "C" fn mi_theap_visit_areas_page(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    vfun: *mut c_void,
    arg: *mut c_void,
) -> bool {
    crate::MI_UNUSED!(pq);
    let fun = *(vfun.cast::<mi_theap_area_visit_fun>());
    let mut xarea = mi_theap_area_ex_t {
        page,
        ..mi_theap_area_ex_t::default()
    };
    _mi_heap_area_init(ptr::addr_of_mut!(xarea.area), page);
    fun(theap.cast_const(), ptr::addr_of!(xarea), arg)
}

unsafe fn mi_theap_visit_areas(
    theap: *const mi_theap_t,
    visitor: Option<mi_theap_area_visit_fun>,
    arg: *mut c_void,
) -> bool {
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

#[inline(never)]
unsafe extern "C" fn mi_theap_area_visitor(
    theap: *const mi_theap_t,
    xarea: *const mi_theap_area_ex_t,
    arg: *mut c_void,
) -> bool {
    let args = arg.cast::<mi_visit_blocks_args_t>();
    if !((*args).visitor)(
        _mi_theap_heap(theap.cast_mut()),
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
    let Some(visitor) = visitor else {
        return false;
    };
    let mut args = mi_visit_blocks_args_t {
        visit_blocks,
        visitor,
        arg,
    };
    mi_theap_visit_areas(
        theap,
        Some(mi_theap_area_visitor),
        ptr::addr_of_mut!(args).cast::<c_void>(),
    )
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
    if theap.is_null() || (*theap).page_count == 0 {
        return false;
    }

    let mut count = 0usize;
    let total = (*theap).page_count;
    let max_bin = if include_full {
        MI_BIN_FULL
    } else {
        MI_BIN_FULL - 1
    };

    for i in 0..=max_bin {
        let pq = core::ptr::addr_of_mut!((*theap).pages[i]);
        let mut page = (*pq).first;
        while !page.is_null() {
            let next = (*page).next;
            crate::mi_assert_internal!(mi_page_theap(page) == theap);
            if crate::internal::MI_DEBUG > 1 {
                count += 1;
            }
            if let Some(visitor) = fn_page
                && !visitor(theap, pq, page, arg1, arg2)
            {
                return false;
            }
            page = next;
        }
    }

    crate::mi_assert_internal!(!include_full || crate::internal::MI_DEBUG <= 1 || count == total);
    true
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
    crate::MI_UNUSED!(arg1);
    crate::MI_UNUSED!(arg2);
    crate::MI_UNUSED!(pq);
    crate::mi_assert_internal!(mi_page_theap(page) == theap);
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
    true
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
    crate::MI_UNUSED!(arg2);
    crate::MI_UNUSED!(theap);
    crate::mi_assert_internal!(mi_theap_page_is_valid(
        theap,
        pq,
        page,
        core::ptr::null_mut(),
        core::ptr::null_mut()
    ));

    let collect = *(arg_collect as *const mi_collect_t);
    _mi_page_free_collect(page, collect as i32 >= mi_collect_t::MI_FORCE as i32);

    if mi_page_all_free(page) {
        if collect as i32 >= mi_collect_t::MI_FORCE as i32 || (*page).retire_expire == 0 {
            _mi_page_free(page, pq);
        }
    } else if collect == mi_collect_t::MI_ABANDON {
        _mi_page_abandon(page, pq);
    }

    true
}

/// # Safety
///
/// `theap` must point to an initialized thread heap.
pub unsafe fn mi_theap_is_valid(theap: *mut mi_theap_t) -> bool {
    crate::mi_assert_internal!(!theap.is_null());
    mi_theap_visit_pages(
        theap,
        Some(mi_theap_page_is_valid),
        true,
        core::ptr::null_mut(),
        core::ptr::null_mut(),
    );
    for bin in 0..MI_BIN_COUNT {
        crate::mi_assert_internal!(_mi_page_queue_is_valid(
            theap,
            core::ptr::addr_of!((*theap).pages[bin]),
        ));
    }
    true
}

/// # Safety
///
/// `theap` must point to an initialized thread heap whose backing heap remains
/// valid for stats merging.
pub unsafe fn mi_theap_merge_stats(theap: *mut mi_theap_t) {
    crate::mi_assert_internal!(mi_theap_is_initialized(theap));
    _mi_stats_merge_into(
        core::ptr::addr_of_mut!((*_mi_theap_heap(theap).cast::<mi_heap_t>()).stats),
        core::ptr::addr_of_mut!((*theap).stats),
    );
}

unsafe fn mi_theap_collect_ex(theap: *mut mi_theap_t, mut collect: mi_collect_t) {
    if theap.is_null() || !mi_theap_is_initialized(theap) {
        return;
    }
    crate::mi_assert_expensive!(mi_theap_is_valid(theap));

    let force = collect as i32 >= mi_collect_t::MI_FORCE as i32;
    _mi_deferred_free(theap, force);
    _mi_theap_collect_retired(theap, force);
    mi_theap_visit_pages(
        theap,
        Some(mi_theap_page_collect),
        collect != mi_collect_t::MI_NORMAL,
        core::ptr::addr_of_mut!(collect).cast(),
        core::ptr::null_mut(),
    );
    _mi_arenas_collect(
        collect == mi_collect_t::MI_FORCE,
        collect as i32 >= mi_collect_t::MI_FORCE as i32,
        (*theap).tld,
    );
    mi_theap_merge_stats(theap);
}

/// # Safety
///
/// `theap` must be null or point to an initialized thread heap that can be
/// collected and abandoned.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_collect_abandon(theap: *mut mi_theap_t) {
    mi_theap_collect_ex(theap, mi_collect_t::MI_ABANDON);
}

/// # Safety
///
/// `theap` must be null or point to an initialized thread heap owned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_collect(theap: *mut mi_theap_t, force: bool) {
    mi_theap_collect_ex(
        theap,
        if force {
            mi_collect_t::MI_FORCE
        } else {
            mi_collect_t::MI_NORMAL
        },
    );
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect(force: bool) {
    mi_theap_collect(_mi_theap_default(), force);
}

/// # Safety
///
/// `heap` must be null or point to a heap managed by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_collect(heap: *mut mi_heap_t, force: bool) {
    mi_theap_collect(crate::heap::mi_heap_theap(heap), force);
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_get_default() -> *mut mi_theap_t {
    let mut theap = _mi_theap_default();
    if !mi_theap_is_initialized(theap) {
        crate::init::mi_thread_init();
        theap = _mi_theap_default();
        crate::mi_assert_internal!(mi_theap_is_initialized(theap));
    }
    theap
}

/// # Safety
///
/// `theap`, when non-null, must point to a valid initialized thread heap whose
/// lifetime covers subsequent default-thread-heap use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_set_default(theap: *mut mi_theap_t) -> *mut mi_theap_t {
    let old = mi_theap_get_default();
    if !theap.is_null() {
        crate::init::_mi_theap_default_set(theap);
    }
    old
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

    let memid = (*theap).memid;
    _mi_memcpy_aligned(
        theap.cast(),
        __mi_theap_empty().cast_const().cast(),
        size_of::<mi_theap_t>(),
    );
    (*theap).memid = memid;
    crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))
        .store(1, crate::core_types::MemoryOrder::SeqCst);
    (*theap).tld = tld;
    (*theap).heap = heap;
    crate::mi_assert_internal!((*theap).stats.size == size_of::<crate::stats::mi_stats_t>());

    _mi_theap_options_init(theap);
    if (*(*theap).tld).is_in_threadpool && (*theap).page_full_retain > 0 {
        (*theap).page_full_retain /= 4;
    }

    mi_lock_acquire(ptr::addr_of_mut!((*(*theap).tld).theaps_lock));
    let head: *mut mi_theap_t = (*(*theap).tld).theaps;
    (*theap).tprev = ptr::null_mut();
    (*theap).tnext = head;
    if !head.is_null() {
        (*head).tprev = theap;
    }
    (*(*theap).tld).theaps = theap;
    mi_lock_release(ptr::addr_of_mut!((*(*theap).tld).theaps_lock));

    if head.is_null() {
        #[cfg(any(target_os = "macos", windows))]
        crate::internal::_mi_random_init_weak(ptr::addr_of_mut!((*theap).random));
        #[cfg(not(any(target_os = "macos", windows)))]
        _mi_random_init(ptr::addr_of_mut!((*theap).random));
    } else {
        _mi_random_split(
            ptr::addr_of_mut!((*head).random),
            ptr::addr_of_mut!((*theap).random),
        );
    }
    (*theap).cookie = _mi_random_next(ptr::addr_of_mut!((*theap).random)) | 1;
    _mi_theap_guarded_init(theap);
    __mi_stat_increase_mt(&mut (*_mi_subproc()).stats.theaps, 1);

    mi_lock_acquire(ptr::addr_of_mut!((*heap).theaps_lock));
    let head = (*heap).theaps;
    (*theap).hprev = ptr::null_mut();
    (*theap).hnext = head;
    if !head.is_null() {
        (*head).hprev = theap;
    }
    (*heap).theaps = theap;
    mi_lock_release(ptr::addr_of_mut!((*heap).theaps_lock));
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

    let mut memid = mi_memid_t::default();
    let theap = if (*heap).exclusive_arena.is_null() {
        _mi_meta_zalloc(size_of::<mi_theap_t>(), ptr::addr_of_mut!(memid)).cast::<mi_theap_t>()
    } else {
        let size = _mi_align_up(size_of::<mi_theap_t>(), MI_ARENA_MIN_OBJ_SIZE);
        let theap = _mi_arenas_alloc(
            heap,
            size,
            true,
            true,
            (*heap).exclusive_arena,
            (*tld).thread_seq,
            (*tld).numa_node,
            ptr::addr_of_mut!(memid),
        )
        .cast::<mi_theap_t>();
        if !theap.is_null() {
            let allocated_size = match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => {
                    a.range.count_usize() * crate::arena::MI_ARENA_SLICE_SIZE
                }
                crate::core_types::MemIdView::Os(os) => os.size.0,
                crate::core_types::MemIdView::External(external) => external.size.0,
                _ => 0,
            };
            crate::mi_assert_internal!(allocated_size >= size);
        }
        theap
    };

    if theap.is_null() {
        _mi_error_message(ENOMEM, c"unable to allocate theap meta-data\n".as_ptr());
        return ptr::null_mut();
    }

    (*theap).memid = memid;
    _mi_theap_init(theap, heap, tld);
    theap
}

/// # Safety
///
/// `theap` must point to a valid initialized thread heap.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_random_next(theap: *mut mi_theap_t) -> usize {
    _mi_random_next(ptr::addr_of_mut!((*theap).random))
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
    if theap.is_null() {
        return;
    }

    __mi_stat_decrease_mt(&mut (*_mi_subproc()).stats.theaps, 1);
    let memid = (*theap).memid;
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::HeapMain(_) => {
            crate::mi_assert_internal!(_mi_is_heap_main(_mi_theap_heap(theap).cast_const()));
            mi_free(theap.cast());
        }
        crate::core_types::MemIdView::Meta(_) => {
            _mi_meta_free(theap.cast(), size_of::<mi_theap_t>(), memid);
        }
        crate::core_types::MemIdView::Arena(_) => {
            _mi_arenas_free(
                theap.cast(),
                _mi_align_up(size_of::<mi_theap_t>(), MI_ARENA_MIN_OBJ_SIZE),
                memid,
            );
        }
        _ => {
            _mi_arenas_free(theap.cast(), size_of::<mi_theap_t>(), memid);
        }
    }
}

/// # Safety
///
/// `theap` must be null or point to a valid thread heap metadata object.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_incref(theap: *mut mi_theap_t) {
    if !theap.is_null() && mi_memid_is_refcounted((*theap).memid) {
        crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))
            .fetch_add(1, crate::core_types::MemoryOrder::AcqRel);
    }
}

#[inline(always)]
unsafe fn mi_theap_decref_inline(theap: *mut mi_theap_t) {
    if !theap.is_null()
        && mi_memid_is_refcounted((*theap).memid)
        && crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))
            .fetch_sub(1, crate::core_types::MemoryOrder::AcqRel)
            == 1
    {
        mi_theap_free_mem(theap);
    }
}

/// # Safety
///
/// `theap` must be null or point to a valid thread heap metadata object.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_decref(theap: *mut mi_theap_t) {
    mi_theap_decref_inline(theap);
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
    if theap.is_null() {
        return true;
    }

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
    mi_theap_decref_inline(theap);
    true
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
    crate::mi_assert!(!theap.is_null());
    crate::mi_assert!(mi_theap_is_initialized(theap));
    crate::mi_assert_expensive!(mi_theap_is_valid(theap));
    if theap.is_null() || !mi_theap_is_initialized(theap) {
        return;
    }

    _mi_theap_collect_abandon(theap);
    crate::mi_assert_internal!((*theap).page_count == 0);
    _mi_theap_free(theap, true, acquire_tld_theaps_lock);
}

#[used]
static MI_THEAP_CREATE_KEEP: unsafe extern "C" fn(
    *mut mi_heap_t,
    *mut mi_tld_t,
) -> *mut mi_theap_t = _mi_theap_create;

#[used]
static MI_THEAP_DELETE_KEEP: unsafe extern "C" fn(*mut mi_theap_t, bool) = _mi_theap_delete;
