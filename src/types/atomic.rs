//! DT-13: Atomic field wrappers.

use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use super::freelist::ThreadFreeWord;
use super::page::{PageOwnerId, PageThreadId};
use super::ptr::BlockPtr;

mod arena_cells;
mod guards;
mod page_thread;
mod thread_free;
mod thread_heap_refcount;
pub(crate) use arena_cells::{AtomicAbandonedCount, AtomicArenaPage};
pub(crate) use guards::{AtomicGuard, MiLockGuard, MiLockPtr, OnceGuard, OnceStatePtr};
pub(crate) use page_thread::AtomicPageThreadId;
pub(crate) use thread_free::{AtomicThreadFree, RemoteFreeList, RemotePushOutcome};
pub(crate) use thread_heap_refcount::AtomicThreadHeapRefcount;

pub(crate) type MemoryOrder = Ordering;

/// Atomic access to a raw `usize` field.
///
/// # Safety
/// The pointer must be non-null, properly aligned for `AtomicUsize`, and valid
/// for the duration of each operation. While this wrapper is used, all
/// concurrent accesses to the same storage must be atomic and use compatible
/// aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);

impl AtomicUsizeField {
    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder) {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_add(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_sub(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_or(self, bits: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_or(bits, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_and(self, bits: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_and(bits, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(
        self,
        expected: &mut usize,
        desired: usize,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_weak(
        self,
        expected: &mut usize,
        desired: usize,
        success: MemoryOrder,
        failure: MemoryOrder,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-usize-field]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        match atomic.compare_exchange_weak(*expected, desired, success, failure) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

/// Atomic access to a raw `i64` field.
///
/// # Safety
/// The pointer must be non-null, properly aligned for `AtomicI64`, and valid
/// for the duration of each operation. While this wrapper is used, all
/// concurrent accesses to the same storage must be atomic and use compatible
/// aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicI64Field(pub(crate) *mut i64);

impl AtomicI64Field {
    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64 {
        // SAFETY[AUDIT:typed-atomic-i64-field]: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder) {
        // SAFETY[AUDIT:typed-atomic-i64-field]: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool {
        // SAFETY[AUDIT:typed-atomic-i64-field]: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

/// Atomic access to a raw pointer field.
///
/// # Safety
/// The pointer-to-pointer must be non-null, properly aligned for
/// `AtomicPtr<T>`, and valid for the duration of each operation. While this
/// wrapper is used, all concurrent accesses to the same storage must be atomic
/// and use compatible aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);

impl<T> AtomicPtrField<T> {
    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T {
        // SAFETY[AUDIT:typed-atomic-pointer-field]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder) {
        // SAFETY[AUDIT:typed-atomic-pointer-field]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T {
        // SAFETY[AUDIT:typed-atomic-pointer-field]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.swap(value, Ordering::AcqRel)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_release(
        self,
        expected: &mut *mut T,
        desired: *mut T,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-pointer-field]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(
        self,
        expected: &mut *mut T,
        desired: *mut T,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-pointer-field]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

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
