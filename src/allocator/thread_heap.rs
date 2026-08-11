//! Mechanical translation of the early `src/theap.c` helper slice.

// Retains translated thread-heap helper variants used by selected lifecycle
// paths and upstream parity checks.
#![allow(dead_code, non_camel_case_types)]

mod lists;

use core::{ffi::c_void, mem::size_of, ptr};

use crate::allocator::arena::_mi_arenas_collect;
use crate::allocator::options::_mi_error_message_fmt;
use crate::allocator::page::{
    _mi_page_abandon_internal, _mi_page_free_active_internal, _mi_page_free_collect,
    _mi_page_free_full_internal, _mi_page_is_valid, _mi_theap_collect_retired,
    mi_page_is_abandoned, mi_page_queue_is_valid_typed, mi_page_theap,
};
use crate::allocator::stats::{__mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_stats_merge_into};
use crate::bits::{MI_INTPTR_BITS, mi_ctz};
use crate::core_types::{
    Bin, BlockVisitCallback, BlockVisitCallbackArg, BorrowedHeap, Errno, HeapPtr, PageBlockLayout,
    PagePtr, PageQueuePtr, ThreadHeapLocal, ThreadHeapPtr, ThreadHeapState, TldPtr,
};
use crate::internal::memid::mi_memid_t;
use crate::internal::{
    _mi_align_up, _mi_arenas_alloc, _mi_arenas_free, _mi_deferred_free, _mi_divide_up,
    _mi_is_heap_main, _mi_memcpy, _mi_meta_free, _mi_meta_zalloc, _mi_subproc,
    _mi_theap_guarded_init, _mi_theap_heap, _mi_theap_options_init, mi_page_all_free, mi_page_heap,
    mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::mimalloc::mi_free;
use crate::prim::{__mi_theap_empty, _mi_theap_default};
#[cfg(not(any(target_os = "macos", windows)))]
use crate::random::_mi_random_init;
use crate::random::{_mi_random_next, _mi_random_split};
use crate::types::{
    MI_ARENA_MIN_OBJ_SIZE, MI_BIN_COUNT, MI_BIN_FULL, MI_SMALL_PAGE_SIZE, mi_block_visit_fun,
    mi_heap_area_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_theap_t, mi_tld_t,
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
/// that are valid only for the synchronous invocation. `arg1` and `arg2` must
/// satisfy the callback's contract and remain valid through that invocation.
/// The callback must not unwind across the C ABI.
// SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
        // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
/// that are valid only for the synchronous invocation. `arg` must satisfy the
/// callback's contract and remain valid through that invocation. The callback
/// must not unwind across the C ABI.
// SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
        // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
        unsafe { Self(*vfun.cast::<mi_theap_area_visit_fun>()) }
    }

    #[inline(always)]
    unsafe fn invoke(
        self,
        theap: ThreadHeapPtr,
        area: *const mi_theap_area_ex_t,
        arg: *mut c_void,
    ) -> bool {
        // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
/// `page` must have initialized geometry and count metadata that remain stable
/// during construction. The page metadata and block backing must remain live,
/// and its geometry must remain stable, for every use of the returned value or
/// any pointer carried in it. The value itself does not extend either lifetime
/// or grant access to the block backing.
pub(crate) unsafe fn heap_area_for_page(page: PagePtr) -> Option<mi_heap_area_t> {
    // SAFETY: the caller keeps all page geometry stable for this snapshot.
    let layout = unsafe { PageBlockLayout::from_current_page(page) }?;
    let page_area = layout.area();
    let block_size = layout.block_size();
    // SAFETY: the caller also proves initialized, stable page counts.
    let (capacity, used) = unsafe { (page.capacity(), page.used()) };
    let committed = block_size.get().checked_mul(capacity.as_usize())?;
    let full_block_size = crate::core_types::FullBlockSize::from_block_size(block_size);

    Some(mi_heap_area_t {
        blocks: page_area.start().as_ptr().cast(),
        reserved: page_area.byte_len(),
        committed,
        used: used.as_usize(),
        block_size: block_size.usable().get(),
        full_block_size: full_block_size.get(),
        reserved1: page.as_ptr().cast(),
    })
}

/// # Safety
///
/// `area` and `page` must describe the same valid allocator page, and `visitor`
/// must be valid to call for each live block. The page metadata and block
/// backing must remain live, and block geometry must remain stable, throughout
/// this call. A visitor may allocate and alter availability, but returning
/// `true` promises it did not free or invalidate this page's backing or change
/// its block geometry, or mutate the shared `area` descriptor, before the next
/// invocation.
#[inline(never)]
pub(crate) unsafe fn visit_page_blocks(
    area: &mi_heap_area_t,
    page_ptr: PagePtr,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
) -> bool {
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
    unsafe {
        let area = ptr::from_ref(area);
        let page = page_ptr.as_ptr();
        let mut abandoned_owner = if mi_page_is_abandoned(page) {
            let abandoned = crate::core_types::assume_abandoned_page(page_ptr);
            let Ok(owner) = abandoned.try_claim_ownership() else {
                return false;
            };
            crate::allocator::page::collect_page_free_lists_abandoned(&owner, true);
            Some(owner)
        } else {
            crate::internal::mi_assert_internal!(!mi_page_theap(page).is_null());
            _mi_page_free_collect(page, true);
            None
        };
        crate::internal::mi_assert_internal!(page_ptr.local_free().is_empty());
        let result = (|| {
            let used = crate::allocator::page::mi_page_used_count(page);
            if used.get() == 0 {
                return true;
            }

            // SAFETY: local traversal ownership, or the abandoned claim above,
            // keeps the page live while its geometry is captured. Callbacks
            // must obey the documented non-invalidation contract.
            let Some(block_layout) = PageBlockLayout::from_current_page(page_ptr) else {
                crate::internal::mi_assert_internal!(false);
                return false;
            };
            let heap = HeapPtr::new_unchecked(mi_page_heap(page));
            let ubsize = mi_page_usable_block_size(page);

            let capacity = crate::allocator::page::mi_page_capacity_count(page);
            let Some(block_range) = block_layout.nonempty_range(0, capacity.as_usize()) else {
                crate::internal::mi_assert_internal!(false);
                return false;
            };
            if capacity.get() == 1 {
                crate::internal::mi_assert_internal!(used.get() == 1 && page_ptr.free().is_empty());
                return visitor.invoke(
                    heap,
                    area,
                    block_range.first().as_ptr().cast(),
                    ubsize.get(),
                    arg,
                );
            }

            if used.get() == capacity.get() {
                for block_index in 0..capacity.as_usize() {
                    let Some(block) = block_range.block_at(block_index) else {
                        crate::internal::mi_assert_internal!(false);
                        return false;
                    };
                    if !visitor.invoke(heap, area, block.as_ptr().cast(), ubsize.get(), arg) {
                        return false;
                    }
                }
                return true;
            }

            let mut free_map = [0usize; MI_MAX_BLOCKS / MI_INTPTR_BITS];
            let bmapsize = _mi_divide_up(capacity.as_usize(), MI_INTPTR_BITS);
            if !capacity.as_usize().is_multiple_of(MI_INTPTR_BITS) {
                let shift = capacity.as_usize() % MI_INTPTR_BITS;
                free_map[bmapsize - 1] = usize::MAX << shift;
            }

            let mut free_count = 0usize;
            let mut free = page_ptr.free().as_raw();
            while !free.is_null() {
                free_count += 1;
                // Keep a decoded link raw until exact block-start membership
                // is established; corrupt interior addresses are not blocks.
                let Some(blockidx) = block_range.index_of(free) else {
                    crate::internal::mi_assert_internal!(false);
                    return false;
                };
                crate::internal::mi_assert_internal!(blockidx < MI_MAX_BLOCKS);
                let bitidx = blockidx / MI_INTPTR_BITS;
                let bit = blockidx - (bitidx * MI_INTPTR_BITS);
                free_map[bitidx] |= 1usize << bit;
                free = crate::internal::mi_block_next(page, free);
            }
            crate::internal::mi_assert_internal!(
                capacity.as_usize() == free_count + used.as_usize()
            );

            for (map_index, mask) in free_map.iter().take(bmapsize).enumerate() {
                let first_block_index = map_index * MI_INTPTR_BITS;
                if *mask == 0 {
                    for bit_index in 0..MI_INTPTR_BITS {
                        let Some(block) = block_range.block_at(first_block_index + bit_index)
                        else {
                            crate::internal::mi_assert_internal!(false);
                            return false;
                        };
                        if !visitor.invoke(heap, area, block.as_ptr().cast(), ubsize.get(), arg) {
                            return false;
                        }
                    }
                } else {
                    let mut used_mask = !*mask;
                    while used_mask != 0 {
                        let bitidx = mi_ctz(used_mask);
                        let Some(block) = block_range.block_at(first_block_index + bitidx) else {
                            crate::internal::mi_assert_internal!(false);
                            return false;
                        };
                        if !visitor.invoke(heap, area, block.as_ptr().cast(), ubsize.get(), arg) {
                            return false;
                        }
                        used_mask &= used_mask - 1;
                    }
                }
            }
            true
        })();

        if let Some(owner) = abandoned_owner.take() {
            let atomic = page_ptr.thread_free_cell();
            loop {
                let mut expected = atomic.load_relaxed();
                crate::internal::mi_assert_internal!(expected.is_owned());
                if expected.head().is_some() {
                    crate::allocator::page::collect_page_free_lists_abandoned(&owner, true);
                    continue;
                }
                if atomic.release_acq_rel(&mut expected) {
                    break;
                }
            }
            let _ = owner.into_abandoned();
        }
        result
    }
}

// SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
unsafe extern "C" fn mi_theap_visit_areas_page(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    vfun: *mut c_void,
    arg: *mut c_void,
) -> bool {
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
    unsafe {
        crate::internal::MI_UNUSED(&pq);
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
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
    unsafe {
        crate::internal::mi_assert_internal!(mi_page_theap(page.as_ptr()) == theap.as_ptr());
        // SAFETY: page traversal keeps the page metadata and block backing
        // live, with stable geometry, through the callback invocation below.
        let Some(area) = heap_area_for_page(page) else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };
        let xarea = mi_theap_area_ex_t {
            area,
            page: page.as_ptr(),
        };
        visitor.invoke(theap, ptr::addr_of!(xarea), arg)
    }
}

/// # Safety
///
/// When non-null, `theap` must point to a live initialized thread heap whose
/// page backing and geometry remain valid through the synchronous traversal.
/// When present, `visitor` and `arg` must satisfy the area callback contract
/// for that traversal, and the callback must not unwind across the C ABI.
unsafe fn mi_theap_visit_areas(
    theap: *const mi_theap_t,
    visitor: Option<mi_theap_area_visit_fun>,
    arg: *mut c_void,
) -> bool {
    let Some(visitor) = visitor else {
        return false;
    };
    let mut visitor = visitor;
    let visitor_arg = ptr::addr_of_mut!(visitor).cast();
    let page_visitor = Some(mi_theap_visit_areas_page as theap_page_visitor_fun);

    // SAFETY: the caller supplies the live initialized heap and keeps the
    // opaque callback argument valid. The synchronous internal adapter does
    // not retain its stack-local `visitor_arg` slot.
    unsafe { mi_theap_visit_pages(theap.cast_mut(), page_visitor, true, visitor_arg, arg) }
}

// SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
#[inline(never)]
unsafe extern "C" fn mi_theap_area_visitor(
    theap: *const mi_theap_t,
    xarea: *const mi_theap_area_ex_t,
    arg: *mut c_void,
) -> bool {
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
            // SAFETY: the area callback supplies a live page and its matching
            // area projection. A continuing callback must preserve the page
            // backing and geometry for the duration of block traversal.
            visit_page_blocks(
                &(*xarea).area,
                PagePtr::new_unchecked((*xarea).page),
                (*args).visitor,
                (*args).arg,
            )
        } else {
            true
        }
    }
}

