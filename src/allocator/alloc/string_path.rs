#![allow(dead_code)]

use core::ffi::{c_char, c_int, c_long, c_void};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::internal::{_mi_memcpy, _mi_strlen, _mi_strnlen};
use crate::mimalloc::mi_free;
use crate::page::{_mi_theap_or_default_typed, MI_MAX_ALLOC_SIZE};
use crate::prim::_mi_theap_default;
use crate::types::{ENOMEM, mi_heap_t, mi_theap_t};

use super::{mi_heap_borrowed_theap_local, mi_theap_malloc, mi_zalloc};

// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[cfg(all(unix, not(target_os = "wasi")))]
unsafe extern "C" {
    fn pathconf(path: *const c_char, name: c_int) -> c_long;
    fn realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn mi_set_errno(err: c_int) {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        unsafe extern "C" {
            fn __errno_location() -> *mut c_int;
        }
        *__errno_location() = err;
    }
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        unsafe extern "C" {
            fn __error() -> *mut c_int;
        }
        *__error() = err;
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

unsafe fn mi_theap_default_local() -> Option<ThreadHeapLocal> {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    let theap = unsafe { _mi_theap_or_default_typed(_mi_theap_default()) }?;
    // SAFETY: `_mi_theap_or_default_typed` returned a non-null thread-heap
    // pointer for the current/default thread heap.
    Some(unsafe { ThreadHeapLocal::new_unchecked(theap) })
}

unsafe fn mi_theap_strdup_typed(theap: ThreadHeapLocal, s: *const c_char) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        if s.is_null() {
            return ptr::null_mut();
        }
        let len = _mi_strlen(s);
        if len > MI_MAX_ALLOC_SIZE - 1 {
            return ptr::null_mut();
        }
        let t = mi_theap_malloc(theap.ptr().as_ptr(), len + 1).cast::<c_char>();
        if t.is_null() {
            return ptr::null_mut();
        }
        _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
        *t.add(len) = 0;
        t
    }
}

unsafe fn mi_theap_strndup_typed(
    theap: ThreadHeapLocal,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        if s.is_null() {
            return ptr::null_mut();
        }
        let len = _mi_strnlen(s, n);
        if len > MI_MAX_ALLOC_SIZE - 1 {
            return ptr::null_mut();
        }
        let t = mi_theap_malloc(theap.ptr().as_ptr(), len + 1).cast::<c_char>();
        if t.is_null() {
            return ptr::null_mut();
        }
        _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
        *t.add(len) = 0;
        t
    }
}

/// # Safety
/// `s` must be null or point to a valid NUL-terminated C string.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strdup(s: *const c_char) -> *mut c_char {
    unsafe {
        let Some(theap) = mi_theap_default_local() else {
            return ptr::null_mut();
        };
        mi_theap_strdup_typed(theap, s)
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `s` must be null or point to a valid NUL-terminated C
/// string.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strdup(heap: *mut mi_heap_t, s: *const c_char) -> *mut c_char {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_strdup_typed(heap, s)
    }
}

unsafe fn mi_heap_strdup_typed(heap: BorrowedHeap, s: *const c_char) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_strdup_typed(theap, s)
    }
}

/// # Safety
/// `s` must be null or readable for at most `n` bytes up to a NUL terminator.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char {
    unsafe {
        let Some(theap) = mi_theap_default_local() else {
            return ptr::null_mut();
        };
        mi_theap_strndup_typed(theap, s, n)
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `s` must be null or readable for at most `n` bytes up to a
/// NUL terminator.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strndup(
    heap: *mut mi_heap_t,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_strndup_typed(heap, s, n)
    }
}

unsafe fn mi_heap_strndup_typed(heap: BorrowedHeap, s: *const c_char, n: usize) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_strndup_typed(theap, s, n)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_path_max() -> usize {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        static PATH_MAX: AtomicUsize = AtomicUsize::new(0);
        let pmax = PATH_MAX.load(Ordering::Acquire);
        if pmax != 0 {
            return pmax;
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        const _PC_PATH_MAX: c_int = 4;
        #[cfg(target_vendor = "apple")]
        const _PC_PATH_MAX: c_int = 5;

        let m = pathconf(c"/".as_ptr(), _PC_PATH_MAX);
        let pmax = if m <= 0 {
            4096
        } else if m < 256 {
            256
        } else if m > (64 * 1024) {
            64 * 1024
        } else {
            m as usize
        };
        let _ = PATH_MAX.compare_exchange(0, pmax, Ordering::AcqRel, Ordering::Acquire);
        PATH_MAX.load(Ordering::Acquire)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_theap_realpath_typed(
    theap: ThreadHeapLocal,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        if !resolved_name.is_null() {
            return realpath(fname, resolved_name);
        }

        let n = mi_path_max();
        let buf = mi_zalloc(n + 1).cast::<c_char>();
        if buf.is_null() {
            mi_set_errno(ENOMEM);
            return ptr::null_mut();
        }
        let rname = realpath(fname, buf);
        let result = mi_theap_strndup_typed(theap, rname, n);
        mi_free(buf.cast::<c_void>());
        result
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must point to a valid NUL-terminated path. If `resolved_name` is non-null it must be
/// writable as required by `realpath`.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_theap_realpath(
    theap: *mut mi_theap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        if !resolved_name.is_null() {
            return realpath(fname, resolved_name);
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_realpath_typed(ThreadHeapLocal::new_unchecked(theap), fname, resolved_name)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `heap` must be a valid heap pointer. `fname` must point to a valid NUL-terminated path. If
/// `resolved_name` is non-null it must be writable as required by `realpath`.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realpath(
    heap: *mut mi_heap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_realpath_typed(heap, fname, resolved_name)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_heap_realpath_typed(
    heap: BorrowedHeap,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_realpath_typed(theap, fname, resolved_name)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must point to a valid NUL-terminated path. If `resolved_name` is non-null it must be
/// writable as required by `realpath`.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realpath(
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    // SAFETY: realpath forwards caller-owned
    // path and output-buffer pointers through the allocation entry boundary.
    unsafe {
        if !resolved_name.is_null() {
            return realpath(fname, resolved_name);
        }
        let Some(theap) = mi_theap_default_local() else {
            return ptr::null_mut();
        };
        mi_theap_realpath_typed(theap, fname, resolved_name)
    }
}
