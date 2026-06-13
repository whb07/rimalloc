use core::ffi::c_void;
use core::ptr;

use mechanical_mimalloc::mimalloc::*;

fn assert_aligned_at(p: *mut c_void, alignment: usize, offset: usize) {
    assert!(!p.is_null());
    assert_eq!((p as usize + offset) % alignment, 0);
}

unsafe fn assert_zeroed(p: *mut c_void, size: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(p.cast::<u8>(), size) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

#[test]
fn global_aligned_exports_link_and_allocate() {
    unsafe {
        let p = mi_malloc_aligned(129, 64);
        assert_aligned_at(p, 64, 0);
        mi_free(p);

        let p = mi_malloc_aligned_at(129, 64, 8);
        assert_aligned_at(p, 64, 8);
        mi_free(p);

        let p = mi_zalloc_aligned(129, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 129);
        mi_free(p);

        let p = mi_zalloc_aligned_at(129, 64, 16);
        assert_aligned_at(p, 64, 16);
        assert_zeroed(p, 129);
        mi_free(p);

        let p = mi_calloc_aligned(17, 8, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 17 * 8);
        mi_free(p);

        let p = mi_calloc_aligned_at(17, 8, 64, 24);
        assert_aligned_at(p, 64, 24);
        assert_zeroed(p, 17 * 8);
        mi_free(p);
    }
}

#[test]
fn aligned_realloc_exports_link_and_allocate_from_null() {
    unsafe {
        let p = mi_realloc_aligned(ptr::null_mut(), 96, 64);
        assert_aligned_at(p, 64, 0);
        mi_free(p);

        let p = mi_realloc_aligned_at(ptr::null_mut(), 96, 64, 8);
        assert_aligned_at(p, 64, 8);
        mi_free(p);

        let p = mi_rezalloc_aligned(ptr::null_mut(), 96, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 96);
        mi_free(p);

        let p = mi_rezalloc_aligned_at(ptr::null_mut(), 96, 64, 16);
        assert_aligned_at(p, 64, 16);
        assert_zeroed(p, 96);
        mi_free(p);

        let p = mi_recalloc_aligned(ptr::null_mut(), 12, 8, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 12 * 8);
        mi_free(p);

        let p = mi_recalloc_aligned_at(ptr::null_mut(), 12, 8, 64, 24);
        assert_aligned_at(p, 64, 24);
        assert_zeroed(p, 12 * 8);
        mi_free(p);
    }
}

#[test]
fn heap_aligned_exports_link_and_allocate_from_requested_heap() {
    unsafe {
        let heap = mi_heap_new();
        assert!(!heap.is_null());

        let p = mi_heap_malloc_aligned(heap, 96, 64);
        assert_aligned_at(p, 64, 0);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_malloc_aligned_at(heap, 96, 64, 8);
        assert_aligned_at(p, 64, 8);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_zalloc_aligned_at(heap, 96, 64, 16);
        assert_aligned_at(p, 64, 16);
        assert_zeroed(p, 96);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_calloc_aligned(heap, 12, 8, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 12 * 8);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_calloc_aligned_at(heap, 12, 8, 64, 24);
        assert_aligned_at(p, 64, 24);
        assert_zeroed(p, 12 * 8);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_realloc_aligned(heap, ptr::null_mut(), 96, 64);
        assert_aligned_at(p, 64, 0);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_realloc_aligned_at(heap, ptr::null_mut(), 96, 64, 8);
        assert_aligned_at(p, 64, 8);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_rezalloc_aligned(heap, ptr::null_mut(), 96, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 96);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_rezalloc_aligned_at(heap, ptr::null_mut(), 96, 64, 16);
        assert_aligned_at(p, 64, 16);
        assert_zeroed(p, 96);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_recalloc_aligned(heap, ptr::null_mut(), 12, 8, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 12 * 8);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        let p = mi_heap_recalloc_aligned_at(heap, ptr::null_mut(), 12, 8, 64, 24);
        assert_aligned_at(p, 64, 24);
        assert_zeroed(p, 12 * 8);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);

        mi_heap_delete(heap);
    }
}

#[test]
fn usable_size_and_typed_aligned_exports_link_and_report_block_size() {
    unsafe {
        let good = mi_good_size(33);
        assert!(good >= 33);
        assert_eq!(mi_malloc_good_size(33), good);

        let p = mi_malloc(33);
        assert!(!p.is_null());
        assert!(mi_malloc_size(p) >= 33);
        assert_eq!(mi_malloc_size(p), mi_malloc_usable_size(p));
        mi_free(p);

        let mut block_size = 0;
        let p = mi_umalloc_aligned(96, 64, &mut block_size);
        assert_aligned_at(p, 64, 0);
        assert!(block_size >= 96);
        mi_free(p);

        let mut block_size = 0;
        let p = mi_uzalloc_aligned(96, 64, &mut block_size);
        assert_aligned_at(p, 64, 0);
        assert!(block_size >= 96);
        assert_zeroed(p, 96);
        mi_free(p);

        let mut block_size = 0;
        let p = mi_uzalloc_small(32, &mut block_size);
        assert!(!p.is_null());
        assert!(block_size >= 32);
        assert_zeroed(p, 32);
        mi_free(p);
    }
}
