//! DT-10: Page queue types.

use super::ptr::{PagePtr, PageQueuePtr};
use core::marker::PhantomData;
use core::mem::size_of;

#[derive(Debug)]
pub(crate) enum NormalQueue {}
#[derive(Debug)]
pub(crate) enum FullQueue {}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
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
    /// `ptr` must point to an initialized queue accepted by normal-page
    /// traversal, proven by heap queue lookup or initialization. This includes
    /// the huge sentinel queue when the traversal handles its huge-page branch,
    /// but excludes the full queue.
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
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct QueuedPage<K> {
    page: PagePtr,
    queue: PageQueue<K>,
}

impl<K> QueuedPage<K> {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }
}

impl QueuedPage<NormalQueue> {
    #[inline(always)]
    pub(crate) fn normal_queue(&self) -> NormalPageQueue {
        NormalPageQueue(self.queue.ptr)
    }

    #[inline(always)]
    pub(crate) fn into_active(self) -> crate::core_types::ActivePage {
        unsafe { crate::core_types::ActivePage::new_unchecked(self.page) }
    }

    /// # Safety
    /// `page` and `queue` must belong to the same initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn try_queue(
        page: crate::core_types::ActivePage,
        queue: NormalPageQueue,
    ) -> Result<Self, crate::core_types::ActivePage> {
        let queue_ptr = queue.ptr().as_ptr();
        let page_ptr = page.page().as_ptr();
        if unsafe {
            crate::allocator::page::mi_page_queue_contains(queue_ptr, page_ptr)
                && !crate::allocator::page::mi_page_is_in_full(page_ptr)
                && (queue
                    .ptr()
                    .block_size()
                    .is_some_and(|block_size| page.page().block_size().get() == block_size.get())
                    || (crate::allocator::page::mi_page_is_huge(page_ptr)
                        && crate::allocator::page::mi_page_queue_is_huge(queue_ptr)))
        } {
            Ok(Self {
                page: page.page(),
                queue: PageQueue::normal(queue),
            })
        } else {
            Err(page)
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

    /// # Safety
    /// `page` must have just been linked into `queue` by the caller.
    #[inline(always)]
    pub(crate) unsafe fn from_enqueued(
        page: crate::core_types::ActivePage,
        queue: NormalPageQueue,
    ) -> Self {
        unsafe { Self::new_unchecked(page, queue) }
    }

    #[inline(always)]
    /// # Safety
    /// The page and normal queue must remain live with `self.page` still linked
    /// in `self.queue`. The caller must exclusively own unlinking and stabilize
    /// all adjacent page links and queue metadata for the operation.
    pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage {
        unsafe {
            crate::allocator::page::mi_page_queue_remove_typed(self.queue.ptr, self.page);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }

    #[inline(always)]
    /// # Safety
    /// `self` must remain linked in its live normal queue and `to` must be the
    /// owning heap's live full queue. The caller must exclusively own the move,
    /// adjacent links, queue metadata, and in-full flag transition.
    pub(crate) unsafe fn into_full(self, to: FullPageQueue) -> crate::core_types::FullPage {
        unsafe {
            crate::allocator::page::mi_page_queue_enqueue_from_raw(
                to.ptr(),
                self.queue.ptr,
                true,
                self.page,
            );
            crate::allocator::page::mi_page_set_in_full(self.page.as_ptr(), true);
            crate::core_types::FullPage::new_unchecked(self.page)
        }
    }
}

impl QueuedPage<FullQueue> {
    /// Construct the exact full-queue membership capability without rechecking metadata.
    ///
    /// # Safety
    /// `page` must be a live initialized page currently linked in `queue`, marked full, and owned
    /// by the same live initialized heap as that queue. The page, adjacent links, queue metadata,
    /// and owning heap association must remain stable, and the caller must exclude local relink or
    /// teardown while the returned capability is used. Protocol atomic remote-free publication is
    /// permitted.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(
        page: crate::core_types::FullPage,
        queue: FullPageQueue,
    ) -> Self {
        Self {
            page: page.page(),
            queue: PageQueue::full(queue),
        }
    }

    /// # Safety
    /// `page` and `queue` must belong to the same initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn try_queue_full(
        page: crate::core_types::FullPage,
        queue: FullPageQueue,
    ) -> Result<Self, crate::core_types::FullPage> {
        if unsafe {
            crate::allocator::page::mi_page_queue_contains(
                queue.ptr().as_ptr(),
                page.page().as_ptr(),
            ) && crate::allocator::page::mi_page_is_in_full(page.page().as_ptr())
        } {
            Ok(Self {
                page: page.page(),
                queue: PageQueue::full(queue),
            })
        } else {
            Err(page)
        }
    }

    #[inline(always)]
    /// # Safety
    /// `self` must remain linked in its live full queue and `to` must be the
    /// owning heap's compatible live normal queue. The caller must exclusively
    /// own the move, adjacent links, queue metadata, and in-full flag transition.
    pub(crate) unsafe fn unfull(self, to: NormalPageQueue) -> crate::core_types::ActivePage {
        unsafe {
            crate::allocator::page::mi_page_queue_enqueue_from_raw(
                to.ptr(),
                self.queue.ptr,
                true,
                self.page,
            );
            crate::allocator::page::mi_page_set_in_full(self.page.as_ptr(), false);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }

    #[inline(always)]
    /// # Safety
    /// The page and full queue must remain live with `self.page` still linked
    /// in `self.queue`. The caller must exclusively own unlinking and stabilize
    /// all adjacent page links and queue metadata for the operation.
    pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage {
        unsafe {
            crate::allocator::page::mi_page_queue_remove_typed(self.queue.ptr, self.page);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }
}

const _: unsafe fn(PageQueuePtr) -> Option<NormalPageQueue> = NormalPageQueue::new;
const _: unsafe fn(PageQueuePtr) -> Option<HugePageQueue> = HugePageQueue::new;
const _: unsafe fn(PageQueuePtr) -> Option<FullPageQueue> = FullPageQueue::new;
const _: unsafe fn(
    crate::core_types::ActivePage,
    NormalPageQueue,
) -> Result<QueuedPage<NormalQueue>, crate::core_types::ActivePage> =
    QueuedPage::<NormalQueue>::try_queue;
const _: unsafe fn(
    crate::core_types::FullPage,
    FullPageQueue,
) -> Result<QueuedPage<FullQueue>, crate::core_types::FullPage> =
    QueuedPage::<FullQueue>::try_queue_full;
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
mod tests;
