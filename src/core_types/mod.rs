//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.
//
mod arena;
mod callbacks;
mod free_actions;
mod freelist;
mod freelist_wrappers;
#[cfg(feature = "guarded")]
mod guarded;
mod layout;
mod memid;
mod page;
mod page_queues;
mod page_states;
mod ptr;
mod scalar;
mod track;
mod validation;
mod validation_extra;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, InteriorUserPtr, OsMemPtr, PageConstPtr, PagePtr, PageQueuePtr,
    PageStart, SliceStart, SubprocPtr, ThreadHeapPtr, TldPtr, UserPtr, ptr_addr_diff,
    ptr_align_down, ptr_align_up, ptr_exposed_addr, ptr_is_aligned,
};

pub(crate) use scalar::{
    Alignment, AlignmentOffset, ArenaCount, ArenaIndex, Bin, BlockSize, DirectBin, EffectiveSize,
    Errno, FullBlockSize, GoodSize, HeapCount, HeapSeq, Heartbeat, MaybeAlignment, NumaNode,
    OsPageSize, PaddedSize, PageCapacity, PageMapIndex, PageMapSubIndex, PageQueueCount,
    PageReclaimLimit, PageReclaimOnFree, RequestedSize, ReservedBlockCount, RetireExpire,
    SliceCount, SliceIndex, StatsBin, SubprocSeq, ThreadCount, ThreadSeq, UsableSize,
    UsedBlockCount, WordCount,
};

// Keep staged Phase 1 re-exports exercised in non-test builds without lint allowances.
const _: usize = core::mem::size_of::<(
    PageConstPtr,
    InteriorUserPtr,
    PageStart,
    SliceStart,
    ArenaIndex,
    Alignment,
    RegisteredPageRange,
    MainHeap,
    OwnedHeap,
    BorrowedHeap,
    MainSubproc,
    BorrowedSubproc,
    ThreadHeapBorrowed,
    CallbackArg,
    DeferredFreeCallbackArg,
    OutputCallbackArg,
    ErrorCallbackArg,
    HeapVisitCallbackArg,
    BlockVisitCallbackArg,
    ThreadLocalClaimCallbackArg,
    DeferredFreeCallback,
    OutputCallback,
    ErrorCallback,
    HeapVisitCallback,
    BlockVisitCallback,
    ThreadLocalClaimCallback,
)>();

pub(crate) use layout::{
    AllocProduct, CallocRequest, LayoutRequest, MallocGenericFlags, MaybeHugeAlignment,
    OldAllocation, PreserveBytes, ReallocRequest, Zeroed,
};

pub(crate) use page::{
    FreePathKind, PageFlags, PageOwnerId, PageThreadId, RegisteredPageRange, ThreadId,
};

pub(crate) use freelist::{ThreadFreeOwner, ThreadFreeWord};

pub(crate) use validation::{
    BlockForFree, BlockOffset, GuardedBlock, PointerInvalidReason, PointerLocation, PointerLookup,
    ValidatedFree, WasGuarded,
};

pub(crate) use memid::{
    ArenaMem, BlockIndex, CommittedBytes, ExternalMem, InitiallyCommitted, InitiallyZero, MemId,
    MemIdView, MemKindCode, MetaMem, NoneMem, OsMem, OsMemId, OsMemKind, Pinned, ReservedBytes,
    StaticMem,
};

pub(crate) use page_states::{
    AbandonedPageType as AbandonedPage, Active, ActivePage, AllFreePage, AvailablePage,
    DestroyedPage, ForeignPage, Full, FullPage, LocalPage, OwnedAbandonedPage, Page,
    PageAbandonOutcome, PageForAbandon, PageFreeOwner, ThreadHeapLocal, Uninit,
};

pub(crate) use validation_extra::FreeBlock;

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedPtr, EncodedWord, ForceCollect, LocalFreeList,
    PageKeys,
};

pub(crate) use free_actions::LocalFreeMode;

