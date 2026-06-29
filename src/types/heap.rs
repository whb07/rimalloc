//! DT-10.1: Heap and Thread-Heap wrappers.

use crate::core_types::{HeapPtr, SubprocPtr, ThreadHeapLocal, ThreadHeapPtr, TldPtr};
use crate::types::mi_heap_t;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MainHeap(HeapPtr);

impl MainHeap {
    /// # Safety
    /// `heap` must point to initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(heap: HeapPtr) -> Option<Self> {
        if unsafe { crate::internal::_mi_is_heap_main(heap.as_ptr()) } {
            Some(Self(heap))
        } else {
            None
        }
    }

    /// # Safety
    /// `heap` must point to the process main heap.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(heap: HeapPtr) -> Self {
        Self(heap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> HeapPtr {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct OwnedHeap(HeapPtr);

impl OwnedHeap {
    /// # Safety
    /// `heap` must point to initialized heap metadata owned by the current
    /// delete or destroy path.
    #[inline(always)]
    pub(crate) unsafe fn new(heap: HeapPtr) -> Option<Self> {
        if unsafe { crate::internal::_mi_is_heap_main(heap.as_ptr()) } {
            None
        } else {
            Some(Self(heap))
        }
    }

    /// # Safety
    /// `heap` must point to a non-main heap owned by the current deletion or
    /// destruction path.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(heap: HeapPtr) -> Self {
        Self(heap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> HeapPtr {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct BorrowedHeap(HeapPtr);

impl BorrowedHeap {
    /// # Safety
    /// `heap` must be null or point to a live heap borrowed from an allocation
    /// API boundary.
    #[inline(always)]
    pub(crate) unsafe fn from_allocation_raw(heap: *mut mi_heap_t) -> Option<Self> {
        HeapPtr::new(heap).map(Self)
    }

    /// # Safety
    /// `heap` must be null or point to a live heap returned by a raw heap lookup.
    #[inline(always)]
    pub(crate) unsafe fn from_lookup_raw(heap: *mut mi_heap_t) -> Option<Self> {
        HeapPtr::new(heap).map(Self)
    }

    /// # Safety
    /// `heap` must point to a live heap borrowed for the duration of the current
    /// allocator operation.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(heap: HeapPtr) -> Self {
        Self(heap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> HeapPtr {
        self.0
    }
}

const _: unsafe fn(HeapPtr) -> Option<MainHeap> = MainHeap::new;
const _: unsafe fn(HeapPtr) -> MainHeap = MainHeap::new_unchecked;
const _: fn(MainHeap) -> HeapPtr = MainHeap::ptr;
const _: unsafe fn(HeapPtr) -> Option<OwnedHeap> = OwnedHeap::new;
const _: unsafe fn(HeapPtr) -> OwnedHeap = OwnedHeap::new_unchecked;
const _: fn(OwnedHeap) -> HeapPtr = OwnedHeap::ptr;
const _: unsafe fn(*mut mi_heap_t) -> Option<BorrowedHeap> = BorrowedHeap::from_allocation_raw;
const _: unsafe fn(*mut mi_heap_t) -> Option<BorrowedHeap> = BorrowedHeap::from_lookup_raw;
const _: unsafe fn(HeapPtr) -> BorrowedHeap = BorrowedHeap::new_unchecked;
const _: fn(BorrowedHeap) -> HeapPtr = BorrowedHeap::ptr;
const _: unsafe fn(ThreadHeapPtr) -> Option<ThreadHeapBorrowed> = ThreadHeapBorrowed::new;
const _: unsafe fn(ThreadHeapPtr) -> ThreadHeapBorrowed = ThreadHeapBorrowed::new_unchecked;
const _: fn(ThreadHeapBorrowed) -> ThreadHeapPtr = ThreadHeapBorrowed::ptr;
const _: unsafe fn(ThreadHeapPtr) -> ThreadHeapState = ThreadHeapPtr::state;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct ThreadHeapBorrowed(ThreadHeapPtr);

pub(crate) enum ThreadHeapState {
    Uninitialized(ThreadHeapPtr),
    Initialized(ThreadHeapBorrowed),
    Main(ThreadHeapBorrowed),
    Deleted(ThreadHeapPtr),
}

impl ThreadHeapBorrowed {
    /// # Safety
    /// `theap` must point to thread-heap metadata owned by this allocator.
    #[inline(always)]
    pub(crate) unsafe fn new(theap: ThreadHeapPtr) -> Option<Self> {
        if unsafe { theap.heap() }.is_some() {
            Some(Self(theap))
        } else {
            None
        }
    }

    /// # Safety
    /// `theap` must point to initialized thread-heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self {
        Self(theap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ThreadHeapPtr {
        self.0
    }

    /// # Safety
    /// The caller must pass the current thread id for the executing thread.
    #[inline(always)]
    pub(crate) unsafe fn try_local(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<ThreadHeapLocal, Self> {
        match unsafe { self.0.try_local(current) } {
            Ok(local) => Ok(local),
            Err(_) => Err(self),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MainSubproc(SubprocPtr);

impl MainSubproc {
    /// # Safety
    /// `subproc` must point to the process-global main subproc metadata.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(subproc: SubprocPtr) -> Self {
        Self(subproc)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> SubprocPtr {
        self.0
    }
}

const _: unsafe fn(SubprocPtr) -> MainSubproc = MainSubproc::new_unchecked;
const _: fn(MainSubproc) -> SubprocPtr = MainSubproc::ptr;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct BorrowedSubproc(SubprocPtr);

impl BorrowedSubproc {
    /// # Safety
    /// `subproc` must point to live subproc metadata borrowed for the current
    /// allocator operation.
    #[inline(always)]
    pub(crate) unsafe fn new(subproc: SubprocPtr) -> Self {
        Self(subproc)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> SubprocPtr {
        self.0
    }
}

const _: unsafe fn(SubprocPtr) -> BorrowedSubproc = BorrowedSubproc::new;
const _: fn(BorrowedSubproc) -> SubprocPtr = BorrowedSubproc::ptr;

impl ThreadHeapPtr {
    /// # Safety
    /// `self` must point to thread-heap metadata allocated by this allocator.
    #[inline]
    pub(crate) unsafe fn state(self) -> ThreadHeapState {
        let Some(heap) = (unsafe { self.heap() }) else {
            if unsafe { self.tld() }.is_none() {
                return ThreadHeapState::Deleted(self);
            }
            return ThreadHeapState::Uninitialized(self);
        };

        let borrowed = unsafe { ThreadHeapBorrowed::new_unchecked(self) };
        if unsafe { crate::internal::_mi_is_heap_main(heap.as_ptr()) } {
            ThreadHeapState::Main(borrowed)
        } else {
            ThreadHeapState::Initialized(borrowed)
        }
    }

    /// If this thread-heap belongs to `current` (i.e. its `tld.thread_id` matches,
    /// or the tld is the main-thread tld with `thread_id == 0`), return it as a
    /// `ThreadHeapLocal` witness; otherwise return `Err(self)`.
    ///
    /// # Safety
    ///
    /// `self` must point to a valid `mi_theap_t`. The read of
    /// `(*(*theap).tld).thread_id` is a plain (non-atomic) field access; this is
    /// sound because the invariant holds:
    /// - `(*theap).tld` is set once during thread-heap initialization and never
    ///   mutated for the lifetime of the theap, so the pointer deref is race-free.
    /// - `(*tld).thread_id` is written once when the tld is created and never
    ///   subsequently modified, so a plain read is sufficient. (This matches the
    ///   existing raw pattern at `src/alloc.rs` and `src/arena.rs` which read
    ///   `(*(*theap).tld).thread_id` without atomics.)
    ///
    /// Callers must not invoke this on a theap whose tld is concurrently being
    /// torn down.
    #[inline]
    pub(crate) unsafe fn try_local(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<crate::core_types::ThreadHeapLocal, ThreadHeapPtr> {
        match unsafe { self.state() } {
            ThreadHeapState::Initialized(_) | ThreadHeapState::Main(_) => {}
            ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return Err(self),
        }

        let theap = self.as_ptr();
        // SAFETY: see the function-level safety doc — `tld` and `thread_id` are
        // stable after init, so this plain read is race-free.
        let tld_thread_id = unsafe { (*(*theap).tld).thread_id };
        if tld_thread_id == 0 || tld_thread_id == current.get() {
            Ok(unsafe { crate::core_types::ThreadHeapLocal::new_unchecked(self) })
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

pub(super) fn __lint_touch() {
    let theap = unsafe { ThreadHeapPtr::new_unchecked(core::ptr::dangling_mut()) };
    if let ThreadHeapState::Uninitialized(ptr) = ThreadHeapState::Uninitialized(theap) {
        let _ = ptr.as_ptr();
    }
    if let ThreadHeapState::Deleted(ptr) = ThreadHeapState::Deleted(theap) {
        let _ = ptr.as_ptr();
    }
}

#[cfg(test)]
mod tests;
