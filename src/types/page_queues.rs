//! DT-10: Page queue types.

use super::page_states::Page;
use super::ptr::{PagePtr, PageQueuePtr};
use core::marker::PhantomData;
use core::mem::size_of;

#[derive(Debug)]
pub(crate) enum NormalQueue {}
#[derive(Debug)]
pub(crate) enum HugeQueue {}
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
        // SAFETY: callers provide initialized
        // queue metadata; this checked constructor only reads the sentinel
        // block-size field before packaging a normal-queue witness.
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
        // SAFETY: callers provide initialized
        // queue metadata; this checked constructor only reads the sentinel
        // block-size field before packaging a huge-queue witness.
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
        // SAFETY: callers provide initialized
        // queue metadata; this checked constructor only reads the sentinel
        // block-size field before packaging a full-queue witness.
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

    #[inline(always)]
    pub(crate) fn queue(&self) -> &PageQueue<K> {
        &self.queue
    }
}

impl QueuedPage<NormalQueue> {
    #[inline(always)]
    pub(crate) fn normal_queue(&self) -> NormalPageQueue {
        NormalPageQueue(self.queue.ptr)
    }

    #[inline(always)]
    pub(crate) fn into_active(self) -> crate::core_types::ActivePage {
        // SAFETY: forgetting the more specific
        // normal-queue membership proof preserves the page's active state.
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
        // SAFETY: callers provide page and
        // queue metadata from the same initialized heap; this constructor
        // delegates membership, full-state, block-size, and huge-sentinel
        // checks to the existing C-shaped page helpers before accepting the
        // queued-normal witness.
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
    pub(in crate::types) unsafe fn new_unchecked(
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
        // SAFETY: queue mutation
        // completed immediately before this consuming transition.
        unsafe { Self::new_unchecked(page, queue) }
    }

    #[inline(always)]
    pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage {
        // SAFETY: `self` proves that the page is
        // linked in `self.queue`; removal delegates list mutation to the page
        // helper and then restores the active-page witness for the same page.
        unsafe {
            crate::allocator::page::mi_page_queue_remove_typed(self.queue.ptr, self.page);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn into_full(self, to: FullPageQueue) -> crate::core_types::FullPage {
        // SAFETY: `self` proves a queued normal
        // page and `to` proves the full queue; the helper performs the raw list
        // move, after which the in-full bit is set before returning a FullPage.
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
    /// # Safety
    /// `page` and `queue` must belong to the same initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn try_queue_full(
        page: crate::core_types::FullPage,
        queue: FullPageQueue,
    ) -> Result<Self, crate::core_types::FullPage> {
        // SAFETY: callers provide page and
        // queue metadata from the same initialized heap; this constructor
        // accepts the full-queue witness only when the page is linked in the
        // queue and still marked full by the existing page helpers.
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
    pub(crate) unsafe fn unfull(self, to: NormalPageQueue) -> crate::core_types::ActivePage {
        // SAFETY: `self` proves a queued full
        // page and `to` proves a normal queue; the helper performs the raw list
        // move, after which the in-full bit is cleared before returning
        // ActivePage.
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
    pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage {
        // SAFETY: `self` proves that the full
        // page is linked in `self.queue`; removal delegates list mutation to
        // the page helper and returns the page to the active witness domain.
        unsafe {
            crate::allocator::page::mi_page_queue_remove_typed(self.queue.ptr, self.page);
            crate::core_types::ActivePage::new_unchecked(self.page)
        }
    }
}

/// Witness that a page is definitively *not* in any queue.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct UnqueuedPage<S> {
    page: Page<S>,
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
    // SAFETY: the AllFreePage and
    // ThreadHeapLocal witnesses prove this page is owned by the local heap and
    // all blocks are free before arena free transitions it to DestroyedPage.
    unsafe { crate::core_types::page_states::DestroyedPage::new_unchecked(page.page()) }
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
