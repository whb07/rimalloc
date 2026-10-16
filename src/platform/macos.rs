//! Rust entry points for the macOS malloc-zone shim in `src/macos_interpose.c`.
//!
//! libmalloc and dyld hand the shim pointers owned by every zone in the
//! process. These calls classify ownership with one checked page-map lookup
//! and leave foreign pointers for the shim to return to libmalloc.

use core::ffi::c_void;

/// Frees `p` and returns true when this allocator owns it; returns false and
/// leaves `p` untouched otherwise.
///
/// # Safety
/// `p` may be null or arbitrary. A pointer owned by this allocator must be a
/// live allocation that is not concurrently freed or reallocated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_macos_free(p: *mut c_void) -> bool {
    unsafe { crate::allocator::free::mi_free_if_owned(p) }
}

/// Returns the usable size of the allocation that starts at `p`, or 0 when
/// this allocator did not return `p`, as `malloc_size` and a zone's `size`
/// callback require.
///
/// # Safety
/// `p` may be null or arbitrary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_macos_size(p: *const c_void) -> usize {
    unsafe { crate::allocator::free::mi_allocation_size(p) }
}
