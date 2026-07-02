use super::*;
use crate::types::{mi_page_queue_t, mi_page_t};
use core::mem::MaybeUninit;

fn with_queue(block_size: usize, f: impl FnOnce(PageQueuePtr)) {
    let mut queue = MaybeUninit::<mi_page_queue_t>::zeroed();
    let raw = queue.as_mut_ptr();
    // SAFETY[AUDIT:typed-page-queue-test-fixtures]: the fixture initializes the
    // queue fields needed by page-queue constructors before building a typed
    // non-null queue witness over stack-local metadata.
    unsafe {
        (*raw).block_size = block_size;
        f(PageQueuePtr::new_unchecked(raw));
    }
}

fn with_page_in_queue(block_size: usize, linked: bool, f: impl FnOnce(PagePtr, PageQueuePtr)) {
    with_page_in_queue_with_page_block_size(block_size, block_size, linked, f);
}

fn with_page_in_queue_with_page_block_size(
    queue_block_size: usize,
    page_block_size: usize,
    linked: bool,
    f: impl FnOnce(PagePtr, PageQueuePtr),
) {
    let mut page = MaybeUninit::<mi_page_t>::zeroed();
    let page_raw = page.as_mut_ptr();
    let mut queue = MaybeUninit::<mi_page_queue_t>::zeroed();
    let queue_raw = queue.as_mut_ptr();
    // SAFETY[AUDIT:typed-page-queue-test-fixtures]: the fixture initializes
    // only the page and queue fields needed by queue membership checks before
    // constructing typed witnesses over stack-local metadata.
    unsafe {
        (*page_raw).block_size = page_block_size;
        (*queue_raw).block_size = queue_block_size;
        if linked {
            (*queue_raw).first = page_raw;
            (*queue_raw).last = page_raw;
            (*queue_raw).count = 1;
        }
        f(
            PagePtr::new_unchecked(page_raw),
            PageQueuePtr::new_unchecked(queue_raw),
        );
    }
}

#[test]
fn normal_queue_constructor_accepts_initialized_non_special_queues() {
    with_queue(64, |queue| {
        // SAFETY[AUDIT:typed-page-queue-test-fixtures]: `with_queue` supplies an
        // initialized non-special queue witness for constructor classification.
        let normal = unsafe { NormalPageQueue::new(queue) }
            .expect("ordinary block-size queue should be normal");
        assert_eq!(normal.ptr().as_ptr(), queue.as_ptr());
        // SAFETY[AUDIT:typed-page-queue-test-fixtures]: the same initialized
        // non-special queue must not classify as a huge or full sentinel queue.
        assert!(unsafe { HugePageQueue::new(queue) }.is_none());
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });
}

#[test]
fn special_queue_constructors_accept_only_matching_sentinel_sizes() {
    let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
    with_queue(huge_size, |queue| {
        // SAFETY[AUDIT:typed-page-queue-test-fixtures]: `with_queue` supplies a
        // stack-local queue whose block size is the huge-queue sentinel.
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        let huge =
            unsafe { HugePageQueue::new(queue) }.expect("huge sentinel queue should be huge");
        assert_eq!(huge.ptr().as_ptr(), queue.as_ptr());
        // SAFETY[AUDIT:typed-page-queue-test-fixtures]: the huge sentinel queue
        // must not also classify as a full sentinel queue.
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });

    let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
    with_queue(full_size, |queue| {
        // SAFETY[AUDIT:typed-page-queue-test-fixtures]: `with_queue` supplies a
        // stack-local queue whose block size is the full-queue sentinel.
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        assert!(unsafe { HugePageQueue::new(queue) }.is_none());
        let full =
            unsafe { FullPageQueue::new(queue) }.expect("full sentinel queue should be full");
        assert_eq!(full.ptr().as_ptr(), queue.as_ptr());
    });
}

