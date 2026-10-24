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
    /// Lend a detached page and its compatible insertion queue together.
    ///
    /// # Safety
    /// The queue/heap conditions of `with_cursor` must hold through `f`.
    /// `page` must be live, owned by this heap, detached from every queue,
    /// and compatible with this queue. Both page counts must permit insertion.
    #[inline(always)]
    pub(crate) unsafe fn with_detached<R>(
        self,
        heap: super::ThreadHeapLocal,
        page: super::ActivePage,
        f: impl for<'scope> FnOnce(DetachedPage<'scope, NormalQueue>) -> R,
    ) -> R {
        self.lend_member(heap, page.page(), |page| f(DetachedPage(page)))
    }

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

/// Queue class used only by traversal that handles both normal and full pages.
pub(crate) enum AnyQueue {}

impl PageQueue<AnyQueue> {
    #[inline(always)]
    pub(crate) fn for_visit(ptr: PageQueuePtr) -> Self {
        Self {
            ptr,
            _kind: PhantomData,
        }
    }
}

/// A short owner phase over one queue's remaining linked chain.
pub(crate) struct QueueCursor<'scope, K> {
    heap: super::ThreadHeapLocal,
    queue: PageQueuePtr,
    next: Option<PagePtr>,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
    _kind: PhantomData<K>,
}

/// Membership lent by a queue cursor, or established at a raw ownership ingress.
#[must_use = "queue membership must be consumed or left linked"]
pub(crate) struct QueuedPage<'scope, K> {
    heap: super::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
    _kind: PhantomData<K>,
}

/// A uniquely detached page retaining its compatible source queue.
#[must_use = "detached pages must be reinserted or transferred to their lifecycle owner"]
pub(crate) struct DetachedPage<'scope, K>(QueuedPage<'scope, K>);

impl<K> PageQueue<K> {
    /// Lend the initialized queue during one local ownership phase.
    ///
    /// # Safety
    /// `heap` must own this live initialized queue and its finite, consistent
    /// linked chain. The caller exclusively owns queue links/counts, the heap
    /// cache/count, and page flags. Unvisited successors must remain live.
    /// A visit may detach or destroy its current page after consuming its
    /// membership. Other relinking, teardown, and incompatible callback reentry
    /// must be excluded. Protocol remote-free atomics remain permitted.
    #[inline(always)]
    pub(crate) unsafe fn with_cursor<R>(
        self,
        heap: super::ThreadHeapLocal,
        f: impl for<'scope> FnOnce(&mut QueueCursor<'scope, K>) -> R,
    ) -> R {
        let next = unsafe { self.ptr.first() };
        f(&mut QueueCursor {
            heap,
            queue: self.ptr,
            next,
            _scope: PhantomData,
            _kind: PhantomData,
        })
    }

    #[inline(always)]
    fn lend_member<R>(
        self,
        heap: super::ThreadHeapLocal,
        page: PagePtr,
        f: impl for<'scope> FnOnce(QueuedPage<'scope, K>) -> R,
    ) -> R {
        f(QueuedPage {
            heap,
            queue: self.ptr,
            page,
            _scope: PhantomData,
            _kind: PhantomData,
        })
    }
}

impl PageQueue<NormalQueue> {
    /// Lend a known linked active page without repeating the queue search.
    ///
    /// # Safety
    /// The queue/heap ownership conditions of `with_cursor` must hold through
    /// `f`. `page` must be linked exactly once here, not full, and compatible
    /// with this queue's block size (including the huge sentinel case).
    #[inline(always)]
    pub(crate) unsafe fn with_member<R>(
        self,
        heap: super::ThreadHeapLocal,
        page: super::ActivePage,
        f: impl for<'scope> FnOnce(QueuedPage<'scope, NormalQueue>) -> R,
    ) -> R {
        self.lend_member(heap, page.page(), f)
    }

    /// Check membership before lending the matching active page.
    ///
    /// # Safety
    /// The heap, queue and active page must remain live and locally owned
    /// under the same mutation/callback conditions as `with_cursor`.
    #[inline(always)]
    pub(crate) unsafe fn try_with_member<R>(
        self,
        heap: super::ThreadHeapLocal,
        page: super::ActivePage,
        f: impl for<'scope> FnOnce(QueuedPage<'scope, NormalQueue>) -> R,
    ) -> Result<R, super::ActivePage> {
        let queue = self.ptr.as_ptr();
        let raw = page.page().as_ptr();
        if unsafe {
            crate::allocator::page::mi_page_queue_contains(queue, raw)
                && !crate::allocator::page::mi_page_is_in_full(raw)
                && (self
                    .ptr
                    .block_size()
                    .is_some_and(|size| page.page().block_size().get() == size.get())
                    || (crate::allocator::page::mi_page_is_huge(raw)
                        && crate::allocator::page::mi_page_queue_is_huge(queue)))
        } {
            Ok(self.lend_member(heap, page.page(), f))
        } else {
            Err(page)
        }
    }
}

