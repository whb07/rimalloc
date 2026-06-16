//! Core types for mechanical_mimalloc.
//! Includes DT-1 scalar domain types and DT-2 pointer domain types.

mod ptr;
mod scalar;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, InteriorUserPtr, OsMemPtr, PageConstPtr, PagePtr, PageQueuePtr,
    PageStart, SliceStart, SubprocPtr, ThreadHeapPtr, TldPtr, UserPtr,
};

pub(crate) use scalar::{
    Alignment, AllocSizeError, ArenaIndex, Bin, BlockSize, DirectBin, EffectiveSize,
    MaybeAlignment, PaddedSize, RequestedSize, SliceCount, SliceIndex, UsableSize,
};