/// # Safety
///
/// `theap`, when non-null, must point to a live initialized thread heap whose
/// page backing and geometry remain valid through the synchronous traversal.
/// `visitor`, when present, and `arg` must satisfy the block callback contract
/// for every invocation and remain valid until traversal returns. The callback
/// must not unwind across the C ABI. A callback that returns `true` may allocate
/// and change availability, but must not free or invalidate the current page
/// backing, change its block geometry, or mutate the current area descriptor
/// before the next callback invocation or traversal return.
// SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
        visitor: BlockVisitCallback::new(Some(visitor)),
        arg: BlockVisitCallbackArg::from_raw(arg),
    };
    let args_ptr = ptr::addr_of_mut!(args).cast::<c_void>();

    // SAFETY: the public contract supplies the live heap, callback, and
    // callback argument. The traversal is synchronous, so the stack-local
    // dispatch state at `args_ptr` remains live for every callback invocation.
    unsafe { mi_theap_visit_areas(theap, Some(mi_theap_area_visitor), args_ptr) }
}

/// # Safety
///
/// When non-null, `theap` must point to a live initialized thread heap whose
/// queues, page backing, and page geometry remain valid through the synchronous
/// traversal. When present, `fn_page`, `arg1`, and `arg2` must satisfy the page
/// callback contract for every invocation and remain valid until traversal
/// returns. The callback must not unwind across the C ABI. A null `theap`
/// returns `false` without invoking the callback.
pub unsafe fn mi_theap_visit_pages(
    theap: *mut mi_theap_t,
    fn_page: Option<theap_page_visitor_fun>,
    include_full: bool,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return false;
    };
    let state = TheapPageVisitState {
        visitor: TheapPageVisitCallback::new(fn_page),
        arg1,
        arg2,
    };

    // SAFETY: the caller supplies the live initialized heap and keeps `arg1`
    // and `arg2` valid under the callback contract. The typed traversal does
    // not itself retain the operation-local callback state.
    unsafe { mi_theap_visit_pages_typed(theap, include_full, state) }
}

