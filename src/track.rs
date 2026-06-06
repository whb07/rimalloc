//! Mechanical translation of `include/mimalloc/track.h`.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;

pub const MI_TRACK_H: () = ();

pub const MI_TRACK_VALGRIND: usize = 0;
pub const MI_TRACK_ASAN: usize = 0;
pub const MI_TRACK_ETW: usize = 0;

pub const MI_TRACK_ENABLED: usize =
    if MI_TRACK_VALGRIND != 0 || MI_TRACK_ASAN != 0 || MI_TRACK_ETW != 0 {
        1
    } else {
        0
    };

pub const MI_TRACK_HEAP_DESTROY: usize = if MI_TRACK_VALGRIND != 0 || MI_TRACK_ETW != 0 {
    1
} else {
    0
};

pub const MI_TRACK_TOOL: &str = if MI_TRACK_VALGRIND != 0 {
    "valgrind"
} else if MI_TRACK_ASAN != 0 {
    "asan"
} else if MI_TRACK_ETW != 0 {
    "ETW"
} else {
    "none"
};

#[inline]
pub unsafe fn mi_track_malloc_size(_p: *mut c_void, _reqsize: usize, _size: usize, _zero: bool) {}

#[inline]
pub unsafe fn mi_track_free_size(_p: *mut c_void, _size: usize) {}

#[inline]
pub unsafe fn mi_track_mem_defined(_p: *mut c_void, _size: usize) {}

#[inline]
pub unsafe fn mi_track_mem_undefined(_p: *mut c_void, _size: usize) {}

#[inline]
pub unsafe fn mi_track_mem_noaccess(_p: *mut c_void, _size: usize) {}

#[inline]
pub unsafe fn mi_track_resize(p: *mut c_void, oldsize: usize, newsize: usize) {
    unsafe { mi_track_free_size(p, oldsize) };
    unsafe { mi_track_malloc(p, newsize, false) };
}

#[inline]
pub unsafe fn mi_track_align(p: *mut c_void, _alignedp: *mut c_void, offset: usize, _size: usize) {
    unsafe { mi_track_mem_noaccess(p, offset) };
}

#[inline]
pub unsafe fn mi_track_init() {}

#[inline]
pub unsafe fn mi_track_done() {}

#[inline]
pub unsafe fn mi_track_malloc(p: *mut c_void, reqsize: usize, zero: bool) {
    if p.is_null() {
        return;
    }
    let size = unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) };
    debug_assert!(size >= reqsize);
    unsafe { mi_track_malloc_size(p, reqsize, size, zero) };
}
