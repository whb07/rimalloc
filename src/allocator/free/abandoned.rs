//! Abandoned-page collection from cross-thread free publication.
//!
//! This module owns the free path's temporary `OwnedAbandonedPage` lifecycle:
//! collect the remote free list, then either free, reclaim, reabandon, or
//! release ownership back to the thread-free word.

use super::*;

unsafe fn try_free_abandoned(
    page: crate::core_types::OwnedAbandonedPage,
) -> Result<crate::core_types::DestroyedPage, crate::core_types::OwnedAbandonedPage> {
    // SAFETY: the owned witness keeps this page's initialized used count stable
    // while only protocol remote-head publication may race.
    if !unsafe { mi_page_all_free(page.page().as_ptr()) } {
        return Err(page);
    }
    // SAFETY: active claimed ownership keeps heap/MemId/list publication stable
    // while unabandon removes the matching bitmap/list/stat accounting.
    unsafe { _mi_arenas_page_unabandon_internal(&page, None) };
    let all_free = match page.try_mark_all_free() {
        Ok(all_free) => all_free,
        Err(page) => {
            crate::internal::mi_assert_internal!(false);
            return Err(page);
        }
    };
    // SAFETY: the retained claim refined into the exact detached all-free page;
    // its atomic heap stats remain live through terminal release.
    Ok(unsafe { _mi_arenas_page_free_internal(all_free, None) })
}

/// Try to re-publish a claimed abandoned page through mapped-abandoned state.
///
/// # Safety
/// `page` must be the unique live ownership claim for an initialized abandoned,
/// non-singleton, non-all-free page. Its count fields, flags, MemId, associated heap and stats,
/// arena membership, page-map registration, metadata, and backing must remain
/// live and stable until this call returns or consumes the claim. Concurrent
/// claim, reassignment, relink, backing release, or teardown is excluded;
/// protocol remote-free publication may continue through allocator atomics.
/// Arena, purge, commit, tracking, and diagnostic callbacks and opaque arguments
/// must remain valid for synchronous invocation, must not incompatibly reenter
/// this page, heap, or arena, and must not unwind. Every rejected path returns
/// the original ownership witness unchanged; success consumes it into the exact
/// terminal disposition.
unsafe fn try_reabandon_mapped(
    page: crate::core_types::OwnedAbandonedPage,
) -> Result<crate::core_types::AbandonedDisposition, crate::core_types::OwnedAbandonedPage> {
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::reabandon_attempt();
    let page_raw = page.page().as_ptr();
    // SAFETY: the unique claimed witness keeps the initialized used/reserved
    // fields live and stable while protocol remote publication may continue.
    if unsafe { mi_page_is_mostly_used(page_raw) } {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::reabandon_fail_mostly_used();
        return Err(page);
    }
    // SAFETY: the contract keeps this claimed page's MemId field initialized,
    // readable, and stable through the classification and possible transition.
    let memid = unsafe { (*page_raw).memid };
    let arena_memid = match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(arena_memid) => arena_memid,
        _ => {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::reabandon_fail_non_arena();
            return Err(page);
        }
    };
    // SAFETY: the unique claim keeps the atomic owner/flag word live; protocol
    // remote-head publication does not change its mapped-abandoned flag.
    if unsafe { mi_page_is_abandoned_mapped(page_raw) } {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::reabandon_fail_already_mapped();
        return Err(page);
    }

    // SAFETY: the claimed page is initialized and count-stable. The preceding
    // mostly-used rejection excludes a full page.
    crate::internal::mi_assert!(!unsafe { mi_page_is_full(page_raw) });
    // SAFETY: the retained unique claim, decoded matching arena MemId, stable
    // backing/map/heap state, and callback obligations establish the complete
    // consuming reabandon transaction. Failure returns the same witness.
    let reabandoned = unsafe { _mi_arenas_page_try_reabandon_to_mapped_arena(page, arena_memid) };
    #[cfg(feature = "bench-diagnostics")]
    {
        if reabandoned.is_ok() {
            crate::allocator::bench_diagnostics::reabandon_success();
        } else {
            crate::allocator::bench_diagnostics::reabandon_fail_arena_reject();
        }
    }
    reabandoned
}

