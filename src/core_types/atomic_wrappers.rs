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
pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);

#[repr(transparent)]
pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);

impl AtomicPageThreadId {
    pub(crate) fn load_relaxed(self) -> PageThreadId {
        unimplemented!()
    }

    pub(crate) fn set_flags(self, _flags: PageFlags, _set: bool) -> PageThreadId {
        unimplemented!()
    }

    pub(crate) fn set_owner_release(self, _owner: PageOwnerId) {
        unimplemented!()
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
