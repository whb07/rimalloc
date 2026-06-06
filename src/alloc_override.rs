//! Mechanical translation of `src/alloc-override.c`.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unused_macros,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

#[cfg(target_os = "macos")]
use core::ffi::c_char;
use core::ffi::c_void;

pub type mi_nothrow_t = *mut c_void;

pub const MI_OSX_IS_INTERPOSED: usize = 0;

#[macro_export]
macro_rules! MI_FORWARD1 {
    ($fun:path, $x:expr) => {{
        return unsafe { $fun($x) };
    }};
}

#[macro_export]
macro_rules! MI_FORWARD3 {
    ($fun:path, $x:expr, $y:expr, $z:expr) => {{
        return unsafe { $fun($x, $y, $z) };
    }};
}

#[macro_export]
macro_rules! MI_FORWARD02 {
    ($fun:path, $x:expr, $y:expr) => {{
        unsafe { $fun($x, $y) };
        return;
    }};
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_interpose_s {
    pub replacement: *const c_void,
    pub target: *const c_void,
}

#[macro_export]
macro_rules! MI_INTERPOSE_FUN {
    ($oldfun:path, $newfun:path) => {
        $crate::alloc_override::mi_interpose_s {
            replacement: ($newfun as *const ()) as *const core::ffi::c_void,
            target: ($oldfun as *const ()) as *const core::ffi::c_void,
        }
    };
}

unsafe extern "C" {
    pub fn vfree(p: *mut c_void);
    pub fn malloc_size(p: *const c_void) -> usize;
    pub fn malloc_good_size(size: usize) -> usize;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn strndup(s: *const c_char, n: usize) -> *mut c_char;
    fn aligned_alloc(alignment: usize, size: usize) -> *mut c_void;
}

#[cfg(target_os = "macos")]
pub static _mi_interposes_10_7: [mi_interpose_s; 1] =
    [MI_INTERPOSE_FUN!(strndup, crate::mimalloc::mi_strndup)];

#[cfg(target_os = "macos")]
pub static _mi_interposes_10_15: [mi_interpose_s; 1] = [MI_INTERPOSE_FUN!(
    aligned_alloc,
    crate::mimalloc::mi_aligned_alloc
)];

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_size_checked(p: *mut c_void) -> usize {
    if !unsafe { crate::mimalloc::mi_is_in_heap_region(p.cast_const()) } {
        return 0;
    }
    unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) }
}

#[inline]
pub unsafe fn mi_forward1<R, A>(fun: unsafe extern "C" fn(A) -> R, x: A) -> R {
    unsafe { fun(x) }
}

#[inline]
pub unsafe fn mi_forward3<R, A, B, C>(
    fun: unsafe extern "C" fn(A, B, C) -> R,
    x: A,
    y: B,
    z: C,
) -> R {
    unsafe { fun(x, y, z) }
}

#[inline]
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

#[allow(improper_ctypes_definitions)]
#[inline]
pub unsafe fn mi_malloc_good_size_forward(size: usize) -> usize {
    unsafe { crate::mimalloc::mi_malloc_good_size(size) }
}

#[inline]
pub unsafe fn mi_vfree_forward(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[inline]
pub unsafe fn mi_malloc_size_forward(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

#[inline]
pub unsafe fn mi_aligned_alloc_forward(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}
