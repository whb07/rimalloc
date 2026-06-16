//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.

mod ptr;
mod scalar;
mod layout;
mod page;
mod freelist;
mod validation;
mod memid;
mod page_states;
mod validation_extra;
mod freelist_wrappers;
mod free_actions;
mod page_queues;
mod arena;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, InteriorUserPtr, OsMemPtr, PageConstPtr, PagePtr, PageQueuePtr,
    PageStart, SliceStart, SubprocPtr, ThreadHeapPtr, TldPtr, UserPtr,
};

pub(crate) use scalar::{
    Alignment, AllocSizeError, ArenaIndex, Bin, BlockSize, DirectBin, EffectiveSize,
    MaybeAlignment, PaddedSize, RequestedSize, SliceCount, SliceIndex, UsableSize,
};

pub(crate) use layout::{
    CallocRequest, ItemCount, LayoutError, LayoutRequest, MallocGenericFlags,
    MaybeHugeAlignment, OldAllocation, ReallocRequest, Zeroed,
};

pub(crate) use page::{
    FreePathKind, PageFlags, PageOwnerId, PageThreadId, ThreadId, XThreadDelta,
};

pub(crate) use freelist::{
    ThreadFreeOwner, ThreadFreeSnapshot, ThreadFreeWord,
};

pub(crate) use validation::{
    BlockForFree, BlockOffset, GuardedBlock, PointerInvalidReason, PointerLocation, PointerLookup,
    ValidatedFree, WasGuarded,
};

pub(crate) use memid::{
    AnyMem, ArenaMem, ArenaMemId, BlockIndex, ExternalMem, HeapMainMem, InitiallyCommitted,
    InitiallyZero, MemId, MemIdView, MemKind, MemKindCode, MetaMem, MetaMemId, NoneMem, OsMem,
    OsMemId, OsMemKind, Pinned, ReservedBytes, StaticMem,
};

pub(crate) use page_states::{
    Abandoned, AbandonedMapped, Active, Destroyed, Full, Retired, Uninit,
    ActivePage, AvailablePage, LocalPage, ForeignPage, AbandonedPageType as AbandonedPage,
    AbandonedMappedPageType as AbandonedMappedPage, OwnedAbandonedPage, PageFreeOwner,
    FullPage, RetiredPage, DestroyedPage, AllFreePage, Page, ReclaimResult, ThreadHeapLocal
};

pub(crate) use validation_extra::{AllocatedBlock, FreeBlock};

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedFreeList, EncodedPtr, EncodedWord, ForceCollect,
    LocalFreeList, PageKeys, RemoteFreeList,
};

pub(crate) use free_actions::{
    LocalFreeMode, free_block_local, free_block_remote, try_reclaim_abandoned
};

pub(crate) use page_queues::{
    FullPageQueue, FullQueue, HugePageQueue, HugeQueue, NormalPageQueue, NormalQueue, PageQueue,
    QueuedPage, UnqueuedPage, free_all_free_page_to_arena, move_to_full_queue, unfull_page,
};

pub(crate) use arena::{SliceError, SliceRange, SharedArena, ExclusiveArena, ParentArena, ArenaPageTablePtr, AbandonedCountCell, ArenaPageCell};
mod heap;

pub(crate) use heap::{
    BorrowedHeap, BorrowedSubproc, MainHeap, MainSubproc, OwnedHeap, ThreadHeapBorrowed,
    ThreadHeapState,
};
mod atomic_wrappers;

pub(crate) use atomic_wrappers::{
    AtomicAbandonedCount, AtomicArenaPage, AtomicGuard, AtomicPageThreadId, AtomicThreadFree,
    AtomicUsizeField, MemoryOrder, MiLockGuard, MiLockPtr, OnceGuard, OnceStatePtr,
    RemotePushOutcome,
};
mod padding;

pub(crate) use padding::{Padding, PaddingCanary, PaddingDelta, PaddingPtr};
