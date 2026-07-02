use core::ffi::c_void;
use core::ptr;

use crate::core_types::BorrowedHeap;
use crate::types::mi_heap_t;

use super::mi_heap_try_new_typed;

// SAFETY[AUDIT:allocation-cxx-new-try-boundary]: C++ new retry helpers route allocation failures through the new-handler policy.
#[inline(never)]
unsafe extern "C" fn mi_heap_try_new(
    heap: *mut mi_heap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    // SAFETY[AUDIT:allocation-cxx-new-try-boundary]: C++ new retry helpers route allocation failures through the new-handler policy.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_try_new_typed(heap, size, nothrow)
    }
}

// SAFETY[AUDIT:allocation-cxx-new-try-boundary]: C++ new retry helpers route allocation failures through the new-handler policy.
#[used]
static MI_HEAP_TRY_NEW_KEEP: unsafe extern "C" fn(*mut mi_heap_t, usize, bool) -> *mut c_void =
    mi_heap_try_new;
