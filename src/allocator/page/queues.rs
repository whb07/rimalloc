use super::*;

/// Return whether a nonnull page identity occurs in a queue's linked chain.
///
/// # Safety
/// `queue` must be nonnull and identify a live initialized page queue whose finite linked chain,
/// head, and every visited page and forward/backward link remain live, initialized, readable, and
/// stable throughout traversal. `page` must be nonnull; it is used only as a comparison identity
/// and is not independently dereferenced. The caller must hold current-thread local ownership or
/// exclusive initialization, collection, or teardown access and exclude concurrent local relink,
/// non-protocol link mutation, page/queue teardown, and incompatible diagnostic callback reentry.
/// Protocol atomic remote-free publication is permitted because this function reads only queue
/// links. Any diagnostic callback and opaque arguments reached by a failed assertion must remain
/// valid and must not unwind.
#[inline]
pub unsafe fn mi_page_queue_contains(queue: *mut mi_page_queue_t, page: *const mi_page_t) -> bool {
    crate::internal::mi_assert_internal!(!page.is_null());
    // SAFETY: the caller keeps the nonnull initialized queue metadata live throughout traversal.
    let queue_ptr = unsafe { super::counts::page_queue_ptr(queue) };
    // SAFETY: the caller keeps the initialized queue head stable and every nonnull head live.
    let mut list = unsafe { queue_ptr.first() }
        .map(crate::core_types::PagePtr::as_ptr)
        .unwrap_or(ptr::null_mut());
    while let Some(list_ptr) = crate::core_types::PagePtr::new(list) {
        // SAFETY: the current and next linked pages remain live with stable initialized links.
        // Keeping both reads in the assertion operand preserves assertion-only evaluation and the
        // original forward-then-backward read order.
        crate::internal::mi_assert_internal!(unsafe {
            list_ptr
                .queue_next()
                .is_none_or(|next| next.queue_prev() == Some(list_ptr))
        });
        // SAFETY: the current and previous linked pages remain live with stable initialized links.
        // Keeping both reads in the assertion operand preserves assertion-only evaluation and the
        // original backward-then-forward read order.
        crate::internal::mi_assert_internal!(unsafe {
            list_ptr
                .queue_prev()
                .is_none_or(|prev| prev.queue_next() == Some(list_ptr))
        });
        if ptr::eq(list, page.cast_mut()) {
            break;
        }
        // SAFETY: the current page remains live and its initialized forward link stays stable.
        // This deliberately repeats the next-link read after the assertion-only topology check.
        list = unsafe { list_ptr.queue_next() }
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
    }
    ptr::eq(list, page.cast_mut())
}

/// Resolve the exact queue containing a non-abandoned page.
///
/// # Safety
/// `page` must be nonnull and point to live initialized non-abandoned page
/// metadata linked exactly once in its owning live initialized thread heap's
/// compatible normal, huge, or full queue. The page's owner association,
/// classification, block size, geometry, backing, and forward/backward links,
/// and the heap's complete queue table, selected queue endpoints/count, and
/// finite linked chain must remain initialized, readable, live, and stable.
/// The caller must hold current-thread local ownership or exclusive
/// initialization/collection/teardown access and exclude concurrent local
/// relink, owner/classification/geometry mutation, page/heap teardown, and
/// incompatible diagnostic callback reentry. Protocol atomic remote-free
/// publication is permitted. Any diagnostic callback and opaque argument
/// reached by membership assertions must remain valid and must not unwind.
#[inline]
pub unsafe fn mi_page_queue_of(page: *const mi_page_t) -> *mut mi_page_queue_t {
    // SAFETY: the caller keeps the non-abandoned page, its owner heap, selected queue, and complete
    // finite link topology live and stable. Resolve the owner, derive the queue, then validate exact
    // membership in that order as one coherent non-owning lookup epoch.
    unsafe {
        let theap = crate::allocator::page::mi_page_theap(page);
        let pq = super::mi_theap_page_queue_of(theap, page);
        crate::internal::mi_assert_expensive!(mi_page_queue_contains(pq, page));
        pq
    }
}

/// Detach one page from its owning heap queue.
///
/// # Safety
/// `queue` must be nonnull and point to a live initialized normal, huge, or full queue in a live
/// initialized locally owned heap under current-thread ownership or exclusive collection/teardown
/// access. `page` must be nonnull and point to a live initialized page owned by that heap, linked
/// exactly once in `queue`, and block-size/classification compatible with it. The queue's first/
/// last links and nonzero count, every linked page and adjacent link, the heap's direct cache and
/// nonzero page count, and the page's owner, class, flag, and link metadata must remain live,
/// initialized, and internally consistent.
///
/// The caller must exclusively own local queue-link, queue-count, direct-cache, page-count, and
/// page-flag mutation throughout the removal; protocol atomic remote-free publication is
/// permitted. Allocator diagnostics must remain valid, must not unwind, and must not incompatibly
/// reenter page, queue, or heap mutation. On return the page is detached with both links and its
/// full-queue flag cleared, both counts are decremented, and the direct cache reflects a changed
/// queue front where applicable.
pub(crate) unsafe fn mi_page_queue_remove_raw(queue: *mut mi_page_queue_t, page: *mut mi_page_t) {
    // SAFETY: the raw contract supplies one live queue and its exactly linked live page under an
    // exclusive local removal epoch. Constructing non-owning typed views and immediately
    // dispatching preserves that ownership and does not extend any lifetime.
    unsafe {
        let queue = PageQueuePtr::new_unchecked(queue);
        let page = PagePtr::new_unchecked(page);
        mi_page_queue_remove_typed(queue, page);
    }
}