pub(crate) use track::TrackMode;

#[cfg(feature = "guarded")]
pub(crate) use guarded::{
    GuardedConfig, GuardedDecision, GuardedSampleCount, GuardedSampleRate, GuardedSizeMax,
    GuardedSizeMin,
};

pub(crate) use page_queues::{
    FullPageQueue, FullQueue, HugePageQueue, NormalPageQueue, NormalQueue, QueuedPage,
    free_all_free_page_to_arena,
};

pub(crate) use heap::{
    BorrowedHeap, BorrowedSubproc, MainHeap, MainSubproc, OwnedHeap, ThreadHeapBorrowed,
    ThreadHeapState,
};

pub(crate) use arena::{AllowPinned, ArenaAllocation, Exclusive, MatchNuma, SliceRange};
pub(crate) use callbacks::{
    BlockVisitCallback, BlockVisitCallbackArg, CallbackArg, DeferredFreeCallback,
    DeferredFreeCallbackArg, ErrorCallback, ErrorCallbackArg, HeapVisitCallback,
    HeapVisitCallbackArg, OutputCallback, OutputCallbackArg, ThreadLocalClaimCallback,
    ThreadLocalClaimCallbackArg,
};
mod atomic_wrappers;
mod heap;

pub(crate) use atomic_wrappers::{
    AtomicAbandonedCount, AtomicArenaPage, AtomicGuard, AtomicI64Field, AtomicPageThreadId,
    AtomicPtrField, AtomicThreadFree, AtomicThreadHeapRefcount, AtomicUsizeField, MemoryOrder,
    MiLockGuard, MiLockPtr, OnceGuard, OnceStatePtr, RemoteFreeList,
};
mod padding;

pub(crate) use padding::{Padding, PaddingCanary, PaddingDelta, PaddingPtr};

#[cfg(test)]
mod layout_tests {
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
        assert_layout!(PageStart, NonNull<u8>);
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
        assert_eq!(size_of::<Option<PageStart>>(), size_of::<*mut u8>());
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
        assert_layout!(BlockSize, NonZeroUsize);
        assert_layout!(FullBlockSize, NonZeroUsize);
        assert_layout!(Alignment, NonZeroUsize);
        assert_layout!(AlignmentOffset, usize);
        assert_layout!(WordCount, usize);
        assert_layout!(PageQueueCount, usize);
        assert_layout!(PageReclaimOnFree, core::ffi::c_long);
        assert_layout!(PageReclaimLimit, core::ffi::c_long);
        assert_layout!(UsedBlockCount, u16);
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
        assert_layout!(ThreadHeapBorrowed, ThreadHeapPtr);
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
        assert_layout!(atomic_wrappers::RemoteFreeList, Option<BlockPtr>);
        assert_layout!(freelist_wrappers::EncodedFreeList, Option<BlockPtr>);
        assert_layout!(PaddingCanary, u32);
        assert_layout!(PaddingDelta, u32);
        assert_layout!(atomic_wrappers::AtomicUsizeField, *mut usize);
        assert_layout!(atomic_wrappers::AtomicI64Field, *mut i64);
        assert_layout!(atomic_wrappers::AtomicPtrField<u8>, *mut *mut u8);
        assert_layout!(AtomicPageThreadId, *mut usize);
        assert_layout!(AtomicThreadFree, *mut usize);
        assert_layout!(AtomicThreadHeapRefcount, *mut usize);
        assert_layout!(atomic_wrappers::AtomicAbandonedCount, *mut usize);
        assert_layout!(atomic_wrappers::AtomicArenaPage, *mut usize);
    }

    #[test]
    fn memid_wrapper_matches_raw_layout() {
        assert_layout!(MemId, crate::internal::mi_memid_t);
        assert_layout!(memid::MemId<memid::OsMem>, crate::internal::mi_memid_t);
        assert_layout!(memid::MemId<memid::ArenaMem>, crate::internal::mi_memid_t);
    }
}
