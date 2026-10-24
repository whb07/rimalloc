use static_assertions::assert_not_impl_any;

use super::arena::ArenaAllocation;
use super::atomic::{AtomicGuard, DetachedRemoteList, MiLockGuard, OnceGuard, PublishedRemoteHead};
use super::freelist_wrappers::{AvailableFreeList, LocalFreeList};
use super::heap::OwnedHeap;
use super::layout::{OldAllocation, ReallocRequest};
use super::page_batch::{LocalFreeBatch, RemoteFreeBatch};
use super::page_queues::{NormalQueue, QueuedPage};
use super::page_states::{
    AbandonedMappedPageType, AbandonedPageType, Active, ActivePage, AllFreePage, AvailablePage,
    DestroyedPage, ForeignPage, FreeRouteSnapshot, FullPage, LocalPage, OwnedAbandonedPage, Page,
    PageAbandonOutcome, PageForAbandon, RetiredPage, Uninit,
};
use super::validation::{
    BlockForFree, FreePage, FreeRoute, GuardedBlock, LocatedBlock, PointerLocation, PointerLookup,
    ScopedFreeBlock, ValidPointerLookup, ValidatedFree,
};
use super::validation_extra::AllocatedBlock;

assert_not_impl_any!(super::local_page::PageAccess<'_, AvailablePage>: Copy, Clone, Send, Sync);
assert_not_impl_any!(super::local_page::AllocatedBlockStorage<'_>: Copy, Clone, Send, Sync);
assert_not_impl_any!(super::local_page::AllocatedBlockStorage<'_, super::local_page::Writable>: Copy, Clone, Send, Sync);
assert_not_impl_any!(super::local::LocalHeapAccess<'_>: Copy, Clone, Send, Sync);

assert_not_impl_any!(GuardedBlock: Copy, Clone);
assert_not_impl_any!(PointerLocation: Copy, Clone);
assert_not_impl_any!(LocatedBlock: Copy, Clone);
assert_not_impl_any!(ValidPointerLookup: Copy, Clone);
assert_not_impl_any!(BlockForFree: Copy, Clone);
assert_not_impl_any!(ValidatedFree: Copy, Clone);
assert_not_impl_any!(FreeRoute<'_>: Copy, Clone);
assert_not_impl_any!(FreePage<'_, LocalPage>: Copy, Clone);
assert_not_impl_any!(FreePage<'_, ForeignPage>: Copy, Clone);
assert_not_impl_any!(ScopedFreeBlock<'_>: Copy, Clone);
assert_not_impl_any!(PointerLookup: Copy, Clone);

assert_not_impl_any!(AllocatedBlock: Copy, Clone);
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
assert_not_impl_any!(FreeRouteSnapshot: Copy, Clone);
assert_not_impl_any!(QueuedPage<NormalQueue>: Copy, Clone);

assert_not_impl_any!(OwnedHeap: Copy, Clone);
assert_not_impl_any!(ArenaAllocation: Copy, Clone);
assert_not_impl_any!(OldAllocation: Copy, Clone);
assert_not_impl_any!(ReallocRequest: Copy, Clone);
assert_not_impl_any!(AtomicGuard<'_>: Copy, Clone, Send, Sync);
assert_not_impl_any!(OnceGuard<'_>: Copy, Clone, Send, Sync);
assert_not_impl_any!(MiLockGuard<'_>: Copy, Clone, Send, Sync);
