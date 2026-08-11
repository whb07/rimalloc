//! Page management

// Retains upstream page helpers and constants used by selected debug,
// collection, and feature paths outside the routine Rust test target.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;

use crate::allocator::arena::{
    _mi_arenas_collect, _mi_arenas_page_abandon_internal, _mi_arenas_page_alloc,
    _mi_arenas_page_free_typed, MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE,
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
    MI_PAGE_MIN_COMMIT_SIZE, MI_SMALL_SIZE_MAX, MI_STAT, PageBlockLayout, PageBlockRange,
    mi_block_t, mi_page_queue_t, mi_page_t, mi_theap_t,
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
    mi_page_queue_enqueue_from_raw, mi_page_queue_remove_raw, mi_page_queue_remove_typed,
};
use queues::{
    mi_page_queue_move_to_front_typed, mi_page_queue_push_at_end_typed, mi_page_queue_push_raw,
    mi_page_queue_push_typed, mi_theap_contains_queue, mi_theap_direct_cache_start,
    mi_theap_queue_index,
};
pub(crate) use stats_bin::mi_page_bin_typed;
#[allow(unused_imports)]
pub use stats_bin::{_mi_page_stats_bin, mi_page_bin};
pub use validation::_mi_page_is_valid;
pub(crate) use validation::mi_page_is_valid_init;

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

/// Move a full page back to its owning heap's normal block-size queue.
///
/// # Safety
/// `page_wrapper` must identify a live initialized current-thread page linked in its owning heap's
/// full queue and marked full. The heap must disallow page abandonment and its association, full
/// queue, target block-size queue, page geometry, and direct-page cache must remain live and stable.
/// The caller must exclusively own local queue links, counts, cache entries, and the full-flag
/// transition, excluding relink, non-protocol metadata mutation, and page/heap teardown. Protocol
/// atomic remote-free publication remains permitted.
#[inline(never)]
pub(crate) unsafe fn _mi_page_unfull_internal(
    page_wrapper: crate::core_types::FullPage,
) -> crate::core_types::ActivePage {
    // SAFETY: the caller-proven full-page epoch keeps the page, owner, source/target queues, and
    // direct cache live under exclusive local mutation. The temporary flag clear is restored before
    // constructing the linked-full witness; consuming it moves the page and clears the flag once.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(mi_page_is_in_full(page));
        crate::internal::mi_assert_internal!(!(*mi_page_theap(page)).allow_page_abandon);
        let theap = mi_page_theap(page);
        let theap_ptr = ThreadHeapPtr::new_unchecked(theap);
        let pqfull = theap_ptr.page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
        mi_page_set_in_full(page, false);
        let pq = mi_theap_page_queue_of(theap, page);
        mi_page_set_in_full(page, true);
        let from = crate::core_types::FullPageQueue::new_unchecked(pqfull);
        let queued = crate::core_types::QueuedPage::<crate::core_types::FullQueue>::new_unchecked(
            page_wrapper,
            from,
        );
        let to = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        queued.unfull(to)
    }
}

/// Move an unavailable active page to the full queue or abandon it according to heap policy.
///
/// # Safety
/// `page` must identify a live initialized active current-thread page linked in `pq`, its owning
/// source queue (the matching normal block-size queue or huge-page queue). The page must have no
/// immediately available blocks. The heap, source/full queues, page geometry, free-list metadata,
/// direct cache, statistics, and arena
/// backing must remain live and stable. The caller must exclusively own local queue/page/heap
/// mutation, excluding relink, non-protocol mutation, and teardown; protocol atomic remote-free
/// publication remains permitted. If abandonment is enabled, stored arena callbacks and arguments
/// reached synchronously must remain valid, must not incompatibly reenter these page/heap queues,
/// and must not unwind.
#[inline]
unsafe fn mi_page_to_full(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    // SAFETY: the caller-proven current-thread page/queue epoch stabilizes policy and metadata.
    // The abandon arm consumes Active directly; the full arm constructs exact linked-source and
    // compatible source/full queue capabilities, consumes them through the move, then collects
    // through returned Full.
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
            let full_queue = ThreadHeapPtr::new_unchecked(theap)
                .page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
            let active = crate::core_types::assume_active_page(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            );
            let queued =
                crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(
                    active,
                    normal_queue,
                );
            let full =
                queued.into_full(crate::core_types::FullPageQueue::new_unchecked(full_queue));
            _mi_page_free_collect(full.page().as_ptr(), false);
        }
    }
}

