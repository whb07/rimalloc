use super::*;

/// # Safety
/// `queue` must be valid, and `page` must be null or page metadata compatible with that queue.
#[inline]
pub unsafe fn mi_page_queue_contains(queue: *mut mi_page_queue_t, page: *const mi_page_t) -> bool {
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let queue_ptr = super::counts::page_queue_ptr(queue);
        let mut list = queue_ptr
            .first()
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        while let Some(list_ptr) = crate::core_types::PagePtr::new(list) {
            crate::internal::mi_assert_internal!(
                list_ptr
                    .queue_next()
                    .is_none_or(|next| next.queue_prev() == Some(list_ptr))
            );
            crate::internal::mi_assert_internal!(
                list_ptr
                    .queue_prev()
                    .is_none_or(|prev| prev.queue_next() == Some(list_ptr))
            );
            if ptr::eq(list, page.cast_mut()) {
                break;
            }
            list = list_ptr
                .queue_next()
                .map(crate::core_types::PagePtr::as_ptr)
                .unwrap_or(ptr::null_mut());
        }
        ptr::eq(list, page.cast_mut())
    }
}

/// # Safety
/// `page` must point to non-abandoned page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_queue_of(page: *const mi_page_t) -> *mut mi_page_queue_t {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let theap = crate::allocator::page::mi_page_theap(page);
        let pq = super::mi_theap_page_queue_of(theap, page);
        crate::internal::mi_assert_expensive!(mi_page_queue_contains(pq, page));
        pq
    }
}

/// # Safety
/// `queue` and `page` must be valid, and `page` must currently be linked into `queue`.
pub(crate) unsafe fn mi_page_queue_remove_raw(queue: *mut mi_page_queue_t, page: *mut mi_page_t) {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let queue = PageQueuePtr::new_unchecked(queue);
        let page = PagePtr::new_unchecked(page);
        mi_page_queue_remove_typed(queue, page);
    }
}

/// # Safety
/// `queue` and `page` must be valid, and `page` must currently be linked into `queue`.
#[inline]
pub(crate) unsafe fn mi_page_queue_remove_typed(queue: PageQueuePtr, page: PagePtr) {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let queue_ptr = queue;
        let queue = queue.as_ptr();
        let page_ptr = page;
        let page = page.as_ptr();
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_expensive!(mi_page_queue_contains(queue, page));
        crate::internal::mi_assert_internal!(super::mi_page_queue_count_typed(queue).get() >= 1);
        crate::internal::mi_assert_internal!(
            crate::internal::mi_page_block_size(page)
                == super::counts::mi_page_queue_block_size_raw(queue)
                || (super::mi_page_is_huge(page) && super::mi_page_queue_is_huge(queue))
                || (super::mi_page_is_in_full(page) && super::mi_page_queue_is_full(queue))
        );
        let theap = super::mi_page_theap(page);
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

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
pub(super) unsafe fn mi_page_queue_push_raw(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
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

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
#[inline]
unsafe fn mi_page_queue_push_internal(
    theap: crate::core_types::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
) {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        let queue_raw = queue.as_ptr();
        let page_raw = page.as_ptr();
        crate::internal::mi_assert_internal!(mi_page_theap(page_raw) == theap_raw);
        crate::internal::mi_assert_internal!(!mi_page_queue_contains(queue_raw, page_raw));
        crate::internal::mi_assert_internal!(
            mi_page_block_size(page_raw) == mi_page_queue_block_size_raw(queue_raw)
                || (mi_page_is_huge(page_raw) && mi_page_queue_is_huge(queue_raw))
                || (mi_page_is_in_full(page_raw) && mi_page_queue_is_full(queue_raw))
        );

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

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
#[inline]
unsafe fn mi_page_queue_push_at_end_internal(
    theap: crate::core_types::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
) {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        let queue_raw = queue.as_ptr();
        let page_raw = page.as_ptr();
        crate::internal::mi_assert_internal!(mi_page_theap(page_raw) == theap_raw);
        crate::internal::mi_assert_internal!(!mi_page_queue_contains(queue_raw, page_raw));
        crate::internal::mi_assert_internal!(
            mi_page_block_size(page_raw) == mi_page_queue_block_size_raw(queue_raw)
                || (mi_page_is_huge(page_raw) && mi_page_queue_is_huge(queue_raw))
                || (mi_page_is_in_full(page_raw) && mi_page_queue_is_full(queue_raw))
        );

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

#[inline]
pub(super) unsafe fn mi_page_queue_push_at_end_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    page: crate::core_types::ActivePage,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        mi_page_queue_push_at_end_internal(theap, queue.ptr(), page.page());
        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::from_enqueued(page, queue)
    }
}

#[inline]
pub(super) unsafe fn mi_page_queue_push_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    page: crate::core_types::ActivePage,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        mi_page_queue_push_internal(theap, queue.ptr(), page.page());
        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::from_enqueued(page, queue)
    }
}

/// # Safety
/// `theap` must be the local owner of the page, and `queue` must be the exact
/// normal queue represented by `queued`.
#[inline]
pub(super) unsafe fn mi_page_queue_move_to_front_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let page_ptr = queued.page();
        let page = page_ptr.as_ptr();
        crate::internal::mi_assert_internal!(mi_page_theap(page) == theap.ptr().as_ptr());
        crate::internal::mi_assert_internal!(queue.ptr() == queued.normal_queue().ptr());
        let queue_ptr = queue.ptr();
        let Some(first) = queue_ptr.first() else {
            crate::internal::mi_assert_internal!(false);
            return queued;
        };
        if ptr::eq(first.as_ptr(), page) {
            return queued;
        }

        // The typed witness carries the exact normal queue containing this
        // page, so relinking preserves its non-full flag and both page counts.
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

pub(crate) unsafe fn mi_page_queue_enqueue_from_raw(
    to: crate::core_types::PageQueuePtr,
    from: crate::core_types::PageQueuePtr,
    enqueue_at_end: bool,
    page: crate::core_types::PagePtr,
) {
    // SAFETY: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let to_ptr = to;
        let from_ptr = from;
        let to = to.as_ptr();
        let from = from.as_ptr();
        let page_ptr = page;
        let page = page.as_ptr();
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

        let theap = mi_page_theap(page);

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
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
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
    // SAFETY: queue validation walks linked pages without mutating queue ownership.
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
    fn move_to_front_relinks_without_changing_counts_or_flags() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut pages = [const { MaybeUninit::<mi_page_t>::zeroed() }; 3];

        // SAFETY: the stack-local heap,
        // queue, and page metadata are initialized for the exact fields read
        // and written by the typed normal-queue relink operation.
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
            let active = crate::core_types::assume_active_page(middle);
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

            let active = crate::core_types::assume_active_page(last);
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
        }
    }
}
