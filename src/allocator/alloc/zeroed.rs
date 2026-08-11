use core::ffi::c_void;
use core::ptr;

use crate::allocator::page::_mi_theap_or_default_typed;
use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::prim::{_mi_theap_default, MI_THEAP_INITASNULL};
use crate::types::{mi_heap_t, mi_theap_t};

use super::{
    _mi_theap_malloc_zero, _mi_theap_malloc_zero_typed, calloc_request_or_report,
    malloc_layout_or_report, mi_heap_borrowed_theap_local,
};

/// # Safety
/// `theap` may be null only when lazy initialization is enabled; otherwise it
/// must identify a live initialized current-thread heap with exclusive access
/// to its local allocation state. Protocol atomic remote-free publication is
/// permitted. Allocator callbacks and their opaque arguments reached
/// synchronously must remain valid, must not unwind, and must not incompatibly
/// reenter heap mutation. A nonnull result is allocator-owned and may be
/// released with `mi_free`.
// SAFETY: this exported entry preserves the raw thread-heap and scalar-size ABI
// while validating layout before typed zeroing allocation dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    if theap.is_null() && MI_THEAP_INITASNULL {
        // SAFETY: the null lazy sentinel is supported in this configuration;
        // allocator lifecycle and callbacks satisfy the raw zeroing dispatch.
        return unsafe { _mi_theap_malloc_zero(theap, size, true, ptr::null_mut()) };
    }
    let Some(layout) = malloc_layout_or_report(size, true) else {
        return ptr::null_mut();
    };
    // SAFETY: the caller keeps any nonnull current-thread heap live and locally
    // exclusive while default/sentinel resolution creates the local witness.
    let theap = unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        ThreadHeapLocal::new_unchecked(theap)
    };
    // SAFETY: the resolved heap stays live and locally exclusive, and `layout`
    // is the validated zeroing layout for this exact request.
    unsafe { mi_theap_zalloc_typed(theap, layout) }
}

/// # Safety
/// `theap` must remain a live current-thread heap with exclusive access to its
/// local allocation state; protocol atomic remote-free publication is
/// permitted. `layout` must be the validated zeroing layout for this request.
/// Allocator callbacks and their opaque arguments reached synchronously must
/// remain valid, must not unwind, and must not incompatibly reenter heap
/// mutation. A nonnull result is allocator-owned and may be released with
/// `mi_free`.
pub(super) unsafe fn mi_theap_zalloc_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    // SAFETY: the caller-proven current-thread heap epoch and validated zeroing
    // layout satisfy the central typed allocation boundary.
    unsafe { _mi_theap_malloc_zero_typed(theap, layout, ptr::null_mut()) }
}

/// Allocate and zero `size` bytes from the current thread's default heap.
///
/// A nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub extern "C" fn mi_zalloc(size: usize) -> *mut c_void {
    // SAFETY: allocator lifecycle supplies the current default/lazy heap and
    // exclusive local allocation epoch for this validated raw-size dispatch.
    unsafe { _mi_theap_malloc_zero(_mi_theap_default(), size, true, ptr::null_mut()) }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread,
/// with a stable thread-heap association and exclusive access to local
/// allocation state; protocol atomic remote-free publication is permitted.
/// Allocator callbacks and their opaque arguments reached synchronously must
/// remain valid, must not unwind, and must not incompatibly reenter heap
/// mutation. A nonnull result is allocator-owned and may be released with
/// `mi_free`.
// SAFETY: this exported heap entry validates and localizes the raw heap before
// typed zeroing allocation dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller keeps the raw heap live and current-thread owned while
    // it is validated. This intentionally precedes layout validation.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return ptr::null_mut();
    };
    let Some(layout) = malloc_layout_or_report(size, true) else {
        return ptr::null_mut();
    };
    // SAFETY: the validated heap and stable association remain live and
    // current-thread owned while the local witness is projected.
    let Some(theap) = (unsafe { mi_heap_borrowed_theap_local(heap) }) else {
        return ptr::null_mut();
    };
    // SAFETY: the localized heap stays live and locally exclusive, and
    // `layout` is the validated zeroing layout for this request.
    unsafe { mi_theap_zalloc_typed(theap, layout) }
}