/// Detach an all-free active page from its owning source queue and release it terminally.
///
/// # Safety
/// `page_wrapper` must represent a live initialized registered current-thread page with zero used
/// blocks, linked in `queue`, with a stable owner association, geometry, MemId, page-map/arena
/// registration, backing, keys, free lists, and thread-free claim. `queue` must be the owning
/// compatible normal or huge source queue. The caller must exclusively own local queue links,
/// counts, direct cache, page metadata, heap statistics, and release, excluding competing claim,
/// relink, non-protocol mutation, and page/heap/TLD/subprocess/arena teardown; protocol atomic
/// remote-free publication remains permitted. Stored release, purge, or commit callbacks and their
/// arguments reached synchronously must remain valid and must not unwind. The page capability is
/// consumed and the returned identity must not be used to access released metadata.
pub(crate) unsafe fn _mi_page_free_active_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,
    queue: crate::core_types::NormalPageQueue,
) -> crate::core_types::DestroyedPage {
    // SAFETY: the caller-proven all-free page remains linked in this exact live source queue under
    // exclusive local mutation. Clear auxiliary classification, detach once, then consume the sole
    // all-free capability through terminal arena release without later page access.
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

/// Detach an all-free full page from its owning full queue and release it terminally.
///
/// # Safety
/// `page_wrapper` must represent a live initialized registered current-thread page with zero used
/// blocks, linked in `queue` and marked full, with a stable owner association, geometry, MemId,
/// page-map/arena registration, backing, keys, free lists, and thread-free claim. `queue` must be
/// its owning heap's live full queue. The caller must exclusively own local queue links, counts,
/// direct cache, full flag, page metadata, heap statistics, and release, excluding competing claim,
/// relink, non-protocol mutation, and page/heap/TLD/subprocess/arena teardown; protocol atomic
/// remote-free publication remains permitted. Stored release, purge, or commit callbacks and their
/// arguments reached synchronously must remain valid and must not unwind. The page capability is
/// consumed and the returned identity must not be used to access released metadata.
pub(crate) unsafe fn _mi_page_free_full_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,
    queue: crate::core_types::FullPageQueue,
) -> crate::core_types::DestroyedPage {
    // SAFETY: the caller-proven all-free full page remains linked in this exact live full queue
    // under exclusive local mutation. Clear auxiliary classification, detach the original witness
    // once, then consume it through terminal arena release without constructing an alias or later
    // accessing the page.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let pq = queue.ptr().as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(pq == mi_page_queue_of(page));
        crate::internal::mi_assert_internal!(mi_page_all_free(page));

        mi_page_set_has_interior_pointers(page_wrapper.page(), false);
        mi_page_queue_remove_raw(pq, page);

        // SAFETY: the raw full-queue removal above cleared both queue links and the full flag while
        // retaining current-thread ownership and the original all-free capability.
        let page_wrapper = page_wrapper.into_active_after_detach();
        _mi_page_free_after_remove(page_wrapper)
    }
}

/// Consume a queue-detached all-free page through terminal arena release.
///
/// # Safety
/// `page_wrapper` must represent a live initialized registered page with zero used blocks, no
/// queue links, and the thread-free ownership claim held for this release. Its current `theap`
/// must identify the live current-thread owner and statistics account, with a nonnull initialized
/// TLD whose subprocess association remains stable. Page geometry, MemId, page-map registration,
/// heap arena bit, arena metadata, and backing must remain valid until release completes. The
/// caller must exclusively own local page/heap mutation and exclude queue relinking, competing
/// claims, non-protocol metadata mutation, and heap/TLD/subprocess/arena teardown; protocol atomic
/// remote-free publication remains permitted. Stored release, purge, or commit callbacks and their
/// arguments reached synchronously must remain valid and must not unwind. The returned witness is
/// terminal identity only; the released page must not be accessed again.
unsafe fn _mi_page_free_after_remove<S>(
    page_wrapper: crate::core_types::AllFreePage<S>,
) -> crate::core_types::DestroyedPage {
    let page = page_wrapper.page().as_ptr();
    // SAFETY: the caller keeps the detached all-free page and its current-thread owner/TLD live
    // and stable. Snapshot both owner capabilities before the terminal page transition begins.
    let (owner, tld) = unsafe {
        let theap = mi_page_theap(page);
        crate::internal::mi_assert_internal!(!theap.is_null());
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let tld = (*theap).tld;
        crate::internal::mi_assert_internal!(!tld.is_null());
        let tld = crate::core_types::TldPtr::new_unchecked(tld);
        (owner, tld)
    };
    // SAFETY: the captured owner and caller-proven page state cover this coherent terminal epoch:
    // clear the page association, then consume the all-free page exactly once through arena release.
    let destroyed = unsafe {
        mi_page_set_theap(page, ptr::null_mut());
        _mi_arenas_page_free_typed(page_wrapper, owner)
    };
    // SAFETY: the captured live current-thread TLD and stable subprocess association outlive the
    // released page; collection is synchronous and does not access the terminal page identity.
    unsafe { _mi_arenas_collect(false, false, tld.as_ptr()) };
    destroyed
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

/// Retire a sole all-free normal page, or release it immediately when it cannot be retained.
///
/// # Safety
/// `page_wrapper` must represent a live initialized all-free active page linked in its current
/// heap's live queue, with stable page geometry, backing, keys, countdown, queue links, and owner
/// association. Its owner must be the live current-thread heap with exclusive local mutation of
/// the queue, page metadata, retirement bounds, and statistics; the bounds must remain within the
/// page-queue bin domain. Protocol atomic remote-free publication is permitted, but queue relink,
/// competing ownership, non-protocol metadata mutation, and page/heap/TLD/arena teardown must not
/// race this call. Any release or collection callback reached by the immediate-free path and its
/// opaque argument must remain valid for synchronous invocation, must not incompatibly reenter and
/// mutate this page, queue, or heap, and must not unwind. On return the page is either the sole page
/// retained with an initialized countdown, or it has been detached and terminally released.
pub(crate) unsafe fn _mi_page_retire_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,
) {
    let page_ptr = page_wrapper.page();
    let page = page_ptr.as_ptr();
    // SAFETY: the caller keeps this all-free page, its current queue, and adjacent queue metadata
    // live and stable under exclusive local ownership. This preflight validates the page, clears
    // the interior-pointer flag, and snapshots whether it is the sole non-special queue member.
    let (queue, is_sole) = unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::internal::mi_assert_internal!(mi_page_all_free(page));

        if !mi_page_retire_expire(page).is_zero() {
            return;
        }
        mi_page_set_has_interior_pointers(page_ptr, false);

        let pq = mi_page_queue_of(page);
        let queue = page_queue_ptr(pq);
        let is_sole = !mi_page_queue_is_special(pq)
            && queue
                .first()
                .is_some_and(|first| ptr::eq(first.as_ptr(), page))
            && queue
                .last()
                .is_some_and(|last| ptr::eq(last.as_ptr(), page));
        (queue, is_sole)
    };

    if is_sole {
        let retired = page_wrapper.into_retired();
        let retired_page = retired.page();
        let retired_raw = retired_page.as_ptr();
        // SAFETY: the consumed all-free witness established this live sole-page retirement state;
        // all subsequent metadata reads and writes use the replacement RetiredPage capability.
        unsafe {
            let theap = mi_page_theap(retired_raw);
            if MI_STAT > 0 {
                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);
            }
            let block_size = retired_page.block_size();
            mi_page_set_retire_expire(
                retired_raw,
                crate::allocator::policy::ActivePolicy::retire_expire(block_size),
            );
            let index = mi_theap_queue_index(ThreadHeapPtr::new_unchecked(theap), queue);
            crate::internal::mi_assert_internal!(index < MI_BIN_FULL);
            if index < (*theap).page_retired_min {
                (*theap).page_retired_min = index;
            }
            if index > (*theap).page_retired_max {
                (*theap).page_retired_max = index;
            }
            crate::internal::mi_assert_internal!(mi_page_all_free(retired_raw));
        }
        return;
    }

    // SAFETY: preflight kept the live queue and all-free page stable; this branch consumes the
    // original witness through queue removal and terminal release, with no later page access.
    unsafe {
        let queue = crate::core_types::NormalPageQueue::new_unchecked(queue);
        let _ = _mi_page_free_active_internal(page_wrapper, queue);
    }
}