unsafe fn mi_theap_visit_pages_typed(
    theap: ThreadHeapPtr,
    include_full: bool,
    state: TheapPageVisitState,
) -> bool {
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
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
            let bin = Bin::new_unchecked(i as u16);
            let queue = theap.page_queue(bin);
            let mut page = queue.first();
            while let Some(current) = page {
                let current_ptr = current.as_ptr();
                let next = current.queue_next();
                crate::internal::mi_assert_internal!(mi_page_theap(current_ptr) == theap_ptr);
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

        crate::internal::mi_assert_internal!(
            !include_full || crate::internal::MI_DEBUG <= 1 || count == total
        );
        true
    }
}

/// # Safety
///
/// `theap`, `pq`, and `page` must be valid allocator objects from the same
/// thread heap.
// SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
pub unsafe extern "C" fn mi_theap_page_is_valid(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
    unsafe {
        crate::internal::MI_UNUSED(&arg1);
        crate::internal::MI_UNUSED(&arg2);
        crate::internal::MI_UNUSED(&pq);
        let theap = ThreadHeapPtr::new_unchecked(theap);
        let page = PagePtr::new_unchecked(page);
        mi_theap_page_is_valid_typed(theap, page)
    }
}

unsafe fn mi_theap_page_is_valid_typed(theap: ThreadHeapPtr, page: PagePtr) -> bool {
    // SAFETY: visitation callbacks and page-area walks keep allocator metadata behind typed wrappers.
    unsafe {
        let page = page.as_ptr();
        crate::internal::mi_assert_internal!(mi_page_theap(page) == theap.as_ptr());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        true
    }
}

