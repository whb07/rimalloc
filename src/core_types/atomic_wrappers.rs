//! DT-13: Atomic field wrappers.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use crate::core_types::{
    BlockPtr, PageFlags, PageOwnerId, PageThreadId, RemoteFreeList, ThreadFreeOwner, ThreadFreeWord,
};

pub(crate) type MemoryOrder = Ordering;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);

impl AtomicUsizeField {
    /// # Safety
    /// `self.0` must be a valid, properly aligned pointer to an atomic-compatible
    /// `usize` field for the lifetime of the access.
    #[inline(always)]
    pub(crate) unsafe fn as_atomic(self) -> &'static AtomicUsize {
        // SAFETY: caller guarantees `self.0` is valid and aligned. The borrow is
        // unbounded ('static) because the field lives as long as the containing
        // page/heap, matching the raw-pointer lifetime model used throughout the
        // allocator.
        unsafe { &*self.0.cast::<AtomicUsize>() }
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.load(order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: usize, order: Ordering) {
        unsafe { self.as_atomic() }.store(value, order);
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn fetch_add(self, value: usize, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.fetch_add(value, order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn fetch_sub(self, value: usize, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.fetch_sub(value, order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn fetch_or(self, bits: usize, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.fetch_or(bits, order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn fetch_and(self, bits: usize, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.fetch_and(bits, order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(
        self,
        expected: &mut usize,
        desired: usize,
    ) -> bool {
        match unsafe { self.as_atomic() }.compare_exchange(
            *expected,
            desired,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_weak(
        self,
        expected: &mut usize,
        desired: usize,
        success: Ordering,
        failure: Ordering,
    ) -> bool {
        match unsafe { self.as_atomic() }
            .compare_exchange_weak(*expected, desired, success, failure)
        {
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
pub(crate) struct AtomicI64Field(pub(crate) *mut i64);

impl AtomicI64Field {
    /// # Safety
    /// `self.0` must be a valid, properly aligned pointer to an atomic-compatible
    /// `i64` field for the lifetime of the access.
    #[inline(always)]
    pub(crate) unsafe fn as_atomic(self) -> &'static AtomicI64 {
        // SAFETY: caller guarantees `self.0` is valid and aligned. The borrow is
        // unbounded for the same raw-pointer lifetime model as `AtomicUsizeField`.
        unsafe { &*self.0.cast::<AtomicI64>() }
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: Ordering) -> i64 {
        unsafe { self.as_atomic() }.load(order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: i64, order: Ordering) {
        unsafe { self.as_atomic() }.store(value, order);
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool {
        match unsafe { self.as_atomic() }.compare_exchange(
            *expected,
            desired,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
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
pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);

impl<T: 'static> AtomicPtrField<T> {
    /// # Safety
    /// `self.0` must be a valid, properly aligned pointer to an atomic-compatible
    /// pointer field for the lifetime of the access.
    #[inline(always)]
    pub(crate) unsafe fn atomic_ref(self) -> &'static AtomicPtr<T> {
        // SAFETY: caller guarantees `self.0` is valid and aligned. The borrow is
        // unbounded for the same raw-pointer lifetime model as `AtomicUsizeField`.
        unsafe { &*self.0.cast::<AtomicPtr<T>>() }
    }

    /// # Safety
    /// See `atomic_ref`.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: Ordering) -> *mut T {
        unsafe { self.atomic_ref() }.load(order)
    }

    /// # Safety
    /// See `atomic_ref`.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: *mut T, order: Ordering) {
        unsafe { self.atomic_ref() }.store(value, order);
    }

    /// # Safety
    /// See `atomic_ref`.
    #[inline(always)]
    pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T {
        unsafe { self.atomic_ref() }.swap(value, Ordering::AcqRel)
    }

    /// # Safety
    /// See `atomic_ref`.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_release(
        self,
        expected: &mut *mut T,
        desired: *mut T,
    ) -> bool {
        match unsafe { self.atomic_ref() }.compare_exchange(
            *expected,
            desired,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    /// # Safety
    /// See `atomic_ref`.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(
        self,
        expected: &mut *mut T,
        desired: *mut T,
    ) -> bool {
        match unsafe { self.atomic_ref() }.compare_exchange(
            *expected,
            desired,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
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
    pub(crate) unsafe fn load_relaxed(self) -> PageThreadId {
        PageThreadId::new(unsafe { self.0.load(Ordering::Relaxed) })
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
    pub(crate) unsafe fn set_owner_release(self, owner: PageOwnerId) {
        let current = unsafe { self.0.load(Ordering::Relaxed) };
        // `with_owner` preserves the existing flag bits, so no separate
        // `.with_flags(...)` is needed.
        let new = PageThreadId::new(current).with_owner(owner);
        unsafe { self.0.store(new.raw(), Ordering::Relaxed) };
    }

    /// Release-CAS the page's tagged thread id from `expected` to `new`.
    /// On failure, updates `expected` with the observed typed value.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_release(
        self,
        expected: &mut PageThreadId,
        new: PageThreadId,
    ) -> bool {
        let atomic = unsafe { self.0.as_atomic() };
        match atomic.compare_exchange_weak(
            expected.raw(),
            new.raw(),
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(value) => {
                *expected = PageThreadId::new(value);
                false
            }
        }
    }
}

pub(crate) enum RemotePushOutcome {
    Success,
    Failed(BlockPtr),
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

    pub(crate) fn compare_exchange_push(
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

    /// Clear the owned bit if `expected` still matches the live thread-free
    /// word, preserving the expected remote-list head. On failure, updates
    /// `expected` with the current typed word for the caller's collect/retry
    /// protocol.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn compare_exchange_release_ownership(
        self,
        expected: &mut ThreadFreeWord,
    ) -> bool {
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
    pub(crate) unsafe fn take_head_preserving_owner(self) -> RemoteFreeList {
        let mut old = ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) });
        loop {
            let head = old.head();
            if head.is_none() {
                return RemoteFreeList(None);
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
                return RemoteFreeList(head);
            }
            old = ThreadFreeWord(raw_old);
        }
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
    pub(crate) unsafe fn claim_ownership(self) -> bool {
        let old = unsafe { self.field().fetch_or(1, Ordering::AcqRel) };
        (old & 1) == 0
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
pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);

#[repr(transparent)]
pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);

pub(crate) struct AtomicGuard<'a> {
    pub(crate) field: AtomicUsizeField,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

#[repr(transparent)]
pub(crate) struct OnceStatePtr(pub(crate) *mut usize);

pub(crate) struct OnceGuard<'a> {
    pub(crate) once: OnceStatePtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

#[repr(transparent)]
pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>);

pub(crate) struct MiLockGuard<'a> {
    pub(crate) lock: MiLockPtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize};

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
    fn atomic_thread_free_claims_and_releases_ownership_bit() {
        let atomic = AtomicUsize::new(0);
        let field = AtomicThreadFree(AtomicUsizeField(
            (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
        ));

        assert!(!field.load_relaxed().is_owned());
        assert!(unsafe { field.claim_ownership() });
        assert!(field.load_relaxed().is_owned());
        assert!(!unsafe { field.claim_ownership() });
        assert!(unsafe { field.release_ownership(None) });
        assert!(!field.load_relaxed().is_owned());
        assert!(!unsafe { field.release_ownership(None) });
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

        let list = unsafe { field.take_head_preserving_owner() };
        assert_eq!(list.0.map(BlockPtr::as_ptr), Some(head.as_ptr()));

        let remaining = field.load_relaxed();
        assert!(remaining.is_owned());
        assert!(remaining.head().is_none());
    }
}