/// # Safety
/// `theap` must be nonnull and identify a live initialized current-thread heap. The caller must
/// exclusively own its local page queues, retirement countdowns, and retirement-bound fields; the
/// stored bounds must be the empty sentinel or a bounded normal-bin interval. Queue heads, queued
/// page metadata, backing, owner association, and TLD/arena state must remain live and stable.
/// Protocol atomic remote-free publication is permitted, but queue relinking, non-protocol page
/// mutation, and heap/page/TLD/arena teardown must not race this collection. Any release or purge
/// callback reached while freeing an expired page and its opaque argument must remain valid for
/// synchronous invocation, must not incompatibly reenter and mutate this heap or its queues, and
/// must not unwind.
pub(crate) unsafe extern "C" fn _mi_theap_collect_retired(theap: *mut mi_theap_t, force: bool) {
    // SAFETY: the caller supplies the nonnull live current-thread heap and exclusive retired-queue
    // epoch required to construct the local witness and synchronously delegate collection.
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        _mi_theap_collect_retired_typed(theap, force);
    }
}

#[inline]
/// # Safety
/// `theap` must remain the live current-thread heap with exclusive mutation of its local queues,
/// retirement countdowns, and bounded retirement min/max fields. Each nonempty retired-bin head
/// and its page metadata, owner association, keys, geometry, and backing must remain live and
/// stable until that bin is processed. Protocol atomic remote-free publication is permitted, but
/// queue relinking, non-protocol page mutation, and heap/page/TLD/arena teardown must not race the
/// scan. Release and purge callbacks reached by terminal page freeing must remain valid for
/// synchronous invocation, must not incompatibly reenter and mutate this heap or queues, and must
/// not unwind. An expired page consumed by terminal release must not be accessed afterward.
unsafe fn _mi_theap_collect_retired_typed(theap: crate::core_types::ThreadHeapLocal, force: bool) {
    let theap_ptr = theap.ptr();
    let theap_raw = theap_ptr.as_ptr();
    // SAFETY: the caller keeps the live current-thread heap stable and exclusively owns its
    // initialized retirement bounds while they are snapshotted for this scan.
    let (retired_min, retired_max) =
        unsafe { ((*theap_raw).page_retired_min, (*theap_raw).page_retired_max) };
    crate::internal::mi_assert_internal!(retired_min <= MI_BIN_FULL);
    crate::internal::mi_assert_internal!(retired_max < MI_BIN_FULL);

    let mut min = MI_BIN_FULL;
    let mut max = 0usize;
    for bin in retired_min..=retired_max {
        // SAFETY: the bounded bin belongs to the caller-owned live heap. Its queue head and any
        // retired page remain stable for this coherent countdown/reset-or-terminal-free epoch.
        // A terminal free consumes the page and this block returns without accessing it again.
        let remains_retired = unsafe {
            let queue_ptr = theap_ptr.page_queue(Bin::new_unchecked(bin as u16));
            let Some(page_ptr) = queue_ptr.first() else {
                continue;
            };
            let page = page_ptr.as_ptr();
            if mi_page_retire_expire(page).is_zero() {
                false
            } else {
                let retired = crate::core_types::assume_retired_page(page_ptr);
                if mi_page_all_free(page) {
                    let expire = mi_page_decrement_retire_expire(page);
                    if expire.is_zero() || force {
                        if !expire.is_zero() {
                            mi_page_reset_retire_expire(page);
                        }
                        let active = retired.into_active_after_countdown_cleared();
                        if let Ok(all_free) = active.try_mark_all_free() {
                            let queue =
                                crate::core_types::NormalPageQueue::new_unchecked(queue_ptr);
                            let _ = _mi_page_free_active_internal(all_free, queue);
                        } else {
                            crate::internal::mi_assert_internal!(false);
                        }
                        false
                    } else {
                        true
                    }
                } else {
                    mi_page_reset_retire_expire(page);
                    let _ = retired.into_active_after_countdown_cleared();
                    false
                }
            }
        };

        if remains_retired {
            min = min.min(bin);
            max = max.max(bin);
        }
    }

    // SAFETY: the caller exclusively owns these live heap fields. All page epochs are complete, so
    // publishing the recomputed bounded range cannot access any page released during the scan.
    unsafe {
        (*theap_raw).page_retired_min = min;
        (*theap_raw).page_retired_max = max;
    }
}

