use super::*;
use crate::types::{
    MI_BIN_FULL, PageFlags, PageOwnerId, PageThreadId, ThreadId, mi_page_queue_t, mi_page_t,
    mi_theap_t,
};
use core::{mem::MaybeUninit, ptr};

fn fixture_page_thread_id(flags: PageFlags) -> usize {
    let owner = ThreadId::new(0x1000).expect("valid fixture thread id");
    PageThreadId::from_parts(PageOwnerId::Thread(owner), flags).raw()
}

fn with_queue(block_size: usize, f: impl FnOnce(PageQueuePtr)) {
    let mut queue = MaybeUninit::<mi_page_queue_t>::zeroed();
    let raw = queue.as_mut_ptr();
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
    let mut heap = MaybeUninit::<mi_theap_t>::zeroed();
    let heap_raw = heap.as_mut_ptr();
    unsafe {
        let queue_raw = ptr::addr_of_mut!((*heap_raw).pages[0]);
        (*page_raw).theap = heap_raw;
        (*heap_raw).page_count = usize::from(linked);
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

fn owner_of(page: PagePtr) -> crate::core_types::ThreadHeapLocal {
    unsafe {
        crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked((*page.as_ptr()).theap),
        )
    }
}

#[test]
fn normal_queue_constructor_accepts_initialized_non_special_queues() {
    with_queue(64, |queue| {
        let normal = unsafe { NormalPageQueue::new(queue) }
            .expect("ordinary block-size queue should be normal");
        assert_eq!(normal.ptr().as_ptr(), queue.as_ptr());
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
        assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
        // SAFETY: this fixture's live queue has the initialized huge sentinel
        // block size required to construct the huge-queue witness.
        let huge =
            unsafe { HugePageQueue::new(queue) }.expect("huge sentinel queue should be huge");
        assert_eq!(huge.ptr().as_ptr(), queue.as_ptr());
        assert!(unsafe { FullPageQueue::new(queue) }.is_none());
    });

    let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
    with_queue(full_size, |queue| {
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
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: the stack-local queue remains live with its ordinary non-zero
        // block-size class initialized for normal-queue classification.
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        let (member, linked_queue) = unsafe {
            PageQueue::normal(normal).try_with_member(owner_of(page), active, |queued| {
                (queued.page(), queued.queue)
            })
        }
        .expect("linked page");
        assert_eq!(member.as_ptr(), page.as_ptr());
        assert_eq!(linked_queue.as_ptr(), queue.as_ptr());
    });

    with_page_in_queue(64, false, |page, queue| {
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: only page linkage is absent; the live stack-local queue still
        // has a valid initialized normal block-size class.
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        // SAFETY: both fixtures remain live and stable; the constructor reads
        // their initialized metadata and must reject the deliberately absent link.
        let active =
            unsafe { PageQueue::normal(normal).try_with_member(owner_of(page), active, |_| ()) }
                .expect_err("unlinked page is returned");
        assert_eq!(active.page().as_ptr(), page.as_ptr());
    });

    with_page_in_queue_with_page_block_size(64, 128, true, |page, queue| {
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: the stack-local queue remains live with a valid initialized
        // normal block-size class despite the page's mismatched class.
        let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
        // SAFETY: both fixtures remain live and stable while validation reads
        // and rejects their deliberately mismatched block-size fields.
        assert!(
            unsafe { PageQueue::normal(normal).try_with_member(owner_of(page), active, |_| ()) }
                .is_err()
        );
    });

    let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
    let page_huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + 1;
    with_page_in_queue_with_page_block_size(huge_size, page_huge_size, true, |page, queue| {
        unsafe {
            (*page.as_ptr()).capacity = 1;
            (*page.as_ptr()).reserved = 1;
        }
        let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
        // SAFETY: the live sentinel queue intentionally bypasses ordinary
        // normal classification for the huge-page compatibility edge case.
        let normal = unsafe { NormalPageQueue::new_unchecked(queue) };
        unsafe {
            PageQueue::normal(normal)
                .try_with_member(owner_of(page), active, |queued| {
                    assert_eq!(queued.page().as_ptr(), page.as_ptr());
                    assert_eq!(queued.queue.as_ptr(), queue.as_ptr());
                })
                .expect("huge page");
        }
    });
}

#[test]
fn queued_full_page_requires_page_linked_in_full_queue() {
    let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
    let mut page = MaybeUninit::<mi_page_t>::zeroed();
    let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
    // SAFETY: the stack-local fixture initializes the page association and the embedded full queue
    // fields used by this identity-only capability construction. Both remain live and stable until
    // the consumed queued witness has been inspected and dropped without performing queue mutation.
    unsafe {
        let page_raw = page.as_mut_ptr();
        let theap_raw = theap.as_mut_ptr();
        let queue_raw = ptr::addr_of_mut!((*theap_raw).pages[MI_BIN_FULL]);
        (*page_raw).theap = theap_raw;
        (*page_raw).xthread_id = fixture_page_thread_id(PageFlags::IN_FULL_QUEUE);
        (*queue_raw).block_size = full_size;
        (*queue_raw).first = page_raw;
        (*queue_raw).last = page_raw;
        (*queue_raw).count = 1;
        let page_ptr = PagePtr::new_unchecked(page_raw);
        let queue_ptr = PageQueuePtr::new_unchecked(queue_raw);
        let full_page = crate::core_types::FullPage::new_unchecked(page_ptr);
        let full_queue = FullPageQueue::new(queue_ptr).expect("full queue");
        PageQueue::full(full_queue).with_member(owner_of(page_ptr), full_page, |queued| {
            assert_eq!(queued.page().as_ptr(), page_raw);
            assert_eq!(queued.queue.as_ptr(), queue_raw);
        });
    }

    with_page_in_queue(full_size, true, |page, queue| {
        unsafe {
            (*page.as_ptr()).xthread_id = fixture_page_thread_id(PageFlags::IN_FULL_QUEUE);
        }
        let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
        // SAFETY: the live stack-local queue retains its initialized full
        // sentinel block size for full-queue classification.
        let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
        unsafe {
            PageQueue::full(full_queue)
                .try_with_member(owner_of(page), full_page, |queued| {
                    assert_eq!(queued.page().as_ptr(), page.as_ptr());
                    assert_eq!(queued.queue.as_ptr(), queue.as_ptr());
                })
                .expect("linked page");
        }
    });

    with_page_in_queue(full_size, true, |page, linked_queue| {
        // SAFETY: the fixture's owner bits remain intact while adding the full
        // flag required by a valid FullPage witness linked in `linked_queue`.
        unsafe {
            (*page.as_ptr()).xthread_id = fixture_page_thread_id(PageFlags::IN_FULL_QUEUE);
        }
        unsafe {
            let owner = owner_of(page);
            let raw = ptr::addr_of_mut!((*owner.ptr().as_ptr()).pages[1]);
            (*raw).block_size = full_size;
            let other_queue = PageQueuePtr::new_unchecked(raw);
            let full_page = crate::core_types::FullPage::new_unchecked(page);
            let other_full = FullPageQueue::new(other_queue).expect("full queue");
            let full_page = PageQueue::full(other_full)
                .try_with_member(owner, full_page, |_| ())
                .expect_err("page linked in a different full queue is returned");
            assert_eq!(full_page.page().as_ptr(), page.as_ptr());
            assert_ne!(linked_queue.as_ptr(), other_queue.as_ptr());
        }
    });
}
