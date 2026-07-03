//! DT-13: Atomic field wrappers.

use core::sync::atomic::Ordering;

use super::freelist::ThreadFreeWord;
use super::page::{PageOwnerId, PageThreadId};
use super::ptr::BlockPtr;

mod arena_cells;
mod fields;
mod guards;
mod page_thread;
mod thread_free;
mod thread_heap_refcount;
pub(crate) use arena_cells::{AtomicAbandonedCount, AtomicArenaPage};
pub(crate) use fields::{AtomicI64Field, AtomicPtrField, AtomicUsizeField};
pub(crate) use guards::{AtomicGuard, MiLockGuard, MiLockPtr, OnceGuard, OnceStatePtr};
pub(crate) use page_thread::AtomicPageThreadId;
pub(crate) use thread_free::{AtomicThreadFree, RemoteFreeList, RemotePushOutcome};
pub(crate) use thread_heap_refcount::AtomicThreadHeapRefcount;

pub(crate) type MemoryOrder = Ordering;

pub(super) fn __lint_touch() {
    let _ = AtomicPageThreadId::load_relaxed as unsafe fn(AtomicPageThreadId) -> PageThreadId;
    let _ = AtomicPageThreadId::set_owner_release as unsafe fn(AtomicPageThreadId, PageOwnerId);
    let _ = AtomicPageThreadId::compare_exchange_release
        as unsafe fn(AtomicPageThreadId, &mut PageThreadId, PageThreadId) -> bool;
    let _ = core::mem::size_of::<RemotePushOutcome>();
    if let RemotePushOutcome::Failed(block) =
        // SAFETY[AUDIT:typed-atomic-thread-free]: lint-only fixture uses a dangling
        // non-dereferenced block identity to keep enum field reachability checked.
        RemotePushOutcome::Failed(unsafe {
            BlockPtr::new_unchecked(core::ptr::dangling_mut())
        })
    {
        let _ = block.as_ptr();
    }
    let _ = RemotePushOutcome::Success;
    let _ = AtomicThreadFree::compare_exchange_push
        as fn(
            AtomicThreadFree,
            &mut ThreadFreeWord,
            BlockPtr,
        ) -> Result<ThreadFreeWord, ThreadFreeWord>;
    let _ = AtomicThreadFree::compare_exchange_release_ownership
        as unsafe fn(AtomicThreadFree, &mut ThreadFreeWord) -> bool;
    let _ = AtomicThreadFree::take_head_preserving_owner
        as unsafe fn(AtomicThreadFree) -> RemoteFreeList;
    let _ = AtomicThreadFree::claim_ownership as unsafe fn(AtomicThreadFree) -> bool;
    let _ = AtomicThreadFree::release_ownership
        as unsafe fn(AtomicThreadFree, Option<BlockPtr>) -> bool;
}

#[cfg(test)]
mod tests;

#[cfg(all(test, not(miri)))]
mod concurrency_models;
