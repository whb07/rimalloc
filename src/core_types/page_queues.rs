//! DT-10: Page queue types.

use crate::core_types::page_states::Page;
use crate::core_types::{PagePtr, PageQueuePtr};
use core::marker::PhantomData;
use core::mem::size_of;

pub(crate) enum NormalQueue {}
pub(crate) enum HugeQueue {}
pub(crate) enum FullQueue {}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct PageQueue<K> {
    ptr: PageQueuePtr,
    _kind: PhantomData<K>,
}

impl PageQueue<NormalQueue> {
    #[inline(always)]
    pub(crate) fn normal(queue: NormalPageQueue) -> Self {
        Self {
            ptr: queue.ptr(),
            _kind: PhantomData,
        }
    }
}

impl PageQueue<FullQueue> {
    #[inline(always)]
    pub(crate) fn full(queue: FullPageQueue) -> Self {
        Self {
            ptr: queue.ptr(),
            _kind: PhantomData,
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct NormalPageQueue(PageQueuePtr);

impl NormalPageQueue {
    /// # Safety
    /// `ptr` must point to initialized page queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(ptr: PageQueuePtr) -> Option<Self> {
        let block_size = unsafe { ptr.block_size()? };
        if block_size.get() <= crate::types::MI_LARGE_MAX_OBJ_SIZE {
            Some(Self(ptr))
        } else {
            None
        }
    }

    /// # Safety
    /// `ptr` must point to a heap normal page queue, proven by queue lookup or
    /// queue initialization code.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: PageQueuePtr) -> Self {
        Self(ptr)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> PageQueuePtr {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct HugePageQueue(PageQueuePtr);

impl HugePageQueue {
    /// # Safety
    /// `ptr` must point to initialized page queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(ptr: PageQueuePtr) -> Option<Self> {
        let block_size = unsafe { ptr.block_size()? };
        if block_size.get() == crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>() {
            Some(Self(ptr))
        } else {
            None
        }
    }

    /// # Safety
    /// `ptr` must point to a heap huge page queue, proven by queue lookup or
    /// queue initialization code.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: PageQueuePtr) -> Self {
        Self(ptr)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> PageQueuePtr {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct FullPageQueue(PageQueuePtr);

impl FullPageQueue {
    /// # Safety
    /// `ptr` must point to initialized page queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(ptr: PageQueuePtr) -> Option<Self> {
        let block_size = unsafe { ptr.block_size()? };
        if block_size.get() == crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>()) {
            Some(Self(ptr))
        } else {
            None
        }
    }

    /// # Safety
    /// `ptr` must point to the heap full page queue, proven by queue lookup or
    /// queue initialization code.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: PageQueuePtr) -> Self {
        Self(ptr)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> PageQueuePtr {
        self.0
    }
}

/// Witness that a page is currently inside a specific queue.
pub(crate) struct QueuedPage<K> {
    pub(crate) page: PagePtr,
    pub(crate) queue: PageQueue<K>,
}

impl<K> QueuedPage<K> {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }

    #[inline(always)]
    pub(crate) fn queue(&self) -> &PageQueue<K> {
        &self.queue
    }
}

impl QueuedPage<NormalQueue> {
    /// # Safety
    /// `page` and `queue` must belong to the same initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(
        page: crate::core_types::ActivePage,
        queue: NormalPageQueue,
    ) -> Option<Self> {
        let queue_ptr = queue.ptr().as_ptr();
        let page_ptr = page.page().as_ptr();
        if unsafe {
            crate::page::mi_page_queue_contains(queue_ptr, page_ptr)
                && !crate::page::mi_page_is_in_full(page_ptr)
                && (queue
                    .ptr()
                    .block_size()
                    .is_some_and(|block_size| page.page().block_size().get() == block_size.get())
                    || (crate::page::mi_page_is_huge(page_ptr)
                        && crate::page::mi_page_queue_is_huge(queue_ptr)))
        } {
            Some(Self {
                page: page.page(),
                queue: PageQueue::normal(queue),
            })
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage {
        unsafe {
            crate::page::mi_page_queue_remove_typed(self.queue.ptr, self.page);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn into_full(self, to: FullPageQueue) -> crate::core_types::FullPage {
        unsafe {
            crate::page::mi_page_queue_enqueue_from_raw(to.ptr(), self.queue.ptr, true, self.page);
            crate::page::mi_page_set_in_full(self.page.as_ptr(), true);
            crate::core_types::FullPage::new_unchecked(self.page)
        }
    }
}

impl QueuedPage<FullQueue> {
    /// # Safety
    /// `page` and `queue` must belong to the same initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(
        page: crate::core_types::FullPage,
        queue: FullPageQueue,
    ) -> Option<Self> {
        if unsafe {
            crate::page::mi_page_queue_contains(queue.ptr().as_ptr(), page.page().as_ptr())
                && crate::page::mi_page_is_in_full(page.page().as_ptr())
        } {
            Some(Self {
                page: page.page(),
                queue: PageQueue::full(queue),
            })
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn unfull(self, to: NormalPageQueue) -> crate::core_types::ActivePage {
        unsafe {
            crate::page::mi_page_queue_enqueue_from_raw(to.ptr(), self.queue.ptr, true, self.page);
            crate::page::mi_page_set_in_full(self.page.as_ptr(), false);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage {
        unsafe {
            crate::page::mi_page_queue_remove_typed(self.queue.ptr, self.page);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }
}

/// Witness that a page is definitively *not* in any queue.
pub(crate) struct UnqueuedPage<S> {
    pub(crate) page: Page<S>,
}

// Strictly typed transitions to prevent illegal states, per DT-10.

#[inline]
pub(crate) fn free_all_free_page_to_arena<S>(
    page: crate::core_types::page_states::AllFreePage<S>,
    _owner: crate::core_types::page_states::ThreadHeapLocal,
) -> crate::core_types::page_states::DestroyedPage {
    unsafe { crate::core_types::page_states::DestroyedPage::new_unchecked(page.page()) }
}

const _: unsafe fn(PageQueuePtr) -> Option<NormalPageQueue> = NormalPageQueue::new;
const _: unsafe fn(PageQueuePtr) -> Option<HugePageQueue> = HugePageQueue::new;
const _: unsafe fn(PageQueuePtr) -> Option<FullPageQueue> = FullPageQueue::new;
const _: unsafe fn(
    crate::core_types::ActivePage,
    NormalPageQueue,
) -> Option<QueuedPage<NormalQueue>> = QueuedPage::<NormalQueue>::new;
const _: unsafe fn(crate::core_types::FullPage, FullPageQueue) -> Option<QueuedPage<FullQueue>> =
    QueuedPage::<FullQueue>::new;
const _: unsafe fn(QueuedPage<FullQueue>, NormalPageQueue) -> crate::core_types::ActivePage =
    QueuedPage::<FullQueue>::unfull;
const _: unsafe fn(QueuedPage<FullQueue>) -> crate::core_types::ActivePage =
    QueuedPage::<FullQueue>::remove;
const _: unsafe fn(QueuedPage<NormalQueue>) -> crate::core_types::ActivePage =
    QueuedPage::<NormalQueue>::remove;
const _: unsafe fn(QueuedPage<NormalQueue>, FullPageQueue) -> crate::core_types::FullPage =
    QueuedPage::<NormalQueue>::into_full;
const _: fn(&QueuedPage<NormalQueue>) -> PagePtr = QueuedPage::<NormalQueue>::page;
const _: fn(&QueuedPage<FullQueue>) -> PagePtr = QueuedPage::<FullQueue>::page;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{mi_page_queue_t, mi_page_t};
    use core::mem::MaybeUninit;

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
        let mut queue = MaybeUninit::<mi_page_queue_t>::zeroed();
        let queue_raw = queue.as_mut_ptr();
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
            let normal = unsafe { NormalPageQueue::new(queue) }
                .expect("ordinary block-size queue should be normal");
            assert_eq!(normal.ptr().as_ptr(), queue.as_ptr());
            assert!(unsafe { HugePageQueue::new(queue) }.is_none());
            assert!(unsafe { FullPageQueue::new(queue) }.is_none());
        });
    }

    #[test]
    fn special_queue_constructors_accept_only_matching_sentinel_sizes() {
        let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
        with_queue(huge_size, |queue| {
            assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
            let huge =
                unsafe { HugePageQueue::new(queue) }.expect("huge sentinel queue should be huge");
            assert_eq!(huge.ptr().as_ptr(), queue.as_ptr());
            assert!(unsafe { FullPageQueue::new(queue) }.is_none());
        });

        let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
        with_queue(full_size, |queue| {
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
            assert!(unsafe { NormalPageQueue::new(queue) }.is_none());
            assert!(unsafe { HugePageQueue::new(queue) }.is_none());
            assert!(unsafe { FullPageQueue::new(queue) }.is_none());
        });
    }

    #[test]
    fn queued_normal_page_requires_page_linked_in_normal_queue() {
        with_page_in_queue(64, true, |page, queue| {
            let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
            let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
            let queued =
                unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.expect("linked page");
            assert_eq!(queued.page().as_ptr(), page.as_ptr());
            assert_eq!(queued.queue().ptr.as_ptr(), queue.as_ptr());
        });

        with_page_in_queue(64, false, |page, queue| {
            let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
            let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
            assert!(unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.is_none());
        });

        with_page_in_queue(64, true, |page, queue| {
            unsafe {
                (*page.as_ptr()).xthread_id = crate::types::MI_PAGE_IN_FULL_QUEUE;
            }
            let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
            let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
            assert!(unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.is_none());
        });

        with_page_in_queue_with_page_block_size(64, 128, true, |page, queue| {
            let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
            let normal = unsafe { NormalPageQueue::new(queue) }.expect("normal queue");
            assert!(unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.is_none());
        });

        let huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>();
        let page_huge_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + 1;
        with_page_in_queue_with_page_block_size(huge_size, page_huge_size, true, |page, queue| {
            unsafe {
                (*page.as_ptr()).capacity = 1;
                (*page.as_ptr()).reserved = 1;
            }
            let active = unsafe { crate::core_types::ActivePage::new_unchecked(page) };
            let normal = unsafe { NormalPageQueue::new_unchecked(queue) };
            let queued =
                unsafe { QueuedPage::<NormalQueue>::new(active, normal) }.expect("huge page");
            assert_eq!(queued.page().as_ptr(), page.as_ptr());
            assert_eq!(queued.queue().ptr.as_ptr(), queue.as_ptr());
        });
    }

    #[test]
    fn queued_full_page_requires_page_linked_in_full_queue() {
        let full_size = crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>());
        with_page_in_queue(full_size, true, |page, queue| {
            unsafe {
                (*page.as_ptr()).xthread_id = crate::types::MI_PAGE_IN_FULL_QUEUE;
            }
            let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
            let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
            let queued = unsafe { QueuedPage::<FullQueue>::new(full_page, full_queue) }
                .expect("linked page");
            assert_eq!(queued.page().as_ptr(), page.as_ptr());
            assert_eq!(queued.queue().ptr.as_ptr(), queue.as_ptr());
        });

        with_page_in_queue(full_size, false, |page, queue| {
            unsafe {
                (*page.as_ptr()).xthread_id = crate::types::MI_PAGE_IN_FULL_QUEUE;
            }
            let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
            let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
            assert!(unsafe { QueuedPage::<FullQueue>::new(full_page, full_queue) }.is_none());
        });

        with_page_in_queue(full_size, true, |page, queue| {
            let full_page = unsafe { crate::core_types::FullPage::new_unchecked(page) };
            let full_queue = unsafe { FullPageQueue::new(queue) }.expect("full queue");
            assert!(unsafe { QueuedPage::<FullQueue>::new(full_page, full_queue) }.is_none());
        });
    }
}
