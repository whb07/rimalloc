use core::ffi::c_void;
use core::ptr;
use std::os::raw::c_long;

use mechanical_mimalloc::raw::mimalloc::*;

struct ShowErrorsGuard(c_long);

impl ShowErrorsGuard {
    unsafe fn disabled() -> Self {
        let option = mi_option_t::mi_option_show_errors;
        let previous = unsafe { mi_option_get(option) };
        unsafe { mi_option_set(option, 0) };
        Self(previous)
    }
}

impl Drop for ShowErrorsGuard {
    fn drop(&mut self) {
        unsafe { mi_option_set(mi_option_t::mi_option_show_errors, self.0) };
    }
}

fn assert_freeable(p: *mut c_void) {
    assert!(!p.is_null());
    unsafe { mi_free(p) };
}

fn assert_aligned_at(p: *mut c_void, alignment: usize, offset: usize) {
    assert!(!p.is_null());
    assert_eq!((p as usize + offset) % alignment, 0);
}

#[test]
fn zero_size_allocations_return_freeable_pointers() {
    unsafe {
        assert_freeable(mi_malloc(0));
        assert_freeable(mi_zalloc(0));

        let p = mi_malloc_aligned(0, 64);
        assert_aligned_at(p, 64, 0);
        mi_free(p);

        let p = mi_malloc_aligned_at(0, 64, 8);
        assert_aligned_at(p, 64, 8);
        mi_free(p);
    }
}

#[test]
fn calloc_overflow_returns_null() {
    let _quiet_expected_error = unsafe { ShowErrorsGuard::disabled() };
    let p = unsafe { mi_calloc(usize::MAX, 2) };
    assert!(p.is_null());
}

#[test]
fn realloc_null_allocates_and_existing_realloc_preserves_prefix() {
    unsafe {
        let p = mi_realloc(ptr::null_mut(), 16).cast::<u8>();
        assert!(!p.is_null());
        for i in 0..16 {
            *p.add(i) = i as u8;
        }

        let q = mi_realloc(p.cast(), 64).cast::<u8>();
        assert!(!q.is_null());
        for i in 0..16 {
            assert_eq!(*q.add(i), i as u8);
        }
        mi_free(q.cast());
    }
}
