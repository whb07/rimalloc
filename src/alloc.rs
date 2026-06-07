//! Core allocation / free paths + libc overrides

#![allow(dead_code, clippy::missing_safety_doc, unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_int, c_long, c_void};
use core::{
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::internal::{_mi_memcpy, _mi_strlen, _mi_strnlen, mi_count_size_overflow};
use crate::mimalloc::{mi_free, mi_theap_malloc, mi_theap_realloc, mi_zalloc};
use crate::page::{MI_MAX_ALLOC_SIZE, mi_good_size};
use crate::prim::_mi_theap_default;
use crate::types::{ENOMEM, mi_theap_t};

pub const _DEFAULT_SOURCE: () = ();

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe extern "C" {
    fn pathconf(path: *const c_char, name: c_int) -> c_long;
    fn realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    *__errno_location() = err;
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    *__error() = err;
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

unsafe fn mi_theap_strdup(theap: *mut mi_theap_t, s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    let len = _mi_strlen(s);
    if len > MI_MAX_ALLOC_SIZE - 1 {
        return ptr::null_mut();
    }
    let t = mi_theap_malloc(theap, len + 1).cast::<c_char>();
    if t.is_null() {
        return ptr::null_mut();
    }
    _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
    *t.add(len) = 0;
    t
}

unsafe fn mi_theap_strndup(theap: *mut mi_theap_t, s: *const c_char, n: usize) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    let len = _mi_strnlen(s, n);
    if len > MI_MAX_ALLOC_SIZE - 1 {
        return ptr::null_mut();
    }
    let t = mi_theap_malloc(theap, len + 1).cast::<c_char>();
    if t.is_null() {
        return ptr::null_mut();
    }
    _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
    *t.add(len) = 0;
    t
}

pub extern "C" fn mi_malloc_good_size(size: usize) -> usize {
    mi_good_size(size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_mallocn(count: usize, size: usize) -> *mut c_void {
    let mut total = 0usize;
    if mi_count_size_overflow(count, size, &mut total) {
        return ptr::null_mut();
    }
    mi_theap_malloc(_mi_theap_default(), total)
}

unsafe fn mi_theap_reallocf(theap: *mut mi_theap_t, p: *mut c_void, newsize: usize) -> *mut c_void {
    let newp = mi_theap_realloc(theap, p, newsize);
    if newp.is_null() && !p.is_null() {
        mi_free(p);
    }
    newp
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strdup(s: *const c_char) -> *mut c_char {
    mi_theap_strdup(_mi_theap_default(), s)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char {
    mi_theap_strndup(_mi_theap_default(), s, n)
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_path_max() -> usize {
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

#[cfg(all(unix, not(target_os = "wasi")))]
pub unsafe fn mi_theap_realpath(
    theap: *mut mi_theap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
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
    let result = mi_theap_strndup(theap, rname, n);
    mi_free(buf.cast::<c_void>());
    result
}
