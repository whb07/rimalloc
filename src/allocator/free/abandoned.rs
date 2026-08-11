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

unsafe fn try_reabandon_mapped(
    page: crate::core_types::OwnedAbandonedPage,
) -> Result<crate::core_types::AbandonedDisposition, crate::core_types::OwnedAbandonedPage> {
    // SAFETY: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::reabandon_attempt();
        let page_raw = page.page().as_ptr();
        if mi_page_is_mostly_used(page_raw) {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::reabandon_fail_mostly_used();
            return Err(page);
        }
        let arena_memid = match crate::core_types::MemId::view((*page_raw).memid) {
            crate::core_types::MemIdView::Arena(arena_memid) => arena_memid,
            _ => {
                #[cfg(feature = "bench-diagnostics")]
                crate::allocator::bench_diagnostics::reabandon_fail_non_arena();
                return Err(page);
            }
        };
        if mi_page_is_abandoned_mapped(page_raw) {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::reabandon_fail_already_mapped();
            return Err(page);
        }

        crate::internal::mi_assert!(!mi_page_is_full(page_raw));
        let reabandoned = _mi_arenas_page_try_reabandon_to_mapped_arena(page, arena_memid);
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

unsafe fn mi_page_queue_len_is_atmost(
    theap: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
    atmost: crate::core_types::PageQueueCount,
) -> bool {
    // SAFETY: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let pq = mi_page_queue(theap.ptr().as_ptr(), block_size);
        crate::internal::mi_assert_internal!(!pq.is_null());
        mi_page_queue_count_typed(pq).get() <= atmost.get()
    }
}

unsafe fn try_reclaim_abandoned(
    page: crate::core_types::OwnedAbandonedPage,
    decision: crate::allocator::policy::ReclaimDecision,
) -> Result<crate::core_types::ActivePage, crate::core_types::OwnedAbandonedPage> {
    // SAFETY: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let reclaim_on_free = decision.reclaim_on_free();
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::abandoned_reclaim_attempt();
        let page_raw = page.page().as_ptr();
        let block_size = page.page().block_size_raw();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_all_free(page_raw));
        crate::internal::mi_assert_internal!(block_size <= MI_MEDIUM_MAX_OBJ_SIZE);
        crate::internal::mi_assert_internal!(reclaim_on_free.is_enabled());

        if !_mi_thread_is_initialized() {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::abandoned_reclaim_fail_thread_uninit();
            return Err(page);
        }

        let Some(theap) = _mi_page_associated_theap_peek(page.page()) else {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::abandoned_reclaim_fail_no_theap();
            return Err(page);
        };
        let theap = theap.as_ptr();
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

        let decision = crate::allocator::policy::ReclaimDecision::new(
            reclaim_on_free,
            max_reclaim.as_queue_count(),
        );
        if let Some(max_reclaim) = decision.max_queue_count()
            && !mi_page_queue_len_is_atmost(
                crate::core_types::ThreadHeapLocal::new_unchecked(
                    crate::core_types::ThreadHeapPtr::new_unchecked(theap),
                ),
                crate::core_types::BlockSize::new_unchecked(block_size),
                max_reclaim,
            )
        {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::abandoned_reclaim_fail_queue_limit();
            return Err(page);
        }

        // SAFETY: the claimed page, associated live heap, MemId, and abandoned
        // publication remain stable while this removes abandoned accounting.
        _mi_arenas_page_unabandon_internal(
            &page,
            Some(crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )),
        );
        let active = _mi_theap_page_reclaim(
            page,
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
        );
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reclaim_on_free), 1);
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::abandoned_reclaim_success();
        Ok(active)
    }
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