#[inline]
/// # Safety
/// `theap` must remain the live current-thread owner of `page`, with exclusive mutation of its
/// random state, local page free lists, capacity, and statistics; protocol atomic remote-free
/// publication is permitted. `page` must be live and initialized with stable encoded-list keys and
/// backing geometry. `block_range` must be a nonempty range for that same page, containing distinct
/// aligned block headers in writable committed backing, and extending current capacity without
/// exceeding reserved blocks. Existing local/free heads and link words must remain stable except
/// for this publication epoch, and tracking state must permit the balanced link accesses.
unsafe fn mi_page_free_list_extend_secure_typed(
    theap: crate::core_types::ThreadHeapLocal,
    page: crate::core_types::PagePtr,
    block_range: PageBlockRange,
) {
    // SAFETY: the caller provides the same-page writable block range, stable encoded keys and
    // existing heads, exclusive local-list/random-state publication, and balanced tracking access
    // for every link word constructed in this coherent randomized-list epoch.
    unsafe {
        let theap = theap.ptr().as_ptr();
        let bsize = block_range.block_size().get();
        let extend = block_range.block_count();
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::free_list_extend(block_range.block_count());
        if MI_SECURE < 3 {
            crate::internal::mi_assert_internal!(page.free().is_empty());
            crate::internal::mi_assert_internal!(page.local_free().is_empty());
        }
        let capacity = page.capacity();
        crate::internal::mi_assert_internal!(
            capacity.as_usize() + extend <= page.reserved().as_usize()
        );
        let mut shift = MI_MAX_SLICE_SHIFT;
        while (extend >> shift) == 0 {
            shift -= 1;
        }
        let slice_count = 1usize << shift;
        let slice_extend = extend / slice_count;
        crate::internal::mi_assert_internal!(slice_extend >= 1);

        let mut blocks = [ptr::null_mut::<mi_block_t>(); 1 << MI_MAX_SLICE_SHIFT];
        let mut counts = [0usize; 1 << MI_MAX_SLICE_SHIFT];
        let head_step = slice_extend * bsize;
        let mut head = block_range.first().as_ptr();
        for i in 0..slice_count {
            blocks[i] = head;
            counts[i] = slice_extend;
            // SAFETY: the prevalidated range contains `extend` blocks, and
            // `slice_count * slice_extend <= extend`, so each recurrence stays
            // within or one-past the live backing block interval.
            head = head.cast::<u8>().add(head_step).cast::<mi_block_t>();
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

#[inline(never)]
/// # Safety
/// `page` must be live and initialized, locally owned for exclusive free-list mutation, and have
/// stable encoded-list keys and backing geometry; protocol atomic remote-free publication is
/// permitted. `block_range` must be a nonempty range for that same page, containing distinct
/// aligned block headers in writable committed backing and extending current capacity without
/// exceeding reserved blocks. Existing local/free heads and link words must remain stable except
/// for this publication epoch, and tracking state must permit the balanced link accesses.
unsafe fn mi_page_free_list_extend_typed(
    page: crate::core_types::PagePtr,
    block_range: PageBlockRange,
) {
    // SAFETY: the caller provides the same-page writable block range, stable encoded keys and
    // existing heads, exclusive local-list publication, and balanced tracking access for every
    // link word constructed in this coherent sequential-list epoch.
    unsafe {
        let bsize = block_range.block_size().get();
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::free_list_extend(block_range.block_count());
        if MI_SECURE < 3 {
            crate::internal::mi_assert_internal!(page.free().is_empty());
            crate::internal::mi_assert_internal!(page.local_free().is_empty());
        }
        let start = block_range.first();
        let last = block_range.last();
        let mut block = start.as_ptr();
        while block < last.as_ptr() {
            let next = block.cast::<u8>().add(bsize).cast::<mi_block_t>();
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(next));
            block = next;
        }
        last.set_next(page, page.free().head());
        page.set_free(crate::core_types::AvailableFreeList::from_head(start));
    }
}

/// # Safety
/// `theap` and `page` must be nonnull and live, with `theap` the current-thread owner of the
/// initialized page. The caller must exclusively own local free-list, capacity, commitment, and
/// statistics mutation while keeping page geometry, encoded keys, backing, and the heap/page
/// association stable; protocol atomic remote-free publication is permitted. Any OS commit
/// callback and opaque argument reached by extension must remain valid for synchronous invocation,
/// obey its threading/reentrancy contract, and not unwind. Tracking must permit balanced access to
/// newly linked blocks followed by final undefined/no-access restoration over the complete reserved
/// [`PageBlockLayout::area`].
#[inline(never)]
unsafe extern "C" fn mi_page_extend_free(theap: *mut mi_theap_t, page: *mut mi_page_t) -> bool {
    // SAFETY: the caller supplies the matching live current-thread heap/page pair, exclusive local
    // extension ownership, stable backing/keys, valid commit callbacks, and balanced tracking.
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_extend_free_typed(theap, PagePtr::new_unchecked(page))
    }
}

#[inline]
/// # Safety
/// `theap` must remain the live current-thread owner of `page_ptr`. The page must be initialized
/// with stable block geometry, encoded-list keys, backing, and
/// `used <= capacity <= reserved`. The caller must exclusively own its local free-list, capacity,
/// commitment, and heap statistics mutation; protocol atomic remote-free publication is permitted.
/// The not-yet-capacity block interval must be writable if already committed or suitable for the
/// page-covering OS commit operation. Any OS commit callback and opaque argument must remain valid
/// for synchronous invocation, obey its threading/reentrancy contract, and not unwind. Tracking
/// must permit link initialization in the new range followed by undefined/no-access restoration
/// over the complete reserved [`PageBlockLayout::area`].
unsafe fn mi_page_extend_free_typed(
    theap: crate::core_types::ThreadHeapLocal,
    page_ptr: PagePtr,
) -> bool {
    let theap_local = theap;
    let theap = theap.ptr().as_ptr();
    let page = page_ptr.as_ptr();
    // SAFETY: the caller supplies a live initialized page owned by this current-thread heap and
    // keeps its geometry, keys, counts, and local list heads stable. This snapshot performs no
    // mutation and reads every field used by the following safe extension policy.
    let (capacity, reserved, block_size) = unsafe {
        crate::internal::mi_assert_expensive!(mi_page_is_valid_init(page_ptr));
        if MI_SECURE < 3 {
            let free_empty = page_ptr.free().is_empty();
            crate::internal::mi_assert!(free_empty);
            crate::internal::mi_assert!(page_ptr.local_free().is_empty());
            if !free_empty {
                return true;
            }
        }
        (
            page_ptr.capacity(),
            page_ptr.reserved(),
            page_ptr.block_size(),
        )
    };
    if capacity.get() >= reserved.get() {
        return true;
    }

    let bsize = block_size.get();
    let Some(extend) =
        crate::allocator::policy::ActivePolicy::page_extend_count(block_size, capacity, reserved)
    else {
        return true;
    };
    let extend = usize::from(extend.get());

    crate::internal::mi_assert_internal!(extend > 0);
    crate::internal::mi_assert_internal!(extend + capacity.as_usize() <= reserved.as_usize());
    crate::internal::mi_assert_internal!(u16::try_from(extend).is_ok());

    // SAFETY: the caller keeps the initialized page start/block-size/reserved geometry and backing
    // stable; `block_size` is the value snapshotted from this same page.
    let Some(block_layout) = (unsafe { PageBlockLayout::from_page(page_ptr, block_size) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    let Some(block_range) = block_layout.nonempty_range(capacity.as_usize(), extend) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    let track_area = if MI_TRACK_ENABLED != 0 {
        Some(block_layout.area())
    } else {
        None
    };

    // SAFETY: this coherent commitment epoch reads the live page's stable current committed
    // boundary. The page-slice start and backing remain live, and the page-covering range requested
    // from the capacity/extension size lies within that page allocation and is suitable for commit.
    // The caller supplies valid non-unwinding callback state, excludes teardown, and exclusively
    // publishes the updated committed boundary.
    unsafe {
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
                )
                .success()
                {
                    return false;
                }
                mi_page_set_slice_committed(
                    page,
                    crate::core_types::CommittedBytes::new(needed_commit),
                );
            }
        }
    }

    // SAFETY: this coherent link/accounting/tracking publication epoch exclusively owns the
    // current-thread heap stats and page local-list/capacity fields. `block_range` is the same-page
    // nonempty range now backed by writable committed memory; page keys and existing heads remain
    // stable, remote publication is confined to its atomic protocol, and tracking permits link
    // accesses followed by restoration of the complete reserved page area.
    unsafe {
        if MI_STAT > 0 {
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);
        }

        if extend < MI_MIN_SLICES || MI_SECURE < 2 {
            mi_page_free_list_extend_typed(page_ptr, block_range);
        } else {
            mi_page_free_list_extend_secure_typed(theap_local, page_ptr, block_range);
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
    }
    true
}

