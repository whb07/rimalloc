use core::ffi::c_void;

use crate::alloc::{_mi_page_malloc_zero_typed, calloc_request_or_report};
use crate::core_types::Errno;
use crate::internal::{mi_likely, mi_page_usable_block_size, mi_theap_malloc_use_guarded};
use crate::options::_mi_error_message_fmt;
use crate::prim::MI_THEAP_INITASNULL;
use crate::track::mi_track_malloc;
use crate::types::{MI_SMALL_SIZE_MAX, mi_theap_t};

use super::{
    MI_GUARDED, MI_PAGE_MAX_OVERALLOC_ALIGN, aligned_layout_or_report,
    mi_theap_malloc_guarded_aligned, mi_theap_malloc_zero_aligned_at_generic,
};

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be a non-zero power of two, and `usable` must be writable when non-null.
pub unsafe fn mi_theap_malloc_zero_aligned_at(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, offset, zero)
    else {
        return core::ptr::null_mut();
    };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
    unsafe { mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, usable) }
}

/// # Safety
/// `theap` must be `None` only when lazy thread-heap initialization is enabled.
/// `usable` must be writable for one `usize` when non-null.
pub(crate) unsafe fn mi_theap_malloc_zero_aligned_at_req(
    theap: Option<crate::core_types::ThreadHeapLocal>,
    layout: crate::core_types::LayoutRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
    usable: *mut usize,
) -> *mut c_void {
    let size = layout.requested().get();
    let alignment_raw = alignment.get();
    let offset_raw = offset.get();
    let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;

    if let Some(local_theap) = theap
        && MI_GUARDED != 0
        && offset_raw == 0
        && alignment_raw < MI_PAGE_MAX_OVERALLOC_ALIGN
        // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
        && unsafe { mi_theap_malloc_use_guarded(local_theap, layout.effective()) }
    {
        // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
        return unsafe { mi_theap_malloc_guarded_aligned(local_theap, layout, alignment, offset) };
    }

    if let Some(local_theap) = theap
        && mi_likely(size <= MI_SMALL_SIZE_MAX && alignment_raw <= size)
    {
        let theap = local_theap.ptr().as_ptr();
        let align_mask = alignment_raw - 1;
        let padded = layout.padded();
        // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
        let page =
            unsafe { crate::internal::_mi_theap_get_free_small_page_typed(local_theap, padded) };
        // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
        if mi_likely(!page.is_null()) {
            // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
            let (page_ptr, free) = unsafe {
                let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
                (page_ptr, page_ptr.free_raw())
            };
            if mi_likely(!free.is_null() && (((free as usize) + offset_raw) & align_mask == 0)) {
                // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
                unsafe {
                    crate::core_types::UsableOut::new(usable)
                        .write_if_present(mi_page_usable_block_size(page));
                }
                // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
                let active = unsafe { crate::core_types::ActivePage::new_unchecked(page_ptr) };
                // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
                let available = unsafe { active.available().unwrap_unchecked() };
                // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
                let p = unsafe {
                    _mi_page_malloc_zero_typed(
                        theap,
                        available,
                        padded,
                        crate::core_types::Zeroed::from_bool(zero),
                        core::ptr::null_mut(),
                    )
                };
                crate::internal::mi_assert_internal!(!p.is_null());
                crate::internal::mi_assert_internal!(
                    ((p as usize) + offset_raw).is_multiple_of(alignment_raw)
                );
                // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
                unsafe { mi_track_malloc(p, size, zero) };
                return p;
            }
        }
    }

    // SAFETY[AUDIT:aligned-page-allocation-boundary]: aligned allocation fast paths inspect page free slots and guarded state through typed witnesses.
    unsafe { mi_theap_malloc_zero_aligned_at_generic(theap, layout, alignment, offset, usable) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be valid for aligned allocation.
pub unsafe fn mi_theap_malloc_aligned_at(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    let Some((layout, alignment, offset)) =
        aligned_layout_or_report(size, alignment, offset, false)
    else {
        return core::ptr::null_mut();
    };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
    unsafe {
        mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, core::ptr::null_mut())
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc_aligned(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, 0, false)
    else {
        return core::ptr::null_mut();
    };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
    unsafe {
        mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, core::ptr::null_mut())
    }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be valid for aligned allocation.
pub unsafe fn mi_theap_zalloc_aligned_at(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, offset, true)
    else {
        return core::ptr::null_mut();
    };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
    unsafe {
        mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, core::ptr::null_mut())
    }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be valid for aligned allocation.
pub unsafe fn mi_theap_zalloc_aligned(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, 0, true)
    else {
        return core::ptr::null_mut();
    };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]: thread-heap aligned allocation builds local heap witnesses after checked layout normalization.
    unsafe {
        mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, core::ptr::null_mut())
    }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be valid for aligned allocation.
pub unsafe fn mi_theap_calloc_aligned_at(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    let Some(request) = calloc_request_or_report(count, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        if crate::internal::MI_DEBUG > 0 {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation requires the alignment to be a power-of-two (size {}, alignment {alignment})\n",
                    request.total().get()
                ),
            );
        }
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    unsafe { mi_theap_calloc_aligned_at_req(theap, request, alignment, offset) }
}

/// # Safety
/// `theap` must be `None` only when lazy thread-heap initialization is enabled.
pub(crate) unsafe fn mi_theap_calloc_aligned_at_req(
    theap: Option<crate::core_types::ThreadHeapLocal>,
    request: crate::core_types::CallocRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    let size = request.total().get();
    let layout = match crate::core_types::LayoutRequest::aligned_at(
        size,
        alignment.get(),
        offset,
        crate::core_types::Zeroed::Yes,
    ) {
        Ok(layout) => layout,
        Err(_) => {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation request is too large (size {size}, alignment {})\n",
                    alignment.get()
                ),
            );
            return core::ptr::null_mut();
        }
    };
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    unsafe {
        mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, core::ptr::null_mut())
    }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `alignment` must be valid for aligned allocation.
pub unsafe fn mi_theap_calloc_aligned(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    let Some(request) = calloc_request_or_report(count, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        if crate::internal::MI_DEBUG > 0 {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation requires the alignment to be a power-of-two (size {}, alignment {alignment})\n",
                    request.total().get()
                ),
            );
        }
        return core::ptr::null_mut();
    };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    unsafe {
        mi_theap_calloc_aligned_at_req(
            theap,
            request,
            alignment,
            crate::core_types::AlignmentOffset::new(0),
        )
    }
}
