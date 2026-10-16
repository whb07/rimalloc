use core::alloc::Layout;
use core::ffi::c_void;
use core::ptr;
use std::os::raw::c_long;

use mechanical_mimalloc::raw::mimalloc::*;
use mechanical_mimalloc::{Allocation, Heap};

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
fn tiny_safe_allocations_preserve_requested_alignment() {
    let heap = Heap::new().expect("heap");
    for alignment in [2, 4, 8, 16] {
        for size in 1..alignment {
            let layout = Layout::from_size_align(size, alignment).expect("layout");
            // Keep distinct blocks live instead of repeatedly reusing one address.
            let allocations: [_; 32] = core::array::from_fn(|_| {
                (
                    Allocation::new(layout).expect("owned allocation"),
                    heap.allocate(layout).expect("heap allocation"),
                    heap.allocate_zeroed(layout).expect("zeroed heap allocation"),
                )
            });
            for (owned, allocation, zeroed) in &allocations {
                assert_eq!(owned.as_ptr() as usize % alignment, 0);
                assert_eq!(allocation.as_ptr() as usize % alignment, 0);
                assert_eq!(zeroed.as_ptr() as usize % alignment, 0);
                assert!(zeroed.as_bytes().iter().all(|&byte| byte == 0));
            }
        }
    }
}

#[test]
fn tiny_heap_resizes_preserve_alignment_and_initialized_contents() {
    let heap = Heap::new().expect("heap");
    for alignment in [8, 16] {
        for size in 1..alignment {
            let large = Layout::from_size_align(64, alignment).expect("large layout");
            let small = Layout::from_size_align(size, alignment).expect("small layout");
            let allocations: [_; 32] = core::array::from_fn(|_| {
                let mut allocation = heap.allocate_zeroed(large).expect("allocation");
                allocation.as_mut_bytes().fill(0xA5);
                allocation
            });
            let resized = allocations.map(|allocation| allocation.resize(small).expect("shrink"));
            for allocation in &resized {
                assert_eq!(allocation.as_ptr() as usize % alignment, 0);
                assert_eq!(allocation.layout(), small);
                assert!(allocation.as_bytes().iter().all(|&byte| byte == 0xA5));
            }
            for allocation in resized {
                let grown = allocation.resize(large).expect("grow");
                assert_eq!(grown.as_ptr() as usize % alignment, 0);
                assert!(grown.as_bytes()[..size].iter().all(|&byte| byte == 0xA5));
                assert!(grown.as_bytes()[size..].iter().all(|&byte| byte == 0));
            }
        }
    }
}
