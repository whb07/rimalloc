use core::ffi::c_void;

use crate::allocator::arena::MI_ARENA_SLICE_ALIGN;
use crate::types::mi_page_t;

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_start(page: *const mi_page_t) -> *mut u8 {
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
    unsafe { (*page).page_start }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator. If non-null, `size` must be valid
/// for a `usize` write.
#[inline]
pub unsafe fn mi_page_area(page: *const mi_page_t, size: *mut usize) -> *mut u8 {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        if !size.is_null() {
            *size = super::page_metadata::mi_page_size(page);
        }
        mi_page_start(page)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_contains_address(page: *const mi_page_t, p: *const c_void) -> bool {
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
    unsafe {
        let mut psize = 0usize;
        let start = mi_page_area(page, &mut psize);
        start <= p.cast_mut().cast::<u8>() && p.cast::<u8>() < start.add(psize)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool {
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
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
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
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
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
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
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
    unsafe {
        ((*page).page_start as usize - mi_page_slice_start(page) as usize)
            + offset_relative_to_page_start
    }
}
