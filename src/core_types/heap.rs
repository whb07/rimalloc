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
    // Basic structural stubs mandated by DT-10
    pub(crate) fn try_local(
        self,
        _current: crate::core_types::ThreadId,
    ) -> Result<crate::core_types::ThreadHeapLocal, ThreadHeapPtr> {
        unimplemented!()
    }

    pub(crate) fn heap(self) -> Option<HeapPtr> {
        unimplemented!()
    }

    pub(crate) fn tld(self) -> Option<TldPtr> {
        unimplemented!()
    }
}
