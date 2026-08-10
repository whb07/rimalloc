use core::ffi::c_void;
use core::ptr;

use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::page::_mi_theap_or_default_typed;
use crate::prim::{_mi_theap_default, MI_THEAP_INITASNULL};
use crate::types::{mi_heap_t, mi_theap_t};

use super::{
    _mi_theap_malloc_zero, _mi_theap_malloc_zero_typed, calloc_request_or_report,
    malloc_layout_or_report, mi_heap_borrowed_theap_local, mi_theap_calloc_req,
};

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
// SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return _mi_theap_malloc_zero(theap, size, true, ptr::null_mut());
        }
        let Some(layout) = malloc_layout_or_report(size, true) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_zalloc_typed(ThreadHeapLocal::new_unchecked(theap), layout)
    }
}

pub(super) unsafe fn mi_theap_zalloc_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    // SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
    unsafe { _mi_theap_malloc_zero_typed(theap, layout, ptr::null_mut()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc(size: usize) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero(_mi_theap_default(), size, true, ptr::null_mut()) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Some(layout) = malloc_layout_or_report(size, true) else {
            return ptr::null_mut();
        };
        mi_heap_zalloc_typed(heap, layout)
    }
}

unsafe fn mi_heap_zalloc_typed(
    heap: BorrowedHeap,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    // SAFETY: zeroing allocation entrypoints route typed layout requests without hidden allocation or dynamic dispatch.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_zalloc_typed(theap, layout)
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
// SAFETY: calloc entrypoints validate checked products before zeroing allocation dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_calloc(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: calloc entrypoints validate checked products before zeroing allocation dispatch.
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return match calloc_request_or_report(count, size) {
                Some(req) => mi_theap_calloc_req_lazy(theap, req),
                None => ptr::null_mut(),
            };
        }
        let Some(req) = calloc_request_or_report(count, size) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_calloc_typed(ThreadHeapLocal::new_unchecked(theap), req)
    }
}

unsafe fn mi_theap_calloc_typed(
    theap: ThreadHeapLocal,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    // SAFETY: calloc entrypoints validate checked products before zeroing allocation dispatch.
    unsafe { mi_theap_calloc_req(theap, req) }
}

pub(super) unsafe fn mi_theap_calloc_req_lazy(
    theap: *mut mi_theap_t,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    // SAFETY: calloc entrypoints validate checked products before zeroing allocation dispatch.
    unsafe { mi_theap_zalloc(theap, req.layout().requested().get()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
// Retained as an exported C ABI allocation symbol; release cdylib builds may
// reference it only from external dynamic callers.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[allow(dead_code)]
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe {
        if let Some(req) = calloc_request_or_report(count, size) {
            let Some(theap) = _mi_theap_or_default_typed(_mi_theap_default()) else {
                return core::ptr::null_mut();
            };
            mi_theap_calloc_req(ThreadHeapLocal::new_unchecked(theap), req)
        } else {
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Some(req) = calloc_request_or_report(count, size) else {
            return core::ptr::null_mut();
        };
        mi_heap_calloc_typed(heap, req)
    }
}

unsafe fn mi_heap_calloc_typed(
    heap: BorrowedHeap,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_calloc_req(theap, req)
    }
}
