//! Borrowed atomic views over allocator metadata fields.
//!
//! Raw metadata is borrowed at the caller's validity boundary. These views
//! cannot outlive that borrow, and primitive atomic operations are safe.

use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use super::MemoryOrder;

/// Atomic access for the lifetime of a borrowed `AtomicUsize`.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicUsizeField<'a>(&'a AtomicUsize);

impl<'a> AtomicUsizeField<'a> {
    #[inline(always)]
    pub(crate) fn from_cell(field: &'a AtomicUsize) -> Self {
        Self(field)
    }

    #[inline(always)]
    pub(crate) fn load(self, order: MemoryOrder) -> usize {
        self.0.load(order)
    }

    #[inline(always)]
    pub(crate) fn store(self, value: usize, order: MemoryOrder) {
        self.0.store(value, order);
    }

    #[inline(always)]
    pub(crate) fn fetch_add(self, value: usize, order: MemoryOrder) -> usize {
        self.0.fetch_add(value, order)
    }

    #[inline(always)]
    pub(crate) fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize {
        self.0.fetch_sub(value, order)
    }

    #[inline(always)]
    pub(crate) fn fetch_or(self, bits: usize, order: MemoryOrder) -> usize {
        self.0.fetch_or(bits, order)
    }

    #[inline(always)]
    pub(crate) fn fetch_and(self, bits: usize, order: MemoryOrder) -> usize {
        self.0.fetch_and(bits, order)
    }

    #[inline(always)]
    pub(crate) fn compare_exchange_acq_rel(self, expected: &mut usize, desired: usize) -> bool {
        match self
            .0
            .compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    #[inline(always)]
    pub(crate) fn compare_exchange_weak(
        self,
        expected: &mut usize,
        desired: usize,
        success: MemoryOrder,
        failure: MemoryOrder,
    ) -> bool {
        match self
            .0
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

/// Atomic access for the lifetime of a borrowed `AtomicI64`.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicI64Field<'a>(&'a AtomicI64);

impl<'a> AtomicI64Field<'a> {
    #[inline(always)]
    pub(crate) fn from_cell(field: &'a AtomicI64) -> Self {
        Self(field)
    }

    #[inline(always)]
    pub(crate) fn load(self, order: MemoryOrder) -> i64 {
        self.0.load(order)
    }

    #[inline(always)]
    pub(crate) fn store(self, value: i64, order: MemoryOrder) {
        self.0.store(value, order);
    }

    #[inline(always)]
    pub(crate) fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool {
        match self
            .0
            .compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

/// Atomic access for the lifetime of a borrowed `AtomicPtr`.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicPtrField<'a, T>(&'a AtomicPtr<T>);

impl<'a, T> AtomicPtrField<'a, T> {
    #[inline(always)]
    pub(crate) fn from_cell(field: &'a AtomicPtr<T>) -> Self {
        Self(field)
    }

    #[inline(always)]
    pub(crate) fn load(self, order: MemoryOrder) -> *mut T {
        self.0.load(order)
    }

    #[inline(always)]
    pub(crate) fn store(self, value: *mut T, order: MemoryOrder) {
        self.0.store(value, order);
    }

    #[inline(always)]
    pub(crate) fn exchange_acq_rel(self, value: *mut T) -> *mut T {
        self.0.swap(value, Ordering::AcqRel)
    }

    #[inline(always)]
    pub(crate) fn compare_exchange_release(self, expected: &mut *mut T, desired: *mut T) -> bool {
        match self
            .0
            .compare_exchange(*expected, desired, Ordering::Release, Ordering::Relaxed)
        {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    #[inline(always)]
    pub(crate) fn compare_exchange_acq_rel(self, expected: &mut *mut T, desired: *mut T) -> bool {
        match self
            .0
            .compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}