/// # Safety
/// `theap` may be null only when lazy initialization is enabled; otherwise it
/// must identify a live initialized current-thread heap with exclusive access
/// to its local allocation state. Protocol atomic remote-free publication is
/// permitted. Allocator callbacks and their opaque arguments reached
/// synchronously must remain valid, must not unwind, and must not incompatibly
/// reenter heap mutation. A nonnull result is allocator-owned and may be
/// released with `mi_free`.
// SAFETY: this exported entry preserves the raw thread-heap and product ABI and
// validates the calloc request before zeroing allocation dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_calloc(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    if theap.is_null() && MI_THEAP_INITASNULL {
        return match calloc_request_or_report(count, size) {
            // SAFETY: `req` retains the checked product layout, and the null
            // lazy sentinel is supported for the raw zeroing entry.
            Some(req) => unsafe { mi_theap_zalloc(theap, req.layout().requested().get()) },
            None => ptr::null_mut(),
        };
    }
    let Some(req) = calloc_request_or_report(count, size) else {
        return ptr::null_mut();
    };
    // SAFETY: the caller keeps any nonnull current-thread heap live and locally
    // exclusive while default/sentinel resolution creates the local witness.
    let theap = unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        ThreadHeapLocal::new_unchecked(theap)
    };
    // SAFETY: the resolved heap stays live and locally exclusive, and `req`
    // carries the validated checked-product zeroing layout.
    unsafe { mi_theap_zalloc_typed(theap, req.layout()) }
}

/// Allocate and zero the checked product of `count * size` bytes from the
/// current thread's default heap.
///
/// A nonnull result is allocator-owned and may be released with `mi_free`.
// Retained as an exported C ABI allocation symbol; release cdylib builds may
// reference it only from external dynamic callers.
#[allow(dead_code)]
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub extern "C" fn mi_calloc(count: usize, size: usize) -> *mut c_void {
    if let Some(req) = calloc_request_or_report(count, size) {
        // SAFETY: allocator lifecycle keeps the default TLS slot stable and
        // permits initialization after the checked product succeeds.
        let theap = unsafe {
            let Some(theap) = _mi_theap_or_default_typed(_mi_theap_default()) else {
                return core::ptr::null_mut();
            };
            ThreadHeapLocal::new_unchecked(theap)
        };
        // SAFETY: the resolved default heap stays live and locally exclusive,
        // and `req` carries the checked-product zeroing layout.
        unsafe { mi_theap_zalloc_typed(theap, req.layout()) }
    } else {
        core::ptr::null_mut()
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread,
/// with a stable thread-heap association and exclusive access to local
/// allocation state; protocol atomic remote-free publication is permitted.
/// Allocator callbacks and their opaque arguments reached synchronously must
/// remain valid, must not unwind, and must not incompatibly reenter heap
/// mutation. A nonnull result is allocator-owned and may be released with
/// `mi_free`.
// SAFETY: this exported heap entry validates the raw heap before the checked
// product and local zeroing allocation dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the raw heap live and current-thread owned while
    // it is validated. This intentionally precedes product validation.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(req) = calloc_request_or_report(count, size) else {
        return core::ptr::null_mut();
    };
    // SAFETY: the validated heap and stable association remain live and
    // current-thread owned while the local witness is projected.
    let Some(theap) = (unsafe { mi_heap_borrowed_theap_local(heap) }) else {
        return core::ptr::null_mut();
    };
    // SAFETY: the localized heap stays live and locally exclusive, and `req`
    // carries the validated checked-product zeroing layout.
    unsafe { mi_theap_zalloc_typed(theap, req.layout()) }
}