/// # Safety
///
/// `theap`, `pq`, and `page` must be valid allocator objects from the same
/// thread heap. `arg_collect` must point to a valid `mi_collect_t` value.
// SAFETY: collection paths visit initialized thread heaps and merge stats through typed wrappers.
pub unsafe extern "C" fn mi_theap_page_collect(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg_collect: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    // SAFETY: collection paths visit initialized thread heaps and merge stats through typed wrappers.
    unsafe {
        crate::internal::MI_UNUSED(&arg2);
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
    // SAFETY: collection paths visit initialized thread heaps and merge stats through typed wrappers.
    unsafe {
        crate::internal::mi_assert_internal!(mi_theap_page_is_valid_typed(theap.ptr(), page));

        let local_page = crate::core_types::assume_local_page(page);
        let page = local_page.page().as_ptr();
        _mi_page_free_collect(page, collect as i32 >= mi_collect_t::MI_FORCE as i32);

        if mi_page_all_free(page) {
            if collect as i32 >= mi_collect_t::MI_FORCE as i32
                || crate::allocator::page::mi_page_retire_expire(page).is_zero()
            {
                if crate::allocator::page::mi_page_is_in_full(page) {
                    // SAFETY: the release predicate is evaluated after retired-page collection, so
                    // this local page is not retired. Its stable full flag and the visitor's owning
                    // queue identity classify it as Full in the compatible full queue.
                    let full = local_page.into_full_after_classification();
                    let Ok(all_free) = full.try_mark_all_free() else {
                        crate::internal::mi_assert_internal!(false);
                        return true;
                    };
                    let full_queue = crate::core_types::FullPageQueue::new_unchecked(queue);
                    let _ = _mi_page_free_full_internal(all_free, full_queue);
                } else {
                    // SAFETY: the release predicate is evaluated after retired-page collection, so
                    // this local page is not retired. Its clear full flag and the visitor's owning
                    // queue identity classify it as Active in the compatible normal or huge queue.
                    let active = local_page.into_active_after_classification();
                    let Ok(all_free) = active.try_mark_all_free() else {
                        crate::internal::mi_assert_internal!(false);
                        return true;
                    };
                    let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(queue);
                    let _ = _mi_page_free_active_internal(all_free, normal_queue);
                }
            }
        } else if collect == mi_collect_t::MI_ABANDON {
            let page_for_abandon = if crate::allocator::page::mi_page_is_in_full(page) {
                local_page
                    .into_full_after_classification()
                    .into_abandon_candidate()
            } else {
                local_page
                    .into_active_after_classification()
                    .into_abandon_candidate()
            };
            // SAFETY: the current-thread collection epoch keeps page ownership and its full/active
            // classification stable. The consumed capability and owning queue remain live through
            // detach and arena publication, with local relink and teardown excluded.
            let _ = _mi_page_abandon_internal(page_for_abandon, queue);
        }

        true
    }
}

/// # Safety
///
/// `theap` must point to a live initialized thread heap. The caller must own
/// its local queue mutation, either through current-thread local ownership or
/// exclusive initialization/teardown access. Queue links, page backing, and
/// page geometry must remain valid and must not be concurrently destroyed or
/// relinked during validation. Protocol-compliant remote atomic frees remain
/// permitted.
pub unsafe fn mi_theap_is_valid(theap: *mut mi_theap_t) -> bool {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };

    // SAFETY: the caller supplies the live initialized heap, owns its local
    // queue mutation, and keeps queue links, page backing, and geometry stable
    // against destruction or relinking while typed validation traverses them.
    unsafe { mi_theap_is_valid_typed(theap) }
}

/// # Safety
///
/// `theap` must identify a live initialized thread heap. The caller must own
/// its local queue mutation through current-thread local ownership or exclusive
/// initialization/teardown access. Queue links, page backing, and page geometry
/// must remain valid and must not be concurrently destroyed or relinked during
/// validation. Protocol-compliant remote atomic frees remain permitted.
unsafe fn mi_theap_is_valid_typed(theap: ThreadHeapPtr) -> bool {
    // SAFETY: the caller's queue ownership and stability contract permits the
    // synchronous page traversal of each fixed-bin queue from this heap's live
    // table; protocol remote frees preserve the inspected topology invariants.
    unsafe {
        let state = TheapPageVisitState {
            visitor: TheapPageVisitCallback::new(Some(mi_theap_page_is_valid)),
            arg1: core::ptr::null_mut(),
            arg2: core::ptr::null_mut(),
        };
        mi_theap_visit_pages_typed(theap, true, state);
        let theap_wrapper = ThreadHeapLocal::new_unchecked(theap);
        for bin_index in 0..MI_BIN_COUNT {
            let bin = Bin::new_unchecked(bin_index as u16);
            let queue = theap.page_queue(bin);
            // SAFETY: `bin` selects this exact live queue from the validated heap table. The
            // caller-owned validation epoch keeps its topology, classification, pages, and backing
            // stable while the synchronous read-only traversal runs; protocol remote frees do not
            // mutate the inspected fields.
            let is_valid = mi_page_queue_is_valid_typed(theap_wrapper, queue);
            crate::internal::mi_assert_internal!(is_valid);
        }
        true
    }
}

