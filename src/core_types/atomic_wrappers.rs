//! DT-13: Atomic field wrappers.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use crate::core_types::{
    BlockPtr, LocalFreeList, OwnedAbandonedPage, PageFlags, PageOwnerId, PageThreadId,
    ThreadFreeOwner, ThreadFreeWord,
};

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
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder) {
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize {
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
        atomic.fetch_add(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize {
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
        atomic.fetch_sub(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_or(self, bits: usize, order: MemoryOrder) -> usize {
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
        atomic.fetch_or(bits, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_and(self, bits: usize, order: MemoryOrder) -> usize {
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
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
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
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
        let atomic = unsafe { &*self.0.cast::<AtomicUsize>() };
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
        let atomic = unsafe { &*self.0.cast::<AtomicI64>() };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder) {
        let atomic = unsafe { &*self.0.cast::<AtomicI64>() };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool {
        let atomic = unsafe { &*self.0.cast::<AtomicI64>() };
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
        let atomic = unsafe { &*self.0.cast::<AtomicPtr<T>>() };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder) {
        let atomic = unsafe { &*self.0.cast::<AtomicPtr<T>>() };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T {
        let atomic = unsafe { &*self.0.cast::<AtomicPtr<T>>() };
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
        let atomic = unsafe { &*self.0.cast::<AtomicPtr<T>>() };
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
        let atomic = unsafe { &*self.0.cast::<AtomicPtr<T>>() };
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
        PageThreadId::new(unsafe { self.0.load(Ordering::Relaxed) })
    }

    /// Compatibility shim for callers not yet migrated to the `load` name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn load_relaxed(self) -> PageThreadId {
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
        let current = unsafe { self.0.load(Ordering::Relaxed) };
        // `with_owner` preserves the existing flag bits, so no separate
        // `.with_flags(...)` is needed.
        let new = PageThreadId::new(current).with_owner(owner);
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
        unsafe { self.compare_exchange_owner(expected, new) }
    }
}

pub(crate) enum RemotePushOutcome {
    Success,
    Failed(BlockPtr),
}

/// The remote free list collected from a thread-free word.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RemoteFreeList(Option<BlockPtr>);

impl RemoteFreeList {
    #[inline(always)]
    pub(crate) fn from_detached_thread_free_head(head: Option<BlockPtr>) -> Self {
        Self(head)
    }

    #[inline(always)]
    pub(crate) fn head(self) -> Option<BlockPtr> {
        self.0
    }

    /// Collect the remote (thread-free) list into the page's local free list.
    ///
    /// Delegates to `mi_page_thread_collect_to_local_typed`, which splices
    /// `self`'s head into `(*page).local_free`, runs the corruption/length
    /// checks, and decrements `(*page).used`. Returns the updated local free
    /// list.
    ///
    /// Constrained to `OwnedAbandonedPage` (the reclaim path); regular-page
    /// collection uses the raw `_mi_page_free_collect` entry point directly.
    ///
    /// # Safety
    /// `page` must be a valid owned abandoned page; `self` must be the head
    /// popped from `(*page).xthread_free` via the ownership CAS protocol.
    #[inline]
    pub(crate) unsafe fn collect_into_local(
        self,
        page: OwnedAbandonedPage,
        local: LocalFreeList,
    ) -> LocalFreeList {
        unsafe { crate::page::mi_page_thread_collect_to_local_typed(page, self, local) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);

impl AtomicThreadFree {
    #[inline(always)]
    fn field(self) -> AtomicUsizeField {
        self.0
    }

    pub(crate) fn load_relaxed(self) -> ThreadFreeWord {
        ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) })
    }

    pub(crate) fn push_remote_acq_rel(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        let new_tf =
            ThreadFreeWord::from_parts(Some(block), crate::core_types::ThreadFreeOwner::Owned);
        let mut expected = old.raw();
        if unsafe {
            self.field().compare_exchange_weak(
                &mut expected,
                new_tf.raw(),
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
        } {
            Ok(new_tf)
        } else {
            *old = ThreadFreeWord::from_parts(
                ThreadFreeWord(expected).head(),
                if ThreadFreeWord(expected).is_owned() {
                    crate::core_types::ThreadFreeOwner::Owned
                } else {
                    crate::core_types::ThreadFreeOwner::Unowned
                },
            );
            Err(*old)
        }
    }

    pub(crate) fn compare_exchange_push(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        self.push_remote_acq_rel(old, block)
    }

    /// Clear the owned bit if `expected` still matches the live thread-free
    /// word, preserving the expected remote-list head. On failure, updates
    /// `expected` with the current typed word for the caller's collect/retry
    /// protocol.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool {
        if !expected.is_owned() {
            return false;
        }
        let new = ThreadFreeWord::from_parts(expected.head(), ThreadFreeOwner::Unowned);
        let mut raw_expected = expected.raw();
        if unsafe {
            self.field().compare_exchange_weak(
                &mut raw_expected,
                new.raw(),
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
        } {
            true
        } else {
            *expected = ThreadFreeWord(raw_expected);
            false
        }
    }

    pub(crate) unsafe fn compare_exchange_release_ownership(
        self,
        expected: &mut ThreadFreeWord,
    ) -> bool {
        unsafe { self.release_acq_rel(expected) }
    }

    /// Atomically detach the current remote free-list head while preserving the
    /// ownership bit in `xthread_free`.
    ///
    /// Mirrors the CAS loop in `mi_page_thread_free_collect`; returning a
    /// `RemoteFreeList` makes the subsequent abandoned-page collection consume
    /// the typed free-list wrapper instead of a raw `mi_block_t*`.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList {
        let mut old = ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) });
        loop {
            let head = old.head();
            if head.is_none() {
                return RemoteFreeList::from_detached_thread_free_head(None);
            }
            let owner = if old.is_owned() {
                ThreadFreeOwner::Owned
            } else {
                ThreadFreeOwner::Unowned
            };
            let new = ThreadFreeWord::from_parts(None, owner);
            let mut raw_old = old.raw();
            if unsafe {
                self.field().compare_exchange_weak(
                    &mut raw_old,
                    new.raw(),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
            } {
                return RemoteFreeList::from_detached_thread_free_head(head);
            }
            old = ThreadFreeWord(raw_old);
        }
    }

    pub(crate) unsafe fn take_head_preserving_owner(self) -> RemoteFreeList {
        unsafe { self.collect_acq_rel() }
    }

    /// Claim ownership of the page's thread-free list by setting the owned bit
    /// (bit 0 of `xthread_free`). Returns `true` if the bit was previously clear
    /// (i.e. this thread successfully claimed ownership).
    ///
    /// Mirrors `mi_page_claim_ownership` in `internal.rs`: a single
    /// `fetch_or(1, AcqRel)`.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn claim_acq_rel(self) -> bool {
        let old = unsafe { self.field().fetch_or(1, Ordering::AcqRel) };
        (old & 1) == 0
    }

    pub(crate) unsafe fn claim_ownership(self) -> bool {
        unsafe { self.claim_acq_rel() }
    }

    /// Release ownership of the page's thread-free list, preserving the current
    /// list head. Loops with a weak CAS clearing the owned bit, retrying if the
    /// head changed concurrently. Returns `true` if the owned bit was cleared
    /// by this call.
    ///
    /// `expected_head`, if provided, is the head the caller believes is current;
    /// the CAS still validates against the live atomic value, so a mismatch just
    /// causes a retry rather than UB.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field, and the caller
    /// must ensure no concurrent collector can leave the list in an inconsistent
    /// state (see `mi_abandoned_page_unown_from_free` for the full protocol).
    #[inline]
    pub(crate) unsafe fn release_ownership(self, _expected_head: Option<BlockPtr>) -> bool {
        loop {
            let current = unsafe { self.field().load(Ordering::Acquire) };
            // Preserve the head pointer (all bits except the owned bit).
            let new = current & !1usize;
            let mut expected = current;
            if unsafe {
                self.field().compare_exchange_weak(
                    &mut expected,
                    new,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
            } {
                return (current & 1) == 1;
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
        unsafe { self.field().load(Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize {
        unsafe { self.field().fetch_add(value, Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize {
        unsafe { self.field().fetch_sub(value, Ordering::Relaxed) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);

impl AtomicThreadHeapRefcount {
    pub(crate) unsafe fn store_seq_cst(self, value: usize) {
        unsafe { self.0.store(value, Ordering::SeqCst) }
    }

    pub(crate) unsafe fn incref_acq_rel(self) -> usize {
        unsafe { self.0.fetch_add(1, Ordering::AcqRel) }
    }

    pub(crate) unsafe fn decref_acq_rel(self) -> usize {
        unsafe { self.0.fetch_sub(1, Ordering::AcqRel) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);

impl AtomicArenaPage {
    pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t {
        let atomic = unsafe { &*self.0 };
        atomic
            .load(Ordering::Acquire)
            .cast::<crate::types::mi_arena_pages_t>()
    }

    pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t) {
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
        // SAFETY: `AtomicGuard` is constructed only after acquiring this guard word.
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
        // SAFETY: `OnceGuard` is constructed only after entering this once-state.
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
        // SAFETY: `MiLockGuard` is constructed only after acquiring this lock.
        unsafe { crate::atomic::mi_lock_release(self.lock.0.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_types::{OwnedAbandonedPage, PagePtr};
    use crate::types::{mi_block_t, mi_page_t};
    use core::{
        mem::MaybeUninit,
        ptr,
        sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize},
    };

    #[test]
    fn atomic_usize_field_updates_stack_atomic_word() {
        let atomic = AtomicUsize::new(4);
        let field = AtomicUsizeField((&atomic as *const AtomicUsize).cast_mut().cast::<usize>());

        assert_eq!(unsafe { field.load(Ordering::Relaxed) }, 4);
        unsafe { field.store(7, Ordering::Relaxed) };
        assert_eq!(atomic.load(Ordering::Relaxed), 7);
        assert_eq!(unsafe { field.fetch_add(5, Ordering::Relaxed) }, 7);
        assert_eq!(unsafe { field.fetch_sub(2, Ordering::Relaxed) }, 12);
        assert_eq!(unsafe { field.fetch_or(0b1000, Ordering::Relaxed) }, 10);
        assert_eq!(unsafe { field.fetch_and(0b1010, Ordering::Relaxed) }, 10);

        let mut expected = 10;
        assert!(unsafe { field.compare_exchange_acq_rel(&mut expected, 42) });
        assert_eq!(unsafe { field.load(Ordering::Relaxed) }, 42);

        let mut stale = 10;
        assert!(!unsafe { field.compare_exchange_acq_rel(&mut stale, 99) });
        assert_eq!(stale, 42);
    }

    #[test]
    fn atomic_usize_field_weak_cas_updates_expected_on_failure() {
        let atomic = AtomicUsize::new(33);
        let field = AtomicUsizeField((&atomic as *const AtomicUsize).cast_mut().cast::<usize>());

        let mut expected = 11;
        assert!(!unsafe {
            field.compare_exchange_weak(&mut expected, 44, Ordering::AcqRel, Ordering::Acquire)
        });
        assert_eq!(expected, 33);
        assert_eq!(atomic.load(Ordering::Acquire), 33);

        assert!(unsafe {
            field.compare_exchange_weak(&mut expected, 44, Ordering::AcqRel, Ordering::Acquire)
        });
        assert_eq!(atomic.load(Ordering::Acquire), 44);
    }

    #[test]
    fn atomic_i64_field_updates_expected_on_cas_failure() {
        let atomic = AtomicI64::new(-4);
        let field = AtomicI64Field((&atomic as *const AtomicI64).cast_mut().cast::<i64>());

        assert_eq!(unsafe { field.load(Ordering::Relaxed) }, -4);
        unsafe { field.store(11, Ordering::Relaxed) };

        let mut expected = 9;
        assert!(!unsafe { field.compare_exchange_acq_rel(&mut expected, 12) });
        assert_eq!(expected, 11);
        assert_eq!(atomic.load(Ordering::Relaxed), 11);

        assert!(unsafe { field.compare_exchange_acq_rel(&mut expected, 12) });
        assert_eq!(atomic.load(Ordering::Relaxed), 12);
    }

    #[test]
    fn atomic_ptr_field_round_trips_pointer_values() {
        let mut first = 1u8;
        let mut second = 2u8;
        let first_ptr = (&mut first as *mut u8).cast::<c_void>();
        let second_ptr = (&mut second as *mut u8).cast::<c_void>();
        let atomic = AtomicPtr::new(first_ptr);
        let field =
            || AtomicPtrField::<c_void>((&atomic as *const AtomicPtr<c_void>).cast_mut().cast());

        assert_eq!(unsafe { field().load(Ordering::Acquire) }, first_ptr);
        assert_eq!(unsafe { field().exchange_acq_rel(second_ptr) }, first_ptr);
        assert_eq!(atomic.load(Ordering::Acquire), second_ptr);

        let mut expected = first_ptr;
        assert!(!unsafe { field().compare_exchange_acq_rel(&mut expected, first_ptr) });
        assert_eq!(expected, second_ptr);
        assert!(unsafe { field().compare_exchange_release(&mut expected, first_ptr) });
        assert_eq!(atomic.load(Ordering::Acquire), first_ptr);
    }

    #[test]
    fn atomic_arena_page_loads_and_stores_page_table_pointer() {
        let mut pages = MaybeUninit::<crate::types::mi_arena_pages_t>::zeroed();
        let pages_ptr = pages.as_mut_ptr();
        let atomic = AtomicPtr::new(ptr::null_mut::<c_void>());
        let field = AtomicArenaPage(&atomic);

        assert_eq!(unsafe { field.load_acquire() }, ptr::null_mut());
        unsafe { field.store_release(pages_ptr) };
        assert_eq!(unsafe { field.load_acquire() }, pages_ptr);
        assert_eq!(atomic.load(Ordering::Acquire), pages_ptr.cast::<c_void>());
    }

    #[test]
    fn atomic_page_thread_id_updates_owner_and_flags() {
        let initial =
            PageThreadId::from_parts(PageOwnerId::Abandoned, PageFlags::HAS_INTERIOR_POINTERS);
        let atomic = AtomicUsize::new(initial.raw());
        let field = AtomicPageThreadId(AtomicUsizeField(
            (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
        ));

        assert!(unsafe {
            field
                .load()
                .flags()
                .contains(PageFlags::HAS_INTERIOR_POINTERS)
        });

        let old = unsafe { field.set_flags(PageFlags::IN_FULL_QUEUE, true) };
        assert!(!old.flags().contains(PageFlags::IN_FULL_QUEUE));
        assert!(unsafe { field.load().flags().contains(PageFlags::IN_FULL_QUEUE) });

        unsafe { field.store_owner(PageOwnerId::AbandonedMapped) };
        let stored = unsafe { field.load() };
        assert!(stored.is_abandoned_mapped());
        assert!(stored.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));
        assert!(stored.flags().contains(PageFlags::IN_FULL_QUEUE));

        let desired = PageThreadId::from_parts(PageOwnerId::Abandoned, stored.flags());
        let mut expected = PageThreadId::new(0);
        assert!(!unsafe { field.compare_exchange_owner(&mut expected, desired) });
        assert_eq!(expected.raw(), stored.raw());

        assert!(unsafe { field.compare_exchange_owner(&mut expected, desired) });
        assert!(unsafe { field.load().is_abandoned() });
    }

    #[test]
    fn atomic_thread_free_claims_and_releases_ownership_bit() {
        let atomic = AtomicUsize::new(0);
        let field = AtomicThreadFree(AtomicUsizeField(
            (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
        ));

        assert!(!field.load_relaxed().is_owned());
        assert!(unsafe { field.claim_acq_rel() });
        assert!(field.load_relaxed().is_owned());
        assert!(!unsafe { field.claim_acq_rel() });
        let mut expected = field.load_relaxed();
        assert!(unsafe { field.release_acq_rel(&mut expected) });
        assert!(!field.load_relaxed().is_owned());
        let mut expected = field.load_relaxed();
        assert!(!unsafe { field.release_acq_rel(&mut expected) });
    }

    #[test]
    fn atomic_thread_free_take_head_preserves_owner_bit() {
        let mut block = crate::types::mi_block_t::default();
        let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
        let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
        let atomic = AtomicUsize::new(word.raw());
        let field = AtomicThreadFree(AtomicUsizeField(
            (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
        ));

        let list = unsafe { field.collect_acq_rel() };
        assert_eq!(list.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));

        let remaining = field.load_relaxed();
        assert!(remaining.is_owned());
        assert!(remaining.head().is_none());
    }

    #[test]
    fn empty_remote_free_list_collects_through_detached_atomic_path() {
        let mut page = MaybeUninit::<mi_page_t>::uninit();
        let mut local = mi_block_t::default();
        unsafe {
            let raw_page = page.as_mut_ptr();
            ptr::addr_of_mut!((*raw_page).xthread_free)
                .write(ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned).raw());
            ptr::addr_of_mut!((*raw_page).local_free).write(ptr::addr_of_mut!(local));

            let page_ptr = PagePtr::new_unchecked(raw_page);
            let atomic = AtomicThreadFree(AtomicUsizeField(ptr::addr_of_mut!(
                (*raw_page).xthread_free
            )));
            let remote = atomic.collect_acq_rel();
            assert!(remote.head().is_none());

            let local_before = LocalFreeList::from_raw((*raw_page).local_free);
            let local_after = remote
                .collect_into_local(OwnedAbandonedPage::new_unchecked(page_ptr), local_before);
            assert_eq!(
                local_after.0.map(BlockPtr::as_ptr),
                Some(ptr::addr_of_mut!(local))
            );
        }
    }

    #[test]
    fn atomic_abandoned_count_uses_relaxed_cell_operations() {
        let atomic = AtomicUsize::new(2);
        let count = AtomicAbandonedCount(&atomic);

        assert_eq!(unsafe { count.load_relaxed() }, 2);
        assert_eq!(unsafe { count.fetch_add_relaxed(1) }, 2);
        assert_eq!(unsafe { count.load_relaxed() }, 3);
        assert_eq!(unsafe { count.fetch_sub_relaxed(2) }, 3);
        assert_eq!(unsafe { count.load_relaxed() }, 1);
    }

    #[test]
    fn atomic_thread_heap_refcount_uses_acq_rel_operations() {
        let atomic = AtomicUsize::new(0);
        let refcount = AtomicThreadHeapRefcount(AtomicUsizeField(
            (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
        ));

        unsafe { refcount.store_seq_cst(1) };
        assert_eq!(atomic.load(Ordering::SeqCst), 1);
        assert_eq!(unsafe { refcount.incref_acq_rel() }, 1);
        assert_eq!(unsafe { refcount.decref_acq_rel() }, 2);
        assert_eq!(atomic.load(Ordering::Acquire), 1);
    }

    #[test]
    fn atomic_guard_acquires_once_and_releases_on_drop() {
        let atomic = AtomicUsize::new(0);
        let field = AtomicUsizeField((&atomic as *const AtomicUsize).cast_mut().cast::<usize>());

        let guard = unsafe { AtomicGuard::try_acquire(field) }.expect("first acquire succeeds");
        assert_eq!(atomic.load(Ordering::Acquire), 1);
        assert!(unsafe { AtomicGuard::try_acquire(field) }.is_none());

        drop(guard);
        assert_eq!(atomic.load(Ordering::Acquire), 0);
        assert!(unsafe { AtomicGuard::try_acquire(field) }.is_some());
    }

    #[test]
    fn once_guard_enters_once_and_releases_on_drop() {
        let mut once = crate::atomic::mi_atomic_once_t::new();
        let state = OnceStatePtr(ptr::addr_of_mut!(once).cast::<usize>());

        let guard = unsafe { OnceGuard::try_enter(state) }.expect("first enter succeeds");
        assert!(once.tid.load(Ordering::Acquire) > 1);
        assert!(unsafe { OnceGuard::try_enter(state) }.is_none());

        drop(guard);
        assert_eq!(once.tid.load(Ordering::Acquire), 1);
        assert!(unsafe { OnceGuard::try_enter(state) }.is_none());
    }

    #[test]
    fn mi_lock_guard_acquires_and_releases_on_drop() {
        let mut lock = crate::atomic::mi_lock_t::new();
        let lock_ptr = MiLockPtr(unsafe { NonNull::new_unchecked(ptr::addr_of_mut!(lock)) });

        let guard = unsafe { MiLockGuard::acquire(lock_ptr) };
        assert!(!unsafe { crate::atomic::mi_lock_try_acquire(lock_ptr.0.as_ptr()) });

        drop(guard);
        assert!(unsafe { crate::atomic::mi_lock_try_acquire(lock_ptr.0.as_ptr()) });
        unsafe { crate::atomic::mi_lock_release(lock_ptr.0.as_ptr()) };
    }
}