/// Detach one page from its owning heap queue.
///
/// # Safety
/// `queue` must be a live initialized normal, huge, or full queue in the live locally owned heap
/// that owns `page`. `page` must be live, initialized, linked exactly once in `queue`, and block-
/// size/classification compatible with it. The queue's first/last links and nonzero count, every
/// linked page and adjacent link, the heap's direct cache and nonzero page count, and the page's
/// owner, class, flag, and link metadata must remain live, initialized, and internally consistent.
/// Local relinking, cache/count/flag mutation, and heap teardown are excluded; protocol atomic
/// remote-free publication is permitted. Allocator diagnostics must remain valid, non-unwinding,
/// and free of incompatible reentry. On return the page is detached, its links and full flag are
/// cleared, the queue and heap counts are decremented, and any changed normal-queue front has been
/// published to the direct cache.
#[inline]
pub(crate) unsafe fn mi_page_queue_remove_typed(queue: PageQueuePtr, page: PagePtr) {
    let queue_ptr = queue;
    let queue = queue.as_ptr();
    let page_ptr = page;
    let page = page.as_ptr();

    // SAFETY: the contract keeps the live page, owning heap, source queue, classification, count,
    // and existing topology stable while this read-only preflight verifies the removal domain and
    // snapshots the heap association used by the subsequent publication epoch.
    let theap = unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(mi_page_queue_contains(queue, page));
        crate::internal::mi_assert_internal!(super::mi_page_queue_count_typed(queue).get() >= 1);
        crate::internal::mi_assert_internal!(
            crate::internal::mi_page_block_size(page)
                == super::counts::mi_page_queue_block_size_raw(queue)
                || (super::mi_page_is_huge(page) && super::mi_page_queue_is_huge(queue))
                || (super::mi_page_is_in_full(page) && super::mi_page_queue_is_full(queue))
        );
        super::mi_page_theap(page)
    };

    // SAFETY: the successful preflight and function contract give exclusive local ownership of
    // every affected adjacent link, queue endpoint/count, direct-cache entry, heap count, and page
    // flag/link field. This single mutation epoch preserves the original adjacent-link, tail,
    // head/cache, heap count, queue count, link-clear, then full-flag-clear publication order and
    // retains the repeated endpoint reads used by the branch structure.
    unsafe {
        let prev = page_ptr.queue_prev();
        let next = page_ptr.queue_next();
        if let Some(prev) = prev {
            prev.set_queue_next(next);
        }
        if let Some(next) = next {
            next.set_queue_prev(prev);
        }
        if queue_ptr
            .last()
            .is_some_and(|last| ptr::eq(page, last.as_ptr()))
        {
            queue_ptr.set_last(prev);
        }
        if queue_ptr
            .first()
            .is_some_and(|first| ptr::eq(page, first.as_ptr()))
        {
            queue_ptr.set_first(next);
            crate::internal::mi_assert_internal!(mi_theap_contains_queue(theap, queue));
            super::mi_theap_queue_first_update(theap, queue);
        }
        (*theap).page_count -= 1;
        super::counts::mi_page_queue_decrement_count(queue);
        page_ptr.set_queue_next(None);
        page_ptr.set_queue_prev(None);
        super::mi_page_set_in_full(page, false);
    }
}

/// Link one detached page at the front of an owning heap queue.
///
/// # Safety
/// `theap` must identify a live initialized locally owned heap under current-thread ownership or
/// exclusive initialization/reclaim access. `queue` must be a live initialized normal, huge, or
/// full queue in that heap, and `page` must be a live initialized page owned by `theap`, detached
/// from every queue, absent from `queue`, and block-size/classification compatible with `queue`.
/// The queue's first/last links and count, every linked page and adjacent link, the heap's direct
/// cache and page count, and the page's owner, class, flag, and link metadata must remain live,
/// initialized, and internally consistent. Both counts must permit one increment.
///
/// The caller must exclusively own local queue-link, queue-count, direct-cache, page-count, and
/// page-flag mutation throughout the insertion; protocol atomic remote-free publication is
/// permitted. Allocator diagnostics must remain valid, must not unwind, and must not incompatibly
/// reenter page, queue, or heap mutation. On return the page is linked at the queue front, its full
/// flag matches the queue class, both counts are incremented, and the direct cache reflects the new
/// front where applicable.
pub(super) unsafe fn mi_page_queue_push_raw(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    // SAFETY: the raw contract supplies the live nonnull heap, queue, and detached page under one
    // exclusive local insertion epoch. Constructing the non-owning typed views and immediately
    // dispatching preserves that ownership and does not extend any lifetime.
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_queue_push_internal(
            theap,
            PageQueuePtr::new_unchecked(queue),
            PagePtr::new_unchecked(page),
        );
    }
}

/// Link one detached page at the front of an owning heap queue.
///
/// # Safety
/// `theap` must be the live locally exclusive owner of `queue` and `page`. `queue` must be a live
/// initialized queue in that heap with consistent first/last links and count. `page` must be live,
/// initialized, detached from every queue, absent from `queue`, owned by `theap`, and block-size/
/// classification compatible with `queue`. Every linked page and adjacent link, the heap direct
/// cache and page count, and the page owner, class, flag, and link metadata must remain live and
/// stable; the queue and heap counts must permit one increment. Local relinking, cache/count/flag
/// mutation, and heap teardown are excluded; protocol atomic remote-free publication is permitted.
/// Allocator diagnostics must remain valid, non-unwinding, and free of incompatible reentry.
#[inline]
unsafe fn mi_page_queue_push_internal(
    theap: crate::core_types::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
) {
    let theap_raw = theap.ptr().as_ptr();
    let queue_raw = queue.as_ptr();
    let page_raw = page.as_ptr();

    // SAFETY: the contract keeps the live page, owning heap, destination queue, classification,
    // and existing topology stable while this read-only preflight verifies the insertion domain.
    unsafe {
        crate::internal::mi_assert_internal!(mi_page_theap(page_raw) == theap_raw);
        crate::internal::mi_assert_internal!(!mi_page_queue_contains(queue_raw, page_raw));
        crate::internal::mi_assert_internal!(
            mi_page_block_size(page_raw) == mi_page_queue_block_size_raw(queue_raw)
                || (mi_page_is_huge(page_raw) && mi_page_queue_is_huge(queue_raw))
                || (mi_page_is_in_full(page_raw) && mi_page_queue_is_full(queue_raw))
        );
    }

    // SAFETY: the successful preflight and function contract give exclusive local ownership of
    // every affected link, flag, queue/count, cache, and heap-count field. This single publication
    // epoch preserves the original flag, link, queue count, cache, then heap count order, including
    // the repeated queue-front reads used by the empty/nonempty branch.
    unsafe {
        mi_page_set_in_full(page_raw, mi_page_queue_is_full(queue_raw));

        page.set_queue_next(queue.first());
        page.set_queue_prev(None);
        if let Some(first) = queue.first() {
            crate::internal::mi_assert_internal!(first.queue_prev().is_none());
            first.set_queue_prev(Some(page));
            queue.set_first(PagePtr::new(page_raw));
        } else {
            queue.set_first(PagePtr::new(page_raw));
            queue.set_last(PagePtr::new(page_raw));
        }
        mi_page_queue_increment_count(queue_raw);
        mi_theap_queue_first_update(theap_raw, queue_raw);
        (*theap_raw).page_count += 1;
    }
}