unsafe fn release_abandoned_ownership(
    mut page: crate::core_types::OwnedAbandonedPage,
    mut tf_expect: crate::core_types::ThreadFreeWord,
) -> crate::core_types::AbandonedDisposition {
    // SAFETY: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let page_raw = page.page().as_ptr();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_all_free(page_raw));

        let atomic_tf = page.page().thread_free_cell();
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::ownership_release_cas_attempt();
        while !atomic_tf.release_acq_rel(&mut tf_expect) {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::ownership_release_cas_retry();
            crate::internal::mi_assert_internal!(tf_expect.is_owned());
            while tf_expect.head().is_some() {
                #[cfg(feature = "bench-diagnostics")]
                crate::allocator::bench_diagnostics::ownership_release_nested_collection();
                crate::allocator::page::collect_page_free_lists_abandoned(&page, false);
                page = match try_free_abandoned(page) {
                    Ok(page) => return crate::core_types::AbandonedDisposition::Destroyed(page),
                    Err(page) => page,
                };
                page = match try_reabandon_mapped(page) {
                    Ok(disposition) => return disposition,
                    Err(page) => page,
                };
                tf_expect = atomic_tf.load_relaxed();
            }
            crate::internal::mi_assert_internal!(tf_expect.head().is_none());
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::ownership_release_cas_attempt();
        }
        crate::core_types::AbandonedDisposition::StillAbandoned(page.into_abandoned())
    }
}

/// # Safety
/// `page` must be the unique live ownership claim for an abandoned, non-all-free page whose remote
/// free head has already been collected. Its registration, backing, geometry, associated-heap
/// pointer, MemId, and abandoned arena/list accounting must remain live and stable until this call
/// returns or consumes the claim. Any heap selected for reclaim must be initialized, current-thread
/// local, and exclusively mutable for its TLD, queue array, page links, and stats; remote publication
/// may occur only through allocator atomics. Teardown, backing release, heap relink, and incompatible
/// callback reentry must be excluded. Arena, purge, commit, tracking, and diagnostic callbacks and
/// opaque arguments must remain valid for synchronous invocation and must not unwind. `decision`
/// must describe the active reclaim-on-free policy for this operation.
unsafe fn try_reclaim_abandoned(
    page: crate::core_types::OwnedAbandonedPage,
    decision: crate::allocator::policy::ReclaimDecision,
) -> Result<crate::core_types::ActivePage, crate::core_types::OwnedAbandonedPage> {
    let reclaim_on_free = decision.reclaim_on_free();
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::abandoned_reclaim_attempt();

    // SAFETY: the unique claimed page remains live with stable owner, used count, geometry, MemId,
    // and associated-heap metadata while this epoch validates and snapshots its block size.
    let (page_raw, block_size) = unsafe {
        let page_raw = page.page().as_ptr();
        let block_size = page.page().block_size_raw();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_all_free(page_raw));
        crate::internal::mi_assert_internal!(block_size <= MI_MEDIUM_MAX_OBJ_SIZE);
        crate::internal::mi_assert_internal!(reclaim_on_free.is_enabled());
        (page_raw, block_size)
    };

    // SAFETY: allocator lifecycle keeps the current thread-state slot initialized and readable.
    if !unsafe { _mi_thread_is_initialized() } {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::abandoned_reclaim_fail_thread_uninit();
        return Err(page);
    }

    // SAFETY: the claimed page's associated-heap field remains live and stable. A returned heap
    // remains initialized and current-thread local through the reclaim decision and transition.
    let Some(associated) = (unsafe { _mi_page_associated_theap_peek(page.page()) }) else {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::abandoned_reclaim_fail_no_theap();
        return Err(page);
    };
    let theap = associated.as_ptr();

    // SAFETY: the associated heap and its TLD, reclaim flag, arena selection, queue metadata, and
    // stats remain live and stable. Current-thread ownership gives exclusive local mutation rights;
    // the claimed page's owner, used count, MemId, and associated-heap identity remain stable.
    let (owner, max_reclaim) = unsafe {
        if !(*theap).allow_page_reclaim {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::abandoned_reclaim_fail_reclaim_disabled();
            return Err(page);
        }

        let mut max_reclaim = crate::core_types::PageReclaimLimit::new(0);
        if theap == (*page_raw).theap {
            let context = if (*(*theap).tld).is_in_threadpool {
                crate::allocator::policy::ReclaimContext::ThreadPool
            } else {
                crate::allocator::policy::ReclaimContext::SameThreadHeap
            };
            max_reclaim = crate::allocator::policy::ActivePolicy::reclaim_limit(context);
        } else if reclaim_on_free.is_cross_thread_allowed()
            && !(*(*theap).tld).is_in_threadpool
            && !mi_page_is_mostly_used(page_raw)
            && _mi_arena_memid_is_suitable(
                (*page_raw).memid,
                (*_mi_theap_heap(theap)).exclusive_arena,
            )
        {
            max_reclaim = crate::allocator::policy::ActivePolicy::reclaim_limit(
                crate::allocator::policy::ReclaimContext::CrossThreadHeap,
            );
        }
        (
            crate::core_types::ThreadHeapLocal::new_unchecked(associated),
            max_reclaim,
        )
    };

    let decision = crate::allocator::policy::ReclaimDecision::new(
        reclaim_on_free,
        max_reclaim.as_queue_count(),
    );
    if let Some(max_reclaim) = decision.max_queue_count() {
        // SAFETY: `owner` retains the live current-thread heap witness. The validated block size
        // selects its matching live queue, whose count remains stable under exclusive local access.
        let within_limit = unsafe {
            let block_size = crate::core_types::BlockSize::new_unchecked(block_size);
            let pq = mi_page_queue(owner.ptr().as_ptr(), block_size);
            crate::internal::mi_assert_internal!(!pq.is_null());
            mi_page_queue_count_typed(pq).get() <= max_reclaim.get()
        };
        if !within_limit {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::abandoned_reclaim_fail_queue_limit();
            return Err(page);
        }
    }

    // SAFETY: the unique claimed page, its backing/MemId and abandoned publication, and `owner`
    // remain live through this terminal epoch. Unabandon first removes matching arena/list/stats
    // accounting; reclaim then consumes the claim and publishes the Active page to `owner`; only
    // owner stats are touched afterward. Neither `page` nor `page_raw` is used after consumption,
    // and callbacks cannot incompatibly reenter or unwind.
    let active = unsafe {
        _mi_arenas_page_unabandon_internal(&page, Some(owner));
        let active = _mi_theap_page_reclaim(page, owner);
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reclaim_on_free), 1);
        active
    };
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::abandoned_reclaim_success();
    Ok(active)
}

