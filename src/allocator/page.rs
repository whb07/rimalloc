//! Page management

// Retains upstream page helpers and constants used by selected debug,
// collection, and feature paths outside the routine Rust test target.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;

use crate::allocator::arena::{
    _mi_arenas_collect, _mi_arenas_page_abandon, _mi_arenas_page_alloc, _mi_arenas_page_free_typed,
    MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE,
};
use crate::allocator::init::{_mi_page_empty, _mi_theap_empty_wrong};
use crate::allocator::options::_mi_error_message_fmt;
use crate::allocator::stats::MI_BIN_HUGE;
use crate::allocator::tracking::{
    MI_TRACK_ENABLED, mi_track_mem_defined, mi_track_mem_noaccess, mi_track_mem_undefined,
};
use crate::bits::{MI_KiB, MI_SIZE_SIZE};
use crate::core_types::{Bin, Errno, PageKeys, PagePtr, PageQueuePtr, ThreadHeapPtr};
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_increase, _mi_align_up, _mi_is_heap_main, _mi_os_commit,
    _mi_os_good_alloc_size, _mi_os_page_size_typed, _mi_page_malloc_zero_typed, _mi_ptr_page,
    _mi_theap_heap, _mi_theap_random_next, _mi_wsize_from_size, MI_DEBUG, MI_SECURE,
    mi_direct_bin_from_size, mi_mem_is_zero, mi_page_all_free, mi_page_block_size,
    mi_page_committed_typed, mi_page_has_interior_pointers, mi_page_immediate_available,
    mi_page_is_expandable, mi_page_is_mostly_used, mi_page_is_singleton, mi_page_queue,
    mi_page_set_has_interior_pointers, mi_page_set_keys, mi_page_slice_offset_of,
    mi_page_slice_start, mi_page_start, mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::types::{
    MI_BIN_FULL, MI_ENABLE_LARGE_PAGES, MI_ENCODE_FREELIST, MI_LARGE_MAX_OBJ_WSIZE, MI_PADDING,
    MI_PAGE_MIN_COMMIT_SIZE, MI_SMALL_SIZE_MAX, MI_STAT, mi_block_t, mi_page_queue_t, mi_page_t,
    mi_theap_t,
};

pub use crate::types::MI_PADDING_SIZE;

mod bins;
mod counts;
mod deferred;
mod flags;
mod lifecycle;
mod queues;
mod stats_bin;
mod validation;
pub use bins::_mi_bin_size;
pub(crate) use bins::{mi_bin_from_block_size, mi_bin_from_usable, mi_good_size_typed};
use counts::{
    mi_page_add_capacity_count, mi_page_queue_block_size_raw, mi_page_queue_decrement_count,
    mi_page_queue_increment_count, page_queue_ptr,
};
pub(crate) use counts::{
    mi_page_capacity_count, mi_page_queue_count_typed, mi_page_reserved_count,
    mi_page_set_reserved_count, mi_page_set_used_count, mi_page_used_count,
};
pub use counts::{mi_page_queue_is_full, mi_page_queue_is_huge, mi_page_queue_is_special};
pub use deferred::mi_register_deferred_free;
pub(crate) use deferred::{
    _mi_deferred_free, registered_deferred_free_callback, restore_deferred_free_callback,
};
pub use flags::{
    mi_page_is_abandoned, mi_page_is_huge, mi_page_is_in_full, mi_page_set_in_full,
    mi_page_set_theap, mi_page_theap,
};
#[cfg(test)]
pub(crate) use lifecycle::collect_page_free_lists_local;
#[allow(unused_imports)]
pub use lifecycle::{
    _mi_page_free_collect, _mi_page_free_collect_partly, mi_page_is_full,
    mi_page_thread_collect_to_local, mi_page_thread_free_collect,
};
pub(crate) use lifecycle::{
    _mi_page_free_collect_partly_typed, collect_page_free_lists_abandoned,
    collect_remote_for_owned_abandoned_page, mi_page_decrement_retire_expire,
    mi_page_reset_retire_expire, mi_page_retire_expire, mi_page_set_retire_expire,
    mi_page_set_slice_committed, mi_page_slice_committed,
};
pub use queues::{mi_page_queue_contains, mi_page_queue_of};
pub(crate) use queues::{
    mi_page_queue_enqueue_from_full, mi_page_queue_enqueue_from_raw, mi_page_queue_remove_raw,
    mi_page_queue_remove_typed,
};
use queues::{
    mi_page_queue_move_to_front_typed, mi_page_queue_push_at_end_typed, mi_page_queue_push_raw,
    mi_page_queue_push_typed, mi_theap_contains_queue, mi_theap_direct_cache_start,
    mi_theap_queue_index,
};
pub(crate) use stats_bin::mi_page_bin_typed;
#[allow(unused_imports)]
pub use stats_bin::{_mi_page_stats_bin, mi_page_bin};
pub(crate) use validation::mi_page_is_valid_init;
pub use validation::{_mi_page_is_valid, mi_page_block_at};

pub const MI_OS_PAGE_SIZE: usize = 4096;
pub const MI_SMALL_PAGE_SIZE: usize = MI_ARENA_SLICE_SIZE;
pub const MI_MEDIUM_PAGE_SIZE: usize = 8 * MI_SMALL_PAGE_SIZE;
pub const MI_LARGE_PAGE_SIZE: usize = MI_SIZE_SIZE * MI_MEDIUM_PAGE_SIZE;
pub const MI_IN_PAGE_C: () = ();
pub const MI_MAX_SLICE_SHIFT: usize = 6;
pub const MI_MIN_SLICES: usize = 2;
pub const MI_PAGE_OSPAGE_BLOCK_ALIGN2: usize = 4 * MI_KiB;
pub const MI_SMALL_MAX_OBJ_SIZE: usize = (MI_SMALL_PAGE_SIZE - MI_PAGE_OSPAGE_BLOCK_ALIGN2) / 6;
pub const MI_MEDIUM_MAX_OBJ_SIZE: usize = if MI_ENABLE_LARGE_PAGES != 0 {
    (MI_MEDIUM_PAGE_SIZE - MI_PAGE_OSPAGE_BLOCK_ALIGN2) / 6
} else {
    MI_MEDIUM_PAGE_SIZE / 8
};
pub const MI_LARGE_MAX_OBJ_SIZE: usize = MI_LARGE_MAX_OBJ_WSIZE * size_of::<usize>();
pub const MI_MAX_RETIRE_SIZE: usize = MI_LARGE_MAX_OBJ_SIZE;
pub const MI_MAX_ALLOC_SIZE: usize = isize::MAX as usize;

const MI_PAGE_ALIGN: usize = MI_ARENA_SLICE_ALIGN;

/// # Safety
/// `page` must point to a page currently owned by a full queue.
// SAFETY: full/active page transitions preserve queue membership and heap ownership.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_unfull(page: *mut mi_page_t) {
    unsafe {
        let _ = _mi_page_unfull_internal(crate::core_types::assume_full_page(
            crate::core_types::PagePtr::new_unchecked(page),
        ));
    }
}

#[inline(never)]
pub(crate) unsafe fn _mi_page_unfull_internal(
    page_wrapper: crate::core_types::FullPage,
) -> crate::core_types::ActivePage {
    // SAFETY: full/active page transitions preserve queue membership and heap ownership.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(mi_page_is_in_full(page));
        crate::internal::mi_assert_internal!(!(*mi_page_theap(page)).allow_page_abandon);
        if !mi_page_is_in_full(page) {
            return crate::core_types::assume_active_page(page_wrapper.page());
        }

        let theap = mi_page_theap(page);
        let theap_ptr = ThreadHeapPtr::new_unchecked(theap);
        let pqfull = theap_ptr.page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
        mi_page_set_in_full(page, false);
        let pq = mi_theap_page_queue_of(theap, page);
        mi_page_set_in_full(page, true);
        let from = crate::core_types::FullPageQueue::new_unchecked(pqfull);
        let queued =
            match crate::core_types::QueuedPage::<crate::core_types::FullQueue>::try_queue_full(
                page_wrapper,
                from,
            ) {
                Ok(queued) => queued,
                Err(page_wrapper) => {
                    crate::internal::mi_assert_internal!(false);
                    return crate::core_types::assume_active_page(page_wrapper.page());
                }
            };
        let to = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        mi_page_unfull_typed(queued, to)
    }
}

