//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.
//
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod arena;
mod free_actions;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod freelist;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod freelist_wrappers;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod layout;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod memid;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod page;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod page_queues;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod page_states;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod ptr;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod scalar;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod validation;
mod validation_extra;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, OsMemPtr, PagePtr, PageQueuePtr, SubprocPtr, ThreadHeapPtr,
    TldPtr, UserPtr,
};

pub(crate) use scalar::{
    Bin, BlockSize, DirectBin, EffectiveSize, GoodSize, MaybeAlignment, PaddedSize, RequestedSize,
    SliceCount, SliceIndex, StatsBin, UsableSize,
};

pub(crate) use layout::{
    CallocRequest, LayoutRequest, MallocGenericFlags, MaybeHugeAlignment, OldAllocation,
    ReallocRequest, Zeroed,
};

pub(crate) use page::{PageFlags, PageOwnerId, PageThreadId, ThreadId, XThreadDelta};

pub(crate) use freelist::{ThreadFreeOwner, ThreadFreeWord};

pub(crate) use validation::{
    BlockForFree, BlockOffset, GuardedBlock, PointerInvalidReason, PointerLocation, PointerLookup,
    ValidatedFree, WasGuarded,
};

pub(crate) use memid::{
    ArenaMem, BlockIndex, ExternalMem, InitiallyCommitted, InitiallyZero, MemId, MemIdView,
    MemKindCode, MetaMem, NoneMem, OsMem, OsMemKind, Pinned, ReservedBytes, StaticMem,
};

pub(crate) use page_states::{
    AbandonedPageType as AbandonedPage, Active, ActivePage, AllFreePage, AvailablePage,
    DestroyedPage, ForeignPage, FullPage, LocalPage, OwnedAbandonedPage, PageAbandonOutcome,
    PageForAbandon, ThreadHeapLocal,
};

pub(crate) use validation_extra::FreeBlock;

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedPtr, LocalFreeList, PageKeys, RemoteFreeList,
};

pub(crate) use free_actions::LocalFreeMode;

pub(crate) use page_queues::{
    FullPageQueue, NormalPageQueue, free_all_free_page_to_arena, move_to_full_queue, unfull_page,
};

pub(crate) use arena::SliceRange;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod atomic_wrappers;
#[expect(dead_code, reason = "staged strict-type scaffold from TYPES_SPEC.md")]
mod heap;

pub(crate) use atomic_wrappers::{
    AtomicI64Field, AtomicPageThreadId, AtomicPtrField, AtomicThreadFree, AtomicUsizeField,
    MemoryOrder,
};
mod padding;

pub(crate) use padding::{Padding, PaddingCanary, PaddingDelta, PaddingPtr};