/// Detach and abandon a current-thread page, or terminally release it when collection makes every
/// block free.
///
/// # Safety
/// `page_wrapper` must identify a live initialized current-thread page linked in `queue`, whose
/// variant matches its active/full state. The caller must exclusively own local queue links, page
/// lists and metadata, owner association, heap statistics, and the abandon transition. Page
/// geometry, backing, keys, MemId, page-map/arena publication, the owning heap, and its nonnull TLD
/// must remain live and stable. Protocol atomic remote-free publication is permitted, but competing
/// claim, queue relink, non-protocol mutation, and page/heap/TLD/subprocess/arena teardown must not
/// race this call. Stored release, purge, or commit callbacks and their arguments must remain valid
/// for synchronous invocation, must not incompatibly reenter and mutate this page, queue, or heap,
/// and must not unwind. The input capability is consumed; the result reports whether arena abandon
/// publication retained the page or terminal release destroyed it.
pub(crate) unsafe fn _mi_page_abandon_internal(
    page_wrapper: crate::core_types::PageForAbandon,
    queue: crate::core_types::PageQueuePtr,
) -> crate::core_types::PageAbandonOutcome {
    let page_ptr = match &page_wrapper {
        crate::core_types::PageForAbandon::Active(page) => page.page(),
        crate::core_types::PageForAbandon::Full(page) => page.page(),
    };
    let page = page_ptr.as_ptr();
    let pq = queue.as_ptr();
    // SAFETY: the caller keeps this live locally owned page and its free-list metadata stable while
    // protocol remote frees are collected and the terminal all-free decision is snapshotted.
    let all_free = unsafe {
        _mi_page_free_collect(page, false);
        mi_page_all_free(page)
    };
    if all_free {
        // SAFETY: the caller-proven queue/page epoch remains exclusive. Each arm consumes the
        // all-free capability through queue removal and terminal release, with no later page use.
        unsafe {
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
                        mi_page_queue_remove_raw(pq, all_free.page().as_ptr());
                        _mi_page_free_after_remove(all_free)
                    }
                }
            };
            let _ = destroyed;
            crate::core_types::PageAbandonOutcome::Destroyed
        }
    } else {
        // SAFETY: the input capability and live queue still describe the same exclusively owned
        // page. Successful typed removal returns Active; fallback raw removal clears links and the
        // full flag before consuming the retained PageForAbandon into the same detached state.
        let detached = unsafe {
            match page_wrapper {
                crate::core_types::PageForAbandon::Full(page) => {
                    if let Some(full_queue) = crate::core_types::FullPageQueue::new(queue) {
                        match crate::core_types::QueuedPage::<
                            crate::core_types::FullQueue,
                        >::try_queue_full(page, full_queue) {
                            Ok(queued) => queued.remove(),
                            Err(page) => {
                                crate::internal::mi_assert_internal!(false);
                                mi_page_queue_remove_raw(pq, page.page().as_ptr());
                                page.into_active_after_detach()
                            }
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                        page.into_active_after_detach()
                    }
                }
                crate::core_types::PageForAbandon::Active(page) => {
                    if let Some(normal_queue) = crate::core_types::NormalPageQueue::new(queue) {
                        match crate::core_types::QueuedPage::<
                            crate::core_types::NormalQueue,
                        >::try_queue(page, normal_queue) {
                            Ok(queued) => queued.remove(),
                            Err(page) => {
                                crate::internal::mi_assert_internal!(false);
                                mi_page_queue_remove_raw(pq, page.page().as_ptr());
                                page
                            }
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                        page
                    }
                }
            }
        };
        let detached_page = detached.page();
        let detached_raw = detached_page.as_ptr();
        // SAFETY: the detached page retains its live current-thread association. Snapshot the
        // matching local heap and nonnull TLD before owner mutation and possible terminal release.
        let (owner, tld) = unsafe {
            let theap = (*detached_raw).theap;
            crate::internal::mi_assert_internal!(!theap.is_null());
            let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            let tld = (*theap).tld;
            crate::internal::mi_assert_internal!(!tld.is_null());
            (owner, crate::core_types::TldPtr::new_unchecked(tld))
        };
        // SAFETY: the page is detached and exclusively locally owned. Clearing its owner publishes
        // abandoned state while retaining the claimed thread-free bit; restoring only the legacy
        // associated-heap pointer preserves same-heap reclaim, then the Active witness is consumed.
        let owned = unsafe {
            mi_page_set_theap(detached_raw, ptr::null_mut());
            (*detached_raw).theap = owner.ptr().as_ptr();
            detached.into_owned_abandoned_after_owner_clear()
        };
        // SAFETY: the typed owned-abandoned page and matching current-thread heap cover synchronous
        // arena publication/unown; the outcome consumes the page capability.
        let outcome = unsafe { _mi_arenas_page_abandon_internal(owned, owner) };
        // SAFETY: the captured TLD and subprocess association remain live independently of a
        // possibly destroyed page; collection does not access the consumed page capability.
        unsafe { _mi_arenas_collect(false, false, tld.as_ptr()) };
        outcome
    }
}

/// Allocate, reclaim if needed, and publish one page into its target queue.
///
/// # Safety
/// `owner` must identify the live current-thread heap that owns `queue`, which must remain a live
/// initialized normal or huge queue for `block_size`. The heap, TLD, subprocess arena registry,
/// page-map and arena backing, page metadata, OS configuration, and stored release, purge, and
/// commit callbacks and their opaque arguments must remain stable throughout the synchronous
/// allocation/reclaim/queue epoch. Callbacks must not unwind or incompatibly reenter allocator
/// operations that mutate the participating heap, queue, page, arena, or backing. Teardown and
/// non-protocol queue/page relinking must not race; atomic abandoned claims and remote-free
/// publication remain permitted by their protocols.
#[inline(never)]
unsafe fn mi_page_fresh_alloc_typed(
    owner: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::PageQueuePtr,
    block_size: crate::core_types::BlockSize,
    page_alignment: crate::core_types::MaybeAlignment,
) -> Option<crate::core_types::ActivePage> {
    // SAFETY: the contract supplies one stable current-thread allocation epoch
    // across arena allocation or claim, reclamation, free-list extension, and
    // final queue publication; each typed capability is consumed in order.
    unsafe {
        let theap = owner.ptr().as_ptr();
        let pq = queue.as_ptr();
        let allocation = _mi_arenas_page_alloc(owner, block_size, page_alignment)?;
        let page = match allocation {
            crate::allocator::arena::ArenaPageAllocation::Claimed(page) => {
                let page = _mi_theap_page_reclaim(page, owner);
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
            }
            crate::allocator::arena::ArenaPageAllocation::Active(page) => {
                if mi_page_queue_is_huge(pq) {
                    mi_page_queue_push_raw(theap, pq, page.page().as_ptr());
                    page
                } else {
                    let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(queue);
                    mi_page_queue_push_typed(owner, normal_queue, page).into_active()
                }
            }
        };
        crate::internal::mi_assert_internal!(
            mi_page_block_size(page.page().as_ptr()) >= block_size.get()
        );
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page.page().as_ptr()));
        Some(page)
    }
}

