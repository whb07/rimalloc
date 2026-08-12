use core::ffi::{c_char, c_void};
use core::ptr;

use crate::allocator::options::_mi_error_message_fmt;
use crate::core_types::{BorrowedHeap, Errno};
use crate::internal::mi_unlikely;
use crate::mimalloc::mi_malloc_aligned;
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
fn mi_get_new_handler() -> StdNewHandler {
    type GetNewHandler = unsafe extern "C" fn() -> StdNewHandler;

    // SAFETY: `dlsym` is declared with the platform C ABI and receives the process-global lookup
    // handle plus a static NUL-terminated symbol name.
    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    // SAFETY: both arguments satisfy the `dlsym` ABI for a process-global symbol lookup.
    let symbol = unsafe { dlsym(ptr::null_mut(), c"_ZSt15get_new_handlerv".as_ptr()) };
    if symbol.is_null() {
        None
    } else {
        // SAFETY: the fixed Itanium ABI symbol denotes `std::get_new_handler` with exactly the
        // `GetNewHandler` calling convention and return shape used here.
        let get_new_handler = unsafe { core::mem::transmute::<*mut c_void, GetNewHandler>(symbol) };
        // SAFETY: `get_new_handler` was resolved and typed from that exact fixed symbol.
        unsafe { get_new_handler() }
    }
}

#[cfg(not(all(unix, not(target_os = "wasi"))))]
#[allow(non_snake_case)]
fn _ZSt15get_new_handlerv() -> StdNewHandler {
    None
}

#[cfg(not(all(unix, not(target_os = "wasi"))))]
fn mi_get_new_handler() -> StdNewHandler {
    _ZSt15get_new_handlerv()
}

/// # Safety
/// Any nonnull handler returned by the process C++ runtime must remain valid for the synchronous
/// call, use the declared ABI, and not unwind across this Rust/C ABI boundary.
unsafe fn mi_try_new_handler(nothrow: bool) -> bool {
    let h = mi_get_new_handler();
    if let Some(h) = h {
        // SAFETY: the caller guarantees the runtime-provided handler is synchronously callable
        // with this ABI and cannot unwind across the boundary.
        unsafe { h() };
        true
    } else {
        _mi_error_message_fmt(Errno::ENOMEM, format_args!("out of memory in 'new'"));
        if !nothrow {
            // SAFETY: the C `abort` function takes no arguments and does not return.
            unsafe { abort() };
        }
        false
    }
}

/// # Safety
/// `theap` must be null only as a supported default/lazy sentinel, or name a live current-thread
/// heap whose local allocation state may be mutated exclusively by this call. Any nonnull C++
/// new-handler returned during a retry must remain synchronously callable with the declared ABI and
/// must not unwind across this Rust/C ABI boundary.
#[inline(never)]
pub(crate) unsafe extern "C" fn mi_theap_try_new(
    theap: *mut mi_theap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    // SAFETY: the caller provides the heap dispatch/local-mutation epoch and guarantees that every
    // runtime-provided handler is live, ABI-compatible, and non-unwinding for each synchronous retry.
    unsafe {
        let mut p = mi_theap_malloc(theap, size);
        while p.is_null() && mi_try_new_handler(nothrow) {
            p = mi_theap_malloc(theap, size);
        }
        p
    }
}

/// # Safety
/// The allocator default heap must be available for current-thread exclusive local allocation.
/// Any nonnull C++ new-handler returned during a retry must remain synchronously callable with the
/// declared ABI and must not unwind across this Rust/C ABI boundary.
#[inline(never)]
unsafe extern "C" fn mi_try_new(size: usize, nothrow: bool) -> *mut c_void {
    // SAFETY: the caller supplies the default-heap allocation epoch and the handler
    // liveness/ABI/non-unwind guarantee required by the retry loop.
    unsafe { mi_theap_try_new(_mi_theap_default(), size, nothrow) }
}

/// # Safety
/// `heap` must remain live and current-thread local while it is projected to its thread heap, whose
/// allocation state is mutated exclusively by this call. Any nonnull C++ new-handler returned
/// during a retry must remain synchronously callable with the declared ABI and must not unwind
/// across this Rust/C ABI boundary.
unsafe fn mi_heap_try_new_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
    nothrow: bool,
) -> *mut c_void {
    // SAFETY: the caller supplies the live local heap epoch and guarantees every runtime-provided
    // handler is live, ABI-compatible, and non-unwinding for each synchronous retry.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_try_new(theap.ptr().as_ptr(), size.get(), nothrow)
    }
}