/// # Safety
///
/// `theap` must identify a live initialized thread heap whose associated heap
/// remains live and stable for the call. The caller must have exclusive access
/// to the thread heap's source stats because the merge resets them. The backing
/// heap's destination stats must be live, correctly aligned, atomic-compatible
/// storage, with all concurrent access to its fields performed atomically.
unsafe fn mi_theap_merge_stats_typed(theap: ThreadHeapPtr) {
    // SAFETY: the caller keeps the initialized thread heap and its associated
    // backing heap live and stable, exclusively owns the source stats reset,
    // and guarantees atomic-compatible access to the destination heap stats.
    unsafe {
        let theap_raw = theap.as_ptr();
        crate::internal::mi_assert_internal!(mi_theap_is_initialized(theap_raw));
        let heap = HeapPtr::new_unchecked(_mi_theap_heap(theap_raw).cast::<mi_heap_t>());
        _mi_stats_merge_into(heap.stats(), theap.stats());
    }
}

/// # Safety
///
/// `theap` must identify a live initialized thread heap under current-thread
/// local ownership or exclusive teardown ownership. Its TLD, backing heap,
/// page queues, page metadata, and page backing must remain live and stable for
/// the call, with no concurrent destroy or queue relinking. Protocol remote
/// frees may publish through the allocator's atomic remote-free lists. Any
/// registered deferred-free callback and opaque argument must remain valid for
/// invocation on this thread, tolerate allocator reentry, preserve the thread
/// heap and page topology needed after it returns, and must not unwind.
unsafe fn mi_theap_collect_ex_typed(theap: ThreadHeapLocal, mut collect: mi_collect_t) {
    // SAFETY: the caller's local or exclusive-teardown ownership keeps the TLD,
    // backing heap, queues, pages, and stats live for this collection epoch.
    // Protocol remote frees remain atomic, while the registered deferred-free
    // callback is valid on this thread, reentry-safe, topology-preserving, and
    // non-unwinding.
    unsafe {
        let theap_ptr = theap.ptr();
        let theap = theap_ptr.as_ptr();
        crate::internal::mi_assert_expensive!(mi_theap_is_valid(theap));

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
/// `theap` must be null or identify a live initialized thread heap under
/// current-thread local ownership or exclusive teardown ownership. Its TLD,
/// backing heap, queues, pages, and registered deferred-free callback state
/// must satisfy `mi_theap_collect_ex_typed` for the call. No concurrent destroy
/// or queue relinking is permitted; protocol remote frees remain allowed.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_collect_abandon(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the caller keeps this thread heap live and stable while its
    // initialized state is read under local or exclusive teardown ownership.
    if !unsafe { mi_theap_is_initialized(theap.as_ptr()) } {
        return;
    }
    // SAFETY: the caller's ownership proves the initialized thread heap is
    // local for this operation; the collection core's lifecycle and callback
    // requirements hold through abandonment.
    unsafe {
        mi_theap_collect_ex_typed(
            ThreadHeapLocal::new_unchecked(theap),
            mi_collect_t::MI_ABANDON,
        )
    };
}

/// # Safety
///
/// `theap` must be null or identify a live initialized thread heap under
/// current-thread local ownership or exclusive teardown ownership. Its TLD,
/// backing heap, queues, pages, and registered deferred-free callback state
/// must satisfy `mi_theap_collect_ex_typed` for the call. No concurrent destroy
/// or queue relinking is permitted; protocol remote frees remain allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_collect(theap: *mut mi_theap_t, force: bool) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the caller keeps this thread heap live and stable while its
    // initialized state is read under local or exclusive teardown ownership.
    if !unsafe { mi_theap_is_initialized(theap.as_ptr()) } {
        return;
    }
    let collect = if force {
        mi_collect_t::MI_FORCE
    } else {
        mi_collect_t::MI_NORMAL
    };
    // SAFETY: the caller's ownership proves the initialized thread heap is
    // local for this operation, and the collection core's lifecycle and
    // callback requirements hold for the selected mode.
    unsafe { mi_theap_collect_ex_typed(ThreadHeapLocal::new_unchecked(theap), collect) };
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
// SAFETY: collection paths visit initialized thread heaps and merge stats through typed wrappers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect(force: bool) {
    unsafe {
        mi_theap_collect(_mi_theap_default(), force);
    }
}

/// # Safety
///
/// `heap` must be null or point to a heap managed by this allocator.
// SAFETY: collection paths visit initialized thread heaps and merge stats through typed wrappers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_collect(heap: *mut mi_heap_t, force: bool) {
    unsafe {
        mi_heap_collect_typed(BorrowedHeap::from_lookup_raw(heap), force);
    }
}