/// # Safety
/// `owner` must be the live current-thread owner of the registered fresh page,
/// and no other page-state witness or concurrent access may exist during
/// initialization.
#[inline(never)]
pub(crate) unsafe fn mi_page_init_typed(
    owner: crate::core_types::ThreadHeapLocal,
    page_uninit: crate::core_types::Page<crate::core_types::Uninit>,
) -> Result<crate::core_types::ActivePage, crate::core_types::Page<crate::core_types::Uninit>> {
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
            return Err(page_uninit);
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
            return Err(page_uninit);
        }
        let active = page_uninit.initialize(owner);
        crate::internal::mi_assert!(mi_page_immediate_available(active.page().as_ptr()));
        Ok(active)
    }
}

#[inline(never)]
/// # Safety
/// `page_wrapper` must carry the caller's active ownership claim for a live initialized abandoned
/// page that is detached from thread-owned queues and not concurrently claimed, destroyed, or
/// non-protocol mutated. Its slice/block geometry, backing, encoded keys, and abandoned/local/
/// remote free-list metadata must remain stable; protocol atomic remote-free publication is
/// permitted. `target` must remain the live current-thread heap selected to receive the page, with
/// exclusive mutation of its local queues, page association, free lists, and statistics and no
/// concurrent teardown or relinking. The target queue derived from the page block size must remain
/// live and belong to `target`. Any registered diagnostic callback reached while collecting lists
/// must remain valid for synchronous invocation and must not unwind. This call consumes the owned-
/// abandoned transition, reassigns the owner, publishes the page in the target normal queue, and
/// returns the resulting active-page witness.
pub(crate) unsafe fn _mi_theap_page_reclaim(
    page_wrapper: crate::core_types::OwnedAbandonedPage,
    target: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::ActivePage {
    let page = page_wrapper.page().as_ptr();
    let theap = target.ptr().as_ptr();
    // SAFETY: the caller supplies the live exclusively claimed abandoned page, stable geometry and
    // free-list metadata, plus the live current-thread target with exclusive queue mutation. This
    // coherent epoch validates the claim/geometry, reassigns ownership, collects protocol remote
    // frees, derives the target queue, and consumes the claim into its sole queue publication.
    let queued = unsafe {
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
            target,
            crate::core_types::NormalPageQueue::new_unchecked(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ),
            page_wrapper.into_reclaimed(target),
        );
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(queued.page().as_ptr()));
        queued
    };
    queued.into_active()
}

