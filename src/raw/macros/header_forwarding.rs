#![allow(non_snake_case)]

use core::ffi::c_void;

use super::mi_interpose_s;

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
// SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe fn mi_forward1<R, A>(fun: unsafe extern "C" fn(A) -> R, x: A) -> R {
    unsafe { fun(x) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
// SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe fn mi_forward3<R, A, B, C>(
    fun: unsafe extern "C" fn(A, B, C) -> R,
    x: A,
    y: B,
    z: C,
) -> R {
    // SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { fun(x, y, z) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
// SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe fn mi_forward02<A, B>(fun: unsafe extern "C" fn(A, B), x: A, y: B) {
    unsafe { fun(x, y) }
}

#[inline]
pub const fn mi_interpose_fun(oldfun: *const c_void, newfun: *const c_void) -> mi_interpose_s {
    mi_interpose_s {
        replacement: newfun,
        target: oldfun,
    }
}

#[inline]
pub const fn MI_INTERPOSE_DECLS<const N: usize>(
    entries: [mi_interpose_s; N],
) -> [mi_interpose_s; N] {
    entries
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[allow(improper_ctypes_definitions)]
#[inline]
pub unsafe fn mi_malloc_good_size_forward(size: usize) -> usize {
    // SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_malloc_good_size(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_vfree_forward(p: *mut c_void) {
    // SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_malloc_size_forward(p: *const c_void) -> usize {
    // SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_aligned_alloc_forward(alignment: usize, size: usize) -> *mut c_void {
    // SAFETY: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}
