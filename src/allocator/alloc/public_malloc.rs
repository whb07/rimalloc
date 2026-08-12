use core::ffi::c_void;
use core::ptr;

use crate::allocator::page::_mi_theap_or_default_typed;
use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::prim::{_mi_theap_default, MI_THEAP_INITASNULL};
use crate::types::{mi_heap_t, mi_theap_t};

use super::{
    _mi_theap_malloc_zero, _mi_theap_malloc_zero_typed, malloc_layout_or_report,
    mi_heap_borrowed_theap_local, mi_heap_malloc_typed, mi_theap_malloc_small_zero,
    mi_theap_malloc_small_zero_typed,
};

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. A nonnull result is allocator-owned
/// and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_small(size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and allocator lifecycle keep the default
    // heap's local queues, direct cache, free lists, and stats exclusively owned
    // while the small allocation runs.
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
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. `theap` may be null only when lazy
/// thread-heap initialization is enabled; otherwise it must be a live initialized
/// current-thread heap, with exclusive access to its local allocation state. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and heap contract keep the selected
    // current-thread allocation state exclusively owned through dispatch.
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
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown. The caller must have exclusive
/// mutation rights over its local allocation state; protocol atomic remote-free
/// publication is permitted. `size` must not exceed `MI_SMALL_SIZE_MAX`. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller keeps the heap live and current-thread owned while it
    // is classified, localized, and used for the bounded small allocation.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_typed(
            theap,
            size,
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `theap` may be null only when lazy initialization is enabled; otherwise it
/// must identify a live initialized current-thread heap. The caller must have
/// exclusive mutation rights over its local allocation state, while protocol
/// atomic remote-free publication is permitted. A nonnull result is
/// allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller keeps the raw current-thread heap and its local state
    // live and exclusive through lazy handling, layout validation, and ordinary
    // unaligned allocation dispatch.
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
        _mi_theap_malloc_zero_typed(
            ThreadHeapLocal::new_unchecked(theap),
            layout,
            ptr::null_mut(),
        )
    }
}

/// Allocate `size` bytes from the current thread's default heap.
///
/// A nonnull result is allocator-owned and may be released with `mi_free`.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub extern "C" fn mi_malloc(size: usize) -> *mut c_void {
    // SAFETY: allocator lifecycle supplies the current default/lazy heap and
    // exclusive local allocation epoch for this scalar allocation dispatch.
    unsafe { mi_theap_malloc(_mi_theap_default(), size) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: public malloc entrypoints validate raw heap/thread-heap handles before typed dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: heap is validated via BorrowedHeap::from_allocation_raw which checks live heap metadata; layout is validated before typed dispatch.
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
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. A nonnull result is allocator-owned
/// and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_small(size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and allocator lifecycle keep the default
    // heap's local queues, direct cache, free lists, and stats exclusively owned
    // while the zeroing small allocation runs.
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
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. `theap` may be null only when lazy
/// thread-heap initialization is enabled; otherwise it must be a live initialized
/// current-thread heap, with exclusive access to its local allocation state. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and heap contract keep the selected
    // current-thread allocation state exclusively owned through zeroing dispatch.
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
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown. The caller must have exclusive
/// mutation rights over its local allocation state; protocol atomic remote-free
/// publication is permitted. `size` must not exceed `MI_SMALL_SIZE_MAX`. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller keeps the heap live and current-thread owned while it
    // is classified, localized, and used for the bounded zeroing allocation.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_typed(
            theap,
            size,
            crate::core_types::Zeroed::Yes,
            ptr::null_mut(),
        )
    }
}
