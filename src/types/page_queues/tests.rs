use super::*;
use crate::types::{PageFlags, PageOwnerId, PageThreadId, ThreadId, mi_page_queue_t, mi_page_t};
use core::mem::MaybeUninit;

fn fixture_page_thread_id(flags: PageFlags) -> usize {
    let owner = ThreadId::new(0x1000).expect("valid fixture thread id");
    PageThreadId::from_parts(PageOwnerId::Thread(owner), flags).raw()
}

fn with_queue(block_size: usize, f: impl FnOnce(PageQueuePtr)) {
    let mut queue = MaybeUninit::<mi_page_queue_t>::zeroed();
    let raw = queue.as_mut_ptr();
    // SAFETY: the fixture initializes the
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
    // SAFETY: the fixture initializes
    // only the page and queue fields needed by queue membership checks before
    // constructing typed witnesses over stack-local metadata.
    unsafe {
        (*page_raw).block_size = page_block_size;
        (*page_raw).xthread_id = fixture_page_thread_id(PageFlags::new(0).unwrap());
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
        // SAFETY: `with_queue` supplies an
        // initialized non-special queue witness for constructor classification.
        let normal = unsafe { NormalPageQueue::new(queue) }
            .expect("ordinary block-size queue should be normal");
        assert_eq!(normal.ptr().as_ptr(), queue.as_ptr());
        // SAFETY: the same initialized
        // non-special queue must not classify as a huge or full sentinel queue.
        assert!(unsafe { HugePageQueue::new(queue) }.is_none());
        // SAFETY: the same stack-local initialized block-size field remains
        // readable and is not the full-queue sentinel.
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });
}

#[test]
fn special_queue_constructors_accept_only_matching_sentinel_sizes() {
    let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
    with_queue(huge_size, |queue| {
        // SAFETY: `with_queue` supplies a
        // stack-local queue whose block size is the huge-queue sentinel.
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        // SAFETY: this fixture's live queue has the initialized huge sentinel
        // block size required to construct the huge-queue witness.
        let huge =
            unsafe { HugePageQueue::new(queue) }.expect("huge sentinel queue should be huge");
        assert_eq!(huge.ptr().as_ptr(), queue.as_ptr());
        // SAFETY: the huge sentinel queue
        // must not also classify as a full sentinel queue.
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });

    let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
    with_queue(full_size, |queue| {
        // SAFETY: `with_queue` supplies a
        // stack-local queue whose block size is the full-queue sentinel.
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        // SAFETY: this fixture's initialized full sentinel is distinct from
        // the huge sentinel, so huge classification may read and reject it.
        assert!(unsafe { HugePageQueue::new(queue) }.is_none());
        // SAFETY: the same live stack-local queue retains the initialized full
        // sentinel required to construct a full-queue witness.
        let full =
            unsafe { FullPageQueue::new(queue) }.expect("full sentinel queue should be full");
        assert_eq!(full.ptr().as_ptr(), queue.as_ptr());
    });
}

#[test]
fn queue_constructors_reject_uninitialized_block_size() {
    with_queue(0, |queue| {
        // SAFETY: `with_queue` supplies a
        // stack-local queue with an intentionally invalid zero block-size field.
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        // SAFETY: the fixture remains live and readable while huge
        // classification rejects its intentionally zero block-size field.
        assert!(unsafe { HugePageQueue::new(queue) }.is_none());
        // SAFETY: the same zero-initialized field remains readable while full
        // classification rejects it.
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });
}

