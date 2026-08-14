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
pub(crate) mod page_batch;
pub(crate) mod page_queues;
pub(crate) mod page_states;
pub(crate) mod ptr;
pub(crate) mod scalar;
pub(crate) mod track;
pub(crate) mod validation;
pub(crate) mod validation_extra;

pub use constants::*;

pub(crate) use ptr::{
    ArenaPtr, BlockPtr, HeapPtr, InteriorUserPtr, OsMemPtr, PageBlockLayout, PageBlockRange,
    PageConstPtr, PagePtr, PageQueuePtr, SliceStart, SubprocPtr, ThreadHeapPtr, TldPtr, UserPtr,
    ptr_addr_diff, ptr_align_down, ptr_align_up, ptr_exposed_addr, ptr_is_aligned,
};

#[cfg(all(test, target_pointer_width = "32"))]
pub(crate) use scalar::PageMapFlatIndex;
#[cfg(all(test, target_pointer_width = "32"))]
pub(crate) use scalar::PageMapIndex;
#[cfg(not(target_pointer_width = "32"))]
pub(crate) use scalar::PageMapIndex;
pub(crate) use scalar::{
    Alignment, AlignmentOffset, ArenaIndex, BatchLen, Bin, BlockSize, DirectBin, EffectiveSize,
    Errno, FullBlockSize, GoodSize, HeapSeq, Heartbeat, MaybeAlignment, NumaNode, OsPageSize,
    PaddedSize, PageCapacity, PageMapPrimaryIndex, PageMapSubIndex, PageQueueCount,
    PageReclaimLimit, PageReclaimOnFree, RequestedSize, ReservedBlockCount, RetireExpire,
    SliceCount, SliceIndex, StatsBin, SubprocSeq, ThreadSeq, UsableOut, UsableSize, UsedBlockCount,
    WordCount,
};

pub(crate) use layout::{
    AllocProduct, CallocRequest, ItemCount, LayoutRequest, MallocGenericFlags, MaybeHugeAlignment,
    OldAllocation, PreserveBytes, ReallocRequest, Zeroed,
};

pub(crate) use page::{
    FreePathKind, PageFlags, PageMapLocation, PageOwnerId, PageThreadId, RegisteredPageRange,
    ThreadId,
};

pub(crate) use freelist::{ThreadFreeOwner, ThreadFreeWord};

pub(crate) use validation::{
    BlockForFree, BlockOffset, GuardedBlock, PointerLocation, PointerLookup, ValidatedFree,
    ValidatedFreeParts, WasGuarded,
};

pub(crate) use memid::{
    ArenaMem, ArenaMemId, BlockIndex, CommittedBytes, ExternalMem, InitiallyCommitted,
    InitiallyZero, MemId, MemIdView, MemKindCode, MetaMem, NoneMem, OsMem, OsMemId, OsMemKind,
    Pinned, ReservedBytes, StaticMem,
};

pub(crate) use page_states::{
    AbandonedDisposition, AbandonedPageType as AbandonedPage, Active, ActivePage, AllFreePage,
    AvailablePage, DestroyedPage, ForeignPage, FreeRouteSnapshot, Full, FullPage, LocalPage,
    OwnedAbandonedPage, Page, PageAbandonOutcome, PageForAbandon, ThreadHeapLocal, Uninit,
};

pub(crate) use page_batch::{BatchValidationError, LocalFreeBatch, RemoteFreeBatch};

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
};

pub(crate) use heap::{
    BorrowedHeap, BorrowedSubproc, MainHeap, MainSubproc, OwnedHeap, ThreadHeapState,
};

pub(crate) use arena::{AllowPinned, ArenaAllocation, Exclusive, MatchNuma, SliceRange};
pub(crate) use callbacks::{
    BlockVisitCallback, BlockVisitCallbackArg, DeferredFreeCallback, DeferredFreeCallbackArg,
    ErrorCallback, ErrorCallbackArg, HeapVisitCallback, HeapVisitCallbackArg, OutputCallback,
    OutputCallbackArg, ThreadLocalClaimCallback,
};
pub(crate) mod atomic;
mod heap;

pub(crate) use atomic::{
    AtomicAbandonedCount, AtomicArenaPage, AtomicGuard, AtomicI64Field, AtomicPtrField,
    AtomicUsizeField, DetachedRemoteList, MemoryOrder, MiLockGuard, MiLockPtr, OnceGuard,
    OnceStatePtr, PublishedRemoteHead, RemotePushResult,
};
mod padding;

pub(crate) use padding::{Padding, PaddingCanary, PaddingDelta, PaddingPtr};

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod linearity_tests;
