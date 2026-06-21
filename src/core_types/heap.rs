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
mod tests {
    use super::*;
    use crate::heap::mi_heap_new;
    use crate::init::mi_heap_main;
    use crate::mimalloc::mi_heap_delete;
    use crate::types::{mi_heap_t, mi_theap_t, mi_tld_t};
    use core::mem::MaybeUninit;

    fn with_heap_ptr(f: impl FnOnce(HeapPtr)) {
        let mut heap = MaybeUninit::<mi_heap_t>::uninit();
        let ptr = unsafe { HeapPtr::new_unchecked(heap.as_mut_ptr()) };
        f(ptr);
    }

    fn with_thread_heap_ptr(
        heap: Option<HeapPtr>,
        tld: Option<TldPtr>,
        f: impl FnOnce(ThreadHeapPtr),
    ) {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let raw = theap.as_mut_ptr();
        unsafe {
            (*raw).heap = heap.map(HeapPtr::as_ptr).unwrap_or(core::ptr::null_mut());
            (*raw).tld = tld.map(TldPtr::as_ptr).unwrap_or(core::ptr::null_mut());
            f(ThreadHeapPtr::new_unchecked(raw));
        }
    }

    #[test]
    fn heap_witnesses_preserve_heap_pointer() {
        with_heap_ptr(|ptr| {
            let main = unsafe { MainHeap::new_unchecked(ptr) };
            let owned = unsafe { OwnedHeap::new_unchecked(ptr) };
            let borrowed = unsafe { BorrowedHeap::new_unchecked(ptr) };

            assert_eq!(main.ptr().as_ptr(), ptr.as_ptr());
            assert_eq!(owned.ptr().as_ptr(), ptr.as_ptr());
            assert_eq!(borrowed.ptr().as_ptr(), ptr.as_ptr());
        });
    }

    #[test]
    fn main_heap_checked_constructor_accepts_only_main_heap() {
        let main_raw = unsafe { mi_heap_main() };
        let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
        let main = unsafe { MainHeap::new(main_ptr) }.expect("main heap should be accepted");
        assert_eq!(main.ptr().as_ptr(), main_raw);

        let owned_raw = unsafe { mi_heap_new() };
        assert!(!owned_raw.is_null());
        let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
        assert!(
            unsafe { MainHeap::new(owned_ptr) }.is_none(),
            "non-main heaps must not produce MainHeap witnesses"
        );
        unsafe { mi_heap_delete(owned_raw) };
    }

    #[test]
    fn owned_heap_checked_constructor_rejects_main_heap() {
        let main_raw = unsafe { mi_heap_main() };
        let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
        assert!(
            unsafe { OwnedHeap::new(main_ptr) }.is_none(),
            "main heap must not produce an OwnedHeap witness"
        );

        let owned_raw = unsafe { mi_heap_new() };
        assert!(!owned_raw.is_null());
        let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
        let owned = unsafe { OwnedHeap::new(owned_ptr) }.expect("non-main heap should be accepted");
        assert_eq!(owned.ptr().as_ptr(), owned_raw);
        unsafe { mi_heap_delete(owned_raw) };
    }

    #[test]
    fn main_heap_cannot_create_owned_delete_witness() {
        let main_raw = unsafe { mi_heap_main() };
        let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };

