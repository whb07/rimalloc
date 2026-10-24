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

unsafe fn assert_freeable_with_usable(p: *mut c_void, usable: usize) {
    assert!(!p.is_null());
    assert!(usable >= core::mem::size_of::<usize>());
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

        let mut usable = 0;
        let p = mi_umalloc(0, &mut usable);
        assert_freeable_with_usable(p, usable);

        #[cfg(not(feature = "c-abi"))]
        {
            usable = 0;
            let p = mi_uzalloc(0, &mut usable);
            assert_freeable_with_usable(p, usable);
        }

        assert_freeable(mi_calloc(0, 8));
        assert_freeable(mi_calloc(8, 0));
        assert_freeable(mi_mallocn(0, 8));
        assert_freeable(mi_mallocn(8, 0));

        let p = mi_malloc_aligned(0, 64);
        assert_aligned_at(p, 64, 0);
        mi_free(p);

        let p = mi_malloc_aligned_at(0, 64, 8);
        assert_aligned_at(p, 64, 8);
        mi_free(p);
    }
}

#[test]
fn zero_size_heap_allocations_return_freeable_pointers() {
    unsafe {
        let heap = mi_heap_new();
        assert!(!heap.is_null());

        let p = mi_heap_malloc(heap, 0);
        assert!(!p.is_null());
        mi_free(p);

        let p = mi_heap_zalloc(heap, 0);
        assert!(!p.is_null());
        mi_free(p);

        let p = mi_heap_calloc(heap, 0, 8);
        assert!(!p.is_null());
        mi_free(p);

        let p = mi_heap_mallocn(heap, 8, 0);
        assert!(!p.is_null());
        mi_free(p);

        mi_heap_destroy(heap);
    }
}

#[test]
fn zero_size_good_size_uses_minimum_bin() {
    unsafe {
        let good = mi_good_size(0);
        assert_eq!(good, core::mem::size_of::<usize>());
        assert_eq!(mi_malloc_good_size(0), good);
    }
}

#[test]
fn calloc_overflow_returns_null() {
    let _quiet_expected_error = unsafe { ShowErrorsGuard::disabled() };
    let p = unsafe { mi_calloc(usize::MAX, 2) };
    assert!(p.is_null());
}

#[test]
fn product_allocation_paths_preserve_overflow_and_zero_product_behavior() {
    let _quiet_expected_error = unsafe { ShowErrorsGuard::disabled() };
    unsafe {
        assert!(mi_calloc(usize::MAX, 2).is_null());
        assert!(mi_mallocn(usize::MAX, 2).is_null());

        assert_freeable(mi_calloc(1, 32));
        assert_freeable(mi_mallocn(1, 32));
        assert_freeable(mi_calloc(0, usize::MAX));
        assert_freeable(mi_calloc(usize::MAX, 0));
        assert_freeable(mi_mallocn(0, usize::MAX));
        assert_freeable(mi_mallocn(usize::MAX, 0));

        let heap = mi_heap_new();
        assert!(!heap.is_null());

        assert!(mi_heap_calloc(heap, usize::MAX, 2).is_null());
        assert!(mi_heap_mallocn(heap, usize::MAX, 2).is_null());

        assert_freeable(mi_heap_calloc(heap, 1, 32));
        assert_freeable(mi_heap_mallocn(heap, 1, 32));
        assert_freeable(mi_heap_calloc(heap, 0, usize::MAX));
        assert_freeable(mi_heap_calloc(heap, usize::MAX, 0));
        assert_freeable(mi_heap_mallocn(heap, 0, usize::MAX));
        assert_freeable(mi_heap_mallocn(heap, usize::MAX, 0));

        mi_heap_destroy(heap);
    }
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

#[test]
fn realloc_keeps_half_usable_size_and_moves_below_it() {
    unsafe {
        let p = mi_malloc(256).cast::<u8>();
        assert!(!p.is_null());
        let usable = mi_usable_size(p.cast());
        p.write_bytes(0x5a, usable);

        let q = mi_realloc(p.cast(), usable / 2).cast::<u8>();
        assert_eq!(q, p);
        let smaller = usable / 2 - 1;
        let r = mi_realloc(q.cast(), smaller).cast::<u8>();
        assert!(!r.is_null());
        assert_ne!(r, q);
        assert!(
            core::slice::from_raw_parts(r, smaller)
                .iter()
                .all(|&b| b == 0x5a)
        );
        mi_free(r.cast());
    }
}

#[test]
fn realloc_moves_private_heap_and_interior_allocations_with_their_contents() {
    unsafe {
        let heap = mi_heap_new();
        assert!(!heap.is_null());
        let p = mi_heap_malloc(heap, 256).cast::<u8>();
        assert!(!p.is_null());
        p.write_bytes(0x6b, 256);
        let q = mi_realloc(p.cast(), 256).cast::<u8>();
        assert!(!q.is_null());
        assert_ne!(q, p);
        assert!(
            core::slice::from_raw_parts(q, 256)
                .iter()
                .all(|&b| b == 0x6b)
        );
        mi_heap_destroy(heap);
        mi_free(q.cast());

        let p = mi_malloc_aligned_at(128, 64, 8).cast::<u8>();
        assert_aligned_at(p.cast(), 64, 8);
        p.write_bytes(0x7c, 128);
        let q = mi_realloc(p.cast(), 1024).cast::<u8>();
        assert!(!q.is_null());
        assert!(
            core::slice::from_raw_parts(q, 128)
                .iter()
                .all(|&b| b == 0x7c)
        );
        mi_free(q.cast());
    }
}
