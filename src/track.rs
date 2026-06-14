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

#[cfg(all(feature = "track-valgrind", feature = "track-asan"))]
compile_error!("track-valgrind and track-asan are mutually exclusive");

pub const MI_TRACK_VALGRIND: usize = if cfg!(feature = "track-valgrind") {
    1
} else {
    0
};
pub const MI_TRACK_ASAN: usize = if cfg!(feature = "track-asan") { 1 } else { 0 };
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

#[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
unsafe extern "C" {
    fn _mi_track_malloc_size_shim(p: *mut c_void, reqsize: usize, size: usize, zero: bool);
    fn _mi_track_free_size_shim(p: *mut c_void, size: usize);
    fn _mi_track_resize_shim(p: *mut c_void, oldsize: usize, newsize: usize);
    fn _mi_track_mem_defined_shim(p: *mut c_void, size: usize);
    fn _mi_track_mem_undefined_shim(p: *mut c_void, size: usize);
    fn _mi_track_mem_noaccess_shim(p: *mut c_void, size: usize);
}

#[inline]
pub unsafe fn mi_track_malloc_size(p: *mut c_void, reqsize: usize, size: usize, zero: bool) {
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_malloc_size_shim(p, reqsize, size, zero);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        let _ = (p, reqsize, size, zero);
    }
}

#[inline]
pub unsafe fn mi_track_free_size(p: *mut c_void, size: usize) {
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_free_size_shim(p, size);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        let _ = (p, size);
    }
}

#[inline]
pub unsafe fn mi_track_mem_defined(p: *mut c_void, size: usize) {
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_mem_defined_shim(p, size);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        let _ = (p, size);
    }
}

#[inline]
pub unsafe fn mi_track_mem_undefined(p: *mut c_void, size: usize) {
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_mem_undefined_shim(p, size);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        let _ = (p, size);
    }
}

#[inline]
pub unsafe fn mi_track_mem_noaccess(p: *mut c_void, size: usize) {
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_mem_noaccess_shim(p, size);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        let _ = (p, size);
    }
}

#[inline]
pub unsafe fn mi_track_resize(p: *mut c_void, oldsize: usize, newsize: usize) {
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_resize_shim(p, oldsize, newsize);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        unsafe { mi_track_free_size(p, oldsize) };
        unsafe { mi_track_malloc(p, newsize, false) };
    }
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
    if crate::types::MI_PADDING != 0 {
        crate::mi_assert_internal!(
            unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) } == reqsize
        );
        if MI_TRACK_ENABLED != 0 {
            unsafe { mi_track_malloc_size(p, reqsize, reqsize, zero) };
        }
    } else {
        crate::mi_assert_internal!(
            unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) } >= reqsize
        );
        if MI_TRACK_ENABLED != 0 {
            let size = unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) };
            unsafe { mi_track_malloc_size(p, reqsize, size, zero) };
        }
    }
}
