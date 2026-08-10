//! Primitive atomic views over raw allocator metadata fields.
//!
//! These wrappers own the pointer-to-atomic cast boundary. Callers can name the
//! raw field they want to access, but the backing pointer representation stays
//! private and all actual atomic operations remain behind this module.

use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use super::MemoryOrder;

/// Atomic access to a raw `usize` field.
///
/// # Safety
/// The pointer must be non-null, properly aligned for `AtomicUsize`, and valid
/// for the duration of each operation. While this wrapper is used, all
/// concurrent accesses to the same storage must be atomic and use compatible
/// aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicUsizeField(*mut usize);

impl AtomicUsizeField {
    #[inline(always)]
    pub(crate) fn from_raw(field: *mut usize) -> Self {
        Self(field)
    }

    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn raw_field(self) -> *mut usize {
        self.0
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize {
        // SAFETY: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder) {
        // SAFETY: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize {
        // SAFETY: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_add(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize {
        // SAFETY: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_sub(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_or(self, bits: usize, order: MemoryOrder) -> usize {
        // SAFETY: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_or(bits, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_and(self, bits: usize, order: MemoryOrder) -> usize {
        // SAFETY: callers uphold this wrapper's
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
        // SAFETY: callers uphold this wrapper's
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
        // SAFETY: callers uphold this wrapper's
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
pub(crate) struct AtomicI64Field(*mut i64);

impl AtomicI64Field {
    #[inline(always)]
    pub(crate) fn from_raw(field: *mut i64) -> Self {
        Self(field)
    }

    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn raw_field(self) -> *mut i64 {
        self.0
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64 {
        // SAFETY: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder) {
        // SAFETY: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool {
        // SAFETY: callers uphold this wrapper's
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
pub(crate) struct AtomicPtrField<T>(*mut *mut T);

impl<T> AtomicPtrField<T> {
    #[inline(always)]
    pub(crate) fn from_raw(field: *mut *mut T) -> Self {
        Self(field)
    }

    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn raw_field(self) -> *mut *mut T {
        self.0
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T {
        // SAFETY: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder) {
        // SAFETY: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T {
        // SAFETY: callers uphold this wrapper's
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
        // SAFETY: callers uphold this wrapper's
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
        // SAFETY: callers uphold this wrapper's
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
