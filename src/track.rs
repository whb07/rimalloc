//! Mechanical translation of `include/mimalloc/track.h`.

#![allow(non_camel_case_types, non_upper_case_globals, unsafe_op_in_unsafe_fn)]

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

/// # Safety
///
/// `p..p + size` must describe the allocation that should be reported to the
/// enabled tracking backend. `reqsize` must be the requested user size.
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

/// # Safety
///
/// `p..p + size` must describe the allocation being released from the enabled
/// tracking backend.
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

/// # Safety
///
/// `p..p + size` must be a valid tracked memory range to mark as defined.
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

/// # Safety
///
/// `p..p + size` must be a valid tracked memory range to mark as undefined.
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

/// # Safety
///
/// `p..p + size` must be a valid tracked memory range to mark as inaccessible.
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

/// # Safety
///
/// `p..p + oldsize` must describe the old tracked allocation, and
/// `p..p + newsize` must describe its resized range.
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

/// # Safety
///
/// `p..p + offset` must describe the leading inaccessible part of an aligned
/// allocation reported to the tracking backend.
#[inline]
pub unsafe fn mi_track_align(p: *mut c_void, _alignedp: *mut c_void, offset: usize, _size: usize) {
    unsafe { mi_track_mem_noaccess(p, offset) };
}

/// # Safety
///
/// Must be called according to the allocator tracking backend lifecycle.
#[inline]
pub unsafe fn mi_track_init() {}

/// # Safety
///
/// Must be called according to the allocator tracking backend lifecycle, after
/// tracking users have stopped.
#[inline]
pub unsafe fn mi_track_done() {}

/// # Safety
///
/// `p` may be null. A non-null pointer must refer to a live allocation whose
/// usable size is at least `reqsize`.
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