/// Link one detached page at the end of an owning heap queue.
///
/// # Safety
/// `theap` must be the live locally exclusive owner of `queue` and `page`. `queue` must be a live
/// initialized queue in that heap with consistent first/last links and count. `page` must be live,
/// initialized, detached from every queue, absent from `queue`, owned by `theap`, and block-size/
/// classification compatible with `queue`. Every linked page and adjacent link, the heap direct
/// cache and page count, and the page owner, class, flag, and link metadata must remain live and
/// stable; the queue and heap counts must permit one increment. Local relinking, cache/count/flag
/// mutation, and heap teardown are excluded; protocol atomic remote-free publication is permitted.
/// Allocator diagnostics must remain valid, non-unwinding, and free of incompatible reentry.
#[inline]
unsafe fn mi_page_queue_push_at_end_internal(
    theap: crate::core_types::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
) {
    let theap_raw = theap.ptr().as_ptr();
    let queue_raw = queue.as_ptr();
    let page_raw = page.as_ptr();

    // SAFETY: the contract keeps the live page, owning heap, destination queue, classification,
    // and existing topology stable while this read-only preflight verifies the insertion domain.
    unsafe {
        crate::internal::mi_assert_internal!(mi_page_theap(page_raw) == theap_raw);
        crate::internal::mi_assert_internal!(!mi_page_queue_contains(queue_raw, page_raw));
        crate::internal::mi_assert_internal!(
            mi_page_block_size(page_raw) == mi_page_queue_block_size_raw(queue_raw)
                || (mi_page_is_huge(page_raw) && mi_page_queue_is_huge(queue_raw))
                || (mi_page_is_in_full(page_raw) && mi_page_queue_is_full(queue_raw))
        );
    }

    // SAFETY: the successful preflight and function contract give exclusive local ownership of
    // every affected link, flag, queue/count, cache, and heap-count field. This single publication
    // epoch preserves the original flag, link, queue count, conditional cache, then heap count
    // order, including the repeated queue-tail and post-link queue-front reads.
    unsafe {
        mi_page_set_in_full(page_raw, mi_page_queue_is_full(queue_raw));

        page.set_queue_prev(queue.last());
        page.set_queue_next(None);
        if let Some(last) = queue.last() {
            crate::internal::mi_assert_internal!(last.queue_next().is_none());
            last.set_queue_next(Some(page));
            queue.set_last(PagePtr::new(page_raw));
        } else {
            queue.set_first(PagePtr::new(page_raw));
            queue.set_last(PagePtr::new(page_raw));
        }
        mi_page_queue_increment_count(queue_raw);

        if queue
            .first()
            .is_some_and(|first| ptr::eq(first.as_ptr(), page_raw))
        {
            mi_theap_queue_first_update(theap_raw, queue_raw);
        }
        (*theap_raw).page_count += 1;
    }
}

/// Link `page` at the end of its live normal queue and return the exact membership capability.
///
/// # Safety
/// `theap` must be the live current-thread owner of `queue` and `page`, with exclusive local queue,
/// direct-cache, page-count, and page-flag mutation. `queue` must be the live initialized normal
/// queue in `theap` compatible with the live active `page`; the page must be detached from every
/// queue and absent from `queue`. Existing links/counts and all participating metadata and backing
/// must remain live, initialized, and stable, and both counts must permit one increment. Protocol
/// atomic remote-free publication is permitted. Diagnostics must not unwind or incompatibly
/// reenter mutation of the participating page, queue, or heap.
#[inline]
pub(super) unsafe fn mi_page_queue_push_at_end_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    page: crate::core_types::ActivePage,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    // SAFETY: the typed inputs and caller contract establish the unique detached active page and
    // its exact owning normal queue. The insertion completes before that capability is consumed
    // into the resulting queued-page witness.
    unsafe {
        mi_page_queue_push_at_end_internal(theap, queue.ptr(), page.page());
        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::from_enqueued(page, queue)
    }
}

/// Link `page` at the front of its live normal queue and return the exact membership capability.
///
/// # Safety
/// `theap` must be the live current-thread owner of `queue` and `page`, with exclusive local queue,
/// direct-cache, page-count, and page-flag mutation. `queue` must be the live initialized normal
/// queue in `theap` compatible with the live active `page`; the page must be detached from every
/// queue and absent from `queue`. Existing links/counts and all participating metadata and backing
/// must remain live, initialized, and stable, and both counts must permit one increment. Protocol
/// atomic remote-free publication is permitted. Diagnostics must not unwind or incompatibly
/// reenter mutation of the participating page, queue, or heap.
#[inline]
pub(super) unsafe fn mi_page_queue_push_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    page: crate::core_types::ActivePage,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    // SAFETY: the typed inputs and caller contract establish the unique detached active page and
    // its exact owning normal queue. The insertion completes before that capability is consumed
    // into the resulting queued-page witness.
    unsafe {
        mi_page_queue_push_internal(theap, queue.ptr(), page.page());
        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::from_enqueued(page, queue)
    }
}

