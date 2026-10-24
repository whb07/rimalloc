//! RAII helpers for allocator atomic guards, once-state, and mimalloc locks.
//!
//! This module owns the acquire/release pairing invariant for guard words,
//! once-state entry, and `mi_lock_t` locks. Callers receive scoped guard values
//! and release happens only through `Drop`.

use core::marker::PhantomData;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

pub(crate) struct AtomicGuard<'a> {
    field: &'a AtomicUsize,
    _thread: PhantomData<*mut ()>,
}

impl<'a> AtomicGuard<'a> {
    #[inline(always)]
    pub(crate) fn try_acquire(field: &'a AtomicUsize) -> Option<Self> {
        if field
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            Some(Self {
                field,
                _thread: PhantomData,
            })
        } else {
            None
        }
    }
}

impl Drop for AtomicGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        self.field.store(0, Ordering::Release);
    }
}

pub(crate) struct OnceGuard<'a> {
    once: &'a crate::atomic::mi_atomic_once_t,
    _thread: PhantomData<*mut ()>,
}

impl<'a> OnceGuard<'a> {
    #[inline(always)]
    pub(crate) fn try_enter(once: &'a crate::atomic::mi_atomic_once_t) -> Option<Self> {
        // SAFETY: the borrow keeps the atomic once-state and its interior-mutable
        // lock live through release on this thread.
        if unsafe { crate::atomic::_mi_atomic_once_enter(ptr::from_ref(once).cast_mut()) } {
            Some(Self {
                once,
                _thread: PhantomData,
            })
        } else {
            None
        }
    }
}

impl Drop for OnceGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY: this thread entered the borrowed once-state and still holds it.
        unsafe { crate::atomic::_mi_atomic_once_release(ptr::from_ref(self.once).cast_mut()) };
    }
}

pub(crate) struct MiLockGuard<'a> {
    lock: &'a crate::atomic::mi_lock_t,
    _thread: PhantomData<*mut ()>,
}

impl<'a> MiLockGuard<'a> {
    #[inline(always)]
    pub(crate) fn acquire(lock: &'a crate::atomic::mi_lock_t) -> Self {
        // SAFETY: the borrow keeps this interior-mutable lock live until Drop.
        unsafe { crate::atomic::mi_lock_acquire(ptr::from_ref(lock).cast_mut()) };
        Self {
            lock,
            _thread: PhantomData,
        }
    }
}

impl Drop for MiLockGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY: this thread acquired the borrowed lock and still holds it.
        unsafe { crate::atomic::mi_lock_release(ptr::from_ref(self.lock).cast_mut()) };
    }
}