/// # Safety
///
/// `heap` must be `None` or identify a live allocator heap whose associated
/// thread heap is under current-thread local ownership or exclusive teardown
/// ownership. The associated thread heap's TLD, backing heap, queues, pages,
/// and registered deferred-free callback state must remain valid as required
/// by `mi_theap_collect_ex_typed`. No concurrent destroy or queue relinking is
/// permitted; protocol remote frees remain allowed.
#[inline]
unsafe fn mi_heap_collect_typed(heap: Option<BorrowedHeap>, force: bool) {
    // SAFETY: the caller keeps the optional heap live while its associated
    // thread heap is resolved.
    let Some(theap) =
        (unsafe { crate::allocator::heap::mi_heap_theap_typed(heap.map(BorrowedHeap::ptr)) })
    else {
        return;
    };
    // SAFETY: the caller keeps the associated thread heap live and stable
    // while its initialized state is read.
    if !unsafe { mi_theap_is_initialized(theap.as_ptr()) } {
        return;
    }
    let collect = if force {
        mi_collect_t::MI_FORCE
    } else {
        mi_collect_t::MI_NORMAL
    };
    // SAFETY: the caller's ownership proves the initialized associated thread
    // heap is local for this operation, and the collection core's lifecycle
    // and callback requirements hold for the selected mode.
    unsafe { mi_theap_collect_ex_typed(ThreadHeapLocal::new_unchecked(theap), collect) };
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
// SAFETY: default thread-heap shims preserve initialized TLS ownership and raw C ABI behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_get_default() -> *mut mi_theap_t {
    unsafe {
        let mut theap = _mi_theap_default();
        if !mi_theap_is_initialized(theap) {
            crate::allocator::init::mi_thread_init();
            theap = _mi_theap_default();
            crate::internal::mi_assert_internal!(mi_theap_is_initialized(theap));
        }
        theap
    }
}

/// # Safety
///
/// `theap`, when non-null, must point to a valid initialized thread heap whose
/// lifetime covers subsequent default-thread-heap use.
// SAFETY: default thread-heap shims preserve initialized TLS ownership and raw C ABI behavior.
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
    // SAFETY: default thread-heap shims preserve initialized TLS ownership and raw C ABI behavior.
    unsafe {
        crate::allocator::init::_mi_theap_default_set(theap.ptr().as_ptr());
    }
}

/// # Safety
///
/// `theap`, `heap`, and `tld` must point to valid writable allocator metadata
/// objects that are not concurrently initialized elsewhere.
// SAFETY: create and init paths allocate and link thread-heap metadata behind heap and TLD locks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_init(
    theap: *mut mi_theap_t,
    heap: *mut mi_heap_t,
    tld: *mut mi_tld_t,
) {
    crate::internal::mi_assert_internal!(!theap.is_null());
    crate::internal::mi_assert_internal!(!heap.is_null());
    crate::internal::mi_assert_internal!(!tld.is_null());

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
    // SAFETY: create and init paths allocate and link thread-heap metadata behind heap and TLD locks.
    unsafe { _mi_theap_init_typed(theap, heap, tld) };
}

#[inline]
unsafe fn _mi_theap_init_typed(theap: ThreadHeapPtr, heap: HeapPtr, tld: TldPtr) {
    let theap_raw = theap.as_ptr();
    let tld_raw = tld.as_ptr();
    // SAFETY: create and init paths allocate and link thread-heap metadata behind heap and TLD locks.
    unsafe {
        let memid = (*theap_raw).memid;
        _mi_memcpy(
            theap_raw.cast(),
            __mi_theap_empty().cast_const().cast(),
            size_of::<mi_theap_t>(),
        );
        (*theap_raw).memid = memid;
        theap.refcount_cell().store_seq_cst(1);
        theap.set_tld(Some(tld));
        theap.set_heap(Some(heap));
        crate::internal::mi_assert_internal!(
            (*theap_raw).stats.size == size_of::<crate::allocator::stats::mi_stats_t>()
        );

        _mi_theap_options_init(theap_raw);
        if (*tld_raw).is_in_threadpool {
            let configured = crate::allocator::policy::ThreadHeapPolicy::from_fields(
                (*theap_raw).allow_page_reclaim,
                (*theap_raw).allow_page_abandon,
                (*theap_raw).page_full_retain,
            );
            let adjusted =
                crate::allocator::policy::ActivePolicy::adjust_for_thread_pool(configured);
            (*theap_raw).page_full_retain = adjusted.page_full_retain().get();
        }

        let tld_theaps_head = lists::push_tld_front(tld, theap);

        if let Some(tld_theaps_head) = tld_theaps_head {
            _mi_random_split(
                ptr::addr_of_mut!((*tld_theaps_head.as_ptr()).random),
                ptr::addr_of_mut!((*theap_raw).random),
            );
        } else {
            #[cfg(any(target_os = "macos", windows))]
            crate::internal::_mi_random_init_weak(ptr::addr_of_mut!((*theap_raw).random));
            #[cfg(not(any(target_os = "macos", windows)))]
            _mi_random_init(ptr::addr_of_mut!((*theap_raw).random));
        }
        (*theap_raw).cookie = _mi_random_next(ptr::addr_of_mut!((*theap_raw).random)) | 1;
        _mi_theap_guarded_init(theap_raw);
        __mi_stat_increase_mt(ptr::addr_of_mut!((*_mi_subproc()).stats.theaps), 1);

        lists::push_heap_front(heap, theap);
    }
}

