use crate::stats::MI_BIN_HUGE;
use crate::types::{MI_BIN_FULL, mi_page_t};

use super::{mi_bin_from_block_size, mi_page_block_size, mi_page_is_huge, mi_page_is_in_full};

#[inline]
pub(crate) unsafe fn mi_page_bin_typed(page: *const mi_page_t) -> crate::core_types::Bin {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let bin = if mi_page_is_in_full(page) {
            MI_BIN_FULL
        } else if mi_page_is_huge(page) {
            MI_BIN_HUGE
        } else {
            mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(
                mi_page_block_size(page),
            ))
            .as_usize()
        };
        crate::internal::mi_assert_internal!(bin <= MI_BIN_FULL);
        crate::internal::mi_debug_assert!(u16::try_from(bin).is_ok());
        // SAFETY: `MI_BIN_FULL` is the last valid bin, and the assertion above
        // proves `bin < MI_BIN_COUNT`.
        crate::core_types::Bin::new_unchecked(bin as u16)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_bin(page: *const mi_page_t) -> usize {
    // SAFETY: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
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
    unsafe {
        let bin = _mi_page_stats_bin_typed(page).as_usize();
        crate::internal::mi_assert_internal!(bin <= MI_BIN_HUGE);
        bin
    }
}