/// Validate one queue's linked-page topology and classification against its owning thread heap.
///
/// # Safety
/// `theap` must be a live initialized locally owned thread heap, under current-thread ownership or
/// exclusive initialization/teardown access, and `queue` must be the live queue selected from that
/// heap's fixed bin table. Its first/last pointers, count, block-size classification, and every
/// linked page's next/previous links, owner association, classification, geometry, metadata, and
/// backing must remain initialized and readable throughout traversal. The caller must hold the
/// corresponding current-thread local mutation ownership or exclusive initialization/teardown
/// access and exclude concurrent relink, non-protocol metadata mutation, or page/heap teardown.
/// Protocol atomic remote-free publication is permitted because this validation does not inspect
/// or mutate remote free-list heads.
#[inline]
pub(crate) unsafe fn mi_page_queue_is_valid_typed(
    theap: crate::core_types::ThreadHeapLocal,
    pq: crate::core_types::PageQueuePtr,
) -> bool {
    // SAFETY: the caller keeps the exact heap-owned queue and each linked page live with stable
    // non-atomic topology/classification fields. This coherent read-only traversal checks backward
    // links, bin compatibility, owner association, terminal identity, and the final stored count.
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
        if theap_ptr.direct_page(direct_idx).as_ptr() == page {
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
            crate::core_types::PagePtr::new_unchecked(page),
        );
    }
}

/// Search a normal queue for an available page, extending, retiring, releasing, or allocating a
/// page as required.
///
/// # Safety
/// `theap` must be the live current-thread owner of `queue`, and `queue` must be its live normal
/// queue for `block_size`. Every linked page, its backing, geometry, keys, free lists, queue links,
/// owner association, retirement state, and commit metadata must remain valid through the search.
/// The caller must exclusively own local queue, direct-cache, free-list, page-state, random-state,
/// and statistics mutation and exclude teardown, relinking, or other non-protocol page mutation;
/// protocol atomic remote-free publication and abandoned-page claims remain permitted. TLD,
/// subprocess, arena and page-map registries, OS configuration, and any stored release, purge, or commit callbacks and
/// their opaque arguments reached during extension, collection, or fresh allocation must remain
/// valid. Synchronous callbacks must not unwind or incompatibly reenter operations that mutate the
/// participating heap, queue, page, arena, or backing. A successful result is the sole active
/// capability returned from this search epoch.
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
            // SAFETY: this cold fresh-page branch retains the caller-proven current-thread heap
            // epoch and exact live normal queue. Its typed block size and zero/default alignment
            // pass unchanged into the outlined transaction, while the surrounding search contract
            // keeps arena/page-map backing and release, purge, and commit callbacks stable and
            // excludes incompatible callback reentry or unwind.
            let fresh = mi_page_fresh_alloc_typed(
                theap,
                queue.ptr(),
                block_size,
                crate::core_types::MaybeAlignment::new_unchecked(0),
            );
            if let Some(fresh) = fresh {
                crate::internal::mi_assert_internal!(
                    block_size.get() == mi_page_block_size(fresh.page().as_ptr())
                );
                crate::internal::mi_assert_internal!(
                    queue.ptr().as_ptr()
                        == mi_theap_page_queue_of(theap.ptr().as_ptr(), fresh.page().as_ptr())
                );
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

/// Return an available page from the front of a normal queue or from the complete queue-search
/// and fresh-page fallback.
///
/// # Safety
/// `theap` must be the live current-thread owner of `queue`, and `queue` must be its live normal
/// queue for `block_size`. Every linked page, its backing, geometry, keys, free lists, queue links,
/// owner association, retirement state, and commit metadata must remain valid. The caller must
/// exclusively own local queue, direct-cache, free-list, page-state, random-state, and statistics
/// mutation while excluding teardown, relinking, or non-protocol metadata mutation; protocol
/// atomic remote-free publication and abandoned-page claims remain permitted. TLD, subprocess,
/// arena/page-map registries, OS configuration, and stored release, purge, and commit callbacks
/// and their opaque arguments must remain valid. Synchronous callbacks must not unwind or
/// incompatibly reenter mutation of the participating heap, queue, page, arena, or backing.
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
        // SAFETY: the caller-proven current-thread queue epoch covers the complete active search;
        // success establishes immediate availability before this consumed refinement.
        mi_page_queue_find_free_ex_active(theap, queue, block_size, true)
            .and_then(|active| active.try_available().ok())
    }
}

/// Allocate one singleton huge page and return its available-list capability.
///
/// # Safety
/// `owner` must be the live current-thread owner of `queue`, and `queue` must be its live huge
/// queue. `layout` and `page_alignment` must describe the requested huge-page allocation. The
/// caller must exclusively own local huge-queue and statistics mutation while heap/TLD/subprocess,
/// arena/page-map registries, page backing and metadata remain live and stable against teardown,
/// relinking, or non-protocol mutation. Protocol abandoned claims and atomic remote-free
/// publication remain permitted. Stored release, purge, and commit callbacks and their opaque
/// arguments reached synchronously must remain valid and must not unwind or incompatibly reenter
/// operations that mutate the participating heap, queue, page, arena, or backing.
#[inline]
unsafe fn mi_huge_page_alloc_typed(
    owner: crate::core_types::ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    page_alignment: crate::core_types::MaybeAlignment,
    queue: crate::core_types::HugePageQueue,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: the caller-proven current-thread heap and exact huge queue remain live under
    // exclusive local queue/stat mutation. The typed layout/alignment are passed unchanged through
    // the outlined arena/reclaim/publication transaction; backing registries and synchronous
    // release, purge, and commit callbacks remain stable, nonunwinding, and nonreentrant under its
    // contract.
    unsafe {
        let theap = owner.ptr().as_ptr();
        let active = mi_page_fresh_alloc_typed(
            owner,
            queue.ptr(),
            crate::core_types::BlockSize::from_padded(layout.padded()),
            page_alignment,
        )?;
        let page = active.page().as_ptr();
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
        active.try_available().ok()
    }
}

