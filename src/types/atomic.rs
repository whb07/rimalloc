//! DT-13: Atomic field wrappers.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use super::freelist::ThreadFreeWord;
use super::page::{PageOwnerId, PageThreadId};
use super::ptr::BlockPtr;

mod page_thread;
mod thread_free;
pub(crate) use page_thread::AtomicPageThreadId;
pub(crate) use thread_free::{AtomicThreadFree, RemoteFreeList, RemotePushOutcome};

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

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);

impl AtomicAbandonedCount {
    #[inline(always)]
    fn field(self) -> AtomicUsizeField {
        AtomicUsizeField(self.0.cast_mut().cast::<usize>())
    }

    pub(crate) unsafe fn load_relaxed(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-abandoned-count]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed loading.
        unsafe { self.field().load(Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize {
        // SAFETY[AUDIT:typed-atomic-abandoned-count]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed increment.
        unsafe { self.field().fetch_add(value, Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize {
        // SAFETY[AUDIT:typed-atomic-abandoned-count]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed decrement.
        unsafe { self.field().fetch_sub(value, Ordering::Relaxed) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);

impl AtomicThreadHeapRefcount {
    pub(crate) unsafe fn store_seq_cst(self, value: usize) {
        // SAFETY[AUDIT:typed-atomic-thread-heap-refcount]: the caller provides a valid thread
        // heap refcount field and preserves the existing SeqCst store.
        unsafe { self.0.store(value, Ordering::SeqCst) }
    }

    pub(crate) unsafe fn incref_acq_rel(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-thread-heap-refcount]: the caller provides a valid thread
        // heap refcount field and preserves the existing AcqRel increment.
        unsafe { self.0.fetch_add(1, Ordering::AcqRel) }
    }

    pub(crate) unsafe fn decref_acq_rel(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-thread-heap-refcount]: the caller provides a valid thread
        // heap refcount field and preserves the existing AcqRel decrement.
        unsafe { self.0.fetch_sub(1, Ordering::AcqRel) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);

impl AtomicArenaPage {
    pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t {
        // SAFETY[AUDIT:typed-atomic-arena-page]: `AtomicArenaPage` points to an
        // allocator-owned page-table pointer atomic and preserves acquire load.
        let atomic = unsafe { &*self.0 };
        atomic
            .load(Ordering::Acquire)
            .cast::<crate::types::mi_arena_pages_t>()
    }

    pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t) {
        // SAFETY[AUDIT:typed-atomic-arena-page]: `AtomicArenaPage` points to an
        // allocator-owned page-table pointer atomic and preserves release store.
        let atomic = unsafe { &*self.0 };
        atomic.store(value.cast::<c_void>(), Ordering::Release);
    }
}

pub(crate) struct AtomicGuard<'a> {
    pub(crate) field: AtomicUsizeField,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

impl<'a> AtomicGuard<'a> {
    /// # Safety
    /// `field` must refer to a valid atomic guard word.
    #[inline(always)]
    pub(crate) unsafe fn try_acquire(field: AtomicUsizeField) -> Option<Self> {
        let mut expected = 0;
        // SAFETY[AUDIT:typed-atomic-guard-once-lock]: the caller provides a valid guard
        // word and acquisition uses the existing AcqRel/Acquire CAS contract.
        if unsafe { field.compare_exchange_acq_rel(&mut expected, 1) } {
            Some(Self {
                field,
                _borrow: PhantomData,
            })
        } else {
            None
        }
    }
}

impl Drop for AtomicGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-atomic-guard-once-lock]: `AtomicGuard` is constructed only
        // after acquiring this guard word.
        unsafe { self.field.store(0, Ordering::Release) };
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct OnceStatePtr(pub(crate) *mut usize);

pub(crate) struct OnceGuard<'a> {
    pub(crate) once: OnceStatePtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

impl<'a> OnceGuard<'a> {
    /// # Safety
    /// `once` must refer to a valid allocator once-state.
    #[inline(always)]
    pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self> {
        // SAFETY[AUDIT:typed-atomic-guard-once-lock]: the caller provides a valid
        // allocator once-state and the guard releases it in Drop.
        if unsafe {
            crate::atomic::_mi_atomic_once_enter(once.0.cast::<crate::atomic::mi_atomic_once_t>())
        } {
            Some(Self {
                once,
                _borrow: PhantomData,
            })
        } else {
            None
        }
    }
}

impl Drop for OnceGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-atomic-guard-once-lock]: `OnceGuard` is constructed only
        // after entering this once-state.
        unsafe {
            crate::atomic::_mi_atomic_once_release(
                self.once.0.cast::<crate::atomic::mi_atomic_once_t>(),
            )
        };
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>);

pub(crate) struct MiLockGuard<'a> {
    pub(crate) lock: MiLockPtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

impl<'a> MiLockGuard<'a> {
    /// # Safety
    /// `lock` must refer to a valid allocator lock.
    #[inline(always)]
    pub(crate) unsafe fn acquire(lock: MiLockPtr) -> Self {
        // SAFETY[AUDIT:typed-atomic-guard-once-lock]: the caller provides a valid
        // allocator lock and the guard releases it in Drop.
        unsafe { crate::atomic::mi_lock_acquire(lock.0.as_ptr()) };
        Self {
            lock,
            _borrow: PhantomData,
        }
    }
}

impl Drop for MiLockGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-atomic-guard-once-lock]: `MiLockGuard` is constructed only
        // after acquiring this lock.
        unsafe { crate::atomic::mi_lock_release(self.lock.0.as_ptr()) };
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
