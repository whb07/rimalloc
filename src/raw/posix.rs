//! Mechanical translation of `src/alloc-posix.c`.

use crate::internal::{
    _mi_align_up, _mi_is_aligned, _mi_is_power_of_two, _mi_memcpy, mi_mul_overflow,
};
use crate::mimalloc::{
    c_wchar, mi_expand, mi_free, mi_is_in_heap_region, mi_malloc, mi_malloc_aligned, mi_realloc,
    mi_strdup, mi_usable_size,
};
use crate::types::{EINVAL, ENOMEM, EOVERFLOW};
use core::ffi::CStr;
use core::ffi::{c_int, c_void};
use core::{mem::size_of, ptr};

unsafe extern "C" {
    fn getenv(name: *const i8) -> *const i8;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    unsafe { *__errno_location() = err };
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    unsafe { *__error() = err };
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point.
pub unsafe extern "C" fn mi_malloc_usable_size(p: *const c_void) -> usize {
    unsafe { mi_usable_size(p) }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point.
pub unsafe extern "C" fn mi_malloc_size(p: *const c_void) -> usize {
    unsafe { mi_usable_size(p) }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc heap-region
/// inspection and, when owned by mimalloc, valid to free.
pub unsafe extern "C" fn mi_cfree(p: *mut c_void) {
    if unsafe { mi_is_in_heap_region(p.cast_const()) } {
        unsafe { mi_free(p) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` must be valid for writing one pointer. On success the written pointer
/// must be released through mimalloc-compatible free APIs.
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
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
pub unsafe extern "C" fn mi_memalign(alignment: usize, size: usize) -> *mut c_void {
    let p = unsafe { mi_malloc_aligned(size, alignment) };
    crate::mi_assert_internal!(_mi_is_aligned(p, alignment));
    p
}

#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
pub unsafe extern "C" fn mi_valloc(size: usize) -> *mut c_void {
    unsafe { mi_memalign(crate::os::_mi_os_page_size_typed().get(), size) }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
pub unsafe extern "C" fn mi_pvalloc(size: usize) -> *mut c_void {
    let psize = unsafe { crate::os::_mi_os_page_size_typed().get() };
    if size >= usize::MAX - psize {
        return ptr::null_mut();
    }
    let asize = _mi_align_up(size, psize);
    unsafe { mi_malloc_aligned(asize, psize) }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
pub unsafe extern "C" fn mi_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    let p = unsafe { mi_malloc_aligned(size, alignment) };
    crate::mi_assert_internal!(_mi_is_aligned(p, alignment));
    p
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point. The returned pointer, when non-null, must
/// be released through mimalloc-compatible free APIs.
pub unsafe extern "C" fn mi_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        let newp = unsafe { mi_realloc(p, req.total().get()) };
        if newp.is_null() {
            unsafe { mi_set_errno(ENOMEM) };
        }
        newp
    } else {
        unsafe { mi_set_errno(EOVERFLOW) };
        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `ptrp` must be valid for reading and writing one pointer. `*ptrp` may be
/// null or must be a mimalloc-compatible allocation pointer.
pub unsafe extern "C" fn mi_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int {
    crate::mi_assert!(size != 0);
    crate::mi_assert!(!ptrp.is_null());
    if ptrp.is_null() || size == 0 {
        unsafe { mi_set_errno(EINVAL) };
        return EINVAL;
    }

    let Ok(req) = crate::core_types::CallocRequest::new(count, size) else {
        unsafe { mi_set_errno(EOVERFLOW) };
        return EOVERFLOW;
    };
    let total = req.total().get();

    if total == 0 {
        // SAFETY: `ptrp` was checked non-null and is valid for one pointer.
        let oldp = unsafe { *ptrp };
        // SAFETY: `oldp` is null or a mimalloc-compatible allocation pointer
        // by the function contract.
        unsafe { mi_free(oldp) };
        // SAFETY: `ptrp` was checked non-null and is valid for one pointer.
        unsafe { *ptrp = ptr::null_mut() };
        return 0;
    }

    // SAFETY: `ptrp` was checked non-null and is valid for one pointer.
    let oldp = unsafe { *ptrp };
    // SAFETY: `oldp` is null or a mimalloc-compatible allocation pointer by
    // the function contract.
    let newp = unsafe { mi_realloc(oldp, total) };
    if newp.is_null() {
        unsafe { mi_set_errno(ENOMEM) };
        return ENOMEM;
    }
    unsafe { *ptrp = newp };
    0
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point.
pub unsafe extern "C" fn mi__expand(p: *mut c_void, newsize: usize) -> *mut c_void {
    let res = unsafe { mi_expand(p, newsize) };
    if res.is_null() {
        unsafe { mi_set_errno(ENOMEM) };
    }
    res
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `s` may be null or must point to a readable nul-terminated multibyte string.
/// The returned pointer, when non-null, must be freed through mimalloc.
pub unsafe extern "C" fn mi_mbsdup(s: *const u8) -> *mut u8 {
    unsafe { mi_strdup(s.cast::<i8>()).cast::<u8>() }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `s` may be null or must point to a readable nul-terminated wide string. The
/// returned pointer, when non-null, must be freed through mimalloc.
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
/// # Safety
///
/// `buf` and `size`, when non-null, must be valid for writing one value.
/// `_name`, when non-null, must point to a readable nul-terminated wide string.
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
/// # Safety
///
/// `p` may be null or must be a mimalloc-compatible allocation pointer. The
/// returned pointer, when non-null, must be released through mimalloc-compatible
/// free APIs.
pub unsafe extern "C" fn mi_aligned_offset_recalloc(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    unsafe { crate::alloc_aligned::mi_recalloc_aligned_at(p, newcount, size, alignment, offset) }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a mimalloc-compatible allocation pointer. The
/// returned pointer, when non-null, must be released through mimalloc-compatible
/// free APIs.
pub unsafe extern "C" fn mi_aligned_recalloc(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    unsafe { crate::alloc_aligned::mi_recalloc_aligned(p, newcount, size, alignment) }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `buf` and `size`, when non-null, must be valid for writing one value. `name`
/// must point to a readable nul-terminated C string.
pub unsafe extern "C" fn mi_dupenv_s(
    buf: *mut *mut i8,
    size: *mut usize,
    name: *const i8,
) -> c_int {
    if !size.is_null() {
        unsafe { *size = 0 };
    }
    if buf.is_null() || name.is_null() {
        return EINVAL;
    }
    let value = unsafe { getenv(name) };
    if value.is_null() {
        unsafe { *buf = ptr::null_mut() };
        return 0;
    }
    let dup = unsafe { mi_strdup(value) };
    unsafe { *buf = dup };
    if dup.is_null() {
        return ENOMEM;
    }
    if !size.is_null() {
        unsafe { *size = CStr::from_ptr(value).to_bytes_with_nul().len() };
    }
    0
}
