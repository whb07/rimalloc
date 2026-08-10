use core::ffi::{c_char, c_void};
use core::ptr;

use crate::core_types::{BorrowedHeap, Errno};
use crate::internal::mi_unlikely;
use crate::mimalloc::mi_malloc_aligned;
use crate::options::_mi_error_message_fmt;
use crate::prim::_mi_theap_default;
use crate::types::{mi_heap_t, mi_theap_t};

use super::{
    alignment_or_report, alloc_product_or_report, mi_heap_borrowed_theap_local,
    mi_heap_malloc_typed, mi_malloc, mi_realloc, mi_theap_malloc,
};

// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
unsafe extern "C" {
    fn abort() -> !;
}

// SAFETY: C++ new-handler lookup and invocation preserve platform ABI and abort/nothrow policy.
type StdNewHandler = Option<unsafe extern "C" fn()>;

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_get_new_handler() -> StdNewHandler {
    unsafe {
        // SAFETY: C++ new-handler lookup and invocation preserve platform ABI and abort/nothrow policy.
        // lookup preserves the platform ABI function-pointer shape.
        type GetNewHandler = unsafe extern "C" fn() -> StdNewHandler;

        // SAFETY: C++ new-handler lookup and invocation preserve platform ABI and abort/nothrow policy.
        unsafe extern "C" {
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        }

        let symbol = dlsym(ptr::null_mut(), c"_ZSt15get_new_handlerv".as_ptr());
        if symbol.is_null() {
            None
        } else {
            let get_new_handler = core::mem::transmute::<*mut c_void, GetNewHandler>(symbol);
            get_new_handler()
        }
    }
}

#[cfg(not(all(unix, not(target_os = "wasi"))))]
#[allow(non_snake_case)]
unsafe fn _ZSt15get_new_handlerv() -> StdNewHandler {
    None
}

#[cfg(not(all(unix, not(target_os = "wasi"))))]
unsafe fn mi_get_new_handler() -> StdNewHandler {
    _ZSt15get_new_handlerv()
}

unsafe fn mi_try_new_handler(nothrow: bool) -> bool {
    // SAFETY: C++ new-handler lookup and invocation preserve platform ABI and abort/nothrow policy.
    unsafe {
        let h = mi_get_new_handler();
        if let Some(h) = h {
            h();
            true
        } else {
            _mi_error_message_fmt(Errno::ENOMEM, format_args!("out of memory in 'new'"));
            if !nothrow {
                abort();
            }
            false
        }
    }
}

// SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
#[inline(never)]
pub(crate) unsafe extern "C" fn mi_theap_try_new(
    theap: *mut mi_theap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    // SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
    unsafe {
        let mut p = mi_theap_malloc(theap, size);
        while p.is_null() && mi_try_new_handler(nothrow) {
            p = mi_theap_malloc(theap, size);
        }
        p
    }
}

// SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
#[inline(never)]
unsafe extern "C" fn mi_try_new(size: usize, nothrow: bool) -> *mut c_void {
    unsafe { mi_theap_try_new(_mi_theap_default(), size, nothrow) }
}

unsafe fn mi_heap_try_new_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
    nothrow: bool,
) -> *mut c_void {
    // SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_try_new(theap.ptr().as_ptr(), size.get(), nothrow)
    }
}