/// # Safety
/// `theap` must identify the live initialized locally owned heap that owns `queued.page()`, under
/// current-thread ownership or exclusive collection access. `queue` must be the exact live
/// initialized normal queue represented by `queued`, and the page must be live, initialized,
/// linked exactly once in that queue, not marked full, and block-size compatible with it. The
/// queue's initialized block size and first/last links, every adjacent page link, the owning
/// heap's complete page-queue array and direct cache, and the page's owner, class, flag, and link
/// metadata must remain live, initialized, and internally consistent. Any preceding queue entries
/// inspected to derive the direct-cache range must retain their initialized block sizes and array
/// positions.
///
/// The caller must exclusively own local queue-link and direct-cache mutation throughout the
/// operation, and concurrent local relinking, cache mutation, and heap teardown are excluded;
/// protocol atomic remote-free publication is permitted. Allocator diagnostics must remain valid,
/// must not unwind, and must not incompatibly reenter page, queue, or heap mutation. If the page is
/// already first, it is returned unchanged. Otherwise it is relinked at the front and the direct
/// cache is updated; queue and heap page counts and the page's full flag are unchanged.
#[inline]
pub(super) unsafe fn mi_page_queue_move_to_front_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    let page_ptr = queued.page();
    let page = page_ptr.as_ptr();
    let queue_ptr;
    let first;

    // SAFETY: the contract keeps the live page, its exact owning normal queue, heap association,
    // queue endpoints, and link metadata stable while this read-only preflight verifies identity
    // and snapshots the current queue front.
    unsafe {
        crate::internal::mi_assert_internal!(mi_page_theap(page) == theap.ptr().as_ptr());
        crate::internal::mi_assert_internal!(queue.ptr() == queued.normal_queue().ptr());
        queue_ptr = queue.ptr();
        first = queue_ptr.first();
    }
    let Some(first) = first else {
        crate::internal::mi_assert_internal!(false);
        return queued;
    };
    if ptr::eq(first.as_ptr(), page) {
        return queued;
    }

    // SAFETY: the successful preflight and function contract give exclusive local ownership of
    // the page, both affected adjacent links, the queue endpoints, and the owning heap's direct
    // cache. This single mutation epoch preserves the original unlink, tail repair, front-link,
    // front publication, then cache-publication order without changing either count or the full
    // flag. The typed witness remains the sole membership capability for the same page and queue.
    unsafe {
        crate::internal::mi_assert_internal!(first.queue_prev().is_none());
        let Some(prev) = page_ptr.queue_prev() else {
            crate::internal::mi_assert_internal!(false);
            return queued;
        };
        let next = page_ptr.queue_next();
        prev.set_queue_next(next);
        if let Some(next) = next {
            next.set_queue_prev(Some(prev));
        } else {
            queue_ptr.set_last(Some(prev));
        }

        page_ptr.set_queue_prev(None);
        page_ptr.set_queue_next(Some(first));
        first.set_queue_prev(Some(page_ptr));
        queue_ptr.set_first(Some(page_ptr));
        mi_theap_queue_first_update_typed(theap, queue);

        crate::internal::mi_assert_internal!(queue_ptr.first() == Some(page_ptr));
        queued
    }
}

/// Move `page` from `from` to `to` without changing its owning heap or full-queue flag.
///
/// # Safety
/// `page` must be a live initialized page linked exactly once in `from` and absent from `to`.
/// `from` and `to` must be distinct, live, initialized, block-size-compatible queues belonging to
/// the same live locally owned heap as `page`. The source count must be nonzero, and both queues'
/// first/last links, counts, and every adjacent page link must be internally consistent. The page,
/// queues, owning heap, and their backing storage must remain live and stable throughout the move.
///
/// The caller must exclusively own local queue-link, queue-count, direct-cache, and page-link
/// mutation for the operation; protocol atomic remote-free publication is permitted. The caller
/// must retain that exclusivity while applying the corresponding full-queue flag transition after
/// this helper returns. Allocator diagnostics must not unwind or incompatibly reenter page, queue,
/// or heap mutation. This helper leaves the heap page count and the page's full-queue flag unchanged.
pub(crate) unsafe fn mi_page_queue_enqueue_from_raw(
    to: crate::core_types::PageQueuePtr,
    from: crate::core_types::PageQueuePtr,
    enqueue_at_end: bool,
    page: crate::core_types::PagePtr,
) {
    let to_ptr = to;
    let from_ptr = from;
    let to = to.as_ptr();
    let from = from.as_ptr();
    let page_ptr = page;
    let page = page.as_ptr();

    // SAFETY: the caller supplies two live compatible queues and their exactly
    // linked live page under exclusive local ownership. The initialized page
    // geometry, owning-heap association, and adjacent links remain stable
    // while this preflight snapshots the source position.
    let theap = unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_internal!(mi_page_queue_count_typed(from).get() >= 1);
        crate::internal::mi_assert_expensive!(mi_page_queue_contains(from, page));
        crate::internal::mi_assert_expensive!(!mi_page_queue_contains(to, page));
        let bsize = mi_page_block_size(page);
        crate::internal::MI_UNUSED(&bsize);
        crate::internal::mi_assert_internal!(
            (bsize == mi_page_queue_block_size_raw(to)
                && (bsize == mi_page_queue_block_size_raw(from) || mi_page_queue_is_full(from)))
                || (bsize == mi_page_queue_block_size_raw(from) && mi_page_queue_is_full(to))
                || (mi_page_is_huge(page)
                    && (mi_page_queue_is_huge(to) || mi_page_queue_is_full(to)))
        );

        mi_page_theap(page)
    };

    // SAFETY: the preflight snapshot describes the page's unique source
    // position, and the caller exclusively owns every affected source link,
    // source queue field, target count, and direct-cache publication. This
    // coherent epoch preserves the original unlink-then-count-transfer and
    // target-publication order.
    unsafe {
        let prev = page_ptr.queue_prev();
        let next = page_ptr.queue_next();
        if let Some(prev) = prev {
            prev.set_queue_next(next);
        }
        if let Some(next) = next {
            next.set_queue_prev(prev);
        }
        if from_ptr
            .last()
            .is_some_and(|last| ptr::eq(page, last.as_ptr()))
        {
            from_ptr.set_last(prev);
        }
        if from_ptr
            .first()
            .is_some_and(|first| ptr::eq(page, first.as_ptr()))
        {
            from_ptr.set_first(next);
            crate::internal::mi_assert_internal!(mi_theap_contains_queue(theap, from));
            mi_theap_queue_first_update(theap, from);
        }
        mi_page_queue_decrement_count(from);

        mi_page_queue_increment_count(to);
        if enqueue_at_end {
            page_ptr.set_queue_prev(to_ptr.last());
            page_ptr.set_queue_next(None);
            if let Some(last) = to_ptr.last() {
                crate::internal::mi_assert_internal!(theap == mi_page_theap(last.as_ptr()));
                last.set_queue_next(Some(page_ptr));
                to_ptr.set_last(PagePtr::new(page));
            } else {
                to_ptr.set_first(PagePtr::new(page));
                to_ptr.set_last(PagePtr::new(page));
                mi_theap_queue_first_update(theap, to);
            }
        } else if let Some(first) = to_ptr.first() {
            crate::internal::mi_assert_internal!(theap == mi_page_theap(first.as_ptr()));
            let next = first.queue_next();
            page_ptr.set_queue_prev(Some(first));
            page_ptr.set_queue_next(next);
            first.set_queue_next(Some(page_ptr));
            if let Some(next) = next {
                next.set_queue_prev(Some(page_ptr));
            } else {
                to_ptr.set_last(PagePtr::new(page));
            }
        } else {
            page_ptr.set_queue_prev(None);
            page_ptr.set_queue_next(None);
            to_ptr.set_first(PagePtr::new(page));
            to_ptr.set_last(PagePtr::new(page));
            mi_theap_queue_first_update(theap, to);
        }
    }
}

