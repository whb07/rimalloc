//! Atomic helpers for the page `xthread_id` tagged owner word.
//!
//! This module owns the page thread-id word protocol: relaxed flag updates,
//! owner stores, and release CAS transitions over `PageThreadId`.

use core::sync::atomic::Ordering;

use super::AtomicUsizeField;
use crate::core_types::{PageFlags, PageOwnerId, PageThreadId};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicPageThreadId(AtomicUsizeField);

impl AtomicPageThreadId {
    #[inline(always)]
    pub(crate) fn from_field(field: AtomicUsizeField) -> Self {
        Self(field)
    }

    /// Load the page's tagged thread-id word (thread id + flag bits) as a typed
    /// `PageThreadId`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn load(self) -> PageThreadId {
        PageThreadId::new(unsafe { self.0.load(Ordering::Relaxed) })
    }

    /// Set or clear the given flag bits in the page's `xthread_id` field using a
    /// relaxed `fetch_or` / `fetch_and`. Returns the previous `PageThreadId`.
    ///
    /// Mirrors `mi_page_flags_set`: a single relaxed atomic RMW on `xthread_id`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
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
    #[inline(always)]
    pub(crate) unsafe fn store_owner(self, owner: PageOwnerId) {
        let current = unsafe { self.0.load(Ordering::Relaxed) };
        // `with_owner` preserves the existing flag bits, so no separate
        // `.with_flags(...)` is needed.
        let new = PageThreadId::new(current).with_owner(owner);
        unsafe { self.0.store(new.raw(), Ordering::Relaxed) };
    }

    /// Release-CAS the page's tagged thread id from `expected` to `new`.
    /// On failure, updates `expected` with the observed typed value.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_owner(
        self,
        expected: &mut PageThreadId,
        new: PageThreadId,
    ) -> bool {
        let mut raw_expected = expected.raw();
        let exchanged = unsafe {
            self.0.compare_exchange_weak(
                &mut raw_expected,
                new.raw(),
                Ordering::Release,
                Ordering::Relaxed,
            )
        };
        if !exchanged {
            *expected = PageThreadId::new(raw_expected);
        }
        exchanged
    }
}