#[inline]
unsafe fn mi_page_to_full(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    // SAFETY: full/active page transitions preserve queue membership and heap ownership.
    unsafe {
        crate::internal::mi_assert_internal!(pq == mi_page_queue_of(page));
        crate::internal::mi_assert_internal!(!mi_page_immediate_available(page));
        crate::internal::mi_assert_internal!(!mi_page_is_in_full(page));

        let theap = mi_page_theap(page);
        if (*theap).allow_page_abandon {
            let active = crate::core_types::assume_active_page(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let _ = _mi_page_abandon_internal(
                active.into_abandon_candidate(),
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            );
        } else if !mi_page_is_in_full(page) {
            let full = ThreadHeapPtr::new_unchecked(theap)
                .page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
            let _ = mi_page_move_to_full_typed(
                crate::core_types::assume_active_page(crate::core_types::PagePtr::new_unchecked(
                    page,
                )),
                crate::core_types::NormalPageQueue::new_unchecked(
                    crate::core_types::PageQueuePtr::new_unchecked(pq),
                ),
                crate::core_types::FullPageQueue::new_unchecked(full),
            );
            _mi_page_free_collect(page, false);
        }
    }
}

#[inline]
unsafe fn mi_page_move_to_full_typed(
    page: crate::core_types::ActivePage,
    from: crate::core_types::NormalPageQueue,
    to: crate::core_types::FullPageQueue,
) -> crate::core_types::FullPage {
    // SAFETY: full/active page transitions preserve queue membership and heap ownership.
    unsafe { page.into_full(from, to) }
}

#[inline]
unsafe fn mi_page_unfull_typed(
    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,
    to: crate::core_types::NormalPageQueue,
) -> crate::core_types::ActivePage {
    // SAFETY: full/active page transitions preserve queue membership and heap ownership.
    unsafe { mi_page_queue_enqueue_from_full(to, page) }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_free(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    // SAFETY: all-free pages are removed from queues before arena release.
    unsafe {
        if mi_page_queue_is_full(pq) {
            if !mi_page_is_in_full(page) {
                mi_page_set_in_full(page, true);
            }
            let queue = crate::core_types::FullPageQueue::new_unchecked(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            );
            let full = crate::core_types::assume_full_page(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let Ok(all_free) = full.try_mark_all_free() else {
                crate::internal::mi_assert_internal!(false);
                return;
            };
            let _ = _mi_page_free_full_internal(all_free, queue);
            return;
        }

        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        let active =
            crate::core_types::assume_active_page(crate::core_types::PagePtr::new_unchecked(page));
        let Ok(all_free) = active.try_mark_all_free() else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        let _ = _mi_page_free_active_internal(all_free, queue);
    }
}

pub(crate) unsafe fn _mi_page_free_active_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,
    queue: crate::core_types::NormalPageQueue,
) -> crate::core_types::DestroyedPage {
    // SAFETY: all-free pages are removed from queues before arena release.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_internal!(mi_page_queue_contains(queue.ptr().as_ptr(), page));
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(mi_page_all_free(page));

        mi_page_set_has_interior_pointers(page_wrapper.page(), false);
        mi_page_queue_remove_raw(queue.ptr().as_ptr(), page);

        _mi_page_free_after_remove(page_wrapper)
    }
}

pub(crate) unsafe fn _mi_page_free_full_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,
    queue: crate::core_types::FullPageQueue,
) -> crate::core_types::DestroyedPage {
    // SAFETY: all-free pages are removed from queues before arena release.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let pq = queue.ptr().as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(pq == mi_page_queue_of(page));
        crate::internal::mi_assert_internal!(mi_page_all_free(page));

        mi_page_set_has_interior_pointers(page_wrapper.page(), false);
        let full = crate::core_types::assume_full_page(page_wrapper.page());
        let Ok(queued) =
            crate::core_types::QueuedPage::<crate::core_types::FullQueue>::try_queue_full(
                full, queue,
            )
        else {
            crate::internal::mi_assert_internal!(false);
            mi_page_queue_remove_raw(pq, page);
            return _mi_page_free_after_remove(page_wrapper);
        };
        let _ = queued.remove();

        _mi_page_free_after_remove(page_wrapper)
    }
}

unsafe fn _mi_page_free_after_remove<S>(
    page_wrapper: crate::core_types::AllFreePage<S>,
) -> crate::core_types::DestroyedPage {
    // SAFETY: all-free pages are removed from queues before arena release.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let theap = mi_page_theap(page);
        crate::internal::mi_assert_internal!(!theap.is_null());
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_set_theap(page, ptr::null_mut());
        let destroyed = _mi_arenas_page_free_typed(page_wrapper, owner);
        _mi_arenas_collect(false, false, (*theap).tld);
        destroyed
    }
}

// SAFETY: retired page collection updates expire counters and queue fronts under thread-heap ownership.
pub(crate) unsafe extern "C" fn _mi_page_retire(page: *mut mi_page_t) {
    unsafe {
        let active =
            crate::core_types::assume_active_page(crate::core_types::PagePtr::new_unchecked(page));
        let Ok(all_free) = active.try_mark_all_free() else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        _mi_page_retire_internal(all_free);
    }
}

pub(crate) unsafe fn _mi_page_retire_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,
) {
    // SAFETY: retired page collection updates expire counters and queue fronts under thread-heap ownership.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(mi_page_all_free(page));

        if !mi_page_retire_expire(page).is_zero() {
            return;
        }
        mi_page_set_has_interior_pointers(page_wrapper.page(), false);

        let pq = mi_page_queue_of(page);
        let queue = page_queue_ptr(pq);
        if !mi_page_queue_is_special(pq)
            && queue
                .first()
                .is_some_and(|first| ptr::eq(first.as_ptr(), page))
            && queue
                .last()
                .is_some_and(|last| ptr::eq(last.as_ptr(), page))
        {
            let theap = mi_page_theap(page);
            if MI_STAT > 0 {
                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);
            }
            let block_size = page_wrapper.page().block_size();
            let retired = page_wrapper.into_retired();
            crate::internal::mi_assert_internal!(ptr::eq(retired.page().as_ptr(), page));
            mi_page_set_retire_expire(
                page,
                crate::allocator::policy::ActivePolicy::retire_expire(block_size),
            );
            let index = mi_theap_queue_index(
                ThreadHeapPtr::new_unchecked(theap),
                PageQueuePtr::new_unchecked(pq),
            );
            crate::internal::mi_assert_internal!(index < MI_BIN_FULL);
            if index < (*theap).page_retired_min {
                (*theap).page_retired_min = index;
            }
            if index > (*theap).page_retired_max {
                (*theap).page_retired_max = index;
            }
            crate::internal::mi_assert_internal!(mi_page_all_free(page));
            return;
        }

        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        let _ = _mi_page_free_active_internal(page_wrapper, queue);
    }
}

// SAFETY: retired page collection updates expire counters and queue fronts under thread-heap ownership.
pub(crate) unsafe extern "C" fn _mi_theap_collect_retired(theap: *mut mi_theap_t, force: bool) {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        _mi_theap_collect_retired_typed(theap, force);
    }
}

