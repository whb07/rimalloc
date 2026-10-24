//! Page-map pointer lookup helpers.

use super::*;

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    unsafe { _mi_safe_ptr_page_registered(p) }
        .map(|registered| registered.page().as_ptr())
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
#[inline(always)]
pub(crate) unsafe fn _mi_safe_ptr_page_registered(p: *const c_void) -> Option<RegisteredPageRange> {
    if p >= unsafe { mi_page_map_max_address } {
        return None;
    }
    let location = PageMapLocation::from_ptr(p);
    let idx = location.index();
    if unsafe { !mi_page_map_commit.is_null() }
        && !unsafe {
            mi_bitmap_is_set(
                mi_page_map_commit,
                idx.get() / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT,
            )
        }
    {
        return None;
    }
    let ofs = unsafe { *_mi_page_map.add(idx.get()) as usize };
    if ofs == 0 {
        return None;
    }
    let page = (((crate::core_types::ptr_exposed_addr(p) >> MI_ARENA_SLICE_SHIFT) - ofs + 1)
        << MI_ARENA_SLICE_SHIFT) as *mut mi_page_t;
    let page = PagePtr::new(page)?;
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    let slice_count = SliceCount::ONE;
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    unsafe { _mi_safe_ptr_page_registered(p) }
        .map(|registered| registered.page().as_ptr())
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
#[inline(always)]
pub(crate) unsafe fn _mi_safe_ptr_page_registered(p: *const c_void) -> Option<RegisteredPageRange> {
    if p.is_null() {
        return None;
    }
    if p >= unsafe { mi_page_map_max_address } {
        return None;
    }
    let Some(location) = PageMapLocation::from_ptr(p) else {
        crate::internal::mi_assert_internal!(false);
        return None;
    };
    let idx = location.index();
    let sub_idx = location.sub_index();
    if !mi_page_map_commit_status(idx).is_committed() {
        return None;
    }
    let sub = unsafe { _mi_page_map_at(idx) };
    if sub.is_null() {
        return None;
    }
    let page = PagePtr::new(unsafe { *sub.add(sub_idx.get()) })?;
    let slice_count = SliceCount::ONE;
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}
