//! Atomic helpers for arena prefix metadata cells.
//!
//! This module owns the arena prefix atomic cells that carry abandoned page
//! counts and heap arena-page table pointers. The storage stays layout
//! transparent, but callers construct the cells through named constructors
//! instead of tuple fields.

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use super::AtomicUsizeField;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicAbandonedCount(*const AtomicUsize);

impl AtomicAbandonedCount {
    #[inline(always)]
    pub(crate) fn from_cell(cell: *const AtomicUsize) -> Self {
        Self(cell)
    }

    #[inline(always)]
    fn field(self) -> AtomicUsizeField {
        AtomicUsizeField::from_raw(self.0.cast_mut().cast::<usize>())
    }

    pub(crate) unsafe fn load_relaxed(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-abandoned-count]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed loading.
        unsafe { self.field().load(Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize {
        // SAFETY[AUDIT:typed-atomic-abandoned-count]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed increment.
        unsafe { self.field().fetch_add(value, Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize {
        // SAFETY[AUDIT:typed-atomic-abandoned-count]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed decrement.
        unsafe { self.field().fetch_sub(value, Ordering::Relaxed) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicArenaPage(*const AtomicPtr<c_void>);

impl AtomicArenaPage {
    #[inline(always)]
    pub(crate) fn from_cell(cell: *const AtomicPtr<c_void>) -> Self {
        Self(cell)
    }

    pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t {
        // SAFETY[AUDIT:typed-atomic-arena-page]: `AtomicArenaPage` points to an
        // allocator-owned page-table pointer atomic and preserves acquire load.
        let atomic = unsafe { &*self.0 };
        atomic
            .load(Ordering::Acquire)
            .cast::<crate::types::mi_arena_pages_t>()
    }

    pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t) {
        // SAFETY[AUDIT:typed-atomic-arena-page]: `AtomicArenaPage` points to an
        // allocator-owned page-table pointer atomic and preserves release store.
        let atomic = unsafe { &*self.0 };
        atomic.store(value.cast::<c_void>(), Ordering::Release);
    }
}