        assert!(unsafe { MainHeap::new(main_ptr) }.is_some());
        assert!(
            unsafe { OwnedHeap::new(main_ptr) }.is_none(),
            "main heap must remain unable to enter OwnedHeap deletion helpers"
        );
    }

    #[test]
    fn borrowed_heap_raw_constructors_reject_null_and_preserve_heap() {
        assert!(
            unsafe { BorrowedHeap::from_allocation_raw(core::ptr::null_mut()) }.is_none(),
            "allocation raw constructor should reject null heaps"
        );
        assert!(
            unsafe { BorrowedHeap::from_lookup_raw(core::ptr::null_mut()) }.is_none(),
            "lookup raw constructor should preserve failed heap lookups"
        );

        let main_raw = unsafe { mi_heap_main() };
        let main = unsafe { BorrowedHeap::from_allocation_raw(main_raw) }
            .expect("main heap can be borrowed for allocation");
        assert_eq!(main.ptr().as_ptr(), main_raw);

        let owned_raw = unsafe { mi_heap_new() };
        assert!(!owned_raw.is_null());
        let owned = unsafe { BorrowedHeap::from_lookup_raw(owned_raw) }
            .expect("looked-up non-main heap can be borrowed");
        assert_eq!(owned.ptr().as_ptr(), owned_raw);
        unsafe { mi_heap_delete(owned_raw) };
    }

    #[test]
    fn thread_heap_borrowed_accepts_only_initialized_theaps() {
        with_thread_heap_ptr(None, None, |theap| {
            assert!(
                unsafe { ThreadHeapBorrowed::new(theap) }.is_none(),
                "cleared heap field should not produce an initialized thread-heap witness"
            );
        });

        let owned_raw = unsafe { mi_heap_new() };
        assert!(!owned_raw.is_null());
        let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
        with_thread_heap_ptr(Some(owned_ptr), None, |theap| {
            let borrowed = unsafe { ThreadHeapBorrowed::new(theap) }
                .expect("non-null heap field should produce borrowed thread-heap witness");
            assert_eq!(borrowed.ptr().as_ptr(), theap.as_ptr());
        });
        unsafe { mi_heap_delete(owned_raw) };
    }

    #[test]
    fn thread_heap_state_classifies_main_initialized_uninitialized_and_deleted() {
        let main_raw = unsafe { mi_heap_main() };
        let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
        with_thread_heap_ptr(Some(main_ptr), None, |theap| {
            match unsafe { theap.state() } {
                ThreadHeapState::Main(main) => assert_eq!(main.ptr().as_ptr(), theap.as_ptr()),
                _ => panic!("main heap thread-heap should classify as main"),
            }
        });

        let owned_raw = unsafe { mi_heap_new() };
        assert!(!owned_raw.is_null());
        let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
        with_thread_heap_ptr(Some(owned_ptr), None, |theap| {
            match unsafe { theap.state() } {
                ThreadHeapState::Initialized(initialized) => {
                    assert_eq!(initialized.ptr().as_ptr(), theap.as_ptr())
                }
                _ => panic!("non-main initialized thread-heap should classify as initialized"),
            }
        });
        unsafe { mi_heap_delete(owned_raw) };

        let tld =
            unsafe { TldPtr::new_unchecked(core::ptr::NonNull::<mi_tld_t>::dangling().as_ptr()) };
        with_thread_heap_ptr(None, Some(tld), |theap| match unsafe { theap.state() } {
            ThreadHeapState::Uninitialized(raw) => assert_eq!(raw.as_ptr(), theap.as_ptr()),
            _ => panic!("null heap with tld should classify as uninitialized"),
        });

        with_thread_heap_ptr(None, None, |theap| match unsafe { theap.state() } {
            ThreadHeapState::Deleted(raw) => assert_eq!(raw.as_ptr(), theap.as_ptr()),
            _ => panic!("cleared heap and tld should classify as deleted"),
        });
    }

    #[test]
    fn borrowed_thread_heap_converts_to_local_only_for_matching_thread() {
        let owned_raw = unsafe { mi_heap_new() };
        assert!(!owned_raw.is_null());
        let mut tld = MaybeUninit::<mi_tld_t>::zeroed();
        let tld_raw = tld.as_mut_ptr();
        unsafe {
            (*tld_raw).thread_id = 16;
        }

        let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
        let tld_ptr = unsafe { TldPtr::new_unchecked(tld_raw) };
        with_thread_heap_ptr(Some(owned_ptr), Some(tld_ptr), |theap| {
            let borrowed =
                unsafe { ThreadHeapBorrowed::new(theap) }.expect("thread heap is initialized");
            let current = crate::core_types::ThreadId::new(16).expect("valid thread id");
            let local = match unsafe { borrowed.try_local(current) } {
                Ok(local) => local,
                Err(_) => panic!("matching thread id should produce ThreadHeapLocal"),
            };
            assert_eq!(local.ptr().as_ptr(), theap.as_ptr());
            assert_eq!(local.borrowed().ptr().as_ptr(), theap.as_ptr());

            let other = crate::core_types::ThreadId::new(24).expect("valid thread id");
            assert!(
                unsafe { borrowed.try_local(other) }.is_err(),
                "different thread id should not produce ThreadHeapLocal"
            );
        });
        unsafe { mi_heap_delete(owned_raw) };
    }

    #[test]
    fn deleted_thread_heap_cannot_be_promoted_to_local() {
        let current = crate::core_types::ThreadId::new(16).expect("valid thread id");
        with_thread_heap_ptr(None, None, |theap| {
            match unsafe { theap.state() } {
                ThreadHeapState::Deleted(raw) => assert_eq!(raw.as_ptr(), theap.as_ptr()),
                _ => panic!("cleared heap and tld should classify as deleted"),
            }
            assert!(
                unsafe { ThreadHeapBorrowed::new(theap) }.is_none(),
                "deleted thread heaps should not produce borrowed witnesses"
            );
            assert!(
                unsafe { theap.try_local(current) }.is_err(),
                "deleted thread heaps should be rejected before ThreadHeapLocal construction"
            );
        });
    }
}
