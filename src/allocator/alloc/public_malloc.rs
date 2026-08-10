use core::ffi::c_void;
use core::ptr;

use crate::allocator::page::_mi_theap_or_default_typed;
use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::prim::{_mi_theap_default, MI_THEAP_INITASNULL};
use crate::types::{mi_heap_t, mi_theap_t};

use super::{
    _mi_theap_malloc_zero, malloc_layout_or_report, mi_heap_malloc_small_typed,
    mi_heap_malloc_typed, mi_heap_zalloc_small_typed, mi_theap_malloc_small_zero,
    mi_theap_malloc_small_zero_typed, mi_theap_malloc_typed,
};

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: small allocation dispatch routes typed thread heaps through bounded size-class fast paths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_small(size: usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
// SAFETY: small allocation dispatch routes typed thread heaps through bounded size-class fast paths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            theap,
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: small allocation dispatch routes typed thread heaps through bounded size-class fast paths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_malloc_small_typed(heap, size)
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
// SAFETY: public malloc entrypoints validate raw heap/thread-heap handles before typed dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return _mi_theap_malloc_zero(theap, size, false, ptr::null_mut());
        }
        let Some(layout) = malloc_layout_or_report(size, false) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_typed(ThreadHeapLocal::new_unchecked(theap), layout)
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: public malloc entrypoints validate raw heap/thread-heap handles before typed dispatch.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_malloc(size: usize) -> *mut c_void {
    unsafe { mi_theap_malloc(_mi_theap_default(), size) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: public malloc entrypoints validate raw heap/thread-heap handles before typed dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(size) else {
            return core::ptr::null_mut();
        };
        mi_heap_malloc_typed(heap, layout)
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_small(size: usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::Yes,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
// SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return mi_theap_malloc_small_zero(
                theap,
                crate::core_types::RequestedSize::new(size),
                crate::core_types::Zeroed::Yes,
                ptr::null_mut(),
            );
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_typed(
            ThreadHeapLocal::new_unchecked(theap),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::Yes,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_zalloc_small_typed(heap, size)
    }
}