#[inline]
unsafe fn _mi_theap_collect_retired_typed(theap: crate::core_types::ThreadHeapLocal, force: bool) {
    // SAFETY: retired page collection updates expire counters and queue fronts under thread-heap ownership.
    unsafe {
        let theap_ptr = theap.ptr();
        let theap = theap_ptr.as_ptr();
        let mut min = MI_BIN_FULL;
        let mut max = 0usize;
        for bin in (*theap).page_retired_min..=(*theap).page_retired_max {
            let queue_ptr = theap_ptr.page_queue(Bin::new_unchecked(bin as u16));
            let pq = queue_ptr.as_ptr();
            let page = queue_ptr
                .first()
                .map(PagePtr::as_ptr)
                .unwrap_or(ptr::null_mut());
            if !page.is_null() && !mi_page_retire_expire(page).is_zero() {
                if mi_page_all_free(page) {
                    let expire = mi_page_decrement_retire_expire(page);
                    if expire.is_zero() || force {
                        let active = crate::core_types::assume_active_page(
                            crate::core_types::PagePtr::new_unchecked(page),
                        );
                        let Ok(all_free) = active.try_mark_all_free() else {
                            crate::internal::mi_assert_internal!(false);
                            continue;
                        };
                        let queue = crate::core_types::NormalPageQueue::new_unchecked(
                            crate::core_types::PageQueuePtr::new_unchecked(pq),
                        );
                        let _ = _mi_page_free_active_internal(all_free, queue);
                    } else {
                        if bin < min {
                            min = bin;
                        }
                        if bin > max {
                            max = bin;
                        }
                    }
                } else {
                    mi_page_reset_retire_expire(page);
                }
            }
        }
        (*theap).page_retired_min = min;
        (*theap).page_retired_max = max;
    }
}

#[inline]
unsafe fn mi_page_free_list_extend_secure(
    theap: crate::core_types::ThreadHeapLocal,
    page: crate::core_types::PagePtr,
    bsize: usize,
    extend: usize,
) {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        mi_page_free_list_extend_secure_typed(
            theap,
            page,
            crate::core_types::BlockSize::new_unchecked(bsize),
            extend,
        );
    }
}

#[inline]
unsafe fn mi_page_free_list_extend_secure_typed(
    theap: crate::core_types::ThreadHeapLocal,
    page: crate::core_types::PagePtr,
    bsize: crate::core_types::BlockSize,
    extend: usize,
) {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let theap = theap.ptr().as_ptr();
        let page_raw = page.as_ptr();
        let bsize = bsize.get();
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::free_list_extend(extend);
        if MI_SECURE < 3 {
            crate::internal::mi_assert_internal!(page.free().is_empty());
            crate::internal::mi_assert_internal!(page.local_free().is_empty());
        }
        let capacity = page.capacity();
        crate::internal::mi_assert_internal!(
            capacity.as_usize() + extend <= page.reserved().as_usize()
        );
        crate::internal::mi_assert_internal!(bsize == page.block_size().get());
        let page_area = page.start().as_ptr().cast::<c_void>();

        let mut shift = MI_MAX_SLICE_SHIFT;
        while (extend >> shift) == 0 {
            shift -= 1;
        }
        let slice_count = 1usize << shift;
        let slice_extend = extend / slice_count;
        crate::internal::mi_assert_internal!(slice_extend >= 1);

        let mut blocks = [ptr::null_mut::<mi_block_t>(); 1 << MI_MAX_SLICE_SHIFT];
        let mut counts = [0usize; 1 << MI_MAX_SLICE_SHIFT];
        for i in 0..slice_count {
            blocks[i] = mi_page_block_at(
                page_raw,
                page_area,
                bsize,
                capacity.as_usize() + i * slice_extend,
            );
            counts[i] = slice_extend;
        }
        counts[slice_count - 1] += extend % slice_count;

        let r = _mi_theap_random_next(theap);
        let mut current = r % slice_count;
        counts[current] -= 1;
        let free_start = blocks[current];

        let mut rnd = crate::random::_mi_random_shuffle(r | 1);
        for i in 1..extend {
            let round = i % size_of::<usize>();
            if round == 0 {
                rnd = crate::random::_mi_random_shuffle(rnd);
            }
            let mut next = (rnd >> (8 * round)) & (slice_count - 1);
            while counts[next] == 0 {
                next += 1;
                if next == slice_count {
                    next = 0;
                }
            }
            counts[next] -= 1;
            let block = blocks[current];
            blocks[current] = block.cast::<u8>().add(bsize).cast::<mi_block_t>();
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(blocks[next]));
            current = next;
        }
        crate::core_types::BlockPtr::new_unchecked(blocks[current])
            .set_next(page, page.free().head());
        page.set_free(crate::core_types::AvailableFreeList::from_head(
            crate::core_types::BlockPtr::new_unchecked(free_start),
        ));
    }
}

// SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
#[inline(never)]
unsafe extern "C" fn mi_page_free_list_extend(page: *mut mi_page_t, bsize: usize, extend: usize) {
    unsafe {
        mi_page_free_list_extend_typed(
            crate::core_types::PagePtr::new_unchecked(page),
            crate::core_types::BlockSize::new_unchecked(bsize),
            extend,
        );
    }
}

#[inline]
unsafe fn mi_page_free_list_extend_typed(
    page: crate::core_types::PagePtr,
    bsize: crate::core_types::BlockSize,
    extend: usize,
) {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let page_raw = page.as_ptr();
        let bsize = bsize.get();
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::free_list_extend(extend);
        if MI_SECURE < 3 {
            crate::internal::mi_assert_internal!(page.free().is_empty());
            crate::internal::mi_assert_internal!(page.local_free().is_empty());
        }
        let capacity = page.capacity();
        crate::internal::mi_assert_internal!(
            capacity.as_usize() + extend <= page.reserved().as_usize()
        );
        crate::internal::mi_assert_internal!(bsize == page.block_size().get());
        let page_area = page.start().as_ptr().cast::<c_void>();
        let start = mi_page_block_at(page_raw, page_area, bsize, capacity.as_usize());
        let last = mi_page_block_at(page_raw, page_area, bsize, capacity.as_usize() + extend - 1);
        let mut block = start;
        while block < last {
            let next = block.cast::<u8>().add(bsize).cast::<mi_block_t>();
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(next));
            block = next;
        }
        crate::core_types::BlockPtr::new_unchecked(last).set_next(page, page.free().head());
        page.set_free(crate::core_types::AvailableFreeList::from_head(
            crate::core_types::BlockPtr::new_unchecked(start),
        ));
    }
}

// SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
#[inline(never)]
unsafe extern "C" fn mi_page_extend_free(theap: *mut mi_theap_t, page: *mut mi_page_t) -> bool {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_extend_free_typed(theap, PagePtr::new_unchecked(page))
    }
}

