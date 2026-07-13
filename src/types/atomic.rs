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
pub(crate) use thread_free::{
    AtomicThreadFree, DetachedRemoteList, PublishedRemoteHead, RemotePushResult,
};
pub(crate) use thread_heap_refcount::AtomicThreadHeapRefcount;

pub(crate) type MemoryOrder = Ordering;

pub(super) fn __lint_touch() {
    let _ = AtomicPageThreadId::load_relaxed as unsafe fn(AtomicPageThreadId) -> PageThreadId;
    let _ = AtomicPageThreadId::set_owner_release as unsafe fn(AtomicPageThreadId, PageOwnerId);
    let _ = AtomicPageThreadId::compare_exchange_release
        as unsafe fn(AtomicPageThreadId, &mut PageThreadId, PageThreadId) -> bool;
    let _ = core::mem::size_of::<RemotePushResult>();
    let _ = AtomicThreadFree::try_push
        as fn(AtomicThreadFree, ThreadFreeWord, BlockPtr) -> RemotePushResult;
    let _ = AtomicThreadFree::compare_exchange_release_ownership
        as unsafe fn(AtomicThreadFree, &mut ThreadFreeWord) -> bool;
}

#[cfg(test)]
mod tests;

#[cfg(all(test, not(miri)))]
mod concurrency_models;
