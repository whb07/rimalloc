//! DT-10.1: Heap and Thread-Heap wrappers.

use crate::core_types::{HeapPtr, SubprocPtr, ThreadHeapPtr, TldPtr};

#[repr(transparent)]
pub(crate) struct MainHeap(pub(crate) HeapPtr);

#[repr(transparent)]
pub(crate) struct OwnedHeap(pub(crate) HeapPtr);

#[repr(transparent)]
pub(crate) struct BorrowedHeap(pub(crate) HeapPtr);

#[repr(transparent)]
pub(crate) struct ThreadHeapBorrowed(pub(crate) ThreadHeapPtr);

pub(crate) enum ThreadHeapState {
    Uninitialized,
    Initialized,
    Main,
    Deleted,
}

#[repr(transparent)]
pub(crate) struct MainSubproc(pub(crate) SubprocPtr);

#[repr(transparent)]
pub(crate) struct BorrowedSubproc(pub(crate) SubprocPtr);

impl ThreadHeapPtr {
    /// If this thread-heap belongs to `current` (i.e. its `tld.thread_id` matches,
    /// or the tld is the main-thread tld with `thread_id == 0`), return it as a
    /// `ThreadHeapLocal` witness; otherwise return `Err(self)`.
    ///
    /// # Safety
    /// `self` must point to a valid `mi_theap_t`.
    #[inline]
    pub(crate) unsafe fn try_local(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<crate::core_types::ThreadHeapLocal, ThreadHeapPtr> {
        let theap = self.as_ptr();
        // SAFETY: caller guarantees `theap` is valid; `tld` is stable after init.
        let tld_thread_id = unsafe { (*(*theap).tld).thread_id };
        if tld_thread_id == 0 || tld_thread_id == current.get() {
            Ok(crate::core_types::ThreadHeapLocal(self))
        } else {
            Err(self)
        }
    }

    /// The owning heap of this thread-heap, loaded with acquire ordering (it is
    /// concurrently swapped during heap transfer). `None` if the heap field is
    /// null (uninitialized / transferred-away theap).
    ///
    /// # Safety
    /// `self` must point to a valid `mi_theap_t`.
    #[inline]
    pub(crate) unsafe fn heap(self) -> Option<HeapPtr> {
        let raw = unsafe { crate::internal::_mi_theap_heap(self.as_ptr()) };
        if raw.is_null() {
            None
        } else {
            // SAFETY: `raw` is non-null and a valid heap pointer per the acquire load.
            Some(unsafe { HeapPtr::new_unchecked(raw) })
        }
    }

    /// The thread-local data (`tld`) of this thread-heap. `None` if the tld
    /// field is null (should not happen for an initialized theap).
    ///
    /// # Safety
    /// `self` must point to a valid `mi_theap_t`.
    #[inline]
    pub(crate) unsafe fn tld(self) -> Option<TldPtr> {
        let raw = unsafe { (*self.as_ptr()).tld };
        if raw.is_null() {
            None
        } else {
            // SAFETY: `raw` is non-null and a valid tld pointer.
            Some(unsafe { TldPtr::new_unchecked(raw) })
        }
    }
}
