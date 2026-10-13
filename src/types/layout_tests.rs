use super::*;
use core::mem::{align_of, size_of};
use core::num::{NonZeroU32, NonZeroUsize};
use core::ptr::NonNull;

use crate::types::{
    mi_arena_t, mi_block_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_subproc_t, mi_theap_t,
    mi_tld_t,
};

macro_rules! assert_layout {
    ($wrapper:ty, $raw:ty) => {
        assert_eq!(
            size_of::<$wrapper>(),
            size_of::<$raw>(),
            "size mismatch for {}",
            stringify!($wrapper)
        );
        assert_eq!(
            align_of::<$wrapper>(),
            align_of::<$raw>(),
            "alignment mismatch for {}",
            stringify!($wrapper)
        );
    };
}

#[test]
fn pointer_wrappers_match_nonnull_layout() {
    assert_layout!(PagePtr, NonNull<mi_page_t>);
    assert_layout!(BlockPtr, NonNull<mi_block_t>);
    assert_layout!(HeapPtr, NonNull<mi_heap_t>);
    assert_layout!(ThreadHeapPtr, NonNull<mi_theap_t>);
    assert_layout!(TldPtr, NonNull<mi_tld_t>);
    assert_layout!(ArenaPtr, NonNull<mi_arena_t>);
    assert_layout!(SubprocPtr, NonNull<mi_subproc_t>);
    assert_layout!(PageQueuePtr, NonNull<mi_page_queue_t>);
    assert_layout!(UserPtr, NonNull<core::ffi::c_void>);
    assert_layout!(InteriorUserPtr, NonNull<core::ffi::c_void>);
    assert_layout!(ptr::PageStart, NonNull<u8>);
    assert_layout!(SliceStart, NonNull<u8>);
    assert_layout!(OsMemPtr, NonNull<core::ffi::c_void>);
    assert_layout!(PageConstPtr, NonNull<mi_page_t>);
    assert_layout!(MainSubproc, SubprocPtr);
    assert_layout!(BorrowedSubproc, SubprocPtr);
}

#[test]
fn optional_pointer_wrappers_are_pointer_sized() {
    assert_eq!(size_of::<Option<PagePtr>>(), size_of::<*mut mi_page_t>());
    assert_eq!(size_of::<Option<BlockPtr>>(), size_of::<*mut mi_block_t>());
    assert_eq!(size_of::<Option<HeapPtr>>(), size_of::<*mut mi_heap_t>());
    assert_eq!(
        size_of::<Option<ThreadHeapPtr>>(),
        size_of::<*mut mi_theap_t>()
    );
    assert_eq!(size_of::<Option<TldPtr>>(), size_of::<*mut mi_tld_t>());
    assert_eq!(size_of::<Option<ArenaPtr>>(), size_of::<*mut mi_arena_t>());
    assert_eq!(
        size_of::<Option<SubprocPtr>>(),
        size_of::<*mut mi_subproc_t>()
    );
    assert_eq!(
        size_of::<Option<PageQueuePtr>>(),
        size_of::<*mut mi_page_queue_t>()
    );
    assert_eq!(
        size_of::<Option<UserPtr>>(),
        size_of::<*mut core::ffi::c_void>()
    );
    assert_eq!(
        size_of::<Option<InteriorUserPtr>>(),
        size_of::<*mut core::ffi::c_void>()
    );
    assert_eq!(size_of::<Option<ptr::PageStart>>(), size_of::<*mut u8>());
    assert_eq!(size_of::<Option<SliceStart>>(), size_of::<*mut u8>());
    assert_eq!(
        size_of::<Option<OsMemPtr>>(),
        size_of::<*mut core::ffi::c_void>()
    );
    assert_eq!(
        size_of::<Option<PageConstPtr>>(),
        size_of::<*const mi_page_t>()
    );
}

