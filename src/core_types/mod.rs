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
    FullPage, RetiredPage, Page,
};

pub(crate) use validation_extra::{AllocatedBlock, FreeBlock};

pub(crate) use freelist_wrappers::{
    AvailableFreeList, BlockLink, BlockTag, EncodedFreeList, EncodedPtr, EncodedWord, ForceCollect,
    LocalFreeList, PageKeys, RemoteFreeList,
};
