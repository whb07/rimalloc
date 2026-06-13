#[path = "original_testhelper.rs"]
mod original_testhelper;

use std::ffi::c_void;

use mechanical_mimalloc::{alloc, free, heap};

fn test_heap(p_out: *mut *mut c_void) {
    unsafe {
        let test_heap = heap::mi_heap_new();
        assert!(!test_heap.is_null());

        let p = alloc::mi_heap_malloc(test_heap, 256);
        assert!(!p.is_null());
        assert!(heap::mi_heap_contains(test_heap, p));
        assert!(heap::mi_heap_check_owned(test_heap, p));

        if !p_out.is_null() {
            *p_out = p;
        }

        free::mi_free(p);
        heap::mi_heap_delete(test_heap);
    }
}

fn test_large() {
    let size = 8 * 1024 * 1024;
    unsafe {
        let p = alloc::mi_malloc(size) as *mut u8;
        assert!(!p.is_null());
        *p = 0xA5;
        *p.add(size - 1) = 0x5A;
        assert_eq!(*p, 0xA5);
        assert_eq!(*p.add(size - 1), 0x5A);
        assert!(free::mi_usable_size(p.cast()) >= size);
        free::mi_free(p.cast());
    }
}

fn original_main() -> i32 {
    original_testhelper::reset_test_summary();

    let mut p: *mut c_void = std::ptr::null_mut();
    test_heap(&mut p);
    let heap_ok = !p.is_null();
    let _ = original_testhelper::check("main.c:test_heap", heap_ok);

    let large_ok = std::panic::catch_unwind(test_large).is_ok();
    let _ = original_testhelper::check("main.c:test_large", large_ok);

    original_testhelper::print_test_summary()
}

#[test]
fn original_main_harness() {
    assert_eq!(original_main(), 0);
}