#[test]
fn queued_normal_page_requires_page_linked_in_normal_queue() {
    with_page_in_queue(64, true, |page, queue| {
        // SAFETY: the fixture links the
        // stack-local page into the matching normal queue before creating typed
        // active-page and queued-page witnesses.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: the stack-local queue remains live with its ordinary non-zero
        // block-size class initialized for normal-queue classification.
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        // SAFETY: the active page and
        // normal queue witnesses come from the linked stack-local fixture.
        let queued =
            unsafe { QueuedPage::<NormalQueue>::try_queue(active, normal) }.expect("linked page");
        assert_eq!(queued.page().as_ptr(), page.as_ptr());
        assert_eq!(queued.queue.ptr.as_ptr(), queue.as_ptr());
    });

    with_page_in_queue(64, false, |page, queue| {
        // SAFETY: the fixture leaves the
        // stack-local page unlinked to exercise queued-page rejection.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: only page linkage is absent; the live stack-local queue still
        // has a valid initialized normal block-size class.
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        // SAFETY: both fixtures remain live and stable; the constructor reads
        // their initialized metadata and must reject the deliberately absent link.
        let active = unsafe { QueuedPage::<NormalQueue>::try_queue(active, normal) }
            .expect_err("unlinked page is returned");
        assert_eq!(active.page().as_ptr(), page.as_ptr());
    });

    with_page_in_queue_with_page_block_size(64, 128, true, |page, queue| {
        // SAFETY: the fixture links a
        // stack-local page whose block size intentionally mismatches the queue.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: the stack-local queue remains live with a valid initialized
        // normal block-size class despite the page's mismatched class.
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        // SAFETY: both fixtures remain live and stable while validation reads
        // and rejects their deliberately mismatched block-size fields.
        assert!(unsafe { QueuedPage::<NormalQueue>::try_queue(active, normal) }.is_err());
    });

    let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
    let page_huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + 1;
    with_page_in_queue_with_page_block_size(huge_size, page_huge_size, true, |page, queue| {
        // SAFETY: the fixture initializes
        // minimal huge-page capacity metadata and uses the sentinel normal-queue
        // witness required by the queue acceptance edge case.
        unsafe {
            (*page.as_ptr()).capacity = 1;
            (*page.as_ptr()).reserved = 1;
        }
        // SAFETY: the active-page,
        // unchecked normal-queue, and queued-page witnesses use the initialized
        // stack-local huge-page edge-case fixture above.
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: the live sentinel queue intentionally bypasses ordinary
        // normal classification for the huge-page compatibility edge case.
        let normal = unsafe { NormalPageQueue::new_unchecked(queue) };
        // SAFETY: the active
        // page and sentinel queue witnesses remain live for the transition.
        let queued =
            unsafe { QueuedPage::<NormalQueue>::try_queue(active, normal) }.expect("huge page");
        assert_eq!(queued.page().as_ptr(), page.as_ptr());
        assert_eq!(queued.queue.ptr.as_ptr(), queue.as_ptr());
    });
}

#[test]
fn queued_full_page_requires_page_linked_in_full_queue() {
    let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
    with_page_in_queue(full_size, true, |page, queue| {
        // SAFETY: the fixture links the
        // stack-local page into a full queue and sets the full-queue flag before
        // constructing typed full-page and queued-page witnesses.
        unsafe {
            (*page.as_ptr()).xthread_id = fixture_page_thread_id(PageFlags::IN_FULL_QUEUE);
        }
        // SAFETY: the full-page,
        // full-queue, and queued-page witnesses use the linked stack-local
        // fixture with the full-queue flag set above.
        let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
        // SAFETY: the live stack-local queue retains its initialized full
        // sentinel block size for full-queue classification.
        let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
        // SAFETY: the linked full-page
        // and full-queue witnesses satisfy queued full-page construction.
        let queued = unsafe { QueuedPage::<FullQueue>::try_queue_full(full_page, full_queue) }
            .expect("linked page");
        assert_eq!(queued.page().as_ptr(), page.as_ptr());
        assert_eq!(queued.queue.ptr.as_ptr(), queue.as_ptr());
    });

    with_page_in_queue(full_size, true, |page, linked_queue| {
        // SAFETY: the fixture's owner bits remain intact while adding the full
        // flag required by a valid FullPage witness linked in `linked_queue`.
        unsafe {
            (*page.as_ptr()).xthread_id = fixture_page_thread_id(PageFlags::IN_FULL_QUEUE);
        }
        with_queue(full_size, |other_queue| {
            // SAFETY: the page remains a valid FullPage in `linked_queue`, and
            // this separate initialized full queue is used to test mismatch.
            let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
            // SAFETY: `other_queue` is live and initialized with the full sentinel.
            let other_full = unsafe { FullPageQueue::new(other_queue) }.expect("full queue");
            // SAFETY: both queue fixtures and the linked page remain live and
            // stable; validation must reject membership in the other queue.
            let full_page =
                unsafe { QueuedPage::<FullQueue>::try_queue_full(full_page, other_full) }
                    .expect_err("page linked in a different full queue is returned");
            assert_eq!(full_page.page().as_ptr(), page.as_ptr());
            assert_ne!(linked_queue.as_ptr(), other_queue.as_ptr());
        });
    });
}