#[cold]
#[inline(never)]
pub(super) unsafe fn collect_after_remote_free(
    page: crate::core_types::OwnedAbandonedPage,
    published: crate::core_types::PublishedRemoteHead,
) -> crate::core_types::AbandonedDisposition {
    // SAFETY: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let page_raw = page.page().as_ptr();
        let block_size = page.page().block_size_raw();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));

        let mt_free = published.head();
        let expected_thread_free = match crate::allocator::policy::ActivePolicy::remote_collect_mode(
            page.page().block_size(),
        ) {
            crate::allocator::policy::RemoteCollectMode::PartialKeepPublishedHead => {
                crate::internal::mi_assert_internal!(
                    crate::allocator::page::mi_page_reserved_count(page_raw).get() >= 16
                );
                crate::allocator::page::_mi_page_free_collect_partly_typed(&page, published);
                crate::core_types::ThreadFreeWord::from_parts(
                    Some(mt_free),
                    crate::core_types::ThreadFreeOwner::Owned,
                )
            }
            crate::allocator::policy::RemoteCollectMode::FullDetach => {
                published.leave_published();
                crate::allocator::page::collect_page_free_lists_abandoned(&page, false);
                crate::core_types::ThreadFreeWord::from_parts(
                    None,
                    crate::core_types::ThreadFreeOwner::Owned,
                )
            }
        };

        let reclaim_on_free = crate::allocator::policy::ActivePolicy::reclaim_on_free_mode();
        if MI_DEBUG > 1 {
            if mi_page_is_singleton(page_raw) {
                crate::internal::mi_assert_internal!(mi_page_all_free(page_raw));
            }
            if mi_page_is_full(page_raw) {
                crate::internal::mi_assert!(mi_page_is_mostly_used(page_raw));
            }
        }

        let mut page = match try_free_abandoned(page) {
            Ok(page) => return crate::core_types::AbandonedDisposition::Destroyed(page),
            Err(page) => page,
        };
        if block_size <= MI_MEDIUM_MAX_OBJ_SIZE && reclaim_on_free.is_enabled() {
            let decision = crate::allocator::policy::ReclaimDecision::new(reclaim_on_free, None);
            page = match try_reclaim_abandoned(page, decision) {
                Ok(page) => return crate::core_types::AbandonedDisposition::Reclaimed(page),
                Err(page) => page,
            };
        }
        let page = match try_reabandon_mapped(page) {
            Ok(disposition) => return disposition,
            Err(page) => page,
        };

        release_abandoned_ownership(page, expected_thread_free)
    }
}