#[test]
fn queue_constructors_reject_uninitialized_block_size() {
    with_queue(0, |queue| {
        // SAFETY[AUDIT:typed-page-queue-test-fixtures]: `with_queue` supplies a
        // stack-local queue with an intentionally invalid zero block-size field.
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        assert!(unsafe { HugePageQueue::new(queue) }.is_none());
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });
}

#[test]
fn queued_normal_page_requires_page_linked_in_normal_queue() {
    with_page_in_queue(64, true, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture links the
        // stack-local page into the matching normal queue before creating typed
        // active-page and queued-page witnesses.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the active page and
        // normal queue witnesses come from the linked stack-local fixture.
        let queued =
            unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.expect("linked page");
        assert_eq!(queued.page().as_ptr(), page.as_ptr());
        assert_eq!(queued.queue().ptr.as_ptr(), queue.as_ptr());
    });

    with_page_in_queue(64, false, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture leaves the
        // stack-local page unlinked to exercise queued-page rejection.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        assert!(unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.is_none());
    });

    with_page_in_queue(64, true, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture sets only
        // the full-queue ownership flag on stack-local page metadata to verify
        // that normal queued-page construction rejects it.
        unsafe {
            (*page.as_ptr()).xthread_id = crate::types::MI_PAGE_IN_FULL_QUEUE;
        }
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the typed witnesses are
        // built from the flagged stack-local page and matching queue to assert
        // normal queued-page rejection.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        assert!(unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.is_none());
    });

    with_page_in_queue_with_page_block_size(64, 128, true, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture links a
        // stack-local page whose block size intentionally mismatches the queue.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        assert!(unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.is_none());
    });

    let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
    let page_huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + 1;
    with_page_in_queue_with_page_block_size(huge_size, page_huge_size, true, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture initializes
        // minimal huge-page capacity metadata and uses the sentinel normal-queue
        // witness required by the queue acceptance edge case.
        unsafe {
            (*page.as_ptr()).capacity = 1;
            (*page.as_ptr()).reserved = 1;
        }
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the active-page,
        // unchecked normal-queue, and queued-page witnesses use the initialized
        // stack-local huge-page edge-case fixture above.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        let normal = unsafe { NormalPageQueue::new_unchecked(queue) };
        let queued = unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.expect("huge page");
        assert_eq!(queued.page().as_ptr(), page.as_ptr());
        assert_eq!(queued.queue().ptr.as_ptr(), queue.as_ptr());
    });
}

#[test]
fn queued_full_page_requires_page_linked_in_full_queue() {
    let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
    with_page_in_queue(full_size, true, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture links the
        // stack-local page into a full queue and sets the full-queue flag before
        // constructing typed full-page and queued-page witnesses.
        unsafe {
            (*page.as_ptr()).xthread_id = crate::types::MI_PAGE_IN_FULL_QUEUE;
        }
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the full-page,
        // full-queue, and queued-page witnesses use the linked stack-local
        // fixture with the full-queue flag set above.
        let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
        let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the linked full-page
        // and full-queue witnesses satisfy queued full-page construction.
        let queued =
            unsafe { QueuedPage::<FullQueue>::new(full_page, full_queue) }.expect("linked page");
        assert_eq!(queued.page().as_ptr(), page.as_ptr());
        assert_eq!(queued.queue().ptr.as_ptr(), queue.as_ptr());
    });

    with_page_in_queue(full_size, false, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture sets the
        // full-queue flag but leaves the page unlinked to exercise rejection.
        unsafe {
            (*page.as_ptr()).xthread_id = crate::types::MI_PAGE_IN_FULL_QUEUE;
        }
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the full-page and
        // full-queue witnesses come from the unlinked stack-local rejection
        // fixture above.
        let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
        let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
        assert!(unsafe { QueuedPage::<FullQueue>::new(full_page, full_queue) }.is_none());
    });

    with_page_in_queue(full_size, true, |page, queue| {
        // SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]: the fixture links the
        // page into a full queue but intentionally omits the full-queue flag.
        let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
        let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
        assert!(unsafe { QueuedPage::<FullQueue>::new(full_page, full_queue) }.is_none());
    });
}