#[inline(always)]
/// Select an available page for a validated padded allocation request.
///
/// # Safety
/// `theap` must remain the live current-thread heap, with exclusive access to
/// its local direct cache, page queues, free lists, random state, and statistics
/// while protocol atomic remote-free publication and abandoned-page claims may
/// proceed. `size` must be the exact nonzero padded size for the request and
/// `huge_alignment` its validated optional huge alignment. The heap's TLD and
/// subprocess association, queue links and page metadata, arena/page-map
/// registries, backing allocations, and initialized OS configuration must remain
/// live and stable against teardown, relinking, or non-protocol mutation.
/// Registered diagnostic, release, purge, and commit callbacks and their opaque
/// arguments must remain valid for synchronous invocation, must not unwind, and
/// must not incompatibly reenter mutation of the participating heap, queue,
/// page, arena, or backing.
unsafe fn mi_find_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    size: crate::core_types::PaddedSize,
    huge_alignment: crate::core_types::MaybeAlignment,
) -> Option<crate::core_types::AvailablePage> {
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
    // SAFETY: the caller keeps the live current-thread heap and its queue array
    // stable; `size` is nonzero and the huge-bin selector is also nonzero, so
    // the block-size materialization and nonnull queue projection/classification
    // are valid for reuse by exactly one selected branch below.
    let (pq, is_huge) = unsafe {
        let pq = crate::core_types::PageQueuePtr::new_unchecked(mi_page_queue(
            theap.ptr().as_ptr(),
            crate::core_types::BlockSize::new_unchecked(queue_block_size),
        ));
        (pq, mi_page_queue_is_huge(pq.as_ptr()))
    };
    if is_huge || req_size > MI_MAX_ALLOC_SIZE {
        // SAFETY: queue lookup selected the live huge queue for this heap. The
        // caller keeps OS configuration, heap/arena registries, backing, stats,
        // and synchronous callbacks valid through size rounding and allocation.
        unsafe {
            let Some(queue) = crate::core_types::HugePageQueue::new(pq) else {
                crate::internal::mi_assert_internal!(false);
                return None;
            };
            let block_size = _mi_os_good_alloc_size(size.get());
            let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
                crate::core_types::PaddedSize::new_unchecked(block_size),
                crate::core_types::Zeroed::No,
            );
            mi_huge_page_alloc_typed(theap, layout, huge_alignment, queue)
        }
    } else {
        // SAFETY: queue lookup selected a live ordinary size-class queue owned
        // by `theap`; its metadata and linked pages remain stable under the
        // caller's exclusive local allocation epoch through the page search.
        unsafe {
            let Some(queue) = crate::core_types::NormalPageQueue::new(pq) else {
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
/// Resolve an optional or lazy thread-heap pointer to an initialized thread heap for the current
/// allocation path.
///
/// # Safety
/// `theap` must be null, one of the allocator's static empty/wrong-thread sentinels, or a live,
/// aligned allocator thread-heap pointer whose `heap` pointer field is initialized and readable.
/// Any nonnull input metadata must remain live and readable through its probe. The allocator
/// process/TLS lifecycle must permit current-thread initialization and default-heap lookup, with
/// no concurrent teardown or mutation of this thread's default TLS slot. Primitive/TLS dynamic
/// dispatch may already be initialized, or the lifecycle must permit `mi_thread_init` to establish
/// it before subsequent reads. Any initialization callbacks and opaque arguments reached
/// synchronously must remain valid, must not incompatibly reenter thread initialization, and must
/// not unwind.
pub(crate) unsafe fn _mi_theap_or_default_typed(
    mut theap: *mut mi_theap_t,
) -> Option<crate::core_types::ThreadHeapPtr> {
    // SAFETY: the input contract limits `theap` to null, static allocator sentinels, or live
    // readable allocator metadata, so the initialization probe may read its heap pointer field.
    if !unsafe { mi_theap_is_initialized(theap) } {
        if ptr::eq(theap, ptr::addr_of!(_mi_theap_empty_wrong).cast_mut()) {
            return None;
        }
        crate::allocator::init::mi_thread_init();
        // SAFETY: initialization completed for this thread and its default TLS slot remains stable
        // against concurrent teardown or mutation during this lookup.
        theap = unsafe { crate::prim::_mi_theap_default() };
        // SAFETY: the default lookup returns a static sentinel or live allocator thread heap, so
        // its heap pointer field is readable for this post-initialization check.
        if !unsafe { mi_theap_is_initialized(theap) } {
            return None;
        }
        // SAFETY: preserve the original assertion-conditional second default-slot read after
        // successful initialization; the current thread's TLS slot remains stable for both reads.
        crate::internal::mi_assert_internal!(unsafe { crate::prim::_mi_theap_default() } == theap);
    }
    // SAFETY: preserve the original final metadata check; the selected static sentinel or live
    // thread heap remains readable and stable through return.
    crate::internal::mi_assert_internal!(unsafe { mi_theap_is_initialized(theap) });

    // SAFETY: the successful initial or post-initialization probe proves `theap` is nonnull; the
    // returned pointer wrapper carries no additional lifetime or ownership claim.
    Some(unsafe { crate::core_types::ThreadHeapPtr::new_unchecked(theap) })
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
        // SAFETY: `available` was found from `local_theap` during its exclusive
        // allocation epoch; the checks above establish the padded-size fit and
        // backing geometry, while its page/free-list metadata remains live.
        let p = _mi_page_malloc_zero_typed(
            local_theap,
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
