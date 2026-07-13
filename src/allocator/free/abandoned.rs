//! Abandoned-page collection from cross-thread free publication.
//!
//! This module owns the free path's temporary `OwnedAbandonedPage` lifecycle:
//! collect the remote free list, then either free, reclaim, reabandon, or
//! release ownership back to the thread-free word.

use super::*;

unsafe fn mi_abandoned_page_try_free(
    page: crate::core_types::OwnedAbandonedPage,
) -> Result<(), crate::core_types::OwnedAbandonedPage> {
    // SAFETY[AUDIT:free-abandoned-page-boundary]: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let all_free = page.try_mark_all_free()?;
        _mi_arenas_page_unabandon(all_free.page().as_ptr(), ptr::null_mut());
        _mi_arenas_page_free(all_free.page().as_ptr(), ptr::null_mut());
        Ok(())
    }
}

unsafe fn mi_abandoned_page_try_reabandon_to_mapped(
    page: crate::core_types::OwnedAbandonedPage,
) -> Result<(), crate::core_types::OwnedAbandonedPage> {
    // SAFETY[AUDIT:free-abandoned-page-boundary]: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::reabandon_attempt();
        let page_raw = page.page().as_ptr();
        if mi_page_is_mostly_used(page_raw) {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::reabandon_fail_mostly_used();
            return Err(page);
        }
        let arena_memid = match crate::core_types::MemId::view((*page_raw).memid) {
            crate::core_types::MemIdView::Arena(arena_memid) => arena_memid,
            _ => {
                #[cfg(feature = "bench-diagnostics")]
                crate::bench_diagnostics::reabandon_fail_non_arena();
                return Err(page);
            }
        };
        if mi_page_is_abandoned_mapped(page_raw) {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::reabandon_fail_already_mapped();
            return Err(page);
        }

        crate::internal::mi_assert!(!mi_page_is_full(page_raw));
        let reabandoned = _mi_arenas_page_try_reabandon_to_mapped_arena(page, arena_memid);
        #[cfg(feature = "bench-diagnostics")]
        if reabandoned.is_ok() {
            crate::bench_diagnostics::reabandon_success();
        }
        reabandoned.map(|_| ())
    }
}

unsafe fn mi_abandoned_page_unown_from_free(
    mut page: crate::core_types::OwnedAbandonedPage,
    mut tf_expect: crate::core_types::ThreadFreeWord,
) {
    // SAFETY[AUDIT:free-abandoned-page-boundary]: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let page_raw = page.page().as_ptr();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_all_free(page_raw));

        let atomic_tf = page.page().thread_free_cell();
        while !atomic_tf.release_acq_rel(&mut tf_expect) {
            crate::internal::mi_assert_internal!(tf_expect.is_owned());
            while tf_expect.head().is_some() {
                _mi_page_free_collect(page_raw, false);
                page = match mi_abandoned_page_try_free(page) {
                    Ok(()) => return,
                    Err(page) => page,
                };
                page = match mi_abandoned_page_try_reabandon_to_mapped(page) {
                    Ok(()) => return,
                    Err(page) => page,
                };
                tf_expect = atomic_tf.load_relaxed();
            }
            crate::internal::mi_assert_internal!(tf_expect.head().is_none());
        }
    }
}

unsafe fn mi_page_queue_len_is_atmost(
    theap: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
    atmost: crate::core_types::PageQueueCount,
) -> bool {
    // SAFETY[AUDIT:free-abandoned-page-boundary]: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let pq = mi_page_queue(theap.ptr().as_ptr(), block_size);
        crate::internal::mi_assert_internal!(!pq.is_null());
        mi_page_queue_count_typed(pq).get() <= atmost.get()
    }
}

