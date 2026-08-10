use static_assertions::assert_not_impl_any;

use super::atomic::{DetachedRemoteList, PublishedRemoteHead};
use super::freelist_wrappers::{AvailableFreeList, LocalFreeList};
use super::page_batch::{LocalFreeBatch, RemoteFreeBatch};
use super::page_queues::{NormalQueue, QueuedPage};
use super::page_states::{
    AbandonedMappedPageType, AbandonedPageType, Active, ActivePage, AllFreePage, AvailablePage,
    DestroyedPage, ForeignPage, FullPage, LocalPage, OwnedAbandonedPage, Page, PageAbandonOutcome,
    PageForAbandon, PageFreeOwner, RetiredPage, Uninit,
};
use super::validation::{
    BlockForFree, GuardedBlock, LocatedBlock, PointerLocation, PointerLookup, ValidPointerLookup,
    ValidatedFree, ValidatedFreeParts,
};
use super::validation_extra::{AllocatedBlock, FreeBlock};

assert_not_impl_any!(GuardedBlock: Copy, Clone);
assert_not_impl_any!(PointerLocation: Copy, Clone);
assert_not_impl_any!(LocatedBlock: Copy, Clone);
assert_not_impl_any!(ValidPointerLookup: Copy, Clone);
assert_not_impl_any!(BlockForFree: Copy, Clone);
assert_not_impl_any!(ValidatedFree: Copy, Clone);
assert_not_impl_any!(ValidatedFreeParts: Copy, Clone);
assert_not_impl_any!(PointerLookup: Copy, Clone);

assert_not_impl_any!(AllocatedBlock: Copy, Clone);
assert_not_impl_any!(FreeBlock: Copy, Clone);
assert_not_impl_any!(AvailableFreeList: Copy, Clone);
assert_not_impl_any!(LocalFreeList: Copy, Clone);
assert_not_impl_any!(PublishedRemoteHead: Copy, Clone);
assert_not_impl_any!(DetachedRemoteList: Copy, Clone);
assert_not_impl_any!(RemoteFreeBatch: Copy, Clone);
assert_not_impl_any!(LocalFreeBatch: Copy, Clone);

assert_not_impl_any!(Page<Uninit>: Copy, Clone);
assert_not_impl_any!(ActivePage: Copy, Clone);
assert_not_impl_any!(FullPage: Copy, Clone);
assert_not_impl_any!(RetiredPage: Copy, Clone);
assert_not_impl_any!(AvailablePage: Copy, Clone);
assert_not_impl_any!(LocalPage: Copy, Clone);
assert_not_impl_any!(ForeignPage: Copy, Clone);
assert_not_impl_any!(AbandonedPageType: Copy, Clone);
assert_not_impl_any!(AbandonedMappedPageType: Copy, Clone);
assert_not_impl_any!(OwnedAbandonedPage: Copy, Clone);
assert_not_impl_any!(DestroyedPage: Copy, Clone);
assert_not_impl_any!(AllFreePage<Active>: Copy, Clone);
assert_not_impl_any!(PageForAbandon: Copy, Clone);
assert_not_impl_any!(PageAbandonOutcome: Copy, Clone);
assert_not_impl_any!(PageFreeOwner: Copy, Clone);
assert_not_impl_any!(QueuedPage<NormalQueue>: Copy, Clone);
