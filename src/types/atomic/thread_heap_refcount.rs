//! Atomic helper for thread-heap reference counts.
//!
//! This module owns the thread-heap refcount protocol. Callers can construct a
//! refcount cell from the raw metadata field, but cannot bypass the named
//! SeqCst/AcqRel operations by reaching into the backing `AtomicUsizeField`.

use core::sync::atomic::{AtomicUsize, Ordering};

use super::AtomicUsizeField;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadHeapRefcount<'a>(AtomicUsizeField<'a>);

impl<'a> AtomicThreadHeapRefcount<'a> {
    #[inline(always)]
    pub(crate) fn from_cell(field: &'a AtomicUsize) -> Self {
        Self(AtomicUsizeField::from_cell(field))
    }

    pub(crate) unsafe fn store_seq_cst(self, value: usize) {
        self.0.store(value, Ordering::SeqCst)
    }

    pub(crate) unsafe fn incref_acq_rel(self) -> usize {
        self.0.fetch_add(1, Ordering::AcqRel)
    }

    pub(crate) unsafe fn decref_acq_rel(self) -> usize {
        self.0.fetch_sub(1, Ordering::AcqRel)
    }
}
