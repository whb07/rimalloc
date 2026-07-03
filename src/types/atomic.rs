//! DT-13: Atomic field wrappers.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use super::freelist::ThreadFreeWord;
use super::page::{PageFlags, PageOwnerId, PageThreadId};
use super::ptr::BlockPtr;

mod thread_free;
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
pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);

impl AtomicPageThreadId {
    /// Load the page's tagged thread-id word (thread id + flag bits) as a typed
    /// `PageThreadId`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn load(self) -> PageThreadId {
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: the caller provides a valid page
        // `xthread_id` atomic field and this wrapper preserves relaxed loading.
        PageThreadId::new(unsafe { self.0.load(Ordering::Relaxed) })
    }

    /// Compatibility shim for callers not yet migrated to the `load` name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn load_relaxed(self) -> PageThreadId {
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: compatibility shim forwards to the
        // typed relaxed page-thread-id loader with the same caller contract.
        unsafe { self.load() }
    }

    /// Set or clear the given flag bits in the page's `xthread_id` field using a
    /// relaxed `fetch_or` / `fetch_and`. Returns the previous `PageThreadId`.
    ///
    /// Mirrors `mi_page_flags_set`: a single relaxed atomic RMW on `xthread_id`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn set_flags(self, flags: PageFlags, set: bool) -> PageThreadId {
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: the caller provides a valid page
        // `xthread_id` atomic field; the RMW keeps C's relaxed flag update.
        let old = if set {
            unsafe { self.0.fetch_or(flags.bits(), Ordering::Relaxed) }
        } else {
            unsafe { self.0.fetch_and(!flags.bits(), Ordering::Relaxed) }
        };
        PageThreadId::new(old)
    }

    /// Release ownership of the page by writing the owner-encoding of `owner`
    /// (clearing the live thread id, preserving flag bits) with a relaxed store.
    /// Used when a thread abandons a page.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field, and the caller
    /// must hold the page in a state where a relaxed store is safe (no
    /// concurrent free-path readers that require Acq/Rel ordering).
    #[inline(always)]
    pub(crate) unsafe fn store_owner(self, owner: PageOwnerId) {
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: the caller provides a valid page
        // `xthread_id` field and page state that permits relaxed owner updates.
        let current = unsafe { self.0.load(Ordering::Relaxed) };
        // `with_owner` preserves the existing flag bits, so no separate
        // `.with_flags(...)` is needed.
        let new = PageThreadId::new(current).with_owner(owner);
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: the new typed word preserves the
        // existing flag bits and writes only the page owner encoding.
        unsafe { self.0.store(new.raw(), Ordering::Relaxed) };
    }

    /// Compatibility shim for callers not yet migrated to the `store_owner`
    /// name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field, and the caller
    /// must hold the page in a state where a relaxed store is safe.
    #[inline(always)]
    pub(crate) unsafe fn set_owner_release(self, owner: PageOwnerId) {
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: compatibility shim forwards to the
        // typed owner store with the same page-state contract.
        unsafe { self.store_owner(owner) };
    }

    /// Release-CAS the page's tagged thread id from `expected` to `new`.
    /// On failure, updates `expected` with the observed typed value.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_owner(
        self,
        expected: &mut PageThreadId,
        new: PageThreadId,
    ) -> bool {
        let mut raw_expected = expected.raw();
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: the caller provides a valid page
        // `xthread_id` field; failure writes the observed raw word back to a typed
        // `PageThreadId`.
        let exchanged = unsafe {
            self.0.compare_exchange_weak(
                &mut raw_expected,
                new.raw(),
                Ordering::Release,
                Ordering::Relaxed,
            )
        };
        if !exchanged {
            *expected = PageThreadId::new(raw_expected);
        }
        exchanged
    }

    /// Compatibility shim for callers not yet migrated to the
    /// `compare_exchange_owner` name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_release(
        self,
        expected: &mut PageThreadId,
        new: PageThreadId,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-page-thread-id]: compatibility shim forwards to the
        // typed owner CAS with the same page-thread-id contract.
        unsafe { self.compare_exchange_owner(expected, new) }
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
