use core::ffi::{c_int, c_void};

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_linux_libc_malloc(size: usize) -> *mut c_void {
    unsafe { super::mi_unix_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_linux_libc_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { super::mi_unix_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_linux_libc_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { super::mi_unix_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_linux_libc_free(p: *mut c_void) {
    unsafe { super::mi_unix_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_linux_libc_cfree(p: *mut c_void) {
    unsafe { super::mi_unix_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_valloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_pvalloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_posix_memalign(p, alignment, size) }
}