/// # Safety
///
/// `heap` and `tld` must point to valid allocator metadata objects used to own
/// the new thread heap.
// SAFETY: create and init paths allocate and link thread-heap metadata behind heap and TLD locks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_create(
    heap: *mut mi_heap_t,
    tld: *mut mi_tld_t,
) -> *mut mi_theap_t {
    crate::internal::mi_assert_internal!(!tld.is_null());
    crate::internal::mi_assert_internal!(!heap.is_null());

    let Some(heap) = HeapPtr::new(heap) else {
        return ptr::null_mut();
    };
    let Some(tld) = TldPtr::new(tld) else {
        return ptr::null_mut();
    };

    // SAFETY: create and init paths allocate and link thread-heap metadata behind heap and TLD locks.
    unsafe { _mi_theap_create_typed(heap, tld) }
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

#[inline]
unsafe fn _mi_theap_create_typed(heap: HeapPtr, tld: TldPtr) -> Option<ThreadHeapPtr> {
    // SAFETY: create and init paths allocate and link thread-heap metadata behind heap and TLD locks.
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
                        a.range().count_usize() * crate::allocator::arena::MI_ARENA_SLICE_SIZE
                    }
                    crate::core_types::MemIdView::Os(os) => os.size.get(),
                    crate::core_types::MemIdView::External(external) => external.size.get(),
                    _ => 0,
                };
                crate::internal::mi_assert_internal!(allocated_size >= size);
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
// SAFETY: random helpers read initialized thread-heap random state through typed wrappers.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_random_next(theap: *mut mi_theap_t) -> usize {
    unsafe {
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        _mi_theap_random_next_typed(theap)
    }
}

#[inline(always)]
unsafe fn _mi_theap_random_next_typed(theap: ThreadHeapLocal) -> usize {
    // SAFETY: random helpers read initialized thread-heap random state through typed wrappers.
    unsafe {
        let theap = theap.ptr().as_ptr();
        _mi_random_next(ptr::addr_of_mut!((*theap).random))
    }
}

#[inline]
fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::None
        | crate::core_types::MemIdView::External(_)
        | crate::core_types::MemIdView::Static => false,
        crate::core_types::MemIdView::Unknown(code) => {
            code.0 > crate::core_types::MemKindCode::STATIC.0
        }
        _ => true,
    }
}

#[inline(always)]
unsafe fn mi_theap_free_mem(theap: *mut mi_theap_t) {
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
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
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
    unsafe {
        let raw = theap.ptr().as_ptr();
        __mi_stat_decrease_mt(ptr::addr_of_mut!((*_mi_subproc()).stats.theaps), 1);
        match memid {
            crate::core_types::MemIdView::HeapMain => {
                let theap = raw;
                crate::internal::mi_assert_internal!(_mi_is_heap_main(
                    _mi_theap_heap(theap).cast_const()
                ));
                mi_free(theap.cast());
            }
            crate::core_types::MemIdView::Meta(_) => {
                _mi_meta_free(raw.cast(), size.get(), raw_memid);
            }
            crate::core_types::MemIdView::Arena(arena_memid) => {
                mi_theap_free_arena_mem(theap, raw_memid, arena_memid.range(), size);
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
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
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
// SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_incref(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
    unsafe { _mi_theap_incref_typed(theap) };
}

#[inline(always)]
unsafe fn _mi_theap_incref_typed(theap: ThreadHeapPtr) {
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
    unsafe {
        if mi_memid_is_refcounted((*theap.as_ptr()).memid) {
            theap.refcount_cell().incref_acq_rel();
        }
    }
}

#[inline(always)]
unsafe fn mi_theap_decref_inline(theap: ThreadHeapPtr) {
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
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
// SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_decref(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: refcount paths release backing thread-heap metadata only after typed ownership checks.
    unsafe { mi_theap_decref_inline(theap) };
}

/// # Safety
///
/// `theap` must point to a valid thread heap. Lock flags must accurately reflect
/// whether the caller already holds the corresponding heap and TLD locks.
// SAFETY: free and delete paths unlink initialized thread heaps under heap and TLD list locks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_free(
    theap: *mut mi_theap_t,
    acquire_heap_theaps_lock: bool,
    acquire_tld_theaps_lock: bool,
) -> bool {
    crate::internal::mi_assert!(!theap.is_null());
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return true;
    };
    // SAFETY: free and delete paths unlink initialized thread heaps under heap and TLD list locks.
    unsafe { _mi_theap_free_typed(theap, acquire_heap_theaps_lock, acquire_tld_theaps_lock) }
}

#[inline]
unsafe fn _mi_theap_free_typed(
    theap: ThreadHeapPtr,
    acquire_heap_theaps_lock: bool,
    acquire_tld_theaps_lock: bool,
) -> bool {
    // SAFETY: free and delete paths unlink initialized thread heaps under heap and TLD list locks.
    unsafe {
        let theap = match theap.state() {
            ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => {
                theap.ptr().as_ptr()
            }
            ThreadHeapState::Uninitialized | ThreadHeapState::Deleted => return false,
        };

        let heap = crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!((*theap).heap))
            .exchange_acq_rel(ptr::null_mut());
        if heap.is_null() {
            return false;
        }

        _mi_stats_merge_into(
            ptr::addr_of_mut!((*heap).stats),
            ptr::addr_of_mut!((*theap).stats),
        );

        let theap_ptr = ThreadHeapPtr::new_unchecked(theap);
        lists::remove_from_heap(
            HeapPtr::new_unchecked(heap),
            theap_ptr,
            acquire_heap_theaps_lock,
        );
        lists::remove_from_tld(
            TldPtr::new_unchecked((*theap).tld),
            theap_ptr,
            acquire_tld_theaps_lock,
        );

        theap_ptr.set_tld(None);
        mi_theap_decref_inline(theap_ptr);
        true
    }
}

