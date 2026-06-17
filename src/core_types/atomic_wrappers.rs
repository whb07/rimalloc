//! DT-13: Atomic field wrappers.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::core_types::{
    BlockPtr, PageFlags, PageOwnerId, PageThreadId, RemoteFreeList, ThreadFreeWord,
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
    pub(crate) unsafe fn fetch_or(self, bits: usize, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.fetch_or(bits, order)
    }

    /// # Safety
    /// See `as_atomic`.
    #[inline(always)]
    pub(crate) unsafe fn fetch_and(self, bits: usize, order: Ordering) -> usize {
        unsafe { self.as_atomic() }.fetch_and(bits, order)
    }
}

#[repr(transparent)]
pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);

impl AtomicPageThreadId {
    /// Load the page's tagged thread-id word (thread id + flag bits) as a typed
    /// `PageThreadId`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline]
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
    #[inline]
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
    #[inline]
    pub(crate) unsafe fn set_owner_release(self, owner: PageOwnerId) {
        let current = unsafe { self.0.load(Ordering::Relaxed) };
        // `with_owner` preserves the existing flag bits, so no separate
        // `.with_flags(...)` is needed.
        let new = PageThreadId::new(current).with_owner(owner);
        unsafe { self.0.store(new.raw(), Ordering::Relaxed) };
    }
}

pub(crate) enum RemotePushOutcome {
    Success,
    Failed(BlockPtr),
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadFree(pub(crate) *mut usize);

impl AtomicThreadFree {
    pub(crate) fn load_relaxed(self) -> ThreadFreeWord {
        let val =
            unsafe { core::sync::atomic::AtomicUsize::from_ptr(self.0).load(Ordering::Relaxed) };
        ThreadFreeWord(val)
    }

    pub(crate) fn compare_exchange_push(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        let new_tf =
            ThreadFreeWord::from_parts(Some(block), crate::core_types::ThreadFreeOwner::Owned);
        let atomic = unsafe { &*self.0.cast::<core::sync::atomic::AtomicUsize>() };
        match atomic.compare_exchange_weak(
            old.raw(),
            new_tf.raw(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(new_tf),
            Err(val) => {
                *old = ThreadFreeWord::from_parts(
                    ThreadFreeWord(val).head(),
                    if ThreadFreeWord(val).is_owned() {
                        crate::core_types::ThreadFreeOwner::Owned
                    } else {
                        crate::core_types::ThreadFreeOwner::Unowned
                    },
                );
                Err(*old)
            }
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
        let old = crate::atomic::mi_atomic_fetch_or_explicit(
            self.0,
            1,
            crate::atomic::mi_memory_order_e::mi_memory_order_acq_rel,
        );
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
        let atomic = unsafe { &*self.0.cast::<core::sync::atomic::AtomicUsize>() };
        loop {
            let current = atomic.load(Ordering::Acquire);
            // Preserve the head pointer (all bits except the owned bit).
            let new = current & !1usize;
            match atomic.compare_exchange_weak(current, new, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => return (current & 1) == 1,
                Err(_) => continue,
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