unsafe fn mi_abandoned_page_try_reclaim(
    page: crate::core_types::OwnedAbandonedPage,
    reclaim_on_free: crate::core_types::PageReclaimOnFree,
) -> Result<(), crate::core_types::OwnedAbandonedPage> {
    // SAFETY[AUDIT:free-abandoned-page-boundary]: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::abandoned_reclaim_attempt();
        let page_raw = page.page().as_ptr();
        let block_size = page.page().block_size_raw();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_all_free(page_raw));
        crate::internal::mi_assert_internal!(block_size <= MI_MEDIUM_MAX_OBJ_SIZE);
        crate::internal::mi_assert_internal!(reclaim_on_free.is_enabled());

        if !_mi_thread_is_initialized() {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::abandoned_reclaim_fail_thread_uninit();
            return Err(page);
        }

        let Some(theap) = _mi_page_associated_theap_peek(page.page()) else {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::abandoned_reclaim_fail_no_theap();
            return Err(page);
        };
        let theap = theap.as_ptr();
        if !(*theap).allow_page_reclaim {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::abandoned_reclaim_fail_reclaim_disabled();
            return Err(page);
        }

        let mut max_reclaim = crate::core_types::PageReclaimLimit::new(0);
        if theap == (*page_raw).theap {
            let max_reclaim_raw = if (*(*theap).tld).is_in_threadpool {
                _mi_option_get_fast(mi_option_t::mi_option_page_cross_thread_max_reclaim)
            } else {
                _mi_option_get_fast(mi_option_t::mi_option_page_max_reclaim)
            };
            max_reclaim = crate::core_types::PageReclaimLimit::new(max_reclaim_raw);
        } else if reclaim_on_free.is_cross_thread_allowed()
            && !(*(*theap).tld).is_in_threadpool
            && !mi_page_is_mostly_used(page_raw)
            && _mi_arena_memid_is_suitable(
                (*page_raw).memid,
                (*_mi_theap_heap(theap)).exclusive_arena,
            )
        {
            let max_reclaim_raw =
                _mi_option_get_fast(mi_option_t::mi_option_page_cross_thread_max_reclaim);
            max_reclaim = crate::core_types::PageReclaimLimit::new(max_reclaim_raw);
        }

        if let Some(max_reclaim) = max_reclaim.as_queue_count()
            && !mi_page_queue_len_is_atmost(
                crate::core_types::ThreadHeapLocal::new_unchecked(
                    crate::core_types::ThreadHeapPtr::new_unchecked(theap),
                ),
                crate::core_types::BlockSize::new_unchecked(block_size),
                max_reclaim,
            )
        {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::abandoned_reclaim_fail_queue_limit();
            return Err(page);
        }

        _mi_arenas_page_unabandon(page_raw, theap);
        let _ = _mi_theap_page_reclaim(
            page,
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
        );
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reclaim_on_free), 1);
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::abandoned_reclaim_success();
        Ok(())
    }
}

#[used]
static MI_ABANDONED_PAGE_TRY_RECLAIM_KEEP: unsafe fn(
    crate::core_types::OwnedAbandonedPage,
    crate::core_types::PageReclaimOnFree,
) -> Result<
    (),
    crate::core_types::OwnedAbandonedPage,
> = mi_abandoned_page_try_reclaim;

#[cold]
#[inline(never)]
pub(super) unsafe fn mi_free_try_collect_mt_typed(
    page: crate::core_types::OwnedAbandonedPage,
    remote_free: crate::core_types::RemoteFreeList,
) {
    // SAFETY[AUDIT:free-abandoned-page-boundary]: abandoned-page collection owns temporary page ownership before reclaim or reabandon decisions.
    unsafe {
        let page_raw = page.page().as_ptr();
        let block_size = page.page().block_size_raw();
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));

        let Some(mt_free) = remote_free.head() else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        let expected_thread_free = if block_size <= MI_SMALL_SIZE_MAX {
            crate::internal::mi_assert_internal!(
                crate::page::mi_page_reserved_count(page_raw).get() >= 16
            );
            _mi_page_free_collect_partly(page_raw, mt_free.as_ptr());
            crate::core_types::ThreadFreeWord::from_parts(
                Some(mt_free),
                crate::core_types::ThreadFreeOwner::Owned,
            )
        } else {
            _mi_page_free_collect(page_raw, false);
            crate::core_types::ThreadFreeWord::from_parts(
                None,
                crate::core_types::ThreadFreeOwner::Owned,
            )
        };

        let reclaim_on_free_raw = _mi_option_get_fast(mi_option_t::mi_option_page_reclaim_on_free);
        let reclaim_on_free = crate::core_types::PageReclaimOnFree::new(reclaim_on_free_raw);
        if MI_DEBUG > 1 {
            if mi_page_is_singleton(page_raw) {
                crate::internal::mi_assert_internal!(mi_page_all_free(page_raw));
            }
            if mi_page_is_full(page_raw) {
                crate::internal::mi_assert!(mi_page_is_mostly_used(page_raw));
            }
        }

        let mut page = match mi_abandoned_page_try_free(page) {
            Ok(()) => return,
            Err(page) => page,
        };
        if block_size <= MI_MEDIUM_MAX_OBJ_SIZE && reclaim_on_free.is_enabled() {
            page = match mi_abandoned_page_try_reclaim(page, reclaim_on_free) {
                Ok(()) => return,
                Err(page) => page,
            };
        }
        let page = match mi_abandoned_page_try_reabandon_to_mapped(page) {
            Ok(()) => return,
            Err(page) => page,
        };

        mi_abandoned_page_unown_from_free(page, expected_thread_free);
    }
}