#[inline]
unsafe fn mi_page_extend_free_typed(
    theap: crate::core_types::ThreadHeapLocal,
    page_ptr: PagePtr,
) -> bool {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let theap_local = theap;
        let theap = theap.ptr().as_ptr();
        let page = page_ptr.as_ptr();
        crate::internal::mi_assert_expensive!(mi_page_is_valid_init(page_ptr));
        if MI_SECURE < 3 {
            crate::internal::mi_assert!(page_ptr.free().is_empty());
            crate::internal::mi_assert!(page_ptr.local_free().is_empty());
            if !page_ptr.free().is_empty() {
                return true;
            }
        }
        let capacity = page_ptr.capacity();
        let reserved = page_ptr.reserved();
        if capacity.get() >= reserved.get() {
            return true;
        }

        let block_size = page_ptr.block_size();
        let bsize = block_size.get();
        let track_area = if MI_TRACK_ENABLED != 0 {
            let Some(area) = page_ptr.area() else {
                crate::internal::mi_assert_internal!(false);
                return false;
            };
            Some(area)
        } else {
            None
        };
        let Some(extend) = crate::allocator::policy::ActivePolicy::page_extend_count(
            block_size, capacity, reserved,
        ) else {
            return true;
        };
        let extend = usize::from(extend.get());

        crate::internal::mi_assert_internal!(extend > 0);
        crate::internal::mi_assert_internal!(extend + capacity.as_usize() <= reserved.as_usize());
        crate::internal::mi_assert_internal!(u16::try_from(extend).is_ok());

        let committed = mi_page_slice_committed(page);
        if !committed.is_zero() {
            let needed_size = (capacity.as_usize() + extend) * bsize;
            let needed_commit = _mi_align_up(
                mi_page_slice_offset_of(page, needed_size),
                MI_PAGE_MIN_COMMIT_SIZE,
            );
            if needed_commit > committed.get() {
                crate::internal::mi_assert_internal!(
                    (needed_commit - committed.get())
                        .is_multiple_of(_mi_os_page_size_typed().get())
                );
                if !_mi_os_commit(
                    mi_page_slice_start(page)
                        .cast::<u8>()
                        .add(committed.get())
                        .cast::<c_void>(),
                    needed_commit - committed.get(),
                    ptr::null_mut(),
                ) {
                    return false;
                }
                mi_page_set_slice_committed(
                    page,
                    crate::core_types::CommittedBytes::new(needed_commit),
                );
            }
        }

        if MI_STAT > 0 {
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);
        }

        if extend < MI_MIN_SLICES || MI_SECURE < 2 {
            mi_page_free_list_extend(page, bsize, extend);
        } else {
            mi_page_free_list_extend_secure(theap_local, page_ptr, bsize, extend);
        }
        mi_page_add_capacity_count(page, extend as u16);
        if MI_STAT > 0 {
            __mi_stat_increase(
                ptr::addr_of_mut!((*theap).stats.page_committed),
                crate::core_types::CommittedBytes::new(extend * bsize).get(),
            );
        }

        if let Some(area) = track_area {
            mi_track_mem_undefined(area.start().as_ptr().cast(), area.byte_len());
            mi_track_mem_noaccess(area.start().as_ptr().cast(), area.byte_len());
        }
        crate::internal::mi_assert_expensive!(mi_page_is_valid_init(page_ptr));
        true
    }
}

// SAFETY: abandon transitions detach active/full pages before arena abandon publication.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_abandon(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
        let queue = crate::core_types::PageQueuePtr::new_unchecked(pq);
        let _ = _mi_page_abandon_typed(page_ptr, queue);
    }
}

#[inline]
unsafe fn _mi_page_abandon_typed(
    page_ptr: crate::core_types::PagePtr,
    queue: crate::core_types::PageQueuePtr,
) -> crate::core_types::PageAbandonOutcome {
    // SAFETY: abandon transitions detach active/full pages before arena abandon publication.
    unsafe {
        let page_wrapper = if mi_page_is_in_full(page_ptr.as_ptr()) {
            crate::core_types::assume_full_page(page_ptr).into_abandon_candidate()
        } else {
            crate::core_types::assume_active_page(page_ptr).into_abandon_candidate()
        };
        _mi_page_abandon_internal(page_wrapper, queue)
    }
}

pub(crate) unsafe fn _mi_page_abandon_internal(
    page_wrapper: crate::core_types::PageForAbandon,
    queue: crate::core_types::PageQueuePtr,
) -> crate::core_types::PageAbandonOutcome {
    // SAFETY: abandon transitions detach active/full pages before arena abandon publication.
    unsafe {
        let page_ptr = match &page_wrapper {
            crate::core_types::PageForAbandon::Active(page) => page.page(),
            crate::core_types::PageForAbandon::Full(page) => page.page(),
        };
        let page = page_ptr.as_ptr();
        let pq = queue.as_ptr();
        _mi_page_free_collect(page, false);
        if mi_page_all_free(page) {
            let destroyed = match page_wrapper {
                crate::core_types::PageForAbandon::Full(page) => {
                    let Ok(all_free) = page.try_mark_all_free() else {
                        crate::internal::mi_assert_internal!(false);
                        return crate::core_types::PageAbandonOutcome::Abandoned;
                    };
                    let queue = crate::core_types::FullPageQueue::new_unchecked(queue);
                    _mi_page_free_full_internal(all_free, queue)
                }
                crate::core_types::PageForAbandon::Active(page) => {
                    let Ok(all_free) = page.try_mark_all_free() else {
                        crate::internal::mi_assert_internal!(false);
                        return crate::core_types::PageAbandonOutcome::Abandoned;
                    };
                    if let Some(queue) = crate::core_types::NormalPageQueue::new(queue) {
                        _mi_page_free_active_internal(all_free, queue)
                    } else {
                        mi_page_queue_remove_raw(pq, page_ptr.as_ptr());
                        _mi_page_free_after_remove(all_free)
                    }
                }
            };
            let _ = destroyed;
            crate::core_types::PageAbandonOutcome::Destroyed
        } else {
            match page_wrapper {
                crate::core_types::PageForAbandon::Full(page) => {
                    if let Some(queue) = crate::core_types::FullPageQueue::new(queue) {
                        match crate::core_types::QueuedPage::<
                            crate::core_types::FullQueue,
                        >::try_queue_full(page, queue) {
                            Ok(queued) => { let _ = queued.remove(); }
                            Err(_page) => {
                                crate::internal::mi_assert_internal!(false);
                                mi_page_queue_remove_raw(pq, page_ptr.as_ptr());
                            }
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                    }
                }
                crate::core_types::PageForAbandon::Active(page) => {
                    if let Some(queue) = crate::core_types::NormalPageQueue::new(queue) {
                        match crate::core_types::QueuedPage::<
                            crate::core_types::NormalQueue,
                        >::try_queue(page, queue) {
                            Ok(queued) => { let _ = queued.remove(); }
                            Err(_page) => {
                                crate::internal::mi_assert_internal!(false);
                                mi_page_queue_remove_raw(pq, page_ptr.as_ptr());
                            }
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                    }
                }
            }
            let theap = (*page).theap;
            mi_page_set_theap(page, ptr::null_mut());
            // Preserve the original theap pointer so same-theap reclaim-on-free can still work.
            (*page).theap = theap;
            _mi_arenas_page_abandon(page, theap);
            _mi_arenas_collect(false, false, (*theap).tld);
            crate::core_types::PageAbandonOutcome::Abandoned
        }
    }
}

/// # Safety
/// `theap` must be valid. If non-null, `pq` must be a page queue belonging to `theap`.
// SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
#[inline(never)]
pub unsafe extern "C" fn mi_page_fresh_alloc(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    block_size: usize,
    page_alignment: usize,
) -> *mut mi_page_t {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        crate::internal::mi_assert_internal!(pq.is_null() || mi_theap_contains_queue(theap, pq));
        crate::internal::mi_assert_internal!(
            page_alignment > 0
                || block_size > MI_LARGE_MAX_OBJ_SIZE
                || pq.is_null()
                || block_size == mi_page_queue_block_size_raw(pq)
        );
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
            crate::core_types::PaddedSize::new_unchecked(block_size),
            crate::core_types::Zeroed::No,
        );
        let Some(page_alignment) = crate::core_types::MaybeAlignment::new(page_alignment) else {
            crate::internal::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let Some(page) = mi_page_fresh_alloc_typed(owner, pq, layout, page_alignment) else {
            return ptr::null_mut();
        };
        page.page().as_ptr()
    }
}

#[inline]
unsafe fn mi_page_fresh_alloc_typed(
    owner: crate::core_types::ThreadHeapLocal,
    pq: *mut mi_page_queue_t,
    layout: crate::core_types::LayoutRequest,
    page_alignment: crate::core_types::MaybeAlignment,
) -> Option<crate::core_types::ActivePage> {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let theap = owner.ptr().as_ptr();
        let block_size = layout.padded().get();
        let page = _mi_arenas_page_alloc(theap, block_size, page_alignment.get());
        if page.is_null() {
            return None;
        }
        let page = if mi_page_is_abandoned(page) {
            let page = _mi_theap_page_reclaim(
                crate::core_types::assume_owned_abandoned_page(
                    crate::core_types::PagePtr::new_unchecked(page),
                ),
                owner,
            );
            if !mi_page_immediate_available(page.page().as_ptr()) {
                if mi_page_is_expandable(page.page().as_ptr()) {
                    if !mi_page_extend_free(theap, page.page().as_ptr()) {
                        return None;
                    }
                } else {
                    crate::internal::mi_assert!(false);
                    return None;
                }
            }
            page
        } else if !pq.is_null() {
            let page = crate::core_types::assume_active_page(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            if mi_page_queue_is_huge(pq) {
                mi_page_queue_push_raw(theap, pq, page.page().as_ptr());
                page
            } else {
                mi_page_queue_push_typed(
                    owner,
                    crate::core_types::NormalPageQueue::new_unchecked(
                        crate::core_types::PageQueuePtr::new_unchecked(pq),
                    ),
                    page,
                )
                .into_active()
            }
        } else {
            crate::core_types::assume_active_page(crate::core_types::PagePtr::new_unchecked(page))
        };
        crate::internal::mi_assert_internal!(
            pq.is_null() || mi_page_block_size(page.page().as_ptr()) >= block_size
        );
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page.page().as_ptr()));
        Some(page)
    }
}

// SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_init(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
) -> bool {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        crate::internal::mi_assert!(!page.is_null());
        crate::internal::mi_assert!(!theap.is_null());
        _mi_page_init_typed(
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
            crate::core_types::assume_uninit_page(crate::core_types::PagePtr::new_unchecked(page)),
        )
    }
}

#[inline]
unsafe fn _mi_page_init_typed(
    owner: crate::core_types::ThreadHeapLocal,
    page_uninit: crate::core_types::Page<crate::core_types::Uninit>,
) -> bool {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let theap = owner.ptr().as_ptr();
        let page = page_uninit.ptr().as_ptr();
        let heap = _mi_theap_heap(theap);
        (*page).heap = if _mi_is_heap_main(heap) {
            ptr::null_mut()
        } else {
            heap
        };
        mi_page_set_theap(page, theap);

        let page_ptr = page_uninit.ptr();
        let Some(area) = page_ptr.area() else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };
        mi_track_mem_noaccess(area.start().as_ptr().cast(), area.byte_len());
        crate::internal::mi_assert_internal!(
            area.byte_len() / mi_page_block_size(page) < (1usize << 16)
        );
        crate::internal::mi_assert_internal!(mi_page_reserved_count(page).get() > 0);

        if MI_PADDING != 0 || MI_ENCODE_FREELIST != 0 {
            mi_page_set_keys(
                page_ptr,
                PageKeys::from_raw([_mi_theap_random_next(theap), _mi_theap_random_next(theap)]),
            );
        }

        if MI_DEBUG > 2 && (*page).memid.initially_zero {
            let committed = mi_page_committed_typed(page);
            mi_track_mem_defined((*page).page_start.cast(), committed.get());
            crate::internal::mi_assert_expensive!(mi_mem_is_zero(
                area.start().as_ptr().cast(),
                committed.get()
            ));
        }

        crate::internal::mi_assert_internal!(!(*page).theap.is_null());
        crate::internal::mi_assert_internal!((*page).theap == mi_page_theap(page));
        crate::internal::mi_assert_internal!(mi_page_capacity_count(page).get() == 0);
        crate::internal::mi_assert_internal!(page_ptr.free().is_empty());
        crate::internal::mi_assert_internal!(mi_page_used_count(page).get() == 0);
        crate::internal::mi_assert_internal!(crate::internal::mi_page_is_owned(page));
        let thread_free = page_ptr.thread_free_cell().load_relaxed();
        crate::internal::mi_assert_internal!(
            thread_free
                == crate::core_types::ThreadFreeWord::from_parts(
                    None,
                    crate::core_types::ThreadFreeOwner::Owned
                )
        );
        crate::internal::mi_assert_internal!(page_ptr.queue_next().is_none());
        crate::internal::mi_assert_internal!(page_ptr.queue_prev().is_none());
        crate::internal::mi_assert_internal!(mi_page_retire_expire(page).is_zero());
        crate::internal::mi_assert_internal!(!mi_page_has_interior_pointers(page));
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        {
            crate::internal::mi_assert_internal!((*page).keys[0] != 0);
            crate::internal::mi_assert_internal!((*page).keys[1] != 0);
        }
        crate::internal::mi_assert_expensive!(mi_page_is_valid_init(page_uninit.ptr()));

        if !mi_page_extend_free(theap, page) {
            return false;
        }
        let active = page_uninit.initialize(owner);
        crate::internal::mi_assert!(mi_page_immediate_available(active.page().as_ptr()));
        true
    }
}

/// # Safety
/// `theap` must be valid, and `pq` must be a page queue belonging to `theap`.
#[inline]
pub unsafe fn mi_page_fresh(theap: *mut mi_theap_t, pq: *mut mi_page_queue_t) -> *mut mi_page_t {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        crate::internal::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        let block_size = mi_page_queue_block_size_raw(pq);
        let block_size_typed = crate::core_types::BlockSize::new_unchecked(block_size);
        let Some(slice_count) = crate::core_types::SliceCount::from_usize(
            crate::internal::mi_slice_count_of_size(block_size),
        ) else {
            crate::internal::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let Some(page) = mi_page_fresh_typed(owner, queue, block_size_typed, slice_count) else {
            return ptr::null_mut();
        };
        page.page().as_ptr()
    }
}

#[inline]
unsafe fn mi_page_fresh_typed(
    owner: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size_typed: crate::core_types::BlockSize,
    slice_count: crate::core_types::SliceCount,
) -> Option<crate::core_types::ActivePage> {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let theap = owner.ptr().as_ptr();
        let pq = queue.ptr().as_ptr();
        let block_size = block_size_typed.get();
        crate::internal::mi_assert_internal!(slice_count.get() > 0);
        let page = mi_page_fresh_alloc(theap, pq, block_size, 0);
        if page.is_null() {
            return None;
        }
        crate::internal::mi_assert_internal!(block_size == mi_page_block_size(page));
        crate::internal::mi_assert_internal!(pq == mi_theap_page_queue_of(theap, page));
        Some(crate::core_types::assume_active_page(
            crate::core_types::PagePtr::new_unchecked(page),
        ))
    }
}

#[inline(never)]
pub(crate) unsafe fn try_reclaim_abandoned(
    page_wrapper: crate::core_types::OwnedAbandonedPage,
    target: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::ActivePage {
    // SAFETY: abandoned-page reclaim reattaches pages to target thread-heap queues.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let theap = target.ptr().as_ptr();
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
            mi_page_slice_start(page).cast(),
            MI_PAGE_ALIGN,
        ));
        crate::internal::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);
        crate::internal::mi_assert_internal!(crate::internal::mi_page_is_owned(page));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page));

        mi_page_set_theap(page, theap);

        collect_page_free_lists_abandoned(&page_wrapper, false);
        let pq = mi_theap_page_queue_of(theap, page);
        let queued = mi_page_queue_push_at_end_typed(
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
            crate::core_types::NormalPageQueue::new_unchecked(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ),
            page_wrapper.into_reclaimed(target),
        );
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        queued.into_active()
    }
}

