use core::ffi::c_void;

use crate::allocator::arena::MI_ARENA_SLICE_ALIGN;
use crate::types::mi_page_t;

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_start(page: *const mi_page_t) -> *mut u8 {
    unsafe { (*page).page_start }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool {
    unsafe {
        matches!(
            crate::core_types::MemId::view((*page).memid),
            crate::core_types::MemIdView::Arena(_)
        )
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_meta_is_separated(page: *const mi_page_t) -> bool {
    unsafe {
        if super::MI_PAGE_META_IS_SEPARATED != 0 {
            mi_page_is_in_arena(page)
                && page.cast_mut().cast::<c_void>()
                    != super::_mi_align_down_ptr((*page).page_start.cast(), MI_ARENA_SLICE_ALIGN)
        } else {
            false
        }
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_slice_start(page: *const mi_page_t) -> *mut u8 {
    unsafe {
        if mi_page_meta_is_separated(page) {
            super::_mi_align_down_ptr((*page).page_start.cast(), MI_ARENA_SLICE_ALIGN).cast::<u8>()
        } else {
            page.cast_mut().cast::<u8>()
        }
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_slice_offset_of(
    page: *const mi_page_t,
    offset_relative_to_page_start: usize,
) -> usize {
    unsafe {
        ((*page).page_start as usize - mi_page_slice_start(page) as usize)
            + offset_relative_to_page_start
    }
}
