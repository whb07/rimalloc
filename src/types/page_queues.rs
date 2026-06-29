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

    /// # Safety
    /// `page` must be known to be currently linked in `queue`, not marked full,
    /// and have a block size compatible with `queue`.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(
        page: crate::core_types::ActivePage,
        queue: NormalPageQueue,
    ) -> Self {
        Self {
            page: page.page(),
            queue: PageQueue::normal(queue),
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

impl<S> UnqueuedPage<S> {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page.ptr()
    }
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

pub(super) fn __lint_touch() {
    let _ = core::mem::size_of::<HugeQueue>();
    let _ = core::mem::size_of::<PageQueue<HugeQueue>>();
    let _ =
        QueuedPage::<NormalQueue>::queue as fn(&QueuedPage<NormalQueue>) -> &PageQueue<NormalQueue>;
    let _ = QueuedPage::<FullQueue>::queue as fn(&QueuedPage<FullQueue>) -> &PageQueue<FullQueue>;
    let _ = core::mem::size_of::<UnqueuedPage<crate::core_types::Active>>();
    let _ = UnqueuedPage::<crate::core_types::Active>::page
        as fn(&UnqueuedPage<crate::core_types::Active>) -> PagePtr;
}

#[cfg(test)]
mod tests;