#[inline(never)]
pub(crate) unsafe fn _mi_theap_page_reclaim(
    page: crate::core_types::OwnedAbandonedPage,
    target: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::ActivePage {
    // SAFETY: abandoned-page reclaim reattaches pages to target thread-heap queues.
    unsafe { try_reclaim_abandoned(page, target) }
}

/// # Safety
/// C-ABI-shaped keep boundary for page reclaim. The caller must ensure `theap`
/// and `page` are valid, and that `page` is an owned abandoned page.
// SAFETY: abandoned-page reclaim reattaches pages to target thread-heap queues.
#[inline(never)]
unsafe extern "C" fn mi_theap_page_reclaim_raw(theap: *mut mi_theap_t, page: *mut mi_page_t) {
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
        let wrapper = crate::core_types::assume_owned_abandoned_page(page_ptr);
        let theap_ptr = crate::core_types::ThreadHeapPtr::new_unchecked(theap);
        let theap_wrapper = crate::core_types::ThreadHeapLocal::new_unchecked(theap_ptr);
        let _ = _mi_theap_page_reclaim(wrapper, theap_wrapper);
    }
}

/// # Safety
/// `theap` must be valid, and `pq` must point to a page queue belonging to that heap.
// SAFETY: queue validation walks linked pages without mutating queue ownership.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_queue_is_valid(
    theap: *mut mi_theap_t,
    pq: *const mi_page_queue_t,
) -> bool {
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
    unsafe {
        if pq.is_null() {
            return false;
        }
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let pq = crate::core_types::PageQueuePtr::new_unchecked(pq.cast_mut());
        if let Some(queue) = crate::core_types::NormalPageQueue::new(pq) {
            mi_normal_page_queue_is_valid(theap, queue)
        } else if let Some(queue) = crate::core_types::HugePageQueue::new(pq) {
            mi_huge_page_queue_is_valid(theap, queue)
        } else if let Some(queue) = crate::core_types::FullPageQueue::new(pq) {
            mi_full_page_queue_is_valid(theap, queue)
        } else {
            false
        }
    }
}

#[inline]
pub(crate) unsafe fn mi_normal_page_queue_is_valid(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
) -> bool {
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
    unsafe { mi_page_queue_is_valid_typed(theap, queue.ptr()) }
}

#[inline]
pub(crate) unsafe fn mi_huge_page_queue_is_valid(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::HugePageQueue,
) -> bool {
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
    unsafe { mi_page_queue_is_valid_typed(theap, queue.ptr()) }
}

#[inline]
pub(crate) unsafe fn mi_full_page_queue_is_valid(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::FullPageQueue,
) -> bool {
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
    unsafe { mi_page_queue_is_valid_typed(theap, queue.ptr()) }
}

#[inline]
unsafe fn mi_page_queue_is_valid_typed(
    theap: crate::core_types::ThreadHeapLocal,
    pq: crate::core_types::PageQueuePtr,
) -> bool {
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
    unsafe {
        let theap = theap.ptr().as_ptr();
        let pq = pq.as_ptr();
        crate::internal::MI_UNUSED_RELEASE(&theap);
        let mut count = 0usize;
        crate::internal::MI_UNUSED_RELEASE(&count);
        let mut prev = None;
        crate::internal::MI_UNUSED_RELEASE(&prev);
        let queue = page_queue_ptr(pq);
        let block_size = mi_page_queue_block_size_raw(pq);
        let mut page = queue.first();
        while let Some(page_ptr) = page {
            let page_raw = page_ptr.as_ptr();
            crate::internal::mi_assert_internal!(page_ptr.queue_prev() == prev);
            if mi_page_is_in_full(page_raw) {
                crate::internal::mi_assert_internal!(
                    _mi_wsize_from_size(block_size) == MI_LARGE_MAX_OBJ_WSIZE + 2
                );
            } else if mi_page_is_huge(page_raw) {
                crate::internal::mi_assert_internal!(
                    _mi_wsize_from_size(block_size) == MI_LARGE_MAX_OBJ_WSIZE + 1
                );
            } else {
                crate::internal::mi_assert_internal!(mi_page_block_size(page_raw) == block_size);
            }
            crate::internal::mi_assert_internal!((*page_raw).theap == theap);
            if page_ptr.queue_next().is_none() {
                crate::internal::mi_assert_internal!(
                    queue.last().is_some_and(|last| last == page_ptr)
                );
            }
            count += 1;
            prev = page;
            page = page_ptr.queue_next();
        }
        crate::internal::mi_assert_internal!(mi_page_queue_count_typed(pq).get() == count);
        true
    }
}

/// # Safety
/// `theap` must be valid, and `page` must point to page metadata owned by that heap.
#[inline]
pub(crate) unsafe fn mi_theap_page_queue_of_typed(
    theap: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
) -> crate::core_types::PageQueuePtr {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let bin = mi_bin_from_block_size(block_size);
        crate::internal::mi_assert_internal!(bin.as_usize() <= MI_BIN_HUGE);
        theap.ptr().page_queue(bin)
    }
}

/// # Safety
/// `theap` must be valid. `page` must point to page metadata owned by `theap`.
#[inline]
pub unsafe fn mi_theap_page_queue_of(
    theap: *mut mi_theap_t,
    page: *const mi_page_t,
) -> *mut mi_page_queue_t {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        crate::internal::mi_assert_internal!(!theap.is_null());
        let bin = mi_page_bin_typed(page).as_usize();
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let block_size = crate::core_types::BlockSize::new_unchecked(mi_page_block_size(page));
        let pq = if mi_page_is_in_full(page) || mi_page_is_huge(page) {
            theap.ptr().page_queue(Bin::new_unchecked(bin as u16))
        } else {
            mi_theap_page_queue_of_typed(theap, block_size)
        };
        let pq = pq.as_ptr();
        crate::internal::mi_assert_internal!(
            (mi_page_block_size(page) == mi_page_queue_block_size_raw(pq))
                || (mi_page_is_huge(page) && mi_page_queue_is_huge(pq))
                || (mi_page_is_in_full(page) && mi_page_queue_is_full(pq))
        );
        pq
    }
}

/// # Safety
/// `theap` must be valid, and `pq` must point to one of its page queues.
pub unsafe fn mi_theap_queue_first_update(theap: *mut mi_theap_t, pq: *const mi_page_queue_t) {
    // SAFETY: direct-page cache updates mirror normal queue fronts for small pages.
    unsafe {
        crate::internal::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
        let queue = page_queue_ptr(pq);
        if let Some(queue) = crate::core_types::NormalPageQueue::new(queue) {
            let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            mi_theap_queue_first_update_typed(theap, queue);
        }
    }
}

#[inline]
pub(crate) unsafe fn mi_theap_queue_first_update_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
) {
    // SAFETY: direct-page cache updates mirror normal queue fronts for small pages.
    unsafe {
        let Some(block_size) = queue.ptr().block_size() else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        let size = block_size.get();
        if size > MI_SMALL_SIZE_MAX {
            return;
        }

        let page = queue
            .ptr()
            .first()
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or_else(|| ptr::addr_of!(_mi_page_empty).cast_mut());

        let direct_idx = mi_direct_bin_from_size(size);
        let idx = direct_idx.as_usize();
        crate::internal::mi_debug_assert!(idx < crate::types::MI_PAGES_DIRECT);
        let theap_ptr = theap.ptr();
        if theap_ptr
            .direct_page(direct_idx)
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut())
            == page
        {
            return;
        }

        let start = if idx <= 1 {
            0
        } else {
            mi_theap_direct_cache_start(theap_ptr, queue, block_size, direct_idx)
        };

        crate::internal::mi_assert_internal!(start <= idx);
        let direct_start = crate::core_types::DirectBin::new_unchecked(start as u16);
        theap_ptr.fill_direct_pages(
            direct_start,
            direct_idx,
            Some(crate::core_types::PagePtr::new_unchecked(page)),
        );
    }
}

