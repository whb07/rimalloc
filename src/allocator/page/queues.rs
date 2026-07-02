use core::ptr;

use crate::core_types::{PagePtr, PageQueuePtr};
use crate::types::{MI_BIN_FULL, mi_page_queue_t, mi_page_t};

/// # Safety
/// `queue` must be valid, and `page` must be null or page metadata compatible with that queue.
#[inline]
pub unsafe fn mi_page_queue_contains(queue: *mut mi_page_queue_t, page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:page-queue-validation-boundary]: queue validation walks linked pages without mutating queue ownership.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let queue_ptr = super::counts::page_queue_ptr(queue);
        let mut list = queue_ptr
            .first()
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        while !list.is_null() {
            crate::internal::mi_assert_internal!(
                (*list).next.is_null() || (*(*list).next).prev == list
            );
            crate::internal::mi_assert_internal!(
                (*list).prev.is_null() || (*(*list).prev).next == list
            );
            if ptr::eq(list, page.cast_mut()) {
                break;
            }
            list = (*list).next;
        }
        ptr::eq(list, page.cast_mut())
    }
}

/// # Safety
/// `page` must point to non-abandoned page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_queue_of(page: *const mi_page_t) -> *mut mi_page_queue_t {
    // SAFETY[AUDIT:page-stats-public-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let theap = crate::page::mi_page_theap(page);
        let pq = super::mi_theap_page_queue_of(theap, page);
        crate::internal::mi_assert_expensive!(mi_page_queue_contains(pq, page));
        pq
    }
}

/// # Safety
/// `queue` and `page` must be valid, and `page` must currently be linked into `queue`.
pub(crate) unsafe fn mi_page_queue_remove_raw(queue: *mut mi_page_queue_t, page: *mut mi_page_t) {
    // SAFETY[AUDIT:page-queue-link-mutation-boundary]: queue link mutations update first/last pointers, counts, and in-full state together.
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
    // SAFETY[AUDIT:page-queue-link-mutation-boundary]: queue link mutations update first/last pointers, counts, and in-full state together.
    unsafe {
        let queue_ptr = queue;
        let queue = queue.as_ptr();
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
        if !(*page).prev.is_null() {
            (*(*page).prev).next = (*page).next;
        }
        if !(*page).next.is_null() {
            (*(*page).next).prev = (*page).prev;
        }
        if queue_ptr
            .last()
            .is_some_and(|last| ptr::eq(page, last.as_ptr()))
        {
            queue_ptr.set_last(PagePtr::new((*page).prev));
        }
        if queue_ptr
            .first()
            .is_some_and(|first| ptr::eq(page, first.as_ptr()))
        {
            queue_ptr.set_first(PagePtr::new((*page).next));
            crate::internal::mi_assert_internal!(mi_theap_contains_queue(theap, queue));
            super::mi_theap_queue_first_update(theap, queue);
        }
        (*theap).page_count -= 1;
        super::counts::mi_page_queue_decrement_count(queue);
        (*page).next = ptr::null_mut();
        (*page).prev = ptr::null_mut();
        super::mi_page_set_in_full(page, false);
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
    // SAFETY[AUDIT:page-queue-validation-boundary]: queue validation walks linked pages without mutating queue ownership.
    unsafe {
        let start = crate::core_types::ptr_exposed_addr(ptr::addr_of!((*theap).pages[0]));
        let end = crate::core_types::ptr_exposed_addr(ptr::addr_of!((*theap).pages[MI_BIN_FULL]));
        let queue = crate::core_types::ptr_exposed_addr(pq);
        queue >= start && queue <= end
    }
}