impl PageQueue<FullQueue> {
    /// Lend a known linked full page without repeating the queue search.
    ///
    /// # Safety
    /// The queue/heap ownership conditions of `with_cursor` must hold through
    /// `f`. `page` must be linked exactly once in this owning heap's full queue
    /// and retain its full flag.
    #[inline(always)]
    pub(crate) unsafe fn with_member<R>(
        self,
        heap: super::ThreadHeapLocal,
        page: super::FullPage,
        f: impl for<'scope> FnOnce(QueuedPage<'scope, FullQueue>) -> R,
    ) -> R {
        self.lend_member(heap, page.page(), f)
    }

    /// Check membership before lending the matching full page.
    ///
    /// # Safety
    /// The heap, queue and full page must remain live and locally owned
    /// under the same mutation/callback conditions as `with_cursor`.
    #[inline(always)]
    pub(crate) unsafe fn try_with_member<R>(
        self,
        heap: super::ThreadHeapLocal,
        page: super::FullPage,
        f: impl for<'scope> FnOnce(QueuedPage<'scope, FullQueue>) -> R,
    ) -> Result<R, super::FullPage> {
        if unsafe {
            crate::allocator::page::mi_page_queue_contains(self.ptr.as_ptr(), page.page().as_ptr())
                && crate::allocator::page::mi_page_is_in_full(page.page().as_ptr())
        } {
            Ok(self.lend_member(heap, page.page(), f))
        } else {
            Err(page)
        }
    }
}

impl<K> QueueCursor<'_, K> {
    /// Snapshot the successor before lending the current membership. The visit
    /// may consume its page; no access to that page follows the callback.
    #[inline(always)]
    pub(crate) fn visit_next<R>(
        &mut self,
        f: impl for<'visit> FnOnce(QueuedPage<'visit, K>) -> R,
    ) -> Option<R> {
        let current = self.next?;
        self.next = unsafe { current.queue_next() };
        Some(f(QueuedPage {
            heap: self.heap,
            queue: self.queue,
            page: current,
            _scope: PhantomData,
            _kind: PhantomData,
        }))
    }
}

impl<K> QueuedPage<'_, K> {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }

    /// End the membership loan before entering a raw page/lifecycle operation.
    #[inline(always)]
    pub(crate) fn into_raw(self) -> PagePtr {
        self.page
    }
}

impl<'scope, K> QueuedPage<'scope, K> {
    #[inline(always)]
    pub(crate) fn remove(self) -> DetachedPage<'scope, K> {
        unsafe { crate::allocator::page::mi_page_queue_remove_typed(self.queue, self.page) };
        DetachedPage(self)
    }
}

impl<'scope, K> DetachedPage<'scope, K> {
    #[inline(always)]
    pub(crate) fn into_raw(self) -> PagePtr {
        self.0.page
    }
}

impl<'scope> DetachedPage<'scope, NormalQueue> {
    /// Insert only into the compatible queue retained by this ownership value.
    #[inline(always)]
    pub(crate) fn push_front(self) -> QueuedPage<'scope, NormalQueue> {
        unsafe {
            crate::allocator::page::mi_page_queue_push_internal(
                self.0.heap,
                self.0.queue,
                self.0.page,
            );
        }
        self.0
    }

    #[inline(always)]
    pub(crate) fn push_at_end(self) -> QueuedPage<'scope, NormalQueue> {
        unsafe {
            crate::allocator::page::mi_page_queue_push_at_end_internal(
                self.0.heap,
                self.0.queue,
                self.0.page,
            );
        }
        self.0
    }
}

impl QueuedPage<'_, NormalQueue> {
    #[inline(always)]
    pub(crate) fn move_to_front(self) -> Self {
        unsafe {
            crate::allocator::page::mi_page_queue_move_to_front_raw(
                self.heap,
                NormalPageQueue(self.queue),
                self.page,
            );
        }
        self
    }

    #[inline(always)]
    pub(crate) fn into_full(self) -> PagePtr {
        unsafe {
            let full = self
                .heap
                .ptr()
                .page_queue(super::Bin::new_unchecked(crate::types::MI_BIN_FULL as u16));
            crate::allocator::page::mi_page_queue_enqueue_from_raw(
                full, self.queue, true, self.page,
            );
            crate::allocator::page::mi_page_set_in_full(self.page.as_ptr(), true);
        }
        self.page
    }
}

impl QueuedPage<'_, FullQueue> {
    #[inline(always)]
    pub(crate) fn unfull(self) -> PagePtr {
        unsafe {
            let bin = crate::allocator::page::_mi_page_stats_bin_typed(self.page.as_ptr());
            let bin = super::Bin::new_unchecked(bin.as_usize() as u16);
            let to = self.heap.ptr().page_queue(bin);
            crate::allocator::page::mi_page_queue_enqueue_from_raw(to, self.queue, true, self.page);
            crate::allocator::page::mi_page_set_in_full(self.page.as_ptr(), false);
        }
        self.page
    }
}

const _: unsafe fn(PageQueuePtr) -> Option<NormalPageQueue> = NormalPageQueue::new;
const _: unsafe fn(PageQueuePtr) -> Option<HugePageQueue> = HugePageQueue::new;
const _: unsafe fn(PageQueuePtr) -> Option<FullPageQueue> = FullPageQueue::new;

#[cfg(test)]
mod tests;