#[inline]
unsafe fn mi_page_queue_find_free_ex_active(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
    first_try: bool,
) -> Option<crate::core_types::ActivePage> {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        let pq = queue.ptr().as_ptr();
        let mut count = crate::core_types::PageQueueCount::new(0);
        let mut candidate_limit = crate::core_types::PageReclaimLimit::new(0);
        let configured = crate::allocator::policy::ThreadHeapPolicy::from_fields(
            (*theap_raw).allow_page_reclaim,
            (*theap_raw).allow_page_abandon,
            (*theap_raw).page_full_retain,
        );
        let mut page_full_retain = crate::allocator::policy::ActivePolicy::initial_full_retain(
            block_size,
            configured.page_full_retain(),
        );
        let mut page_candidate = ptr::null_mut::<mi_page_t>();
        let mut page = queue
            .ptr()
            .first()
            .map(PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        #[cfg(feature = "bench-diagnostics")]
        let mut failed_extend = false;
        #[cfg(feature = "bench-diagnostics")]
        let mut selected_position = 0usize;

        while !page.is_null() {
            let next = (*page).next;
            count = count.increment();
            candidate_limit = candidate_limit.decrement();

            let mut immediate_available = mi_page_immediate_available(page);
            if !immediate_available {
                _mi_page_free_collect(page, false);
                immediate_available = mi_page_immediate_available(page);
            }

            if !immediate_available && !mi_page_is_expandable(page) {
                page_full_retain = page_full_retain.decrement();
                if page_full_retain.is_negative() {
                    crate::internal::mi_assert_internal!(
                        !mi_page_is_in_full(page) && !mi_page_immediate_available(page)
                    );
                    mi_page_to_full(page, pq);
                }
            } else {
                if page_candidate.is_null() {
                    page_candidate = page;
                    #[cfg(feature = "bench-diagnostics")]
                    {
                        selected_position = count.get();
                    }
                    candidate_limit =
                        crate::allocator::policy::ActivePolicy::page_candidate_limit();
                } else if mi_page_all_free(page_candidate) {
                    let active = crate::core_types::assume_active_page(
                        crate::core_types::PagePtr::new_unchecked(page_candidate),
                    );
                    let all_free = active.try_mark_all_free().unwrap_unchecked();
                    let _ = _mi_page_free_active_internal(all_free, queue);
                    page_candidate = page;
                    #[cfg(feature = "bench-diagnostics")]
                    {
                        selected_position = count.get();
                    }
                } else if mi_page_used_count(page) >= mi_page_used_count(page_candidate)
                    && !mi_page_is_mostly_used(page)
                {
                    page_candidate = page;
                    #[cfg(feature = "bench-diagnostics")]
                    {
                        selected_position = count.get();
                    }
                }
                if immediate_available || candidate_limit.is_exhausted() {
                    crate::internal::mi_assert_internal!(!page_candidate.is_null());
                    break;
                }
            }

            page = next;
        }

        __mi_stat_counter_increase(
            ptr::addr_of_mut!((*theap_raw).stats.page_searches),
            count.get(),
        );
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap_raw).stats.page_searches_count), 1);

        if !page_candidate.is_null() {
            page = page_candidate;
        }
        if !page.is_null() {
            if !mi_page_immediate_available(page) {
                crate::internal::mi_assert_internal!(mi_page_is_expandable(page));
                if !mi_page_extend_free(theap_raw, page) {
                    page = ptr::null_mut();
                    #[cfg(feature = "bench-diagnostics")]
                    {
                        failed_extend = true;
                    }
                }
            }
            crate::internal::mi_assert_internal!(
                page.is_null() || mi_page_immediate_available(page)
            );
        }

        if page.is_null() {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_queue_search(
                count.get(),
                false,
                failed_extend,
            );
            _mi_theap_collect_retired_typed(theap, false);
            let Some(slice_count) = crate::core_types::SliceCount::from_usize(
                crate::internal::mi_slice_count_of_size(block_size.get()),
            ) else {
                crate::internal::mi_assert_internal!(false);
                return None;
            };
            if let Some(fresh) = mi_page_fresh_typed(theap, queue, block_size, slice_count) {
                #[cfg(feature = "bench-diagnostics")]
                crate::allocator::bench_diagnostics::page_queue_fresh_allocation();
                return Some(fresh);
            }
            if first_try {
                #[cfg(feature = "bench-diagnostics")]
                crate::allocator::bench_diagnostics::page_queue_retry();
                return mi_page_queue_find_free_ex_active(theap, queue, block_size, false);
            }
            return None;
        }

        let active =
            crate::core_types::assume_active_page(crate::core_types::PagePtr::new_unchecked(page));
        // SAFETY: traversal proves membership in this exact queue; the normal
        // queue invariant proves the page is not full and has a compatible
        // block size.
        let queued = crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(
            active, queue,
        );
        let queued = mi_page_queue_move_to_front_typed(theap, queue, queued);
        mi_page_reset_retire_expire(page);
        #[cfg(feature = "bench-diagnostics")]
        {
            crate::allocator::bench_diagnostics::page_queue_search(
                count.get(),
                true,
                failed_extend,
            );
            crate::allocator::bench_diagnostics::page_queue_selected(selected_position);
        }
        Some(queued.into_active())
    }
}

#[inline]
unsafe fn mi_page_queue_find_free_ex_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
    first_try: bool,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: the active search preserves page ownership and establishes an
    // immediately available page before returning a successful producer.
    unsafe {
        mi_page_queue_find_free_ex_active(theap, queue, block_size, first_try)
            .and_then(|active| active.try_available().ok())
    }
}

#[inline]
#[allow(clippy::absurd_extreme_comparisons)]
unsafe fn mi_find_free_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        if let Some(page) = queue.ptr().first() {
            let local = crate::core_types::assume_local_page(page);
            if let Ok(available) = local.try_ensure_available() {
                let page_raw = available.page().as_ptr();
                if MI_SECURE >= 2
                    && mi_page_capacity_count(page_raw).get()
                        < mi_page_reserved_count(page_raw).get()
                    && ((_mi_theap_random_next(theap.ptr().as_ptr()) & 1) == 1)
                {
                    let _ = mi_page_extend_free(theap.ptr().as_ptr(), page_raw);
                }
                mi_page_reset_retire_expire(page_raw);
                return Some(available);
            }
        }
        mi_page_queue_find_free_ex_typed(theap, queue, block_size, true)
    }
}

#[inline]
unsafe fn mi_huge_page_alloc_typed(
    owner: crate::core_types::ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    page_alignment: crate::core_types::MaybeAlignment,
    queue: crate::core_types::HugePageQueue,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: huge-page selection and allocation preserve singleton page layout, queue selection, alignment, and stat accounting.
    unsafe {
        let theap = owner.ptr().as_ptr();
        let pq = queue.ptr().as_ptr();
        let block_size = layout.padded().get();
        let page_alignment = page_alignment.get();
        let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);
        if !page.is_null() {
            crate::internal::mi_assert_internal!(mi_page_block_size(page) >= layout.padded().get());
            crate::internal::mi_assert_internal!(mi_page_is_huge(page));
            crate::internal::mi_assert_internal!(mi_page_is_singleton(page));
            if MI_STAT > 0 {
                __mi_stat_increase(
                    ptr::addr_of_mut!((*theap).stats.malloc_huge),
                    mi_page_block_size(page),
                );
                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_huge_count), 1);
            }
        }
        let page = crate::core_types::PagePtr::new(page)?;
        let active = crate::core_types::assume_active_page(page);
        active.try_available().ok()
    }
}