#[test]
fn scalar_wrappers_match_raw_layout() {
    assert_layout!(RequestedSize, usize);
    assert_layout!(EffectiveSize, NonZeroUsize);
    assert_layout!(PaddedSize, NonZeroUsize);
    assert_layout!(UsableSize, usize);
    assert_layout!(UsableOut, *mut usize);
    assert_layout!(BlockSize, NonZeroUsize);
    assert_layout!(Alignment, NonZeroUsize);
    assert_layout!(AlignmentOffset, usize);
    assert_layout!(PageQueueCount, usize);
    assert_layout!(PageReclaimOnFree, core::ffi::c_long);
    assert_layout!(PageReclaimLimit, core::ffi::c_long);
    assert_layout!(UsedBlockCount, u16);
    assert_layout!(BatchLen, core::num::NonZeroU16);
    assert_layout!(PageCapacity, u16);
    assert_layout!(ReservedBlockCount, u16);
    assert_layout!(RetireExpire, u8);
    assert_layout!(NumaNode, i32);
    assert_layout!(ThreadSeq, usize);
    assert_layout!(HeapSeq, usize);
    assert_layout!(SubprocSeq, usize);
    assert_layout!(Heartbeat, u64);
    assert_layout!(OsPageSize, NonZeroUsize);
    assert_layout!(Errno, core::ffi::c_int);
    assert_layout!(PageMapIndex, usize);
    #[cfg(target_pointer_width = "32")]
    assert_layout!(PageMapFlatIndex, usize);
    assert_layout!(PageMapSubIndex, usize);
    assert_layout!(MaybeAlignment, usize);
    assert_layout!(Bin, u16);
    assert_layout!(StatsBin, u16);
    assert_layout!(DirectBin, u16);
    assert_layout!(SliceIndex, usize);
    assert_layout!(SliceCount, NonZeroU32);
    assert_layout!(ArenaIndex, u16);
    assert_layout!(GoodSize, usize);
    assert_layout!(page::PageFlags, usize);
    assert_layout!(page::ThreadId, NonZeroUsize);
    assert_layout!(PageThreadId, usize);
    assert_layout!(page::XThreadDelta, usize);
    assert_layout!(validation::BlockOffset, usize);
    assert_layout!(layout::MaybeHugeAlignment, usize);
    assert_layout!(memid::MemKindCode, i32);
    assert_layout!(memid::CommittedBytes, usize);
    assert_layout!(memid::ReservedBytes, usize);
    assert_layout!(memid::InitiallyCommitted, bool);
    assert_layout!(memid::InitiallyZero, bool);
    assert_layout!(memid::Pinned, bool);
    assert_layout!(memid::BlockIndex, u32);
    assert_layout!(arena::SliceRange, (SliceIndex, SliceCount));
}

#[test]
fn witness_wrappers_match_pointer_layout() {
    assert_layout!(validation_extra::AllocatedBlock, BlockPtr);
    assert_layout!(FreeBlock, BlockPtr);
    assert_layout!(GuardedBlock, BlockPtr);
    assert_layout!(page_states::ActivePage, PagePtr);
    assert_layout!(page_states::FullPage, PagePtr);
    assert_layout!(page_states::RetiredPage, PagePtr);
    assert_layout!(page_states::AvailablePage, PagePtr);
    assert_layout!(page_states::LocalPage, PagePtr);
    assert_layout!(page_states::ForeignPage, PagePtr);
    assert_layout!(page_states::AbandonedPageType, PagePtr);
    assert_layout!(page_states::AbandonedMappedPageType, PagePtr);
    assert_layout!(OwnedAbandonedPage, PagePtr);
    assert_layout!(DestroyedPage, PagePtr);
    assert_layout!(ThreadHeapLocal, ThreadHeapPtr);
    assert_layout!(heap::ThreadHeapBorrowed, ThreadHeapPtr);
    assert_layout!(MainHeap, HeapPtr);
    assert_layout!(OwnedHeap, HeapPtr);
    assert_layout!(BorrowedHeap, HeapPtr);
    assert_layout!(
        page_queues::PageQueue<page_queues::NormalQueue>,
        PageQueuePtr
    );
    assert_layout!(NormalPageQueue, PageQueuePtr);
    assert_layout!(page_queues::HugePageQueue, PageQueuePtr);
    assert_layout!(FullPageQueue, PageQueuePtr);
}

#[test]
fn packed_and_atomic_wrappers_match_raw_layout() {
    assert_layout!(ThreadFreeWord, usize);
    assert_layout!(EncodedWord, usize);
    assert_layout!(BlockTag, usize);
    #[cfg(feature = "guarded")]
    {
        assert_layout!(GuardedSizeMin, usize);
        assert_layout!(GuardedSizeMax, usize);
        assert_layout!(GuardedSampleRate, usize);
        assert_layout!(GuardedSampleCount, usize);
    }
    assert_layout!(EncodedPtr<mi_block_t>, usize);
    assert_layout!(AvailableFreeList, Option<BlockPtr>);
    assert_layout!(LocalFreeList, Option<BlockPtr>);
    assert_layout!(atomic::DetachedRemoteList, Option<BlockPtr>);
    assert_layout!(atomic::PublishedRemoteHead, BlockPtr);
    assert_layout!(PaddingCanary, u32);
    assert_layout!(PaddingDelta, u32);
    assert_layout!(atomic::AtomicUsizeField, *mut usize);
    assert_layout!(atomic::AtomicI64Field, *mut i64);
    assert_layout!(atomic::AtomicPtrField<u8>, *mut *mut u8);
    assert_layout!(atomic::AtomicPageThreadId, *mut usize);
    assert_layout!(atomic::AtomicThreadFree, *mut usize);
    assert_layout!(atomic::AtomicThreadHeapRefcount, *mut usize);
    assert_layout!(atomic::AtomicAbandonedCount, *mut usize);
    assert_layout!(atomic::AtomicArenaPage, *mut usize);
}

#[test]
fn memid_wrapper_matches_raw_layout() {
    assert_layout!(MemId, crate::internal::memid::mi_memid_t);
    assert_layout!(
        memid::MemId<memid::OsMem>,
        crate::internal::memid::mi_memid_t
    );
    assert_layout!(
        memid::MemId<memid::ArenaMem>,
        crate::internal::memid::mi_memid_t
    );
}
