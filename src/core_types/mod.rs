//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.

mod ptr;
mod scalar;
mod layout;
mod page;
mod freelist;
mod validation;

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