/// # Safety
/// `theap` must be null only as a supported default/lazy sentinel, or name a live current-thread
/// heap whose local allocation state may be mutated exclusively by this call. Any nonnull C++
/// new-handler returned during a retry must remain synchronously callable with the declared ABI and
/// must not unwind across this Rust/C ABI boundary.
pub(crate) unsafe extern "C" fn mi_theap_alloc_new(
    theap: *mut mi_theap_t,
    size: usize,
) -> *mut c_void {
    // SAFETY: the caller supplies the heap dispatch/local-mutation epoch and guarantees every
    // runtime-provided handler is live, ABI-compatible, and non-unwinding for synchronous retry.
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
/// Any nonnull C++ new-handler returned for a synchronous allocation retry must remain live, use the
/// declared ABI, and not unwind across this Rust/C ABI boundary. A nonnull result is allocator-owned
/// and may be released through this allocator.
// SAFETY: public C++ new entrypoints validate heap/thread dispatch before retrying allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new(size: usize) -> *mut c_void {
    unsafe { mi_theap_alloc_new(_mi_theap_default(), size) }
}

/// # Safety
/// `heap` must name a live current-thread heap whose local allocation state may be mutated
/// exclusively by this call. Any nonnull C++ new-handler returned during a retry must remain
/// synchronously callable with the declared ABI and must not unwind across this Rust/C ABI boundary.
/// A nonnull result is allocator-owned and may be released through this allocator.
// SAFETY: public C++ new entrypoints validate heap/thread dispatch before retrying allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        // SAFETY: the caller supplies the live current-thread heap epoch and guarantees any
        // runtime-provided handler is live, ABI-compatible, and non-unwinding for synchronous retry.
        mi_heap_alloc_new_typed(heap, size)
    }
}

