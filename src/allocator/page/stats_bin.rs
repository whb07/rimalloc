use crate::allocator::stats::MI_BIN_HUGE;
use crate::types::{MI_BIN_FULL, mi_page_t};

use super::{mi_bin_from_block_size, mi_page_block_size, mi_page_is_huge, mi_page_is_in_full};

/// Classify a page into its owning thread-heap queue bin.
///
/// # Safety
/// `page` must be nonnull and point to live initialized allocator page
/// metadata. Its full flag and the geometry, MemId, reserved count, and block
/// size used by huge/normal classification must remain readable and stable for
/// the reads reached by the full-then-huge short circuit. A normal page's block
/// size must be nonzero. Page teardown, classification/geometry mutation, and
/// incompatible diagnostic callback reentry are excluded; any diagnostic
/// callback and opaque argument reached by an assertion must remain valid and
/// must not unwind.
#[inline]
pub(crate) unsafe fn mi_page_bin_typed(page: *const mi_page_t) -> crate::core_types::Bin {
    // SAFETY: the caller keeps this live page's initialized full flag stable for the first
    // classification read, which intentionally precedes all huge-page metadata reads.
    let bin = if unsafe { mi_page_is_in_full(page) } {
        MI_BIN_FULL
    } else if unsafe {
        // SAFETY: after the full-page fast path, the caller keeps the page's initialized geometry,
        // MemId, reserved count, and block size live and stable for huge classification.
        mi_page_is_huge(page)
    } {
        MI_BIN_HUGE
    } else {
        // SAFETY: the remaining normal page has a live initialized nonzero block size stable for
        // this read and unchecked typed materialization.
        let block_size =
            unsafe { crate::core_types::BlockSize::new_unchecked(mi_page_block_size(page)) };
        mi_bin_from_block_size(block_size).as_usize()
    };
    crate::internal::mi_assert_internal!(bin <= MI_BIN_FULL);
    crate::internal::mi_debug_assert!(u16::try_from(bin).is_ok());
    // SAFETY: `MI_BIN_FULL` is the last valid bin, and the assertion above proves `bin` fits the
    // fixed queue table and the `u16` representation.
    unsafe { crate::core_types::Bin::new_unchecked(bin as u16) }
}

/// # Safety
/// `page` must satisfy `mi_page_bin_typed`; this raw compatibility projection
/// returns only the classified bin index and does not extend the page lifetime.
#[inline]
pub unsafe fn mi_page_bin(page: *const mi_page_t) -> usize {
    // SAFETY: the raw caller provides the live stable page classification required by the typed
    // projection, whose bounded result is returned unchanged.
    unsafe { mi_page_bin_typed(page).as_usize() }
}

#[inline]
pub(crate) unsafe fn _mi_page_stats_bin_typed(
    page: *const mi_page_t,
) -> crate::core_types::StatsBin {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let bin = if mi_page_is_huge(page) {
            MI_BIN_HUGE
        } else {
            mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(
                mi_page_block_size(page),
            ))
            .as_usize()
        };
        crate::internal::mi_assert_internal!(bin <= MI_BIN_HUGE);
        crate::internal::mi_debug_assert!(u16::try_from(bin).is_ok());
        // SAFETY: the assertion above proves `bin <= MI_BIN_HUGE`.
        crate::core_types::StatsBin::new_unchecked(bin as u16)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
// SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_stats_bin(page: *const mi_page_t) -> usize {
    // SAFETY: page is live page metadata; stats bin projection reads typed scalar domains without aliasing.
    unsafe {
        let bin = _mi_page_stats_bin_typed(page).as_usize();
        crate::internal::mi_assert_internal!(bin <= MI_BIN_HUGE);
        bin
    }
}
