//! Mechanical translation of `include/mimalloc/track.h`.

use core::ffi::c_void;

use crate::core_types::TrackMode;

#[allow(dead_code)]
pub const MI_TRACK_H: () = ();

pub(crate) const MI_TRACK_MODE: TrackMode = TrackMode::CURRENT;
pub const MI_TRACK_VALGRIND: usize = MI_TRACK_MODE.valgrind_flag();
pub const MI_TRACK_ASAN: usize = MI_TRACK_MODE.asan_flag();
pub const MI_TRACK_ETW: usize = 0;

pub const MI_TRACK_ENABLED: usize = MI_TRACK_MODE.enabled_flag();

#[allow(dead_code)]
pub const MI_TRACK_HEAP_DESTROY: usize = MI_TRACK_MODE.heap_destroy_flag();

#[allow(dead_code)]
pub const MI_TRACK_TOOL: &str = MI_TRACK_MODE.tool_name();

// SAFETY[AUDIT:tracking-hooks]: the tracking shim symbols are supplied by the
// C tracking bridge for the selected tracking feature and mirror C mimalloc's
// Valgrind/ASan hook signatures.
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
    // SAFETY[AUDIT:tracking-hooks]: callers provide the tracked allocation
    // range and requested size; the cfg-gated shim forwards that range to the
    // selected tracking backend without taking ownership.
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
    // SAFETY[AUDIT:tracking-hooks]: callers provide the tracked allocation
    // range being released; the cfg-gated shim only reports it to the backend.
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
    // SAFETY[AUDIT:tracking-hooks]: callers provide a live tracked memory
    // range; the cfg-gated shim only marks the range defined for the backend.
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
    // SAFETY[AUDIT:tracking-hooks]: callers provide a live tracked memory
    // range; the cfg-gated shim only marks the range undefined for the backend.
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
    // SAFETY[AUDIT:tracking-hooks]: callers provide a live tracked memory
    // range; the cfg-gated shim only marks the range inaccessible for the
    // backend.
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
#[allow(dead_code)]
pub unsafe fn mi_track_resize(p: *mut c_void, oldsize: usize, newsize: usize) {
    // SAFETY[AUDIT:tracking-hooks]: callers provide the old and new tracked
    // ranges for the same allocation; the cfg-gated shim reports the resize.
    #[cfg(any(feature = "track-valgrind", feature = "track-asan"))]
    unsafe {
        _mi_track_resize_shim(p, oldsize, newsize);
    }

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    {
        // SAFETY[AUDIT:tracking-hooks]: without an external tracking backend,
        // resize reporting is equivalent to release followed by allocation
        // reporting for the same caller-provided allocation range.
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
    // SAFETY[AUDIT:tracking-hooks]: callers provide the leading range of an
    // aligned allocation that must be hidden from the tracking backend.
    unsafe { mi_track_mem_noaccess(p, offset) };
}

/// # Safety
///
/// Must be called according to the allocator tracking backend lifecycle.
#[inline]
#[allow(dead_code)]
pub unsafe fn mi_track_init() {}

/// # Safety
///
/// Must be called according to the allocator tracking backend lifecycle, after
/// tracking users have stopped.
#[inline]
#[allow(dead_code)]
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
        // SAFETY[AUDIT:tracking-hooks]: `p` is a non-null allocation pointer
        // from the caller; `mi_usable_size` is the allocator's bounded metadata
        // query for tracking and assertion purposes.
        let usable = unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) };
        crate::internal::mi_assert_internal!(
            usable == reqsize
                || (reqsize == 0 && usable >= crate::core_types::RequestedSize::MIN_ZERO_SIZE)
        );
        if MI_TRACK_ENABLED != 0 {
            // SAFETY[AUDIT:tracking-hooks]: `usable` came from allocator
            // metadata for `p` and bounds the range reported to tracking.
            unsafe { mi_track_malloc_size(p, reqsize, usable, zero) };
        }
    } else {
        // SAFETY[AUDIT:tracking-hooks]: `p` is a non-null allocation pointer
        // from the caller; the usable-size query checks the request bound.
        crate::internal::mi_assert_internal!(
            unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) } >= reqsize
        );
        if MI_TRACK_ENABLED != 0 {
            // SAFETY[AUDIT:tracking-hooks]: `p` is non-null and the
            // usable-size result bounds the tracked allocation range.
            let size = unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) };
            // SAFETY[AUDIT:tracking-hooks]: `size` came from allocator
            // metadata for `p` and bounds the range reported to tracking.
            unsafe { mi_track_malloc_size(p, reqsize, size, zero) };
        }
    }
}
