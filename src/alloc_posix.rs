//! Mechanical translation of `src/alloc-posix.c`.

#![allow(dead_code, clippy::missing_safety_doc, unsafe_op_in_unsafe_fn)]

use core::ffi::{c_int, c_void};
use core::{mem::size_of, ptr};
use std::ffi::{CStr, CString};

use crate::internal::{
    _mi_align_up, _mi_is_aligned, _mi_is_power_of_two, _mi_memcpy, _mi_os_page_size,
    mi_count_size_overflow, mi_mul_overflow,
};
use crate::mimalloc::{
    c_wchar, mi_expand, mi_free, mi_is_in_heap_region, mi_malloc, mi_malloc_aligned, mi_realloc,
    mi_strdup, mi_usable_size,
};
use crate::types::{EINVAL, ENOMEM, EOVERFLOW};

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_usable_size(p: *const c_void) -> usize {
    unsafe { mi_usable_size(p) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_cfree(p: *mut c_void) {
    if unsafe { mi_is_in_heap_region(p.cast_const()) } {
        unsafe { mi_free(p) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_posix_memalign(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    if p.is_null() {
        return EINVAL;
    }
    if !alignment.is_multiple_of(size_of::<*mut c_void>()) {
        return EINVAL;
    }
    if alignment == 0 || !_mi_is_power_of_two(alignment) {
        return EINVAL;
    }
    let q = unsafe { mi_malloc_aligned(size, alignment) };
    if q.is_null() && size != 0 {
        return ENOMEM;
    }
    crate::mi_assert_internal!(_mi_is_aligned(q, alignment));
    unsafe { *p = q };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_memalign(alignment: usize, size: usize) -> *mut c_void {
    let p = unsafe { mi_malloc_aligned(size, alignment) };
    crate::mi_assert_internal!(_mi_is_aligned(p, alignment));
    p
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_valloc(size: usize) -> *mut c_void {
    unsafe { mi_memalign(_mi_os_page_size(), size) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_pvalloc(size: usize) -> *mut c_void {
    let psize = unsafe { _mi_os_page_size() };
    if size >= usize::MAX - psize {
        return ptr::null_mut();
    }
    let asize = _mi_align_up(size, psize);
    unsafe { mi_malloc_aligned(asize, psize) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    let p = unsafe { mi_malloc_aligned(size, alignment) };
    crate::mi_assert_internal!(_mi_is_aligned(p, alignment));
    p
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    let mut total = 0usize;
    if mi_count_size_overflow(count, size, &mut total) {
        mi_set_errno(EOVERFLOW);
        return ptr::null_mut();
    }
    let newp = mi_realloc(p, total);
    if newp.is_null() {
        mi_set_errno(ENOMEM);
    }
    newp
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int {
    crate::mi_assert!(size != 0);
    crate::mi_assert!(!ptrp.is_null());
    if ptrp.is_null() || size == 0 {
        mi_set_errno(EINVAL);
        return EINVAL;
    }

    let mut total = 0usize;
    if mi_count_size_overflow(count, size, &mut total) {
        mi_set_errno(EOVERFLOW);
        return EOVERFLOW;
    }

    if total == 0 {
        mi_free(*ptrp);
        *ptrp = ptr::null_mut();
        return 0;
    }

    let newp = mi_realloc(*ptrp, total);
    if newp.is_null() {
        mi_set_errno(ENOMEM);
        return ENOMEM;
    }
    *ptrp = newp;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi__expand(p: *mut c_void, newsize: usize) -> *mut c_void {
    let res = unsafe { mi_expand(p, newsize) };
    if res.is_null() {
        unsafe { mi_set_errno(ENOMEM) };
    }
    res
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_wcsdup(s: *const c_wchar) -> *mut c_wchar {
    if s.is_null() {
        return ptr::null_mut();
    }
    let mut wlen = 0usize;
    while unsafe { *s.add(wlen) } != 0 && wlen < isize::MAX as usize {
        wlen += 1;
    }
    let mut size = 0usize;
    if mi_mul_overflow(wlen + 1, size_of::<c_wchar>(), &mut size) || size > isize::MAX as usize {
        return ptr::null_mut();
    }
    let p = unsafe { mi_malloc(size) }.cast::<c_wchar>();
    if !p.is_null() {
        unsafe { _mi_memcpy(p.cast::<c_void>(), s.cast::<c_void>(), size) };
    }
    p
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_wdupenv_s(
    buf: *mut *mut c_wchar,
    size: *mut usize,
    _name: *const c_wchar,
) -> c_int {
    if !size.is_null() {
        unsafe { *size = 0 };
    }
    if buf.is_null() || _name.is_null() {
        return EINVAL;
    }
    unsafe { *buf = ptr::null_mut() };
    EINVAL
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_dupenv_s(
    buf: *mut *mut i8,
    size: *mut usize,
    name: *const i8,
) -> c_int {
    if !size.is_null() {
        *size = 0;
    }
    if buf.is_null() || name.is_null() {
        return EINVAL;
    }
    let name = match CStr::from_ptr(name).to_str() {
        Ok(name) => name,
        Err(_) => return EINVAL,
    };
    let value = match std::env::var_os(name).and_then(|v| v.into_string().ok()) {
        Some(value) => value,
        None => {
            *buf = ptr::null_mut();
            return 0;
        }
    };
    let value = match CString::new(value) {
        Ok(value) => value,
        Err(_) => return EINVAL,
    };

    *buf = mi_strdup(value.as_ptr());
    if (*buf).is_null() {
        return ENOMEM;
    }
    if !size.is_null() {
        *size = value.as_bytes_with_nul().len();
    }
    0
}
