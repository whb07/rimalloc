//! Atomic helpers for arena prefix metadata cells.
//!
//! This module owns the arena prefix atomic cells that carry abandoned page
//! counts and heap arena-page table pointers. The storage stays layout
//! transparent, but callers construct the cells through named constructors
//! instead of tuple fields.

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicAbandonedCount<'a>(&'a AtomicUsize);

impl<'a> AtomicAbandonedCount<'a> {
    #[inline(always)]
    pub(crate) fn from_cell(cell: &'a AtomicUsize) -> Self {
        Self(cell)
    }

    pub(crate) fn load_relaxed(self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn fetch_add_relaxed(self, value: usize) -> usize {
        self.0.fetch_add(value, Ordering::Relaxed)
    }

    pub(crate) fn fetch_sub_relaxed(self, value: usize) -> usize {
        self.0.fetch_sub(value, Ordering::Relaxed)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicArenaPage<'a>(&'a AtomicPtr<c_void>);

impl<'a> AtomicArenaPage<'a> {
    #[inline(always)]
    pub(crate) fn from_cell(cell: &'a AtomicPtr<c_void>) -> Self {
        Self(cell)
    }

    pub(crate) fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t {
        self.0.load(Ordering::Acquire).cast()
    }

    pub(crate) fn store_release(self, value: *mut crate::types::mi_arena_pages_t) {
        self.0.store(value.cast(), Ordering::Release);
    }
}
