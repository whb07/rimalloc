//! Mechanical translation of `include/mimalloc/track.h`.

// Retains tracking mode constants and no-op hooks for builds without an active
// tracking backend.
#![allow(dead_code)]

use core::ffi::c_void;

use crate::core_types::TrackMode;

pub const MI_TRACK_H: () = ();

pub(crate) const MI_TRACK_MODE: TrackMode = TrackMode::CURRENT;
pub const MI_TRACK_VALGRIND: usize = MI_TRACK_MODE.valgrind_flag();
pub const MI_TRACK_ASAN: usize = MI_TRACK_MODE.asan_flag();
pub const MI_TRACK_ETW: usize = 0;

pub const MI_TRACK_ENABLED: usize = MI_TRACK_MODE.enabled_flag();

pub const MI_TRACK_HEAP_DESTROY: usize = MI_TRACK_MODE.heap_destroy_flag();

pub const MI_TRACK_TOOL: &str = MI_TRACK_MODE.tool_name();

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
        let usable = unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) };
        crate::internal::mi_assert_internal!(
            usable == reqsize
                || (reqsize == 0 && usable >= crate::core_types::RequestedSize::MIN_ZERO_SIZE)
        );
        if MI_TRACK_ENABLED != 0 {
            unsafe { mi_track_malloc_size(p, reqsize, usable, zero) };
        }
    } else {
        crate::internal::mi_assert_internal!(
            unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) } >= reqsize
        );
        if MI_TRACK_ENABLED != 0 {
            let size = unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) };
            unsafe { mi_track_malloc_size(p, reqsize, size, zero) };
        }
    }
}