/// # Safety
/// `theap` must point to a valid thread heap, and `pq` must be a queue pointer to compare against
/// that heap's page queues.
#[inline]
pub(super) unsafe fn mi_theap_contains_queue(
    theap: *const crate::types::mi_theap_t,
    pq: *const mi_page_queue_t,
) -> bool {
    unsafe {
        let start = crate::core_types::ptr_exposed_addr(ptr::addr_of!((*theap).pages[0]));
        let end = crate::core_types::ptr_exposed_addr(ptr::addr_of!((*theap).pages[MI_BIN_FULL]));
        let queue = crate::core_types::ptr_exposed_addr(pq);
        queue >= start && queue <= end
    }
}

/// # Safety
/// `queue` must point into `theap`'s page queue table.
#[inline]
pub(super) unsafe fn mi_theap_queue_index(
    theap: crate::core_types::ThreadHeapPtr,
    queue: PageQueuePtr,
) -> usize {
    unsafe {
        queue
            .as_ptr()
            .offset_from(ptr::addr_of!((*theap.as_ptr()).pages[0])) as usize
    }
}

/// # Safety
/// `theap` must identify a live initialized heap whose complete page-queue array remains readable.
/// `queue` must be a live normal queue inside that array, and `block_size` must be its initialized
/// nonzero block size. `direct_idx` must be the direct-page index derived from `block_size`. When
/// `direct_idx > 1`, `queue` must be strictly after the array's first entry and every preceding
/// queue entry scanned back through the first entry must retain initialized nonzero block-size
/// metadata and its position in the same array. Concurrent queue-array mutation, relinking, and
/// heap teardown are excluded.
#[inline]
pub(super) unsafe fn mi_theap_direct_cache_start(
    theap: crate::core_types::ThreadHeapPtr,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
    direct_idx: crate::core_types::DirectBin,
) -> usize {
    let idx = direct_idx.as_usize();
    if idx <= 1 {
        return 0;
    }
    let bin = mi_bin_from_block_size(block_size).as_usize();
    // SAFETY: the contract places `queue` strictly inside the live initialized queue array and
    // keeps every preceding entry stable. The scan reads each predecessor before moving backward,
    // stops at the first bin change or array start, and never subtracts before the first entry.
    let start = unsafe {
        let start_queue = ptr::addr_of_mut!((*theap.as_ptr()).pages[0]);
        let pq = queue.ptr().as_ptr();
        crate::internal::mi_assert_internal!(pq > start_queue);
        let mut prev = pq.sub(1);
        while bin
            == mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(
                mi_page_queue_block_size_raw(prev),
            ))
            .as_usize()
            && prev > start_queue
        {
            prev = prev.sub(1);
        }
        1 + _mi_wsize_from_size(mi_page_queue_block_size_raw(prev))
    };
    start.min(idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::MaybeUninit;

    #[test]
    fn remove_detaches_middle_head_tail_sole_and_full_pages() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut pages = [const { MaybeUninit::<mi_page_t>::zeroed() }; 5];

        // SAFETY: the stack-local heap initializes its queue table, direct cache, and page count.
        // The five stack-local pages initialize the owner, class, flags, and exact queue links used
        // by the normal middle/head/tail/sole sequence and sole full-queue case. The heap, queues,
        // pages, and every exclusively mutated field remain live throughout each removal.
        unsafe {
            let theap = theap.as_mut_ptr();
            (*theap).pages = crate::allocator::init::MI_PAGE_QUEUES_EMPTY();
            (*theap).pages_free_direct = crate::allocator::init::MI_SMALL_PAGES_EMPTY();
            (*theap).page_count = 5;

            let block_size = crate::core_types::BlockSize::new_unchecked(64);
            let thread_id = crate::core_types::ThreadId::new(8)
                .expect("fixture thread id must not overlap page flag bits")
                .get();
            let normal = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[mi_bin_from_block_size(block_size).as_usize()]
            ));
            let full = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[MI_BIN_FULL]
            ));
            let page_ptrs = pages.each_mut().map(|page| {
                let page = page.as_mut_ptr();
                (*page).theap = theap;
                (*page).block_size = block_size.get();
                (*page).xthread_id = thread_id;
                PagePtr::new_unchecked(page)
            });
            let [first, middle, remaining, tail, full_page] = page_ptrs;

            first.set_queue_prev(None);
            first.set_queue_next(Some(middle));
            middle.set_queue_prev(Some(first));
            middle.set_queue_next(Some(remaining));
            remaining.set_queue_prev(Some(middle));
            remaining.set_queue_next(Some(tail));
            tail.set_queue_prev(Some(remaining));
            tail.set_queue_next(None);
            normal.set_first(Some(first));
            normal.set_last(Some(tail));
            normal.set_count(crate::core_types::PageQueueCount::new(4));

            let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(normal);
            mi_theap_queue_first_update_typed(owner, normal_queue);
            let direct_idx = mi_direct_bin_from_size(block_size.get());

            mi_page_queue_remove_typed(normal, middle);
            assert_eq!(normal.first(), Some(first));
            assert_eq!(normal.last(), Some(tail));
            assert_eq!(normal.count().get(), 3);
            assert_eq!(first.queue_prev(), None);
            assert_eq!(first.queue_next(), Some(remaining));
            assert_eq!(remaining.queue_prev(), Some(first));
            assert_eq!(remaining.queue_next(), Some(tail));
            assert_eq!(tail.queue_prev(), Some(remaining));
            assert_eq!(tail.queue_next(), None);
            assert_eq!(middle.queue_prev(), None);
            assert_eq!(middle.queue_next(), None);
            assert!(!mi_page_is_in_full(middle.as_ptr()));
            assert_eq!(owner.ptr().direct_page(direct_idx), first);
            assert_eq!((*theap).page_count, 4);

            mi_page_queue_remove_raw(normal.as_ptr(), first.as_ptr());
            assert_eq!(normal.first(), Some(remaining));
            assert_eq!(normal.last(), Some(tail));
            assert_eq!(normal.count().get(), 2);
            assert_eq!(first.queue_prev(), None);
            assert_eq!(first.queue_next(), None);
            assert_eq!(remaining.queue_prev(), None);
            assert_eq!(remaining.queue_next(), Some(tail));
            assert_eq!(tail.queue_prev(), Some(remaining));
            assert_eq!(tail.queue_next(), None);
            assert_eq!(owner.ptr().direct_page(direct_idx), remaining);
            assert_eq!((*theap).page_count, 3);

            mi_page_queue_remove_typed(normal, tail);
            assert_eq!(normal.first(), Some(remaining));
            assert_eq!(normal.last(), Some(remaining));
            assert_eq!(normal.count().get(), 1);
            assert_eq!(remaining.queue_prev(), None);
            assert_eq!(remaining.queue_next(), None);
            assert_eq!(tail.queue_prev(), None);
            assert_eq!(tail.queue_next(), None);
            assert_eq!(owner.ptr().direct_page(direct_idx), remaining);
            assert_eq!((*theap).page_count, 2);

            mi_page_queue_remove_typed(normal, remaining);
            assert_eq!(normal.first(), None);
            assert_eq!(normal.last(), None);
            assert_eq!(normal.count().get(), 0);
            assert_eq!(remaining.queue_prev(), None);
            assert_eq!(remaining.queue_next(), None);
            assert_eq!(
                owner.ptr().direct_page(direct_idx).as_ptr(),
                ptr::addr_of!(_mi_page_empty).cast_mut()
            );
            assert_eq!((*theap).page_count, 1);

            mi_page_set_in_full(full_page.as_ptr(), true);
            full_page.set_queue_prev(None);
            full_page.set_queue_next(None);
            full.set_first(Some(full_page));
            full.set_last(Some(full_page));
            full.set_count(crate::core_types::PageQueueCount::new(1));

            mi_page_queue_remove_typed(full, full_page);
            assert_eq!(full.first(), None);
            assert_eq!(full.last(), None);
            assert_eq!(full.count().get(), 0);
            assert_eq!(full_page.queue_prev(), None);
            assert_eq!(full_page.queue_next(), None);
            assert!(!mi_page_is_in_full(full_page.as_ptr()));
            assert_eq!((*theap).page_count, 0);
        }
    }

    #[test]
    fn push_front_and_tail_publish_empty_and_nonempty_queues() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut pages = [const { MaybeUninit::<mi_page_t>::zeroed() }; 4];

        // SAFETY: the stack-local heap initializes its queue table, direct cache, and page count.
        // Each stack-local page initializes the owner, normal block-size class, flags, and detached
        // links required by the insertion contract. The heap, both queues, all four pages, and their
        // exclusively owned metadata remain live through the empty and nonempty front/tail cases.
        unsafe {
            let theap = theap.as_mut_ptr();
            (*theap).pages = crate::allocator::init::MI_PAGE_QUEUES_EMPTY();
            (*theap).pages_free_direct = crate::allocator::init::MI_SMALL_PAGES_EMPTY();

            let front_size = crate::core_types::BlockSize::new_unchecked(64);
            let tail_size = crate::core_types::BlockSize::new_unchecked(128);
            let thread_id = crate::core_types::ThreadId::new(8)
                .expect("fixture thread id must not overlap page flag bits")
                .get();
            let front = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[mi_bin_from_block_size(front_size).as_usize()]
            ));
            let tail = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[mi_bin_from_block_size(tail_size).as_usize()]
            ));
            let page_ptrs = pages.each_mut().map(|page| {
                let page = page.as_mut_ptr();
                (*page).theap = theap;
                (*page).xthread_id = thread_id;
                PagePtr::new_unchecked(page)
            });
            page_ptrs[..2]
                .iter()
                .for_each(|page| (*page.as_ptr()).block_size = front_size.get());
            page_ptrs[2..]
                .iter()
                .for_each(|page| (*page.as_ptr()).block_size = tail_size.get());
            let [front_first, front_second, tail_first, tail_second] = page_ptrs;

            let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            let front_queue = crate::core_types::NormalPageQueue::new_unchecked(front);
            let tail_queue = crate::core_types::NormalPageQueue::new_unchecked(tail);

            let _ = mi_page_queue_push_typed(
                owner,
                front_queue,
                crate::allocator::raw_bridge::assume_active_page(front_first),
            );
            assert_eq!(front.first(), Some(front_first));
            assert_eq!(front.last(), Some(front_first));
            assert_eq!(front.count().get(), 1);
            assert_eq!(front_first.queue_prev(), None);
            assert_eq!(front_first.queue_next(), None);
            assert!(!mi_page_is_in_full(front_first.as_ptr()));
            assert_eq!(
                owner
                    .ptr()
                    .direct_page(mi_direct_bin_from_size(front_size.get())),
                front_first
            );
            assert_eq!((*theap).page_count, 1);

            let _ = mi_page_queue_push_typed(
                owner,
                front_queue,
                crate::allocator::raw_bridge::assume_active_page(front_second),
            );
            assert_eq!(front.first(), Some(front_second));
            assert_eq!(front.last(), Some(front_first));
            assert_eq!(front.count().get(), 2);
            assert_eq!(front_second.queue_prev(), None);
            assert_eq!(front_second.queue_next(), Some(front_first));
            assert_eq!(front_first.queue_prev(), Some(front_second));
            assert_eq!(front_first.queue_next(), None);
            assert!(!mi_page_is_in_full(front_second.as_ptr()));
            assert_eq!(
                owner
                    .ptr()
                    .direct_page(mi_direct_bin_from_size(front_size.get())),
                front_second
            );
            assert_eq!((*theap).page_count, 2);

            let _ = mi_page_queue_push_at_end_typed(
                owner,
                tail_queue,
                crate::allocator::raw_bridge::assume_active_page(tail_first),
            );
            assert_eq!(tail.first(), Some(tail_first));
            assert_eq!(tail.last(), Some(tail_first));
            assert_eq!(tail.count().get(), 1);
            assert_eq!(tail_first.queue_prev(), None);
            assert_eq!(tail_first.queue_next(), None);
            assert!(!mi_page_is_in_full(tail_first.as_ptr()));
            assert_eq!(
                owner
                    .ptr()
                    .direct_page(mi_direct_bin_from_size(tail_size.get())),
                tail_first
            );
            assert_eq!((*theap).page_count, 3);

            let _ = mi_page_queue_push_at_end_typed(
                owner,
                tail_queue,
                crate::allocator::raw_bridge::assume_active_page(tail_second),
            );
            assert_eq!(tail.first(), Some(tail_first));
            assert_eq!(tail.last(), Some(tail_second));
            assert_eq!(tail.count().get(), 2);
            assert_eq!(tail_first.queue_prev(), None);
            assert_eq!(tail_first.queue_next(), Some(tail_second));
            assert_eq!(tail_second.queue_prev(), Some(tail_first));
            assert_eq!(tail_second.queue_next(), None);
            assert!(!mi_page_is_in_full(tail_second.as_ptr()));
            assert_eq!(
                owner
                    .ptr()
                    .direct_page(mi_direct_bin_from_size(tail_size.get())),
                tail_first
            );
            assert_eq!((*theap).page_count, 4);
        }
    }

    #[test]
    fn enqueue_from_transfers_links_counts_and_preserves_full_flag() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut pages = [const { MaybeUninit::<mi_page_t>::zeroed() }; 3];

        // SAFETY: the stack-local heap embeds both queues, and the three
        // stack-local pages initialize every owner, class, flag, and link field
        // read or written by the queue-transfer operation. The fixture remains
        // exclusively owned and live through both transfers.
        unsafe {
            let theap = theap.as_mut_ptr();
            (*theap).pages = crate::allocator::init::MI_PAGE_QUEUES_EMPTY();
            (*theap).pages_free_direct = crate::allocator::init::MI_SMALL_PAGES_EMPTY();
            (*theap).page_count = 3;

            let block_size = crate::core_types::BlockSize::new_unchecked(64);
            let thread_id = crate::core_types::ThreadId::new(8)
                .expect("fixture thread id must not overlap page flag bits")
                .get();
            let normal = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[mi_bin_from_block_size(block_size).as_usize()]
            ));
            let full = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[MI_BIN_FULL]
            ));
            let page_ptrs = pages.each_mut().map(|page| {
                let page = page.as_mut_ptr();
                (*page).theap = theap;
                (*page).block_size = block_size.get();
                (*page).xthread_id = thread_id;
                PagePtr::new_unchecked(page)
            });
            let [first, middle, last] = page_ptrs;

            first.set_queue_prev(None);
            first.set_queue_next(Some(middle));
            middle.set_queue_prev(Some(first));
            middle.set_queue_next(Some(last));
            last.set_queue_prev(Some(middle));
            last.set_queue_next(None);
            normal.set_first(Some(first));
            normal.set_last(Some(last));
            normal.set_count(crate::core_types::PageQueueCount::new(3));
            let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(normal);
            let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            mi_theap_queue_first_update_typed(owner, normal_queue);
            let direct_idx = mi_direct_bin_from_size(block_size.get());

            mi_page_queue_enqueue_from_raw(full, normal, true, middle);

            assert_eq!(normal.first(), Some(first));
            assert_eq!(normal.last(), Some(last));
            assert_eq!(normal.count().get(), 2);
            assert_eq!(first.queue_prev(), None);
            assert_eq!(first.queue_next(), Some(last));
            assert_eq!(last.queue_prev(), Some(first));
            assert_eq!(last.queue_next(), None);
            assert_eq!(full.first(), Some(middle));
            assert_eq!(full.last(), Some(middle));
            assert_eq!(full.count().get(), 1);
            assert_eq!(middle.queue_prev(), None);
            assert_eq!(middle.queue_next(), None);
            assert!(!mi_page_is_in_full(middle.as_ptr()));
            assert_eq!(owner.ptr().direct_page(direct_idx), first);
            assert_eq!((*theap).page_count, 3);

            mi_page_set_in_full(middle.as_ptr(), true);
            mi_page_queue_enqueue_from_raw(normal, full, true, middle);

            assert_eq!(full.first(), None);
            assert_eq!(full.last(), None);
            assert_eq!(full.count().get(), 0);
            assert_eq!(normal.first(), Some(first));
            assert_eq!(normal.last(), Some(middle));
            assert_eq!(normal.count().get(), 3);
            assert_eq!(first.queue_prev(), None);
            assert_eq!(first.queue_next(), Some(last));
            assert_eq!(last.queue_prev(), Some(first));
            assert_eq!(last.queue_next(), Some(middle));
            assert_eq!(middle.queue_prev(), Some(last));
            assert_eq!(middle.queue_next(), None);
            assert!(mi_page_is_in_full(middle.as_ptr()));
            assert_eq!(owner.ptr().direct_page(direct_idx), first);
            assert_eq!((*theap).page_count, 3);
            mi_page_set_in_full(middle.as_ptr(), false);
        }
    }

    #[test]
    fn move_to_front_relinks_without_changing_counts_or_flags() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut pages = [const { MaybeUninit::<mi_page_t>::zeroed() }; 3];

        unsafe {
            let theap = theap.as_mut_ptr();
            (*theap).pages = crate::allocator::init::MI_PAGE_QUEUES_EMPTY();
            (*theap).pages_free_direct = crate::allocator::init::MI_SMALL_PAGES_EMPTY();
            (*theap).page_count = 3;

            let block_size = crate::core_types::BlockSize::new_unchecked(64);
            let thread_id = crate::core_types::ThreadId::new(8)
                .expect("fixture thread id must not overlap page flag bits")
                .get();
            let bin = mi_bin_from_block_size(block_size);
            let queue = crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap).pages[bin.as_usize()]
            ));
            let page_ptrs = pages.each_mut().map(|page| {
                let page = page.as_mut_ptr();
                (*page).theap = theap;
                (*page).block_size = block_size.get();
                (*page).xthread_id = thread_id;
                PagePtr::new_unchecked(page)
            });
            let [first, middle, last] = page_ptrs;

            first.set_queue_prev(None);
            first.set_queue_next(Some(middle));
            middle.set_queue_prev(Some(first));
            middle.set_queue_next(Some(last));
            last.set_queue_prev(Some(middle));
            last.set_queue_next(None);
            queue.set_first(Some(first));
            queue.set_last(Some(last));
            queue.set_count(crate::core_types::PageQueueCount::new(3));

            let normal = crate::core_types::NormalPageQueue::new_unchecked(queue);
            let direct_idx = mi_direct_bin_from_size(block_size.get());
            let active = crate::allocator::raw_bridge::assume_active_page(middle);
            let queued = crate::core_types::QueuedPage::try_queue(active, normal)
                .expect("middle page is linked in the normal queue");
            let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            let _ = mi_page_queue_move_to_front_typed(owner, normal, queued);

            assert_eq!(queue.first(), Some(middle));
            assert_eq!(queue.last(), Some(last));
            assert_eq!(middle.queue_prev(), None);
            assert_eq!(middle.queue_next(), Some(first));
            assert_eq!(first.queue_prev(), Some(middle));
            assert_eq!(first.queue_next(), Some(last));
            assert_eq!(last.queue_prev(), Some(first));
            assert_eq!(last.queue_next(), None);
            assert_eq!(queue.count().get(), 3);
            assert_eq!((*theap).page_count, 3);
            assert_eq!((*first.as_ptr()).xthread_id, thread_id);
            assert_eq!((*middle.as_ptr()).xthread_id, thread_id);
            assert_eq!((*last.as_ptr()).xthread_id, thread_id);
            assert_eq!(owner.ptr().direct_page(direct_idx), middle);

            let active = crate::allocator::raw_bridge::assume_active_page(last);
            let queued = crate::core_types::QueuedPage::try_queue(active, normal)
                .expect("last page remains linked in the normal queue");
            let _ = mi_page_queue_move_to_front_typed(owner, normal, queued);

            assert_eq!(queue.first(), Some(last));
            assert_eq!(queue.last(), Some(first));
            assert_eq!(last.queue_prev(), None);
            assert_eq!(last.queue_next(), Some(middle));
            assert_eq!(middle.queue_prev(), Some(last));
            assert_eq!(middle.queue_next(), Some(first));
            assert_eq!(first.queue_prev(), Some(middle));
            assert_eq!(first.queue_next(), None);
            assert_eq!(queue.count().get(), 3);
            assert_eq!((*theap).page_count, 3);
            assert_eq!((*first.as_ptr()).xthread_id, thread_id);
            assert_eq!((*middle.as_ptr()).xthread_id, thread_id);
            assert_eq!((*last.as_ptr()).xthread_id, thread_id);
            assert_eq!(owner.ptr().direct_page(direct_idx), last);

            let active = crate::allocator::raw_bridge::assume_active_page(last);
            let queued = crate::core_types::QueuedPage::try_queue(active, normal)
                .expect("front page remains linked in the normal queue");
            let _ = mi_page_queue_move_to_front_typed(owner, normal, queued);

            assert_eq!(queue.first(), Some(last));
            assert_eq!(queue.last(), Some(first));
            assert_eq!(last.queue_prev(), None);
            assert_eq!(last.queue_next(), Some(middle));
            assert_eq!(middle.queue_prev(), Some(last));
            assert_eq!(middle.queue_next(), Some(first));
            assert_eq!(first.queue_prev(), Some(middle));
            assert_eq!(first.queue_next(), None);
            assert_eq!(queue.count().get(), 3);
            assert_eq!((*theap).page_count, 3);
            assert_eq!((*first.as_ptr()).xthread_id, thread_id);
            assert_eq!((*middle.as_ptr()).xthread_id, thread_id);
            assert_eq!((*last.as_ptr()).xthread_id, thread_id);
            assert_eq!(owner.ptr().direct_page(direct_idx), last);
        }
    }
}
