//! DT-13: Atomic field wrappers.

use core::sync::atomic::Ordering;

use super::ptr::BlockPtr;

mod arena_cells;
mod fields;
mod guards;
mod page_thread;
mod thread_free;
mod thread_heap_refcount;
pub(crate) use arena_cells::{AtomicAbandonedCount, AtomicArenaPage};
pub(crate) use fields::{AtomicI64Field, AtomicPtrField, AtomicUsizeField};
pub(crate) use guards::{AtomicGuard, MiLockGuard, OnceGuard};
pub(crate) use page_thread::AtomicPageThreadId;
pub(crate) use thread_free::{
    AtomicThreadFree, DetachedRemoteList, PublishedRemoteHead, RemotePushResult,
};
pub(crate) use thread_heap_refcount::AtomicThreadHeapRefcount;

pub(crate) type MemoryOrder = Ordering;

#[cfg(test)]
mod tests;

#[cfg(all(test, not(miri)))]
mod concurrency_models;