/// # Safety
///
/// `theap` must point to a valid initialized thread heap with no concurrent
/// users. `acquire_tld_theaps_lock` must reflect whether the caller already
/// holds the TLD thread-heap list lock.
// SAFETY: free and delete paths unlink initialized thread heaps under heap and TLD list locks.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_delete(
    theap: *mut mi_theap_t,
    acquire_tld_theaps_lock: bool,
) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: free and delete paths unlink initialized thread heaps under heap and TLD list locks.
    unsafe { _mi_theap_delete_typed(theap, acquire_tld_theaps_lock) };
}

#[inline]
unsafe fn _mi_theap_delete_typed(theap: ThreadHeapPtr, acquire_tld_theaps_lock: bool) {
    // SAFETY: free and delete paths unlink initialized thread heaps under heap and TLD list locks.
    unsafe {
        let theap_raw = match theap.state() {
            ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => {
                theap.ptr().as_ptr()
            }
            ThreadHeapState::Uninitialized | ThreadHeapState::Deleted => return,
        };

        crate::internal::mi_assert!(!theap_raw.is_null());
        crate::internal::mi_assert!(mi_theap_is_initialized(theap_raw));
        crate::internal::mi_assert_expensive!(mi_theap_is_valid(theap_raw));
        if !mi_theap_is_initialized(theap_raw) {
            return;
        }

        _mi_theap_collect_abandon(theap_raw);
        crate::internal::mi_assert_internal!((*theap_raw).page_count == 0);
        _mi_theap_free(theap_raw, true, acquire_tld_theaps_lock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::mi_block_t;
    use core::mem::{MaybeUninit, align_of};

    #[test]
    fn visit_dispatch_rejects_null_heap_and_missing_callbacks() {
        // SAFETY: all three entry points explicitly accept null heaps and
        // absent callbacks without dereferencing either raw argument.
        unsafe {
            assert!(!mi_theap_visit_areas(ptr::null(), None, ptr::null_mut()));
            assert!(!mi_theap_visit_blocks(
                ptr::null(),
                false,
                None,
                ptr::null_mut()
            ));
            assert!(!mi_theap_visit_pages(
                ptr::null_mut(),
                None,
                false,
                ptr::null_mut(),
                ptr::null_mut()
            ));
        }
    }

    #[test]
    fn heap_area_for_page_preserves_all_callback_fields() {
        let mut blocks = [MaybeUninit::<mi_block_t>::uninit(); 8];
        let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
        let full_block_size = 2 * size_of::<mi_block_t>();

        // SAFETY: the fixture initializes every page geometry/count field read
        // by the projection, and its complete block backing remains live.
        unsafe {
            let raw = raw_page.as_mut_ptr();
            ptr::addr_of_mut!((*raw).page_start).write(blocks.as_mut_ptr().cast());
            ptr::addr_of_mut!((*raw).block_size).write(full_block_size);
            ptr::addr_of_mut!((*raw).reserved).write(4);
            ptr::addr_of_mut!((*raw).capacity).write(2);
            ptr::addr_of_mut!((*raw).used).write(1);
            let page = PagePtr::new_unchecked(raw);
            let area = heap_area_for_page(page).expect("valid page projects a heap area");

            assert_eq!(area.blocks, blocks.as_mut_ptr().cast());
            assert_eq!(area.reserved, 4 * full_block_size);
            assert_eq!(area.committed, 2 * full_block_size);
            assert_eq!(area.used, 1);
            assert_eq!(
                area.block_size,
                full_block_size - crate::types::MI_PADDING_SIZE
            );
            assert_eq!(area.full_block_size, full_block_size);
            assert_eq!(area.reserved1, raw.cast());
        }
    }

    #[test]
    fn heap_area_for_page_rejects_overflowing_geometry() {
        let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
        let alignment = align_of::<mi_block_t>();
        let overflowing_block_size = usize::MAX - (usize::MAX % alignment);

        // SAFETY: PageBlockLayout is geometry-only and rejects the overflowing
        // reserved-byte product before deriving or accessing block storage.
        unsafe {
            let raw = raw_page.as_mut_ptr();
            ptr::addr_of_mut!((*raw).page_start)
                .write(ptr::without_provenance_mut::<u8>(alignment));
            ptr::addr_of_mut!((*raw).block_size).write(overflowing_block_size);
            ptr::addr_of_mut!((*raw).reserved).write(2);
            ptr::addr_of_mut!((*raw).capacity).write(0);
            ptr::addr_of_mut!((*raw).used).write(0);
            assert!(heap_area_for_page(PagePtr::new_unchecked(raw)).is_none());
        }
    }
}
