//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.
//
// `core_types` defines thin `#[repr(transparent)]` wrappers whose `unsafe fn`
// bodies deref raw pointers and call `unsafe` constructors (`new_unchecked`).
// Match the crate convention (`src/free.rs` et al.) and allow implicit unsafe
// in unsafe-fn bodies rather than wrapping every operation in `unsafe {}`.
#![allow(unsafe_op_in_unsafe_fn)]
// `core_types` intentionally contains strict typestate/scalar wrappers before
// every scaffolded state is migrated to a live call site. The source-audit
// tests guard the accepted scaffold shape against raw-access regressions.
#![allow(dead_code)]

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
    MemKindCode, MetaMem, NoneMem, OsMem, Pinned, ReservedBytes, StaticMem,
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
mod atomic_wrappers;
mod heap;

pub(crate) use atomic_wrappers::{
    AtomicI64Field, AtomicPageThreadId, AtomicPtrField, AtomicThreadFree, AtomicUsizeField,
    MemoryOrder,
};
mod padding;
