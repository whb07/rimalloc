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
    SliceCount, SliceIndex, StatsBin, SubprocSeq, ThreadCount, ThreadSeq, UsableOut, UsableSize,
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
    ArenaMem, ArenaMemId, BlockIndex, CommittedBytes, ExternalMem, InitiallyCommitted,
    InitiallyZero, MemId, MemIdView, MemKindCode, MetaMem, NoneMem, OsMem, OsMemId, OsMemKind,
    Pinned, ReservedBytes, StaticMem,
};

pub(crate) use page_states::{
    AbandonedPageType as AbandonedPage, Active, ActivePage, AllFreePage, AvailablePage,
    DestroyedPage, ForeignPage, Full, FullPage, LocalPage, OwnedAbandonedPage, Page,
    PageAbandonOutcome, PageForAbandon, PageFreeOwner, ThreadHeapLocal, Uninit,
};

pub(crate) use validation_extra::FreeBlock;

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedPtr, EncodedWord, LocalFreeList, PageKeys,
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

fn __lint_touch() {
    arena::__lint_touch();
    atomic_wrappers::__lint_touch();
    callbacks::__lint_touch();
    freelist_wrappers::__lint_touch();
    #[cfg(feature = "guarded")]
    guarded::__lint_touch();
    heap::__lint_touch();
    layout::__lint_touch();
    memid::__lint_touch();
    page::__lint_touch();
    page_queues::__lint_touch();
    page_states::__lint_touch();
    ptr::__lint_touch();
    scalar::__lint_touch();
    validation::__lint_touch();
}

const _: fn() = __lint_touch;

#[cfg(test)]
mod layout_tests;
