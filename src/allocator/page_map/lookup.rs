//! Page-map pointer lookup helpers.

use super::*;

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
// SAFETY: safe pointer lookup accepts arbitrary input
// and converts a registered range witness back to the page pointer.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    // SAFETY: registered lookup handles null/invalid
    // pointers and returns a witness only for initialized page-map entries.
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
    // SAFETY: max address is initialized with the
    // page-map globals and bounds arbitrary input before indexing.
    if p >= unsafe { mi_page_map_max_address } {
        return None;
    }
    let location = PageMapLocation::from_ptr(p);
    let idx = location.index();
    // SAFETY: optional bitmap is initialized with the
    // byte-map and is checked before reading uncommitted page-map bytes.
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
    // SAFETY: committed byte-map index was validated
    // and optional bitmap confirmed before reading the offset byte.
    let ofs = unsafe { *_mi_page_map.add(idx.get()) as usize };
    if ofs == 0 {
        return None;
    }
    let page = (((crate::core_types::ptr_exposed_addr(p) >> MI_ARENA_SLICE_SHIFT) - ofs + 1)
        << MI_ARENA_SLICE_SHIFT) as *mut mi_page_t;
    let page = PagePtr::new(page)?;
    // SAFETY: 32-bit lookup uses a zero sub-index and
    // one-slice witness for the containing page registration.
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    let slice_count = unsafe { SliceCount::from_usize_const(1).unwrap_unchecked() };
    // SAFETY: nonzero byte-map entry proves the page is
    // registered for this index/sub-index witness.
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
// SAFETY: exported heap-region query forwards arbitrary
// input through the safe page-map lookup boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    // SAFETY: `_mi_safe_ptr_page` handles arbitrary
    // nullable input and returns null for non-registered pointers.
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY: safe pointer lookup accepts arbitrary input
// and converts a registered range witness back to the page pointer.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    // SAFETY: registered lookup handles null/invalid
    // pointers and returns a witness only for initialized page-map entries.
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
    // SAFETY: max address is initialized with the
    // page-map globals and bounds arbitrary input before indexing.
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
    // SAFETY: committed top-level index can be loaded
    // through the audited page-map accessor.
    let sub = unsafe { _mi_page_map_at(idx) };
    if sub.is_null() {
        return None;
    }
    // SAFETY: typed sub-index bounds the submap lookup
    // and `PagePtr::new` rejects null entries.
    let page = PagePtr::new(unsafe { *sub.add(sub_idx.get()) })?;
    // SAFETY: lookup witnesses a single slice at the
    // found page-map entry.
    let slice_count = unsafe { SliceCount::from_usize_const(1).unwrap_unchecked() };
    // SAFETY: non-null submap entry proves the page is
    // registered for this index/sub-index witness.
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY: exported heap-region query forwards arbitrary
// input through the safe page-map lookup boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    // SAFETY: `_mi_safe_ptr_page` handles arbitrary
    // nullable input and returns null for non-registered pointers.
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}