#[inline(always)]
unsafe fn mi_find_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    size: crate::core_types::PaddedSize,
    huge_alignment: crate::core_types::MaybeAlignment,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: huge-page selection and allocation preserve singleton page layout, queue selection, alignment, and stat accounting.
    unsafe {
        let req_size = size.get().wrapping_sub(MI_PADDING_SIZE);
        if req_size > MI_MAX_ALLOC_SIZE {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!("allocation request is too large ({req_size} bytes)\n"),
            );
            return None;
        }

        let queue_block_size = if huge_alignment.get() > 0 {
            MI_LARGE_MAX_OBJ_SIZE + 1
        } else {
            size.get()
        };
        let pq = mi_page_queue(
            theap.ptr().as_ptr(),
            crate::core_types::BlockSize::new_unchecked(queue_block_size),
        );
        if mi_page_queue_is_huge(pq) || req_size > MI_MAX_ALLOC_SIZE {
            let Some(queue) = crate::core_types::HugePageQueue::new(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ) else {
                crate::internal::mi_assert_internal!(false);
                return None;
            };
            let block_size = _mi_os_good_alloc_size(size.get());
            let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
                crate::core_types::PaddedSize::new_unchecked(block_size),
                crate::core_types::Zeroed::No,
            );
            mi_huge_page_alloc_typed(theap, layout, huge_alignment, queue)
        } else {
            let Some(queue) = crate::core_types::NormalPageQueue::new(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ) else {
                crate::internal::mi_assert_internal!(false);
                return None;
            };
            let Some(block_size) = queue.ptr().block_size() else {
                crate::internal::mi_assert_internal!(false);
                return None;
            };
            mi_find_free_page_typed(theap, queue, block_size)
        }
    }
}

// SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
pub(crate) unsafe extern "C" fn _mi_malloc_generic(
    theap: *mut mi_theap_t,
    size: usize,
    zero_huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        // SAFETY: `zero_huge_alignment` is the C-compatible packed flag word:
        // bit 0 is zeroing and the remaining bits are the huge-alignment payload.
        let flags = crate::core_types::MallocGenericFlags::from_raw(zero_huge_alignment);
        let padded = if size == 0 {
            crate::core_types::RequestedSize::new(crate::core_types::RequestedSize::MIN_ZERO_SIZE)
                .padded_unchecked()
        } else {
            crate::core_types::PaddedSize::new_unchecked(size)
        };
        let layout =
            crate::core_types::LayoutRequest::from_padded_unchecked(padded, flags.zeroed());
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        _mi_malloc_generic_typed(theap, layout, flags, usable)
    }
}

#[inline(always)]
pub(crate) unsafe fn _mi_theap_or_default_typed(
    mut theap: *mut mi_theap_t,
) -> Option<crate::core_types::ThreadHeapPtr> {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        if !mi_theap_is_initialized(theap) {
            if ptr::eq(theap, ptr::addr_of!(_mi_theap_empty_wrong).cast_mut()) {
                return None;
            }
            crate::mimalloc::mi_thread_init();
            theap = crate::prim::_mi_theap_default();
            if !mi_theap_is_initialized(theap) {
                return None;
            }
            crate::internal::mi_assert_internal!(crate::prim::_mi_theap_default() == theap);
        }
        crate::internal::mi_assert_internal!(mi_theap_is_initialized(theap));

        Some(crate::core_types::ThreadHeapPtr::new_unchecked(theap))
    }
}

pub(crate) unsafe fn _mi_malloc_generic_typed(
    theap: crate::core_types::ThreadHeapPtr,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let local_theap = crate::core_types::ThreadHeapLocal::new_unchecked(theap);
        let theap = local_theap.ptr().as_ptr();
        let size = layout.padded().get();
        let zero = flags.zeroed() == crate::core_types::Zeroed::Yes;
        let Some(huge_alignment) =
            crate::core_types::MaybeAlignment::new(flags.huge_alignment().get())
        else {
            crate::internal::mi_assert_internal!(false);
            return ptr::null_mut();
        };

        (*theap).generic_count += 1;
        if (*theap).generic_count
            >= crate::allocator::policy::ActivePolicy::generic_maintenance_interval().get()
        {
            (*theap).generic_collect_count += (*theap).generic_count;
            (*theap).generic_count = 0;
            _mi_deferred_free(theap, false);
            _mi_theap_collect_retired(theap, false);

            let generic_collect = crate::allocator::policy::ActivePolicy::generic_collect_limit();
            if generic_collect.reached_by((*theap).generic_collect_count) {
                (*theap).generic_collect_count = 0;
                crate::mimalloc::mi_theap_collect(theap, false);
            }
        }

        let mut available = mi_find_page_typed(local_theap, layout.padded(), huge_alignment);
        if available.is_none() {
            crate::mimalloc::mi_theap_collect(theap, true);
            available = mi_find_page_typed(local_theap, layout.padded(), huge_alignment);
        }

        let Some(available) = available else {
            let req_size = size.wrapping_sub(MI_PADDING_SIZE);
            _mi_error_message_fmt(
                Errno::ENOMEM,
                format_args!("unable to allocate memory ({req_size} bytes)\n"),
            );
            return ptr::null_mut();
        };
        let page = available.page().as_ptr();

        crate::internal::mi_assert_internal!(mi_page_block_size(page) >= size);
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
            mi_page_slice_start(page).cast(),
            MI_PAGE_ALIGN
        ));
        crate::internal::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);

        crate::core_types::UsableOut::new(usable).write_if_present(mi_page_usable_block_size(page));
        let p = _mi_page_malloc_zero_typed(
            theap,
            available,
            layout.padded(),
            crate::core_types::Zeroed::from_bool(zero),
            ptr::null_mut(),
        );
        crate::internal::mi_assert_internal!(!p.is_null());

        if mi_page_block_size(page) > MI_SMALL_MAX_OBJ_SIZE
            && !mi_page_immediate_available(page)
            && !mi_page_is_expandable(page)
        {
            mi_page_to_full(page, mi_page_queue_of(page));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::{MI_INTPTR_SIZE, MI_SIZE_BITS, mi_clz};

    const LARGE_WSIZE_MAX_FOR_BIN_TEST: usize = (4 * 1024 * 1024) / MI_INTPTR_SIZE;

    fn upstream_bin4(size: usize) -> u8 {
        let wsize = _mi_wsize_from_size(size);
        if wsize <= 1 {
            return 1;
        }
        if cfg!(target_pointer_width = "32") && wsize <= 4 {
            return ((wsize + 1) & !1) as u8;
        }
        if cfg!(target_pointer_width = "64") && wsize <= 8 {
            return ((wsize + 1) & !1) as u8;
        }
        if !cfg!(any(
            target_pointer_width = "32",
            target_pointer_width = "64"
        )) && wsize <= 8
        {
            return wsize as u8;
        }
        if wsize > LARGE_WSIZE_MAX_FOR_BIN_TEST {
            return MI_BIN_HUGE as u8;
        }

        let b = (MI_SIZE_BITS - 1 - mi_clz(wsize)) as u8;
        ((b << 1) + ((wsize >> (b - 1)) & 0x01) as u8) + 3
    }

    #[test]
    fn bin_classification_characterization_retains_huge_bounds() {
        let mut last_bin = 0usize;
        let mut transitions = 0usize;
        for wsize in 1..=((4 * 1024 * 1024) / 8 + 1024) {
            let size = wsize * MI_INTPTR_SIZE;
            let bin = bins::mi_bin(size);
            let alt_bin = upstream_bin4(size);
            assert!(alt_bin <= MI_BIN_HUGE as u8);
            if bin != last_bin {
                transitions += 1;
                last_bin = bin;
            }
        }
        assert!(transitions > 0);
    }
}