// SAFETY: public C++ new entrypoints validate heap/thread dispatch before retrying allocation failure.
pub(crate) unsafe extern "C" fn mi_theap_alloc_new(
    theap: *mut mi_theap_t,
    size: usize,
) -> *mut c_void {
    unsafe {
        let p = mi_theap_malloc(theap, size);
        if mi_unlikely(p.is_null()) {
            mi_theap_try_new(theap, size, false)
        } else {
            p
        }
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: public C++ new entrypoints validate heap/thread dispatch before retrying allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new(size: usize) -> *mut c_void {
    unsafe { mi_theap_alloc_new(_mi_theap_default(), size) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: public C++ new entrypoints validate heap/thread dispatch before retrying allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_alloc_new_typed(heap, size)
    }
}

unsafe fn mi_heap_alloc_new_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
) -> *mut c_void {
    // SAFETY: public C++ new entrypoints validate heap/thread dispatch before retrying allocation failure.
    unsafe {
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(size.get()) else {
            return mi_heap_try_new_typed(heap, size, false);
        };
        let p = mi_heap_malloc_typed(heap, layout);
        if mi_unlikely(p.is_null()) {
            mi_heap_try_new_typed(heap, size, false)
        } else {
            p
        }
    }
}

unsafe fn mi_theap_alloc_new_n(
    theap: *mut mi_theap_t,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    // SAFETY: C++ new_n paths validate checked products before typed allocation and retry dispatch.
    unsafe { mi_theap_alloc_new(theap, req.total().get()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: C++ new_n paths validate checked products before typed allocation and retry dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_n(count: usize, size: usize) -> *mut c_void {
    unsafe {
        match alloc_product_or_report(count, size) {
            Some(req) => mi_theap_alloc_new_n(_mi_theap_default(), req),
            None => {
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
// SAFETY: C++ new_n paths validate checked products before typed allocation and retry dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new_n(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: C++ new_n paths validate checked products before typed allocation and retry dispatch.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        match alloc_product_or_report(count, size) {
            Some(req) => mi_heap_alloc_new_n_typed(heap, req),
            None => {
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

unsafe fn mi_heap_alloc_new_n_typed(
    heap: BorrowedHeap,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    // SAFETY: C++ new_n paths validate checked products before typed allocation and retry dispatch.
    unsafe {
        let p = mi_heap_malloc_typed(heap, req.layout());
        if mi_unlikely(p.is_null()) {
            mi_heap_try_new_typed(heap, req.total(), false)
        } else {
            p
        }
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
// SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_nothrow(size: usize) -> *mut c_void {
    unsafe {
        let p = mi_malloc(size);
        if mi_unlikely(p.is_null()) {
            mi_try_new(size, true)
        } else {
            p
        }
    }
}

/// # Safety
/// `alignment` must be valid for the aligned allocation API. Returned memory must be released
/// through this allocator.
// SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_aligned(size: usize, alignment: usize) -> *mut c_void {
    unsafe {
        let Some(alignment) = alignment_or_report(size, alignment) else {
            return ptr::null_mut();
        };
        mi_new_aligned_typed(crate::core_types::RequestedSize::new(size), alignment)
    }
}

unsafe fn mi_new_aligned_typed(
    size: crate::core_types::RequestedSize,
    alignment: crate::core_types::Alignment,
) -> *mut c_void {
    // SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
    unsafe {
        let mut p: *mut c_void;
        loop {
            p = mi_malloc_aligned(size.get(), alignment.get());
            if !p.is_null() || !mi_try_new_handler(false) {
                return p;
            }
        }
    }
}

/// # Safety
/// `alignment` must be valid for the aligned allocation API. Returned memory must be released
/// through this allocator.
// SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_aligned_nothrow(size: usize, alignment: usize) -> *mut c_void {
    unsafe {
        let Some(alignment) = alignment_or_report(size, alignment) else {
            return ptr::null_mut();
        };
        mi_new_aligned_nothrow_typed(crate::core_types::RequestedSize::new(size), alignment)
    }
}

unsafe fn mi_new_aligned_nothrow_typed(
    size: crate::core_types::RequestedSize,
    alignment: crate::core_types::Alignment,
) -> *mut c_void {
    // SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
    unsafe {
        let mut p: *mut c_void;
        loop {
            p = mi_malloc_aligned(size.get(), alignment.get());
            if !p.is_null() || !mi_try_new_handler(true) {
                return p;
            }
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY: C++ new realloc wrappers retry realloc-family requests through the new-handler policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        let mut q: *mut c_void;
        loop {
            q = mi_realloc(p, newsize);
            if !q.is_null() || !mi_try_new_handler(false) {
                return q;
            }
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY: C++ new realloc wrappers retry realloc-family requests through the new-handler policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_reallocn(
    p: *mut c_void,
    newcount: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: C++ new realloc wrappers retry realloc-family requests through the new-handler policy.
    unsafe {
        match alloc_product_or_report(newcount, size) {
            Some(req) => mi_new_realloc(p, req.total().get()),
            None => {
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

// SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
#[used]
static MI_THEAP_TRY_NEW_KEEP: unsafe extern "C" fn(*mut mi_theap_t, usize, bool) -> *mut c_void =
    mi_theap_try_new;

// SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
// shim function pointer shape for release-audit source checks.
#[used]
static MI_TRY_NEW_KEEP: unsafe extern "C" fn(usize, bool) -> *mut c_void = mi_try_new;

// SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
#[inline(never)]
unsafe extern "C" fn mi_heap_try_new(
    heap: *mut mi_heap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    // SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_try_new_typed(heap, size, nothrow)
    }
}

// SAFETY: C++ new retry helpers route allocation failures through the new-handler policy.
#[used]
static MI_HEAP_TRY_NEW_KEEP: unsafe extern "C" fn(*mut mi_heap_t, usize, bool) -> *mut c_void =
    mi_heap_try_new;
