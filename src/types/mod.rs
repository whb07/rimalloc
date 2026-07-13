//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.
//
pub(crate) mod arena;
pub(crate) mod callbacks;
pub mod constants;
pub(crate) mod free_actions;
pub(crate) mod freelist;
pub(crate) mod freelist_wrappers;
pub(crate) mod layout;
pub(crate) mod memid;
pub(crate) mod page;
pub(crate) mod page_queues;
pub(crate) mod page_states;
pub(crate) mod ptr;
pub(crate) mod scalar;
pub(crate) mod track;
pub(crate) mod validation;
pub(crate) mod validation_extra;

pub use constants::*;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, InteriorUserPtr, OsMemPtr, PageConstPtr, PagePtr, PageQueuePtr,
    PageStart, SliceStart, SubprocPtr, ThreadHeapPtr, TldPtr, UserPtr, ptr_addr_diff,
    ptr_align_down, ptr_align_up, ptr_exposed_addr, ptr_is_aligned,
};

pub(crate) use scalar::{
    Alignment, AlignmentOffset, ArenaIndex, Bin, BlockSize, DirectBin, EffectiveSize, Errno,
    FullBlockSize, GoodSize, HeapSeq, Heartbeat, MaybeAlignment, NumaNode, OsPageSize, PaddedSize,
    PageCapacity, PageMapIndex, PageMapSubIndex, PageQueueCount, PageReclaimLimit,
    PageReclaimOnFree, RequestedSize, ReservedBlockCount, RetireExpire, SliceCount, SliceIndex,
    StatsBin, SubprocSeq, ThreadSeq, UsableOut, UsableSize, UsedBlockCount, WordCount,
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
    AllocProduct, CallocRequest, ItemCount, LayoutRequest, MallocGenericFlags, MaybeHugeAlignment,
    OldAllocation, PreserveBytes, ReallocRequest, Zeroed,
};

pub(crate) use page::{
    FreePathKind, PageFlags, PageOwnerId, PageThreadId, RegisteredPageRange, ThreadId,
};

pub(crate) use freelist::{ThreadFreeOwner, ThreadFreeWord};

pub(crate) use validation::{
    BlockForFree, BlockOffset, GuardedBlock, PointerInvalidReason, PointerLocation, PointerLookup,
    ValidatedFree, ValidatedFreeParts, WasGuarded,
};

pub(crate) use memid::{
    ArenaMem, ArenaMemId, BlockIndex, CommittedBytes, ExternalMem, InitiallyCommitted,
    InitiallyZero, MemId, MemIdView, MemKindCode, MetaMem, NoneMem, OsMem, OsMemId, OsMemKind,
    Pinned, ReservedBytes, StaticMem,
};

pub(crate) use page_states::{
    AbandonedDisposition, AbandonedMappedPageType, AbandonedPageType as AbandonedPage, Active,
    ActivePage, AllFreePage, AvailablePage, DestroyedPage, ForeignPage, Full, FullPage, LocalPage,
    OwnedAbandonedPage, Page, PageAbandonOutcome, PageForAbandon, PageFreeOwner, ThreadHeapLocal,
    Uninit,
};

pub(crate) use validation_extra::FreeBlock;

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedPtr, EncodedWord, LocalFreeList, PageKeys,
};

pub(crate) use free_actions::LocalFreeMode;

pub(crate) use track::TrackMode;

#[cfg(feature = "guarded")]
pub(crate) use crate::internal::guarded::{
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
pub(crate) mod atomic;
mod heap;

pub(crate) use atomic::{
    AtomicAbandonedCount, AtomicArenaPage, AtomicGuard, AtomicI64Field, AtomicPtrField,
    AtomicUsizeField, DetachedRemoteList, MemoryOrder, MiLockGuard, MiLockPtr, OnceGuard,
    OnceStatePtr, PublishedRemoteHead, RemotePushResult,
};
mod padding;
mod raw_bridge;

pub(crate) use padding::{Padding, PaddingCanary, PaddingDelta, PaddingPtr};
pub(crate) use raw_bridge::*;

fn __lint_touch() {
    arena::__lint_touch();
    atomic::__lint_touch();
    callbacks::__lint_touch();
    freelist_wrappers::__lint_touch();
    #[cfg(feature = "guarded")]
    crate::internal::guarded::__lint_touch();
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
