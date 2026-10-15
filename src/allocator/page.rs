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
#[allow(unused_imports)]
pub use lifecycle::_mi_page_free_collect;
#[cfg(test)]
pub(crate) use lifecycle::collect_page_free_lists_local;
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
#[allow(unused_imports)]
pub use stats_bin::_mi_page_stats_bin;
pub(crate) use stats_bin::mi_page_bin_typed;
pub use validation::_mi_page_is_valid;
pub(crate) use validation::mi_page_is_valid_init;

pub const MI_OS_PAGE_SIZE: usize = 4096;
pub const MI_SMALL_PAGE_SIZE: usize = MI_ARENA_SLICE_SIZE;
pub const MI_MEDIUM_PAGE_SIZE: usize = 8 * MI_SMALL_PAGE_SIZE;
pub const MI_LARGE_PAGE_SIZE: usize = MI_SIZE_SIZE * MI_MEDIUM_PAGE_SIZE;
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
        // C asserts the theap cannot abandon; here a full page may also have been kept while the
        // process had one thread (`MI_PROCESS_MULTI_THREADED`).
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
/// Set once a thread other than the main thread initializes its allocator state (Rust-only;
/// `UPSTREAM_DIFF.md`). Until then no other thread can reclaim an abandoned page, so full pages
/// stay in their owner's full queue, where the owner's next free returns them without atomics.
pub(crate) static MI_PROCESS_MULTI_THREADED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Abandon the full pages a theap kept while the process had one thread, as C would have when they
/// filled, once another thread exists to reclaim them.
///
/// # Safety
/// `theap` must be the live current-thread theap, with exclusive access to its queues.
pub(crate) unsafe fn mi_theap_abandon_kept_full_pages(theap: *mut mi_theap_t) {
    // SAFETY: the caller owns the live theap and its queues; each iteration abandons (or frees)
    // the first full-queue page, removing it from the queue.
    unsafe {
        if !(*theap).allow_page_abandon
            || !MI_PROCESS_MULTI_THREADED.load(core::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        let full_queue =
            ThreadHeapPtr::new_unchecked(theap).page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
        loop {
            let first = (*full_queue.as_ptr()).first;
            if first.is_null() {
                break;
            }
            let full = crate::allocator::raw_bridge::assume_full_page(
                crate::core_types::PagePtr::new_unchecked(first),
            );
            let _ = _mi_page_abandon_internal(full.into_abandon_candidate(), full_queue);
        }
    }
}

#[inline]
unsafe fn mi_page_to_full(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    // SAFETY: the caller keeps this live active page and exact source queue stable. Preserve the
    // source-queue, immediate-availability, then first full-state assertion reads exactly.
    unsafe {
        crate::internal::mi_assert_internal!(pq == mi_page_queue_of(page));
        crate::internal::mi_assert_internal!(!mi_page_immediate_available(page));
        crate::internal::mi_assert_internal!(!mi_page_is_in_full(page));
    }

    // SAFETY: the caller-proven current-thread ownership keeps the page association live and
    // stable for this owner snapshot after the assertion-only reads.
    let theap = unsafe { mi_page_theap(page) };
    // SAFETY: the captured owner remains live and its local abandonment policy is stable under the
    // caller's exclusive heap mutation epoch.
    let allow_page_abandon = unsafe { (*theap).allow_page_abandon }
        && MI_PROCESS_MULTI_THREADED.load(core::sync::atomic::Ordering::Relaxed);
    if allow_page_abandon {
        // SAFETY: the active page remains linked in the exact source queue with stable owner,
        // geometry, backing, and remote-free state. Consume Active through the complete abandon or
        // terminal-release transition, then do not access the page again in this branch.
        unsafe {
            let active = crate::allocator::raw_bridge::assume_active_page(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let _ = _mi_page_abandon_internal(
                active.into_abandon_candidate(),
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            );
        }
    } else if !unsafe {
        // SAFETY: only the non-abandon path performs this original second full-state read; the live
        // current-thread page remains stable under exclusive local queue mutation.
        mi_page_is_in_full(page)
    } {
        // SAFETY: the non-full active page remains linked in its exact compatible normal or huge
        // source queue. Derive the owner's full queue, consume the linked Active capability through
        // move/full-flag publication, then collect only through the returned Full capability.
        unsafe {
            let full_queue = ThreadHeapPtr::new_unchecked(theap)
                .page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
            let active = crate::allocator::raw_bridge::assume_active_page(
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

pub(crate) unsafe extern "C" fn _mi_page_retire(page: *mut mi_page_t) {
    unsafe {
        let active = crate::allocator::raw_bridge::assume_active_page(
            crate::core_types::PagePtr::new_unchecked(page),
        );
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
        let remains_retired = {
            // SAFETY: the bounded bin belongs to the caller-owned live heap, whose initialized queue
            // array remains stable while its exact queue is projected.
            let queue_ptr = unsafe { theap_ptr.page_queue(Bin::new_unchecked(bin as u16)) };
            // SAFETY: exclusive queue ownership keeps the initialized head link stable for this read.
            let Some(page_ptr) = (unsafe { queue_ptr.first() }) else {
                continue;
            };
            let page = page_ptr.as_ptr();
            // SAFETY: the selected live queue-front page and its initialized countdown remain stable
            // under the caller's exclusive retired-page epoch.
            let retire_expire = unsafe { mi_page_retire_expire(page) };
            if retire_expire.is_zero() {
                false
            } else {
                // SAFETY: the live current-thread normal-queue front has the required nonzero
                // retirement countdown, and the caller excludes relink, conflicting mutation, and
                // teardown.
                let retired =
                    unsafe { crate::allocator::raw_bridge::assume_retired_page(page_ptr) };
                // SAFETY: the caller keeps the retired page metadata live and stable while its
                // used-count predicate is snapshotted before any countdown or state transition.
                let all_free = unsafe { mi_page_all_free(page) };
                if all_free {
                    // SAFETY: the countdown was observed nonzero above and remains exclusively
                    // mutable; preserve the original decrement-before-force-decision order.
                    let expire = unsafe { mi_page_decrement_retire_expire(page) };
                    if expire.is_zero() || force {
                        // SAFETY: the caller owns this coherent reset-and-terminal-transition epoch.
                        // A forced nonzero countdown is cleared before consuming RetiredPage into
                        // Active; the all-free refinement then consumes the sole active capability
                        // into terminal queue removal and release, with no subsequent access through
                        // `page` or `retired`.
                        unsafe {
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
                        }
                        false
                    } else {
                        true
                    }
                } else {
                    // SAFETY: the caller exclusively owns the non-all-free retired page. Clear its
                    // countdown, then consume the retired witness into the matching active state
                    // without accessing the old capability afterward.
                    unsafe {
                        mi_page_reset_retire_expire(page);
                        let _ = retired.into_active_after_countdown_cleared();
                    }
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
    let theap = theap.ptr().as_ptr();
    let bsize = block_range.block_size().get();
    let extend = block_range.block_count();
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::free_list_extend(block_range.block_count());
    if MI_SECURE < 3 {
        // SAFETY: the caller exclusively stabilizes the initialized available-list head. Keeping
        // this read in the assertion operand preserves assertion-only evaluation and read order.
        crate::internal::mi_assert_internal!(unsafe { page.free().is_empty() });
        // SAFETY: the same live page's local-list head remains stable and is read only after the
        // available-list assertion, exactly as required by the secure builder preflight.
        crate::internal::mi_assert_internal!(unsafe { page.local_free().is_empty() });
    }
    // SAFETY: the caller keeps the live initialized page counts stable through partition setup.
    let capacity = unsafe { page.capacity() };
    // SAFETY: the same stable count snapshot permits the assertion-only reserved read after
    // capacity; the caller guarantees the extension stays within the page's reserved range.
    crate::internal::mi_assert_internal!(
        capacity.as_usize() + extend <= unsafe { page.reserved().as_usize() }
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
        head = unsafe { head.cast::<u8>().add(head_step).cast::<mi_block_t>() };
    }
    counts[slice_count - 1] += extend % slice_count;

    // SAFETY: the caller exclusively owns the live current-thread heap's random state; this is the
    // algorithm's single random-state read/mutation after partition construction.
    let r = unsafe { _mi_theap_random_next(theap) };
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
        // SAFETY: `block` is this partition's current in-range block and its remaining count proves
        // advancing by one stable block size stays within or one-past `block_range`.
        blocks[current] = unsafe { block.cast::<u8>().add(bsize).cast::<mi_block_t>() };
        // SAFETY: `block` is nonnull, aligned, and writable in the caller-provided committed range;
        // `blocks[next]` is the next same-page partition head. Exclusive list mutation, stable page
        // keys, and tracking access permit this one encoded-link publication.
        unsafe {
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(blocks[next]));
        }
        current = next;
    }
    // SAFETY: the selected terminal partition retains a nonnull in-range block after the loop.
    // Materialize it before the intentionally late read of the prior page head.
    let tail = unsafe { crate::core_types::BlockPtr::new_unchecked(blocks[current]) };
    // SAFETY: the caller keeps the initialized prior available-list head stable until the new
    // terminal link is installed; this remains the algorithm's late `page.free` read.
    let prior_head = unsafe { page.free().head() };
    // SAFETY: `tail` is writable committed same-page backing, `prior_head` is null or in the same
    // encoded-list domain, and exclusive mutation plus stable keys permit terminal link publication.
    unsafe { tail.set_next(page, prior_head) };
    // SAFETY: `free_start` was selected from the first nonempty partition and is nonnull, aligned,
    // and linked through `tail`; publish the new page head only after every link is initialized.
    unsafe {
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
    let bsize = block_range.block_size().get();
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::free_list_extend(block_range.block_count());
    if MI_SECURE < 3 {
        // SAFETY: the caller exclusively stabilizes the initialized available-list head. Keeping
        // this read in the assertion operand preserves assertion-only evaluation and read order.
        crate::internal::mi_assert_internal!(unsafe { page.free().is_empty() });
        // SAFETY: the same live page's local-list head remains stable and is read only after the
        // available-list assertion, exactly as required by the sequential builder preflight.
        crate::internal::mi_assert_internal!(unsafe { page.local_free().is_empty() });
    }
    let start = block_range.first();
    let last = block_range.last();
    let mut block = start.as_ptr();
    while block < last.as_ptr() {
        // SAFETY: `block` is an in-range block before `last`; advancing by the range's stable block
        // size yields the next block, at most the same allocation's terminal `last` block.
        let next = unsafe { block.cast::<u8>().add(bsize).cast::<mi_block_t>() };
        // SAFETY: `block` and `next` are distinct aligned blocks in the caller-provided committed
        // same-page range. Exclusive list mutation, stable page keys, and tracking access permit
        // this encoded link write before advancing the sequential cursor.
        unsafe {
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(next));
        }
        block = next;
    }
    // SAFETY: the caller keeps the initialized prior available-list head stable until the new
    // terminal link is installed; this remains the algorithm's intentionally late head read.
    let prior_head = unsafe { page.free().head() };
    // SAFETY: `last` is the writable final block in the committed same-page range, and `prior_head`
    // is null or in the same encoded-list domain. Publish the terminal link before the page head.
    unsafe { last.set_next(page, prior_head) };
    // SAFETY: `start` is nonnull and now heads the completely initialized sequential chain; the
    // caller exclusively owns the live page's available-head publication.
    unsafe {
        page.set_free(crate::core_types::AvailableFreeList::from_head(start));
    }
}

/// # Safety
/// `theap` and `page` must be nonnull and live, with `theap` the current-thread owner of the
/// initialized page. The caller must exclusively own local free-list, capacity, commitment,
/// statistics, and, when selected, secure-builder heap random-state mutation while keeping page
/// geometry, encoded keys, backing, and the heap/page association stable; protocol atomic
/// remote-free publication is permitted. OS configuration and current-subprocess commit accounting
/// must be initialized and remain stable and live through extension. Any primitive commit callback
/// and opaque argument reached by extension must remain valid for synchronous invocation, must not
/// incompatibly reenter this heap's local extension state, and must not unwind. Tracking must permit
/// balanced access to newly linked blocks followed by final undefined/no-access restoration over
/// the complete reserved [`PageBlockLayout::area`].
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
/// commitment, heap statistics, and, when selected, secure-builder heap random-state mutation;
/// protocol atomic remote-free publication is permitted. The not-yet-capacity block interval must
/// be writable if already committed or suitable for the page-covering OS commit operation. OS
/// configuration and current-subprocess commit accounting must be initialized and remain stable
/// and live through extension. Any primitive commit callback and opaque argument must remain valid
/// for synchronous invocation, must not incompatibly reenter this heap's local extension state,
/// and must not unwind. Tracking must permit link initialization in the new range followed by
/// undefined/no-access restoration over the complete reserved [`PageBlockLayout::area`].
unsafe fn mi_page_extend_free_typed(
    theap: crate::core_types::ThreadHeapLocal,
    page_ptr: PagePtr,
) -> bool {
    let theap_local = theap;
    let theap = theap.ptr().as_ptr();
    let page = page_ptr.as_ptr();
    // SAFETY: the caller keeps this live initialized page's geometry, counts, lists, ownership, and
    // backing stable. Keeping the read in the assertion operand preserves assertion-only evaluation.
    crate::internal::mi_assert_expensive!(unsafe { mi_page_is_valid_init(page_ptr) });
    if MI_SECURE < 3 {
        // SAFETY: the caller exclusively owns the live page's initialized available-list head. This
        // remains the first list read before the assertion and local-list check.
        let free_empty = unsafe { page_ptr.free().is_empty() };
        crate::internal::mi_assert!(free_empty);
        // SAFETY: the same page's initialized local-list head remains stable. Keeping this read in
        // the assertion operand preserves its original assertion-only evaluation.
        crate::internal::mi_assert!(unsafe { page_ptr.local_free().is_empty() });
        if !free_empty {
            return true;
        }
    }
    // SAFETY: the caller keeps the page's initialized capacity stable for this first count read.
    let capacity = unsafe { page_ptr.capacity() };
    // SAFETY: the initialized reserved count remains stable and is read after capacity exactly as
    // required by the extension policy.
    let reserved = unsafe { page_ptr.reserved() };
    // SAFETY: the initialized nonzero block size remains stable and is read after both counts.
    let block_size = unsafe { page_ptr.block_size() };
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

    // SAFETY: the caller keeps this live page's initialized committed boundary stable for this
    // first read, before any possible commit or boundary publication.
    let committed = unsafe { mi_page_slice_committed(page) };
    if !committed.is_zero() {
        let needed_size = (capacity.as_usize() + extend) * bsize;
        // SAFETY: the page-slice start and stable geometry place `needed_size` within this page's
        // allocation; this computes the same page-relative extent used by the commit policy.
        let needed_commit = _mi_align_up(
            unsafe { mi_page_slice_offset_of(page, needed_size) },
            MI_PAGE_MIN_COMMIT_SIZE,
        );
        if needed_commit > committed.get() {
            // SAFETY: initialized OS configuration remains stable through this assertion-only page
            // size read.
            crate::internal::mi_assert_internal!(
                (needed_commit - committed.get())
                    .is_multiple_of(unsafe { _mi_os_page_size_typed().get() })
            );
            // SAFETY: this coherent commit-and-publication epoch covers a page-backed range derived
            // from the stable slice geometry. OS configuration, current-subprocess accounting, and
            // the primitive callback state remain valid and stable; incompatible reentry and unwind
            // are excluded. The committed boundary is published only after a successful callback.
            let committed_update_succeeded = unsafe {
                if !_mi_os_commit(
                    mi_page_slice_start(page)
                        .cast::<u8>()
                        .add(committed.get())
                        .cast::<c_void>(),
                    needed_commit - committed.get(),
                )
                .success()
                {
                    false
                } else {
                    mi_page_set_slice_committed(
                        page,
                        crate::core_types::CommittedBytes::new(needed_commit),
                    );
                    true
                }
            };
            if !committed_update_succeeded {
                return false;
            }
        }
    }

    if MI_STAT > 0 {
        // SAFETY: the caller exclusively owns this live current-thread heap's statistics. Preserve
        // the pages-extended counter update before any free-list or capacity publication.
        unsafe {
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);
        }
    }

    if extend < MI_MIN_SLICES || MI_SECURE < 2 {
        // SAFETY: this coherent builder-and-capacity publication epoch exclusively owns the live
        // page's local free list and capacity. `block_range` is the same-page nonempty range now
        // backed by writable committed memory; keys and existing heads remain stable, and remote
        // publication is confined to its atomic protocol.
        unsafe {
            mi_page_free_list_extend_typed(page_ptr, block_range);
            mi_page_add_capacity_count(page, extend as u16);
        }
    } else {
        // SAFETY: this coherent secure-builder-and-capacity publication epoch has the same page and
        // backing guarantees as the plain branch. It additionally has exclusive access to this
        // heap's live random state through randomized link construction and capacity publication.
        unsafe {
            mi_page_free_list_extend_secure_typed(theap_local, page_ptr, block_range);
            mi_page_add_capacity_count(page, extend as u16);
        }
    }
    if MI_STAT > 0 {
        // SAFETY: capacity publication is complete and the caller still exclusively owns the live
        // heap statistics. Preserve the page-committed byte update after capacity.
        unsafe {
            __mi_stat_increase(
                ptr::addr_of_mut!((*theap).stats.page_committed),
                crate::core_types::CommittedBytes::new(extend * bsize).get(),
            );
        }
    }

    if let Some(area) = track_area {
        // SAFETY: link construction and capacity publication are complete; tracking permits the
        // complete reserved page area to become undefined and then no-access in this exact order.
        unsafe {
            mi_track_mem_undefined(area.start().as_ptr().cast(), area.byte_len());
            mi_track_mem_noaccess(area.start().as_ptr().cast(), area.byte_len());
        }
    }
    // SAFETY: all commit, link, capacity, accounting, and tracking updates are complete while the
    // page remains live and stable. Keep final validation assertion-only and last.
    crate::internal::mi_assert_expensive!(unsafe { mi_page_is_valid_init(page_ptr) });
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
    let theap = owner.ptr().as_ptr();
    let pq = queue.as_ptr();
    // SAFETY: the caller keeps the current-thread heap, exact target queue, arena/page-map
    // registry, backing, and synchronous allocation callbacks live and stable for this allocation
    // or abandoned-page claim. The typed request values pass through unchanged.
    let allocation = (unsafe { _mi_arenas_page_alloc(owner, block_size, page_alignment) })?;
    let page = match allocation {
        crate::allocator::arena::ArenaPageAllocation::Claimed(page) => {
            // SAFETY: the claimed abandoned page, current-thread owner, arena registration,
            // geometry, and free-list metadata remain live and exclusively owned while reclaim
            // consumes the claim and publishes the matching Active capability.
            let page = unsafe { _mi_theap_page_reclaim(page, owner) };
            // SAFETY: the reclaimed Active page remains live with stable available-list metadata
            // for this read before any optional extension.
            let immediate_available = unsafe { mi_page_immediate_available(page.page().as_ptr()) };
            if !immediate_available {
                // SAFETY: the same reclaimed page keeps its count and commit geometry stable while
                // expandability is classified after the failed availability read.
                let expandable = unsafe { mi_page_is_expandable(page.page().as_ptr()) };
                if expandable {
                    // SAFETY: the live reclaimed page and current-thread heap remain exclusively
                    // locally mutable; commit callbacks and backing stay valid through extension.
                    let extended = unsafe { mi_page_extend_free(theap, page.page().as_ptr()) };
                    if !extended {
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
            // SAFETY: the target queue remains live and initialized; this read classifies the exact
            // queue before consuming the fresh Active capability through one publication path.
            let is_huge = unsafe { mi_page_queue_is_huge(pq) };
            if is_huge {
                // SAFETY: the active page and owning heap remain live and exclusively locally
                // mutable; raw huge-queue publication consumes no duplicate page capability.
                unsafe {
                    mi_page_queue_push_raw(theap, pq, page.page().as_ptr());
                }
                page
            } else {
                // SAFETY: classification proved the exact compatible non-huge queue. Construct the
                // normal witness and consume Active through typed queue publication exactly once.
                unsafe {
                    let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(queue);
                    mi_page_queue_push_typed(owner, normal_queue, page).into_active()
                }
            }
        }
    };
    // SAFETY: either publication path returned a live Active page owned by this heap. Preserve the
    // final block-size assertion before the expensive validity assertion, with metadata stable.
    unsafe {
        crate::internal::mi_assert_internal!(
            mi_page_block_size(page.page().as_ptr()) >= block_size.get()
        );
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page.page().as_ptr()));
    }
    Some(page)
}

/// # Safety
/// `owner` must be the live current-thread owner selected for the registered detached fresh page
/// carried exclusively by `page_uninit`. The page's initialized MemId, geometry, backing,
/// thread-free owner bit, empty links/lists, zero counts, and page-map registration must remain live
/// and stable through initialization or failure return. The owner's heap/TLD association, random
/// state, statistics, and relevant tracking and OS commit configuration must remain live, with
/// exclusive local mutation and no concurrent teardown or non-protocol page mutation. Tracking
/// hooks and any commit or diagnostic callback and opaque argument reached synchronously must remain
/// valid; callbacks must not incompatibly reenter the participating heap/page state and must not
/// unwind. No other page-state witness may exist; failure returns the same `Page<Uninit>`
/// capability, while success consumes it exactly once into `ActivePage`.
#[inline(never)]
pub(crate) unsafe fn mi_page_init_typed(
    owner: crate::core_types::ThreadHeapLocal,
    page_uninit: crate::core_types::Page<crate::core_types::Uninit>,
) -> Result<crate::core_types::ActivePage, crate::core_types::Page<crate::core_types::Uninit>> {
    let theap = owner.ptr().as_ptr();
    let page = page_uninit.ptr().as_ptr();
    // SAFETY: the caller keeps the live current-thread owner's initialized heap association stable
    // for this read, before any page metadata is published.
    let heap = unsafe { _mi_theap_heap(theap) };
    // SAFETY: the returned heap pointer remains live and initialized for this main-heap identity
    // classification, which retains its original position before both page metadata writes.
    let is_heap_main = unsafe { _mi_is_heap_main(heap) };
    let page_heap = if is_heap_main { ptr::null_mut() } else { heap };
    // SAFETY: `page_uninit` is the sole capability for this live detached page. Publish the selected
    // heap field before its thread-heap owner, preserving the initialization order exactly.
    unsafe {
        (*page).heap = page_heap;
        mi_page_set_theap(page, theap);
    }

    let page_ptr = page_uninit.ptr();
    // SAFETY: the registered fresh page's initialized start, reserved count, block size, and backing
    // remain stable for this checked area projection after owner publication.
    let Some(area) = (unsafe { page_ptr.area() }) else {
        crate::internal::mi_assert_internal!(false);
        return Err(page_uninit);
    };
    // SAFETY: the projected live committed block area is placed into tracking no-access state before
    // any geometry validation or free-list initialization, preserving the original transition.
    unsafe {
        mi_track_mem_noaccess(area.start().as_ptr().cast(), area.byte_len());
    }
    // SAFETY: the caller keeps the fresh page's initialized nonzero block size stable. Keeping the
    // read in the assertion operand preserves assertion-only evaluation after tracking transition.
    crate::internal::mi_assert_internal!(unsafe {
        area.byte_len() / mi_page_block_size(page) < (1usize << 16)
    });
    // SAFETY: the caller keeps the initialized reserved count stable. This remains the second
    // geometry assertion and is evaluated only when internal assertions are enabled.
    crate::internal::mi_assert_internal!(unsafe { mi_page_reserved_count(page).get() > 0 });

    if MI_PADDING != 0 || MI_ENCODE_FREELIST != 0 {
        // SAFETY: the caller exclusively owns the live current-thread random state and fresh page
        // keys. Draw key zero, then key one, then publish both together exactly as before.
        unsafe {
            let key0 = _mi_theap_random_next(theap);
            let key1 = _mi_theap_random_next(theap);
            mi_page_set_keys(page_ptr, PageKeys::from_raw([key0, key1]));
        }
    }

    // SAFETY: short-circuiting reads the live page's initialized MemId only when the debug level
    // requires the original-zero validation, preserving the test order.
    if MI_DEBUG > 2 && unsafe { (*page).memid.initially_zero } {
        // SAFETY: the page's initialized committed geometry remains stable for this read before the
        // tracking transition and zero check.
        let committed = unsafe { mi_page_committed_typed(page) };
        // SAFETY: the initialized page start and committed prefix remain live and backed; this makes
        // exactly that prefix defined before its contents are inspected.
        unsafe {
            mi_track_mem_defined((*page).page_start.cast(), committed.get());
        }
        // SAFETY: the page area remains live and the committed prefix was made defined above.
        // Keeping the read in the assertion operand preserves assertion-only evaluation.
        crate::internal::mi_assert_expensive!(unsafe {
            mi_mem_is_zero(area.start().as_ptr().cast(), committed.get())
        });
    }

    // SAFETY: owner publication above initialized this field; keep its first assertion-only read in
    // place before the association check.
    crate::internal::mi_assert_internal!(unsafe { !(*page).theap.is_null() });
    // SAFETY: the caller keeps owner metadata stable; preserve raw owner-field then decoded-owner
    // read order within this assertion operand.
    crate::internal::mi_assert_internal!(unsafe { (*page).theap == mi_page_theap(page) });
    // SAFETY: the registered fresh page's initialized capacity remains stable and zero before
    // extension. This read remains assertion-only.
    crate::internal::mi_assert_internal!(unsafe { mi_page_capacity_count(page).get() == 0 });
    // SAFETY: the sole fresh-page capability keeps its initialized available-list head stable and
    // empty before extension. This read remains assertion-only.
    crate::internal::mi_assert_internal!(unsafe { page_ptr.free().is_empty() });
    // SAFETY: the registered fresh page's initialized used count remains stable and zero before
    // extension. This read remains assertion-only.
    crate::internal::mi_assert_internal!(unsafe { mi_page_used_count(page).get() == 0 });
    // SAFETY: the page's live atomic owner word remains claimed throughout initialization. This read
    // remains assertion-only and precedes the explicit relaxed snapshot below.
    crate::internal::mi_assert_internal!(unsafe { crate::internal::mi_page_is_owned(page) });
    // SAFETY: the live initialized atomic thread-free word remains available for exactly one Relaxed
    // snapshot, with protocol publication excluded by the sole fresh-page capability.
    let thread_free = unsafe { page_ptr.thread_free_cell().load_relaxed() };
    crate::internal::mi_assert_internal!(
        thread_free
            == crate::core_types::ThreadFreeWord::from_parts(
                None,
                crate::core_types::ThreadFreeOwner::Owned
            )
    );
    // SAFETY: the detached fresh page's initialized forward link remains empty. This assertion read
    // intentionally precedes the backward-link read.
    crate::internal::mi_assert_internal!(unsafe { page_ptr.queue_next().is_none() });
    // SAFETY: the detached fresh page's initialized backward link remains empty.
    crate::internal::mi_assert_internal!(unsafe { page_ptr.queue_prev().is_none() });
    // SAFETY: the initialized retirement field remains stable and zero before activation.
    crate::internal::mi_assert_internal!(unsafe { mi_page_retire_expire(page).is_zero() });
    // SAFETY: the fresh page's initialized geometry has no interior-pointer classification before
    // activation. This read remains assertion-only.
    crate::internal::mi_assert_internal!(unsafe { !mi_page_has_interior_pointers(page) });
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    {
        // SAFETY: key initialization above published key zero before any encoded-list operation;
        // keep this first key read in its assertion operand.
        crate::internal::mi_assert_internal!(unsafe { (*page).keys[0] != 0 });
        // SAFETY: preserve the following key-one assertion and its assertion-only evaluation.
        crate::internal::mi_assert_internal!(unsafe { (*page).keys[1] != 0 });
    }
    // SAFETY: all preceding initialization writes and caller-proven geometry remain live and stable
    // for the final pre-extension validity assertion.
    crate::internal::mi_assert_expensive!(unsafe { mi_page_is_valid_init(page_uninit.ptr()) });

    // SAFETY: the caller exclusively owns the current-thread page's capacity/free-list mutation;
    // backing and commit callback state remain valid. Failure leaves the same Uninit capability.
    if !unsafe { mi_page_extend_free(theap, page) } {
        return Err(page_uninit);
    }
    // Successful extension established the initialized available list; the typed transition
    // consumes the sole Uninit capability exactly once with its matching owner.
    let active = page_uninit.initialize(owner);
    // SAFETY: the newly active page remains live and its available-list metadata is stable. Keeping
    // this check in the assertion operand preserves assertion-only evaluation after consumption.
    crate::internal::mi_assert!(unsafe { mi_page_immediate_available(active.page().as_ptr()) });
    Ok(active)
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
/// exclusive initialization/teardown access, and `pq` must be the exact live queue selected from
/// that heap's fixed bin table. The queue's endpoints, count, and block-size classification and the
/// finite linked chain's page links, owner association, classification, geometry, metadata, and
/// backing must remain initialized, live, readable, and stable throughout traversal. The caller
/// must exclude concurrent relink, non-protocol metadata mutation, page or heap teardown, and
/// incompatible diagnostic callback reentry. Protocol atomic remote-free publication is permitted
/// because this validation does not inspect or mutate remote free-list heads. Any diagnostic
/// callback and opaque callback arguments reached by a failed assertion must remain valid and must
/// not unwind.
#[inline]
pub(crate) unsafe fn mi_page_queue_is_valid_typed(
    theap: crate::core_types::ThreadHeapLocal,
    pq: crate::core_types::PageQueuePtr,
) -> bool {
    let theap = theap.ptr().as_ptr();
    let pq = pq.as_ptr();
    crate::internal::MI_UNUSED_RELEASE(&theap);
    let mut count = 0usize;
    crate::internal::MI_UNUSED_RELEASE(&count);
    let mut prev = None;
    crate::internal::MI_UNUSED_RELEASE(&prev);
    // SAFETY: `pq` is the caller-proven non-null fixed-bin queue and remains initialized and live.
    let queue = unsafe { page_queue_ptr(pq) };
    // SAFETY: the caller keeps this queue's initialized block-size classification stable.
    let block_size = unsafe { mi_page_queue_block_size_raw(pq) };
    // SAFETY: the caller keeps the queue head initialized and stable throughout traversal.
    let mut page = unsafe { queue.first() };
    while let Some(page_ptr) = page {
        let page_raw = page_ptr.as_ptr();
        // SAFETY: every linked page remains live and its backward link stays stable. Keeping this
        // read in the assertion operand preserves its assertion-only evaluation.
        crate::internal::mi_assert_internal!(unsafe { page_ptr.queue_prev() } == prev);
        // SAFETY: the linked page remains live with stable initialized owner flags.
        if unsafe { mi_page_is_in_full(page_raw) } {
            crate::internal::mi_assert_internal!(
                _mi_wsize_from_size(block_size) == MI_LARGE_MAX_OBJ_WSIZE + 2
            );
        } else if unsafe {
            // SAFETY: the non-full linked page retains stable initialized shape, block-size, and
            // backing metadata while its huge-page classification is read.
            mi_page_is_huge(page_raw)
        } {
            crate::internal::mi_assert_internal!(
                _mi_wsize_from_size(block_size) == MI_LARGE_MAX_OBJ_WSIZE + 1
            );
        } else {
            // SAFETY: this ordinary linked page retains stable initialized block-size metadata.
            crate::internal::mi_assert_internal!(
                unsafe { mi_page_block_size(page_raw) } == block_size
            );
        }
        // SAFETY: the caller keeps the linked page live and its owner association stable. Keeping
        // this read in the assertion operand preserves its assertion-only evaluation.
        crate::internal::mi_assert_internal!(unsafe { (*page_raw).theap } == theap);
        // SAFETY: every linked page remains live and its forward link stays stable. This is the
        // original terminal-check read; traversal deliberately reads the link again below.
        if unsafe { page_ptr.queue_next() }.is_none() {
            // SAFETY: the caller keeps the queue tail initialized and stable. Keeping this read in
            // the assertion operand preserves its assertion-only evaluation.
            crate::internal::mi_assert_internal!(
                unsafe { queue.last() }.is_some_and(|last| last == page_ptr)
            );
        }
        count += 1;
        prev = page;
        // SAFETY: every linked page remains live and its forward link stays stable. This preserves
        // the original second read used to advance the traversal.
        page = unsafe { page_ptr.queue_next() };
    }
    // SAFETY: the caller keeps the queue count initialized and stable. Keeping this read in the
    // assertion operand preserves its assertion-only evaluation.
    crate::internal::mi_assert_internal!(unsafe { mi_page_queue_count_typed(pq) }.get() == count);
    true
}

/// Select the queue for an ordinary page block size in a local thread heap.
///
/// # Safety
/// `theap` must identify a live initialized thread heap under current-thread
/// ownership or exclusive initialization/reclaim/teardown access. Its complete
/// fixed queue table and the queue selected by `block_size` must remain live,
/// initialized, and stable through the projection. Concurrent local queue-table
/// mutation or heap teardown is excluded; protocol atomic remote-free
/// publication is permitted.
#[inline]
pub(crate) unsafe fn mi_theap_page_queue_of_typed(
    theap: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
) -> crate::core_types::PageQueuePtr {
    let bin = mi_bin_from_block_size(block_size);
    crate::internal::mi_assert_internal!(bin.as_usize() <= MI_BIN_HUGE);
    // SAFETY: the typed bin is within the ordinary/huge portion of the fixed queue table, and the
    // caller keeps that initialized table live and stable for the address projection.
    unsafe { theap.ptr().page_queue(bin) }
}

/// Select the owning heap queue implied by one page's classification.
///
/// # Safety
/// `theap` must be nonnull and point to the live initialized locally owned
/// thread heap associated with `page`, under current-thread ownership or
/// exclusive initialization/reclaim/teardown access. `page` must be nonnull
/// live initialized allocator metadata with a nonzero block size; its full
/// flag, huge-page geometry/MemId/reserved metadata, block size, and owner
/// association must remain stable. The heap's complete fixed queue table and
/// every selected queue block-size field must remain live, initialized, and
/// stable. The page need not yet be linked in the selected queue. Concurrent
/// local classification, owner, geometry, queue-table mutation, or teardown is
/// excluded; protocol atomic remote-free publication is permitted. Any
/// diagnostic callback and opaque argument reached by an assertion must remain
/// valid, must not incompatibly reenter these page/heap metadata, and must not
/// unwind.
#[inline]
pub unsafe fn mi_theap_page_queue_of(
    theap: *mut mi_theap_t,
    page: *const mi_page_t,
) -> *mut mi_page_queue_t {
    crate::internal::mi_assert_internal!(!theap.is_null());
    // SAFETY: the caller keeps the page's initialized classification and geometry live and stable;
    // the helper preserves the full-then-huge-then-normal read order.
    let bin = unsafe { mi_page_bin_typed(page) }.as_usize();
    // SAFETY: the raw contract identifies this nonnull live initialized heap as the page's local
    // owner or the exclusive reclaim target for this operation.
    let theap = unsafe {
        crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        )
    };
    // `mi_page_bin_typed` already selected the full, huge, or size-class bin, as C's
    // `mi_page_bin` does; index the queue table with it rather than recomputing the bin.
    // SAFETY: the bin is bounded by `mi_page_bin_typed`, and the caller keeps the complete
    // initialized queue table live through its exact address projection.
    let pq = unsafe { theap.ptr().page_queue(Bin::new_unchecked(bin as u16)) }.as_ptr();
    // SAFETY: the caller keeps the page and selected queue classification metadata stable. Keeping
    // every repeated read in this assertion operand preserves its block-size, huge, then full
    // short-circuit order and assertion-only evaluation.
    crate::internal::mi_assert_internal!(unsafe {
        (mi_page_block_size(page) == mi_page_queue_block_size_raw(pq))
            || (mi_page_is_huge(page) && mi_page_queue_is_huge(pq))
            || (mi_page_is_in_full(page) && mi_page_queue_is_full(pq))
    });
    pq
}

/// # Safety
/// `theap` must be valid, and `pq` must point to one of its page queues.
pub unsafe fn mi_theap_queue_first_update(theap: *mut mi_theap_t, pq: *const mi_page_queue_t) {
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
/// Refresh the direct-page cache entries represented by a normal queue's current front.
///
/// # Safety
/// `theap` must identify a live initialized locally owned heap under current-thread exclusive
/// queue/cache mutation or exclusive initialization/teardown access. `queue` must be a live normal
/// queue in that heap with stable initialized block-size, head, and link metadata. Its head, when
/// present, must be a live page owned by the heap and suitable for that size class; an empty queue
/// is represented by the allocator's static empty-page sentinel. The full direct cache and the
/// preceding queue entries inspected to derive the cache range must remain initialized, readable,
/// and exclusively writable where updated. Concurrent local queue relinking, cache mutation, and
/// heap teardown are excluded; protocol atomic remote-free publication is permitted.
pub(crate) unsafe fn mi_theap_queue_first_update_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
) {
    // SAFETY: the contract keeps this live normal queue's initialized block-size metadata readable
    // while excluding concurrent local queue mutation and heap teardown.
    let Some(block_size) = (unsafe { queue.ptr().block_size() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    let size = block_size.get();
    if size > MI_SMALL_SIZE_MAX {
        return;
    }

    // SAFETY: the caller keeps this live queue's initialized head link stable under exclusive
    // local queue mutation. Absence is preserved for the sentinel selection immediately below.
    let first = unsafe { queue.ptr().first() };
    let page = first.unwrap_or_else(|| {
        // SAFETY: `_mi_page_empty` has static storage, a stable nonnull address, and the initialized
        // empty-page sentinel representation required by every direct-cache slot.
        unsafe {
            crate::core_types::PagePtr::new_unchecked(ptr::addr_of!(_mi_page_empty).cast_mut())
        }
    });

    let direct_idx = mi_direct_bin_from_size(size);
    let idx = direct_idx.as_usize();
    crate::internal::mi_debug_assert!(idx < crate::types::MI_PAGES_DIRECT);
    let theap_ptr = theap.ptr();
    // SAFETY: the caller keeps the full direct cache initialized and stable under exclusive local
    // mutation; this is the single cache-slot read before the equality fast return.
    let cached = unsafe { theap_ptr.direct_page(direct_idx) };
    if cached == page {
        return;
    }

    let start = if idx <= 1 {
        0
    } else {
        // SAFETY: `direct_idx` was derived from this queue's initialized `block_size`. For this
        // branch the caller keeps the queue strictly after the array's first entry and every
        // predecessor inspected by the backward scan initialized and stable.
        unsafe { mi_theap_direct_cache_start(theap_ptr, queue, block_size, direct_idx) }
    };

    crate::internal::mi_assert_internal!(start <= idx);
    // SAFETY: both branches return `start <= idx`, while `direct_idx` witnesses that `idx` is in
    // the direct-cache domain; therefore `start` is also a representable in-range direct bin.
    let direct_start = unsafe { crate::core_types::DirectBin::new_unchecked(start as u16) };
    // SAFETY: the caller exclusively owns the live initialized direct cache; `direct_start` through
    // `direct_idx` is the bounded inclusive range for which the queue head or static sentinel is
    // suitable. This publication remains the final operation.
    unsafe { theap_ptr.fill_direct_pages(direct_start, direct_idx, page) };
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
    let theap_raw = theap.ptr().as_ptr();
    let pq = queue.ptr().as_ptr();
    let mut count = crate::core_types::PageQueueCount::new(0);
    let mut candidate_limit = crate::core_types::PageReclaimLimit::new(0);
    // SAFETY: the caller keeps the live current-thread heap and its policy fields initialized and
    // stable while these three fields are read in their original order.
    let configured = unsafe {
        crate::allocator::policy::ThreadHeapPolicy::from_fields(
            (*theap_raw).allow_page_reclaim,
            (*theap_raw).allow_page_abandon,
            (*theap_raw).page_full_retain,
        )
    };
    let mut page_full_retain = crate::allocator::policy::ActivePolicy::initial_full_retain(
        block_size,
        configured.page_full_retain(),
    );
    let mut page_candidate = ptr::null_mut::<mi_page_t>();
    // SAFETY: the exact live normal queue remains stable under exclusive local queue ownership
    // while its initialized front link is read once before traversal.
    let mut page = unsafe { queue.ptr().first() }
        .map(PagePtr::as_ptr)
        .unwrap_or(ptr::null_mut());
    #[cfg(feature = "bench-diagnostics")]
    let mut failed_extend = false;
    #[cfg(feature = "bench-diagnostics")]
    let mut selected_position = 0usize;

    while !page.is_null() {
        // SAFETY: the current linked page and its initialized successor remain live and stable;
        // snapshot `next` before any collection, move, callback, or terminal release.
        let next = unsafe { (*page).next };
        count = count.increment();
        candidate_limit = candidate_limit.decrement();

        // SAFETY: the current live page's local available-list metadata is stable for this read.
        let mut immediate_available = unsafe { mi_page_immediate_available(page) };
        if !immediate_available {
            // SAFETY: the current page remains locally owned and live for remote-list collection;
            // callbacks obey the function contract. Preserve collection before the second read.
            unsafe {
                _mi_page_free_collect(page, false);
            }
            // SAFETY: collection returned with the current page live, so reread its availability
            // exactly once in the original position.
            immediate_available = unsafe { mi_page_immediate_available(page) };
        }

        // SAFETY: short-circuiting reaches this initialized page metadata read only when the
        // snapshotted availability is false; the caller keeps the page geometry stable.
        if !immediate_available && !unsafe { mi_page_is_expandable(page) } {
            page_full_retain = page_full_retain.decrement();
            if page_full_retain.is_negative() {
                // SAFETY: the live current page and owning queue remain stable. Preserve the
                // assertion's full-then-availability short circuit, then consume the page through
                // its full-or-abandon transition without accessing it again in this iteration.
                unsafe {
                    crate::internal::mi_assert_internal!(
                        !mi_page_is_in_full(page) && !mi_page_immediate_available(page)
                    );
                    mi_page_to_full(page, pq);
                }
            }
        } else {
            if page_candidate.is_null() {
                page_candidate = page;
                #[cfg(feature = "bench-diagnostics")]
                {
                    selected_position = count.get();
                }
                candidate_limit = crate::allocator::policy::ActivePolicy::page_candidate_limit();
            } else if unsafe {
                // SAFETY: the retained candidate remains a live active page linked in this exact
                // queue with stable used metadata until this classification read.
                mi_page_all_free(page_candidate)
            } {
                // SAFETY: the all-free candidate is live, active, and linked in `queue`. Consume
                // its sole capability through terminal removal and release before replacing the
                // raw candidate identity; do not access the released candidate afterward.
                unsafe {
                    let active = crate::allocator::raw_bridge::assume_active_page(
                        crate::core_types::PagePtr::new_unchecked(page_candidate),
                    );
                    let all_free = active.try_mark_all_free().unwrap_unchecked();
                    let _ = _mi_page_free_active_internal(all_free, queue);
                }
                page_candidate = page;
                #[cfg(feature = "bench-diagnostics")]
                {
                    selected_position = count.get();
                }
            } else if unsafe {
                // SAFETY: the current page and retained candidate remain live with stable used
                // counts and current-page geometry. This expression preserves the original current
                // used, candidate used, then conditional mostly-used read order and short circuit.
                mi_page_used_count(page) >= mi_page_used_count(page_candidate)
                    && !mi_page_is_mostly_used(page)
            } {
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

    // SAFETY: the caller exclusively owns this live heap's local search statistics. Preserve the
    // page-search total update before the search-count update in one coherent accounting epoch.
    unsafe {
        __mi_stat_counter_increase(
            ptr::addr_of_mut!((*theap_raw).stats.page_searches),
            count.get(),
        );
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap_raw).stats.page_searches_count), 1);
    }

    if !page_candidate.is_null() {
        page = page_candidate;
    }
    if !page.is_null() {
        // SAFETY: the selected page remains live in the exact local queue and its availability
        // metadata is stable for this read.
        if !unsafe { mi_page_immediate_available(page) } {
            // SAFETY: the same selected page keeps stable expandable geometry through this
            // assertion-only read.
            crate::internal::mi_assert_internal!(unsafe { mi_page_is_expandable(page) });
            // SAFETY: the selected expandable page and owning heap remain live and exclusively
            // locally mutable through extension and any synchronous commit callback.
            if !unsafe { mi_page_extend_free(theap_raw, page) } {
                page = ptr::null_mut();
                #[cfg(feature = "bench-diagnostics")]
                {
                    failed_extend = true;
                }
            }
        }
        // SAFETY: when nonnull, the selected or successfully extended page remains live and its
        // availability metadata is stable. Preserve the original null short circuit.
        crate::internal::mi_assert_internal!(
            page.is_null() || unsafe { mi_page_immediate_available(page) }
        );
    }

    if page.is_null() {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::page_queue_search(count.get(), false, failed_extend);
        // SAFETY: the caller-proven current-thread heap/queue epoch covers retired collection,
        // which completes before the fresh allocation transaction and preserves its callback,
        // arena, page-map, and backing requirements.
        unsafe {
            _mi_theap_collect_retired_typed(theap, false);
        }
        // SAFETY: retired collection has completed; the caller still owns the live heap and exact
        // normal queue. Preserve the same block size and zero/default alignment materialization.
        let fresh = unsafe {
            mi_page_fresh_alloc_typed(
                theap,
                queue.ptr(),
                block_size,
                crate::core_types::MaybeAlignment::new_unchecked(0),
            )
        };
        if let Some(fresh) = fresh {
            // SAFETY: fresh allocation returned a live initialized page whose block-size metadata
            // remains stable for this assertion-only read.
            crate::internal::mi_assert_internal!(
                block_size.get() == unsafe { mi_page_block_size(fresh.page().as_ptr()) }
            );
            // SAFETY: the fresh page and current-thread heap remain live and stably associated;
            // queue selection is read-only and must reproduce the exact supplied queue.
            crate::internal::mi_assert_internal!(unsafe {
                queue.ptr().as_ptr()
                    == mi_theap_page_queue_of(theap.ptr().as_ptr(), fresh.page().as_ptr())
            });
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_queue_fresh_allocation();
            return Some(fresh);
        }
        if first_try {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_queue_retry();
            // SAFETY: failed fresh allocation preserves the caller's live current-thread heap,
            // queue, and callback epoch for the single original retry with `first_try = false`.
            return unsafe { mi_page_queue_find_free_ex_active(theap, queue, block_size, false) };
        }
        return None;
    }

    // SAFETY: traversal or candidate selection proves this live active page remains linked in the
    // exact compatible normal queue. Consume it into the queued witness, move that witness to the
    // front, then clear retirement only through the returned queued page identity.
    let queued = unsafe {
        let active = crate::allocator::raw_bridge::assume_active_page(
            crate::core_types::PagePtr::new_unchecked(page),
        );
        let queued = crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(
            active, queue,
        );
        let queued = mi_page_queue_move_to_front_typed(theap, queue, queued);
        mi_page_reset_retire_expire(queued.page().as_ptr());
        queued
    };
    #[cfg(feature = "bench-diagnostics")]
    {
        crate::allocator::bench_diagnostics::page_queue_search(count.get(), true, failed_extend);
        crate::allocator::bench_diagnostics::page_queue_selected(selected_position);
    }
    Some(queued.into_active())
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
unsafe fn mi_find_free_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: forwarded contract; the front probe writes nothing on a miss.
    if let Some(available) = unsafe { mi_find_free_page_front(theap, queue) } {
        return Some(available);
    }
    // SAFETY: only front-page absence or failed availability refinement reaches this fallback. The
    // caller-proven current-thread queue epoch covers the complete active search; success
    // establishes immediate availability before the returned Active capability is consumed once.
    unsafe { mi_page_queue_find_free_ex_active(theap, queue, block_size, true) }
        .and_then(|active| active.try_available().ok())
}

/// C `mi_find_free_page`'s fast case: the queue's first page after a quick local collect.
///
/// # Safety
/// Same contract as `mi_find_free_page_typed`. A miss writes nothing.
#[inline(always)]
#[allow(clippy::absurd_extreme_comparisons)]
unsafe fn mi_find_free_page_front(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: the caller keeps the exact live queue and its initialized head stable under
    // exclusive local ownership for this single front-page snapshot.
    if let Some(page) = unsafe { queue.ptr().first() } {
        // SAFETY: the caller proves the front page is live and current-thread owned with stable
        // local/available free lists; construct and consume the Local capability exactly once.
        let local = unsafe { crate::allocator::raw_bridge::assume_local_page(page) };
        // SAFETY: the same exclusive page-local epoch permits the one availability refinement,
        // including its ordered local-to-available transfer when required.
        if let Ok(available) = unsafe { local.try_ensure_available() } {
            let page_raw = available.page().as_ptr();
            if MI_SECURE >= 2
                && unsafe {
                    // SAFETY: the available page and current-thread heap remain live. Preserve the
                    // capacity read, reserved read, comparison short circuit, then random-state
                    // mutation only when every preceding condition succeeds.
                    mi_page_capacity_count(page_raw).get() < mi_page_reserved_count(page_raw).get()
                        && ((_mi_theap_random_next(theap.ptr().as_ptr()) & 1) == 1)
                }
            {
                // SAFETY: the selected page and owner remain live and exclusively locally mutable;
                // extension may synchronously use the caller-proven commit callback and its state.
                let _ = unsafe { mi_page_extend_free(theap.ptr().as_ptr(), page_raw) };
            }
            // SAFETY: extension has completed or was skipped, and the same live selected page's
            // retirement countdown is exclusively locally mutable before returning its witness.
            unsafe { mi_page_reset_retire_expire(page_raw) };
            return Some(available);
        }
    }
    None
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
    let theap = owner.ptr().as_ptr();
    // SAFETY: the caller-proven current-thread heap and exact huge queue remain live under
    // exclusive local queue/stat mutation. The typed layout/alignment are passed unchanged through
    // the outlined arena/reclaim/publication transaction; backing registries and synchronous
    // release, purge, and commit callbacks remain stable, nonunwinding, and nonreentrant under its
    // contract.
    let active = unsafe {
        mi_page_fresh_alloc_typed(
            owner,
            queue.ptr(),
            crate::core_types::BlockSize::from_padded(layout.padded()),
            page_alignment,
        )
    }?;
    let page = active.page().as_ptr();
    // SAFETY: fresh allocation returned a live initialized page whose block-size metadata remains
    // stable. Keep this read in the first assertion operand before classification reads.
    crate::internal::mi_assert_internal!(unsafe {
        mi_page_block_size(page) >= layout.padded().get()
    });
    // SAFETY: the same live page retains its initialized huge-page classification. This remains the
    // second assertion-only metadata read.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_huge(page) });
    // SAFETY: the page's initialized singleton geometry remains stable. This remains the third
    // assertion-only metadata read.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_singleton(page) });
    if MI_STAT > 0 {
        // SAFETY: the caller exclusively owns the live current-thread heap statistics and keeps the
        // returned page metadata stable. Preserve the second block-size read and bytes update before
        // the count update in one coherent statistics epoch.
        unsafe {
            __mi_stat_increase(
                ptr::addr_of_mut!((*theap).stats.malloc_huge),
                mi_page_block_size(page),
            );
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_huge_count), 1);
        }
    }
    active.try_available().ok()
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

/// Generic allocation boundary used by the C-compatible fast allocation path.
///
/// # Safety
/// `theap` must be null, a supported allocator sentinel, or a live initialized current-thread
/// heap; any nonnull metadata must remain live and readable through localization, and the caller
/// must exclude concurrent local mutation or teardown of the selected heap's TLD, counters, page
/// queues, free lists, and stats. Protocol atomic remote-free publication is permitted. `size`
/// must be the allocator's raw padded-size word (zero is the supported minimum-size sentinel, and
/// nonzero words resulting from caller arithmetic are forwarded unchanged). Bit zero of
/// `zero_huge_alignment` selects zeroing and the remaining bits are the huge-alignment payload;
/// invalid payloads are validated downstream and return null. Process/TLS initialization and all
/// synchronously reached deferred-free, collection, arena commit/purge, and error callbacks plus
/// opaque arguments must remain valid, must not incompatibly reenter the selected heap's local
/// allocation state, and must not unwind. If nonnull, `usable` must be aligned and writable for one
/// `usize` when accessed and must not overlap allocator metadata. Heap localization or page-search
/// failure returns null without writing `usable`; success writes it before allocating the returned
/// block.
pub(crate) unsafe extern "C" fn _mi_malloc_generic(
    theap: *mut mi_theap_t,
    size: usize,
    zero_huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    let flags = crate::core_types::MallocGenericFlags::from_raw(zero_huge_alignment);
    // SAFETY: an initialized heap is live and current-thread owned; the empty and wrong-thread
    // sentinels fail the check and take the full path below, which initializes the thread.
    if unsafe { mi_theap_is_initialized(theap) } {
        let fast = unsafe {
            malloc_generic_fast(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
                size,
                flags,
                usable,
            )
        };
        match fast {
            GenericFast::Hit(p) => return p,
            // SAFETY: the fast path accepted this initialized heap and size.
            GenericFast::Counted => {
                return unsafe {
                    malloc_generic_outlined::<true>(
                        crate::core_types::ThreadHeapPtr::new_unchecked(theap),
                        crate::core_types::LayoutRequest::from_padded_unchecked(
                            crate::core_types::PaddedSize::new_unchecked(size),
                            flags.zeroed(),
                        ),
                        flags,
                        usable,
                    )
                };
            }
            GenericFast::NotCounted => {}
        }
    }
    // SAFETY: forwards this function's contract unchanged.
    unsafe { malloc_generic_slow(theap, size, flags, usable) }
}

/// `_mi_malloc_generic` when the fast path did not apply; out of line so the entry keeps no
/// frame.
///
/// # Safety
/// Same contract as `_mi_malloc_generic`.
#[inline(never)]
unsafe fn malloc_generic_slow(
    theap: *mut mi_theap_t,
    size: usize,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the raw-boundary contract supplies the allocator's padded-size word. Zero is
    // normalized exactly as before; every nonzero word is representable by `PaddedSize`, including
    // a result wrapped by caller arithmetic. Only the padded scalar and zeroing policy are consumed
    // by this unchecked layout materialization; the complete decoded flags are passed unchanged to
    // the later generic dispatch.
    let layout = unsafe {
        let padded = if size == 0 {
            crate::core_types::RequestedSize::new(crate::core_types::RequestedSize::MIN_ZERO_SIZE)
                .padded_unchecked()
        } else {
            crate::core_types::PaddedSize::new_unchecked(size)
        };
        crate::core_types::LayoutRequest::from_padded_unchecked(padded, flags.zeroed())
    };

    // SAFETY: the contract permits the supported raw heap states and current-thread
    // initialization, keeps the selected heap's local allocation state exclusive, and keeps all
    // callbacks, backing, registries, and the optional output valid through generic dispatch.
    unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        malloc_generic_outlined::<false>(theap, layout, flags, usable)
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

/// Allocate from the generic page path for a current-thread heap.
///
/// # Safety
/// `theap` must identify a live initialized heap owned by the current thread. The caller must
/// exclude concurrent local mutation of its counters, TLD, page queues, free lists, and stats;
/// protocol atomic remote-free publication is permitted. `layout` must describe a supported
/// padded allocation request, and `flags` must agree with its zeroing policy and contain either no
/// huge alignment or a valid huge alignment. Heap collection, deferred-free, arena commit/purge,
/// and error callbacks plus their opaque arguments must remain valid for synchronous invocation,
/// must not incompatibly reenter this heap's local allocation state, and must not unwind. If
/// nonnull, `usable` must be aligned and writable for one `usize` when accessed and must not
/// overlap allocator metadata.
pub(crate) unsafe fn _mi_malloc_generic_typed(
    theap: crate::core_types::ThreadHeapPtr,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: forwards this function's contract unchanged.
    unsafe {
        match malloc_generic_fast(theap, layout.padded().get(), flags, usable) {
            GenericFast::Hit(p) => p,
            GenericFast::Counted => malloc_generic_outlined::<true>(theap, layout, flags, usable),
            GenericFast::NotCounted => {
                malloc_generic_outlined::<false>(theap, layout, flags, usable)
            }
        }
    }
}

/// Outcome of the generic fast path.
enum GenericFast {
    /// Allocated from the size class's first page.
    Hit(*mut c_void),
    /// Counted toward maintenance (not due) but the first page had no free block.
    Counted,
    /// Not eligible or maintenance due; nothing was written.
    NotCounted,
}

/// C `_mi_malloc_generic` for an ordinary size class when maintenance is not due and the
/// queue's first page has a free block: count the call (first, as C does), quick-collect the
/// first page, and pop a block. Every other case returns without allocating, so the caller
/// continues with exactly C's remaining steps.
///
/// # Safety
/// `theap` must satisfy the `malloc_generic_body` contract; `usable` as for it.
// Unoptimized builds give every inlined temporary its own stack slot, so inlining this would
// keep its frame live under the generic body; loom's 32 KiB model stacks, which run this crate
// as the test global allocator, overflow on a fresh-page allocation.
#[cfg_attr(not(debug_assertions), inline(always))]
#[cfg_attr(debug_assertions, inline(never))]
unsafe fn malloc_generic_fast(
    theap: crate::core_types::ThreadHeapPtr,
    size: usize,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> GenericFast {
    // Bound the requested size, so a padded size that wrapped still reaches C's overflow error.
    let req_size = size.wrapping_sub(MI_PADDING_SIZE);
    if flags.zeroed() != crate::core_types::Zeroed::No
        || flags.huge_alignment().get() != 0
        || req_size.wrapping_sub(1) >= MI_LARGE_MAX_OBJ_SIZE - MI_PADDING_SIZE
    {
        return GenericFast::NotCounted;
    }
    // SAFETY: the contract keeps `theap` live, initialized, and exclusively current-thread
    // owned; an ordinary size (at most `MI_LARGE_MAX_OBJ_SIZE`) selects a normal queue whose
    // first page and free lists are locally owned for this allocation.
    unsafe {
        let raw_theap = theap.as_ptr();
        let count = (*raw_theap).generic_count + 1;
        if count >= crate::allocator::policy::ActivePolicy::generic_maintenance_interval().get() {
            return GenericFast::NotCounted;
        }
        (*raw_theap).generic_count = count;
        let local_theap = crate::core_types::ThreadHeapLocal::new_unchecked(theap);
        let block_size = crate::core_types::BlockSize::new_unchecked(size);
        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(mi_page_queue(raw_theap, block_size)),
        );
        let Some(available) = mi_find_free_page_front(local_theap, queue) else {
            return GenericFast::Counted;
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
            local_theap,
            available,
            crate::core_types::PaddedSize::new_unchecked(size),
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        );
        crate::internal::mi_assert_internal!(!p.is_null());
        if mi_page_block_size(page) > MI_SMALL_MAX_OBJ_SIZE
            && crate::internal::mi_page_is_full(page)
        {
            malloc_generic_page_to_full(page);
        }
        GenericFast::Hit(p)
    }
}

/// Move a large-block page that the fast path just filled to the full queue, as C's generic
/// path does after allocating.
///
/// # Safety
/// `page` must belong to the current thread's heap and have just been allocated from.
#[cold]
#[inline(never)]
unsafe fn malloc_generic_page_to_full(page: *mut mi_page_t) {
    // SAFETY: forwarded contract; the page's queue is the current thread heap's.
    unsafe { mi_page_to_full(page, mi_page_queue_of(page)) }
}

/// The generic path when the fast path did not allocate; `COUNTED` when it already counted the
/// call and found maintenance not due. Out of line so the entries keep small frames.
///
/// # Safety
/// Same contract as `_mi_malloc_generic_typed`.
#[inline(never)]
unsafe fn malloc_generic_outlined<const COUNTED: bool>(
    theap: crate::core_types::ThreadHeapPtr,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: forwards this function's contract unchanged.
    unsafe { malloc_generic_body::<COUNTED>(theap, layout, flags, usable) }
}

/// Body of the generic path, inlined into `malloc_generic_outlined`. With `COUNTED` the fast
/// path has already counted the call and found maintenance not due.
///
/// # Safety
/// Same contract as `_mi_malloc_generic_typed`.
#[inline(always)]
unsafe fn malloc_generic_body<const COUNTED: bool>(
    theap: crate::core_types::ThreadHeapPtr,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    let size = layout.padded().get();
    let zeroed = flags.zeroed();
    let Some(huge_alignment) = crate::core_types::MaybeAlignment::new(flags.huge_alignment().get())
    else {
        crate::internal::mi_assert_internal!(false);
        return ptr::null_mut();
    };

    // SAFETY: the contract keeps `theap` live, initialized, current-thread owned, and exclusively
    // mutable for local allocator state. This epoch preserves the original counter update order,
    // then performs deferred-free, retired-page, and optional full collection synchronously while
    // their callbacks and heap metadata remain valid and incompatible reentry is excluded.
    let local_theap = unsafe {
        let local_theap = crate::core_types::ThreadHeapLocal::new_unchecked(theap);
        let raw_theap = local_theap.ptr().as_ptr();
        // A fast-path miss already counted this call and found maintenance not due.
        if !COUNTED {
            (*raw_theap).generic_count += 1;
        }
        if !COUNTED
            && (*raw_theap).generic_count
                >= crate::allocator::policy::ActivePolicy::generic_maintenance_interval().get()
        {
            (*raw_theap).generic_collect_count += (*raw_theap).generic_count;
            (*raw_theap).generic_count = 0;
            _mi_deferred_free(raw_theap, false);
            _mi_theap_collect_retired(raw_theap, false);
            mi_theap_abandon_kept_full_pages(raw_theap);

            let generic_collect = crate::allocator::policy::ActivePolicy::generic_collect_limit();
            if generic_collect.reached_by((*raw_theap).generic_collect_count) {
                (*raw_theap).generic_collect_count = 0;
                crate::mimalloc::mi_theap_collect(raw_theap, false);
            }
        }
        local_theap
    };

    // SAFETY: the current-thread heap, queue registry, arenas, backing, and synchronous callbacks
    // remain live through this search epoch. Preserve the original first search, forced collection
    // only on failure, and single retry order; concurrent remote frees remain protocol-atomic.
    let available = unsafe {
        let mut available = mi_find_page_typed(local_theap, layout.padded(), huge_alignment);
        if available.is_none() {
            crate::mimalloc::mi_theap_collect(local_theap.ptr().as_ptr(), true);
            available = mi_find_page_typed(local_theap, layout.padded(), huge_alignment);
        }
        available
    };

    let Some(available) = available else {
        let req_size = size.wrapping_sub(MI_PADDING_SIZE);
        _mi_error_message_fmt(
            Errno::ENOMEM,
            format_args!("unable to allocate memory ({req_size} bytes)\n"),
        );
        return ptr::null_mut();
    };

    // SAFETY: `available` was selected from this exclusively locally mutated heap and keeps its
    // live page/free-list/backing relation through allocation. The layout supplies the required
    // padded-size fit; the optional output is valid at its write, which intentionally precedes the
    // allocation attempt. Block allocation and zeroing consume `available`, after which the page
    // remains live for the original large-page fullness test and possible queue transition.
    unsafe {
        let page = available.page().as_ptr();

        crate::internal::mi_assert_internal!(mi_page_block_size(page) >= size);
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
            mi_page_slice_start(page).cast(),
            MI_PAGE_ALIGN
        ));
        crate::internal::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);

        crate::core_types::UsableOut::new(usable).write_if_present(mi_page_usable_block_size(page));
        let p = _mi_page_malloc_zero_typed(
            local_theap,
            available,
            layout.padded(),
            zeroed,
            ptr::null_mut(),
        );
        crate::internal::mi_assert_internal!(!p.is_null());

        if mi_page_block_size(page) > MI_SMALL_MAX_OBJ_SIZE
            && crate::internal::mi_page_is_full(page)
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
    use core::mem::MaybeUninit;

    /// Every generic allocation counts once toward C's maintenance interval, whichever entry,
    /// size class, or page-search outcome it takes; maintenance (and the deferred-free
    /// heartbeat) runs exactly when the counter wraps.
    #[test]
    fn generic_allocations_keep_c_maintenance_cadence() {
        std::thread::spawn(|| unsafe {
            // Allocate the bookkeeping first: in test builds the global allocator is this one,
            // so growing a Vec inside the loop would add generic allocations of its own.
            let mut live = Vec::with_capacity(1000);
            let warm = _mi_malloc_generic(ptr::null_mut(), 16, 0, ptr::null_mut());
            crate::allocator::free::mi_free(warm);
            let theap = crate::prim::_mi_theap_default();
            assert!(mi_theap_is_initialized(theap));
            let interval =
                crate::allocator::policy::ActivePolicy::generic_maintenance_interval().get();
            let mut expected = (*theap).generic_count;
            let mut heartbeat = (*theap).heartbeat;
            for i in 1..=2500usize {
                let size = [8192, 16384, 3000, 0, 4096, 1500, 65536][i % 7];
                let zero = usize::from(i % 11 == 0);
                let p = if i % 5 == 0 {
                    let padded = if size == 0 {
                        crate::core_types::RequestedSize::new(
                            crate::core_types::RequestedSize::MIN_ZERO_SIZE,
                        )
                        .padded_unchecked()
                    } else {
                        crate::core_types::PaddedSize::new_unchecked(size)
                    };
                    let flags = crate::core_types::MallocGenericFlags::from_raw(zero);
                    let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
                        padded,
                        flags.zeroed(),
                    );
                    _mi_malloc_generic_typed(
                        crate::core_types::ThreadHeapPtr::new_unchecked(theap),
                        layout,
                        flags,
                        ptr::null_mut(),
                    )
                } else {
                    _mi_malloc_generic(theap, size, zero, ptr::null_mut())
                };
                assert!(!p.is_null(), "allocation {i} of {size} bytes failed");
                // Keep a third live so pages fill and later searches miss or take fresh pages.
                if i % 3 == 0 {
                    live.push(p);
                } else {
                    crate::allocator::free::mi_free(p);
                }
                expected += 1;
                let wrapped = expected >= interval;
                if wrapped {
                    expected = 0;
                }
                assert_eq!(
                    (*theap).generic_count,
                    expected,
                    "generic count after allocation {i}"
                );
                let now = (*theap).heartbeat;
                assert_eq!(
                    now != heartbeat,
                    wrapped,
                    "heartbeat changed off-cadence at allocation {i}"
                );
                heartbeat = now;
            }
            for p in live {
                crate::allocator::free::mi_free(p);
            }
            let empty = ptr::addr_of!(crate::allocator::init::_mi_theap_empty).cast_mut();
            let before = (*empty).generic_count;
            let p = _mi_malloc_generic(empty, 8192, 0, ptr::null_mut());
            assert!(!p.is_null());
            crate::allocator::free::mi_free(p);
            assert_eq!(
                (*empty).generic_count,
                before,
                "the empty sentinel must not be counted"
            );
        })
        .join()
        .unwrap();
    }

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

    /// An owner free is reused by the next allocation of its size (LIFO, `UPSTREAM_DIFF.md`), and
    /// `calloc` still zeroes it although its page's free list was fresh before.
    #[test]
    fn owner_frees_are_reused_first_and_calloc_still_zeroes() {
        std::thread::spawn(|| {
            // SAFETY: every block is allocated and freed once on this thread and written only
            // while allocated.
            unsafe {
                let live: [*mut c_void; 8] =
                    core::array::from_fn(|_| crate::allocator::alloc::mi_malloc(48));
                let reused = live[3];
                ptr::write_bytes(reused.cast::<u8>(), 0xAB, 48);
                crate::allocator::free::mi_free(reused);
                let p = crate::allocator::alloc::mi_calloc(1, 48);
                assert_eq!(p, reused);
                assert!((0..48).all(|i| *p.cast::<u8>().add(i) == 0));
                crate::allocator::free::mi_free(p);
                assert_eq!(crate::allocator::alloc::mi_malloc(48), reused);
                crate::allocator::free::mi_free(reused);
                for (i, block) in live.into_iter().enumerate() {
                    if i != 3 {
                        crate::allocator::free::mi_free(block);
                    }
                }
            }
        })
        .join()
        .unwrap();
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

    #[test]
    fn queue_validator_and_selector_cover_normal_huge_and_full_queues() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut normal_page = MaybeUninit::<mi_page_t>::zeroed();
        let mut huge_page = MaybeUninit::<mi_page_t>::zeroed();
        let mut full_page = MaybeUninit::<mi_page_t>::zeroed();

        // SAFETY: the stack-local heap receives the allocator's initialized fixed queue table.
        // All three stack-local pages remain live and initialize exactly the owner, classification,
        // geometry, and null link fields read by validation and selection. Queue endpoint/count
        // mutations are exclusive, and the fixture performs no callback, relink, metadata mutation,
        // or teardown during each read-only validation/selection.
        unsafe {
            let theap_raw = theap.as_mut_ptr();
            (*theap_raw).pages = crate::allocator::init::MI_PAGE_QUEUES_EMPTY();
            let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap_raw),
            );

            let block_size = crate::core_types::BlockSize::new_unchecked(64);
            let normal_queue = owner.ptr().page_queue(mi_bin_from_block_size(block_size));
            assert!(mi_page_queue_is_valid_typed(owner, normal_queue));

            let thread_id = crate::core_types::ThreadId::new(0x1000)
                .expect("fixture thread id must not overlap page flag bits");
            let normal_raw = normal_page.as_mut_ptr();
            (*normal_raw).theap = theap_raw;
            (*normal_raw).block_size = block_size.get();
            (*normal_raw).xthread_id = crate::core_types::PageThreadId::from_parts(
                crate::core_types::PageOwnerId::Thread(thread_id),
                crate::core_types::PageFlags::new(0).expect("zero page flags"),
            )
            .raw();
            let normal = PagePtr::new_unchecked(normal_raw);
            normal.set_queue_prev(None);
            normal.set_queue_next(None);
            normal_queue.set_first(Some(normal));
            normal_queue.set_last(Some(normal));
            normal_queue.set_count(crate::core_types::PageQueueCount::new(1));
            assert!(mi_page_queue_is_valid_typed(owner, normal_queue));
            assert_eq!(
                mi_page_bin_typed(normal_raw),
                mi_bin_from_block_size(block_size)
            );
            assert_eq!(
                mi_theap_page_queue_of(theap_raw, normal_raw),
                normal_queue.as_ptr()
            );
            assert_eq!(mi_page_queue_of(normal_raw), normal_queue.as_ptr());

            let huge_queue = owner
                .ptr()
                .page_queue(Bin::new_unchecked(MI_BIN_HUGE as u16));
            let huge_block_size = crate::core_types::BlockSize::new_unchecked(
                MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>(),
            );
            let huge_raw = huge_page.as_mut_ptr();
            (*huge_raw).theap = theap_raw;
            (*huge_raw).reserved = 1;
            (*huge_raw).block_size = huge_block_size.get();
            (*huge_raw).xthread_id = crate::core_types::PageThreadId::from_parts(
                crate::core_types::PageOwnerId::Thread(thread_id),
                crate::core_types::PageFlags::new(0).expect("zero page flags"),
            )
            .raw();
            let huge = PagePtr::new_unchecked(huge_raw);
            huge.set_queue_prev(None);
            huge.set_queue_next(None);
            huge_queue.set_first(Some(huge));
            huge_queue.set_last(Some(huge));
            huge_queue.set_count(crate::core_types::PageQueueCount::new(1));
            assert!(mi_page_queue_is_valid_typed(owner, huge_queue));
            assert_eq!(mi_page_bin_typed(huge_raw).as_usize(), MI_BIN_HUGE);
            assert_eq!(
                mi_theap_page_queue_of(theap_raw, huge_raw),
                huge_queue.as_ptr()
            );
            assert_eq!(mi_page_queue_of(huge_raw), huge_queue.as_ptr());

            let full_queue = owner
                .ptr()
                .page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
            let full_raw = full_page.as_mut_ptr();
            (*full_raw).theap = theap_raw;
            (*full_raw).reserved = 1;
            (*full_raw).block_size = huge_block_size.get();
            (*full_raw).xthread_id = crate::core_types::PageThreadId::from_parts(
                crate::core_types::PageOwnerId::Thread(thread_id),
                crate::core_types::PageFlags::IN_FULL_QUEUE,
            )
            .raw();
            let full = PagePtr::new_unchecked(full_raw);
            full.set_queue_prev(None);
            full.set_queue_next(None);
            full_queue.set_first(Some(full));
            full_queue.set_last(Some(full));
            full_queue.set_count(crate::core_types::PageQueueCount::new(1));
            assert!(mi_page_queue_is_valid_typed(owner, full_queue));
            assert_eq!(mi_page_bin_typed(full_raw).as_usize(), MI_BIN_FULL);
            assert_eq!(
                mi_theap_page_queue_of(theap_raw, full_raw),
                full_queue.as_ptr()
            );
            assert_eq!(mi_page_queue_of(full_raw), full_queue.as_ptr());
        }
    }
}
