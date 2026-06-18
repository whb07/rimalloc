//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.
//
// `core_types` defines thin `#[repr(transparent)]` wrappers whose `unsafe fn`
// bodies deref raw pointers and call `unsafe` constructors (`new_unchecked`).
// Match the crate convention (`src/free.rs` et al.) and allow implicit unsafe
// in unsafe-fn bodies rather than wrapping every operation in `unsafe {}`.
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code, unused_imports)]

mod arena;
mod free_actions;
mod freelist;
mod freelist_wrappers;
mod layout;
mod memid;
mod page;
mod page_queues;
mod page_states;
mod ptr;
mod scalar;
mod validation;
mod validation_extra;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, InteriorUserPtr, OsMemPtr, PageConstPtr, PagePtr, PageQueuePtr,
    PageStart, SliceStart, SubprocPtr, ThreadHeapPtr, TldPtr, UserPtr,
};

pub(crate) use scalar::{
    Alignment, AllocSizeError, ArenaIndex, Bin, BlockSize, DirectBin, EffectiveSize, GoodSize,
    MaybeAlignment, PaddedSize, RequestedSize, SliceCount, SliceIndex, StatsBin, UsableSize,
};

pub(crate) use layout::{
    CallocRequest, ItemCount, LayoutError, LayoutRequest, MallocGenericFlags, MaybeHugeAlignment,
    OldAllocation, ReallocRequest, Zeroed,
};

pub(crate) use page::{FreePathKind, PageFlags, PageOwnerId, PageThreadId, ThreadId, XThreadDelta};

pub(crate) use freelist::{ThreadFreeOwner, ThreadFreeSnapshot, ThreadFreeWord};

pub(crate) use validation::{
    BlockForFree, BlockOffset, GuardedBlock, PointerInvalidReason, PointerLocation, PointerLookup,
    ValidatedFree, WasGuarded,
};

pub(crate) use memid::{
    AnyMem, ArenaMem, ArenaMemId, BlockIndex, ExternalMem, ExternalMemId, HeapMainMem,
    InitiallyCommitted, InitiallyZero, MemId, MemIdView, MemKind, MemKindCode, MetaMem, MetaMemId,
    NoneMem, OsMem, OsMemId, OsMemKind, Pinned, ReservedBytes, StaticMem,
};

pub(crate) use page_states::{
    Abandoned, AbandonedMapped, AbandonedMappedPageType as AbandonedMappedPage,
    AbandonedPageType as AbandonedPage, Active, ActivePage, AllFreePage, AvailablePage, Destroyed,
    DestroyedPage, ForeignPage, Full, FullPage, LocalPage, OwnedAbandonedPage, Page,
    PageAbandonOutcome, PageForAbandon, PageFreeOwner, Retired, RetiredPage, ThreadHeapLocal,
    Uninit,
};

pub(crate) use validation_extra::{AllocatedBlock, FreeBlock};

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedFreeList, EncodedPtr, EncodedWord, ForceCollect,
    LocalFreeList, PageKeys, RemoteFreeList,
};

pub(crate) use free_actions::LocalFreeMode;

pub(crate) use page_queues::{
    FullPageQueue, FullQueue, HugePageQueue, HugeQueue, NormalPageQueue, NormalQueue, PageQueue,
    QueuedPage, UnqueuedPage, free_all_free_page_to_arena, move_to_full_queue, unfull_page,
};

pub(crate) use arena::{
    AbandonedCountCell, ArenaPageCell, ArenaPageTablePtr, ExclusiveArena, ParentArena, SharedArena,
    SliceError, SliceRange,
};
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
