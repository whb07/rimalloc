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
    field: AtomicUsizeField,
    _borrow: PhantomData<&'a ()>,
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
        unsafe { self.field.store(0, Ordering::Release) };
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct OnceStatePtr(*mut usize);

impl OnceStatePtr {
    #[inline(always)]
    pub(crate) fn from_raw(state: *mut usize) -> Self {
        Self(state)
    }

    #[inline(always)]
    fn as_once_ptr(self) -> *mut crate::atomic::mi_atomic_once_t {
        self.0.cast::<crate::atomic::mi_atomic_once_t>()
    }
}

pub(crate) struct OnceGuard<'a> {
    once: OnceStatePtr,
    _borrow: PhantomData<&'a ()>,
}

impl<'a> OnceGuard<'a> {
    /// # Safety
    /// `once` must refer to a valid allocator once-state.
    #[inline(always)]
    pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self> {
        if unsafe { crate::atomic::_mi_atomic_once_enter(once.as_once_ptr()) } {
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
        unsafe { crate::atomic::_mi_atomic_once_release(self.once.as_once_ptr()) };
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MiLockPtr(NonNull<crate::types::mi_lock_t_pub>);

impl MiLockPtr {
    #[inline(always)]
    pub(crate) fn from_nonnull(lock: NonNull<crate::types::mi_lock_t_pub>) -> Self {
        Self(lock)
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *mut crate::types::mi_lock_t_pub {
        self.0.as_ptr()
    }
}

pub(crate) struct MiLockGuard<'a> {
    lock: MiLockPtr,
    _borrow: PhantomData<&'a ()>,
}

impl<'a> MiLockGuard<'a> {
    /// # Safety
    /// `lock` must refer to a valid allocator lock.
    #[inline(always)]
    pub(crate) unsafe fn acquire(lock: MiLockPtr) -> Self {
        unsafe { crate::atomic::mi_lock_acquire(lock.as_ptr()) };
        Self {
            lock,
            _borrow: PhantomData,
        }
    }
}

impl Drop for MiLockGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { crate::atomic::mi_lock_release(self.lock.as_ptr()) };
    }
}
