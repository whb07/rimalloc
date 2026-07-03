//! RAII helpers for allocator atomic guards, once-state, and mimalloc locks.
//!
//! This module owns the acquire/release pairing invariant for guard words,
//! once-state entry, and `mi_lock_t` locks. Callers receive scoped guard values
//! and release happens only through `Drop`.

use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::Ordering;

use super::AtomicUsizeField;

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