/// # Safety
/// `heap` must remain live and current-thread local while its allocation state is mutated exclusively
/// by this call. Any nonnull C++ new-handler returned during a retry must remain synchronously
/// callable with the declared ABI and must not unwind across this Rust/C ABI boundary.
unsafe fn mi_heap_alloc_new_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
) -> *mut c_void {
    // SAFETY: the caller supplies the live local heap epoch and guarantees every runtime-provided
    // handler is live, ABI-compatible, and non-unwinding for synchronous retry.
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

/// # Safety
/// `theap` must be null only as a supported default/lazy sentinel, or name a live current-thread
/// heap whose local allocation state may be mutated exclusively by this call. Any nonnull C++
/// new-handler returned during a retry must remain synchronously callable with the declared ABI and
/// must not unwind across this Rust/C ABI boundary.
unsafe fn mi_theap_alloc_new_n(
    theap: *mut mi_theap_t,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    // SAFETY: the caller supplies the heap dispatch/local-mutation epoch and guarantees any
    // runtime-provided handler is live, ABI-compatible, and non-unwinding for synchronous retry.
    unsafe { mi_theap_alloc_new(theap, req.total().get()) }
}

/// # Safety
/// Any nonnull C++ new-handler returned for a synchronous overflow or allocation retry must remain
/// live, use the declared ABI, and not unwind across this Rust/C ABI boundary. A nonnull result is
/// allocator-owned and may be released through this allocator.
// SAFETY: C++ new_n paths validate checked products before typed allocation and retry dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_n(count: usize, size: usize) -> *mut c_void {
    unsafe {
        match alloc_product_or_report(count, size) {
            Some(req) => {
                // SAFETY: the caller supplies the default-heap allocation epoch and guarantees any
                // runtime-provided handler is live, ABI-compatible, and non-unwinding for retry.
                mi_theap_alloc_new_n(_mi_theap_default(), req)
            }
            None => {
                // SAFETY: the caller guarantees the process runtime's handler, if any, is live,
                // ABI-compatible, and non-unwinding for this synchronous overflow attempt.
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

/// # Safety
/// `heap` must name a live current-thread heap whose local allocation state may be mutated
/// exclusively by this call. Any nonnull C++ new-handler returned for the synchronous overflow or
/// allocation retry must remain live, use the declared ABI, and not unwind across this Rust/C ABI
/// boundary. A nonnull result is allocator-owned and may be released through this allocator.
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
            Some(req) => {
                // SAFETY: the caller supplies the live current-thread heap epoch and guarantees any
                // runtime-provided handler is live, ABI-compatible, and non-unwinding for retry.
                mi_heap_alloc_new_n_typed(heap, req)
            }
            None => {
                // SAFETY: the caller guarantees the process runtime's handler, if any, is live,
                // ABI-compatible, and non-unwinding for this synchronous overflow attempt.
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

/// # Safety
/// `heap` must remain live and current-thread local while its allocation state is mutated exclusively
/// by this call. Any nonnull C++ new-handler returned during a retry must remain synchronously
/// callable with the declared ABI and must not unwind across this Rust/C ABI boundary.
unsafe fn mi_heap_alloc_new_n_typed(
    heap: BorrowedHeap,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    // SAFETY: the caller supplies the live local heap epoch and guarantees every runtime-provided
    // handler is live, ABI-compatible, and non-unwinding for synchronous retry.
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
/// Any nonnull C++ new-handler returned during a synchronous allocation retry must remain live, use
/// the declared ABI, and not unwind across this Rust/C ABI boundary. A nonnull result is
/// allocator-owned and may be released through this allocator.
// SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_nothrow(size: usize) -> *mut c_void {
    unsafe {
        let p = mi_malloc(size);
        if mi_unlikely(p.is_null()) {
            // SAFETY: the caller guarantees any runtime-provided handler is live, ABI-compatible,
            // and non-unwinding for the synchronous nothrow retry.
            mi_try_new(size, true)
        } else {
            p
        }
    }
}

/// # Safety
/// `alignment` must be valid for the aligned allocation API. Returned memory must be released
/// through this allocator. Any nonnull C++ new-handler returned during a retry must remain live,
/// use the declared ABI, and not unwind across this Rust/C ABI boundary.
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

/// # Safety
/// The allocator's default heap must be available for current-thread allocation. Any nonnull C++
/// new-handler returned during a retry must remain synchronously callable with the declared ABI and
/// must not unwind across this Rust/C ABI boundary.
unsafe fn mi_new_aligned_typed(
    size: crate::core_types::RequestedSize,
    alignment: crate::core_types::Alignment,
) -> *mut c_void {
    // SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
    unsafe {
        let mut p: *mut c_void;
        loop {
            p = mi_malloc_aligned(size.get(), alignment.get());
            // SAFETY: the caller guarantees each runtime-provided handler is live, ABI-compatible,
            // and non-unwinding for this synchronous retry.
            if !p.is_null() || !mi_try_new_handler(false) {
                return p;
            }
        }
    }
}

/// # Safety
/// `alignment` must be valid for the aligned allocation API. Returned memory must be released
/// through this allocator. Any nonnull C++ new-handler returned during a retry must remain live,
/// use the declared ABI, and not unwind across this Rust/C ABI boundary.
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

/// # Safety
/// The allocator's default heap must be available for current-thread allocation. Any nonnull C++
/// new-handler returned during a retry must remain synchronously callable with the declared ABI and
/// must not unwind across this Rust/C ABI boundary.
unsafe fn mi_new_aligned_nothrow_typed(
    size: crate::core_types::RequestedSize,
    alignment: crate::core_types::Alignment,
) -> *mut c_void {
    // SAFETY: nothrow and aligned C++ new paths preserve alignment validation and retry policy.
    unsafe {
        let mut p: *mut c_void;
        loop {
            p = mi_malloc_aligned(size.get(), alignment.get());
            // SAFETY: the caller guarantees each runtime-provided handler is live, ABI-compatible,
            // and non-unwinding for this synchronous retry.
            if !p.is_null() || !mi_try_new_handler(true) {
                return p;
            }
        }
    }
}

/// # Safety
/// `p` must be null or a live allocation returned by this allocator, with no concurrent free or
/// realloc for the duration of the call. Any nonnull C++ new-handler returned during a retry must
/// remain synchronously callable with the declared ABI and must not unwind across this Rust/C ABI
/// boundary.
// SAFETY: C++ new realloc wrappers retry realloc-family requests through the new-handler policy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        let mut q: *mut c_void;
        loop {
            q = mi_realloc(p, newsize);
            // SAFETY: the caller guarantees each runtime-provided handler is live, ABI-compatible,
            // and non-unwinding for this synchronous retry.
            if !q.is_null() || !mi_try_new_handler(false) {
                return q;
            }
        }
    }
}

/// # Safety
/// `p` must be null or a live allocation returned by this allocator, with no concurrent free or
/// realloc for the duration of the call. Any nonnull C++ new-handler returned for the synchronous
/// overflow or allocation retry must remain live, use the declared ABI, and not unwind across this
/// Rust/C ABI boundary.
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
                // SAFETY: the caller guarantees the process runtime's handler, if any, is live,
                // ABI-compatible, and non-unwinding for this synchronous overflow attempt.
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}
