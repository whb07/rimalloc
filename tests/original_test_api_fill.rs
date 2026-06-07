#[path = "original_testhelper.rs"]
mod original_testhelper;

use std::ffi::{CStr, CString};
use std::ptr;

use mechanical_mimalloc::{alloc, alloc_aligned, alloc_posix, free, heap, stats};

fn original_test_api_fill_main() -> i32 {
    original_testhelper::reset_test_summary();

    unsafe {
        let p = alloc::mi_malloc(64);
        let _ = original_testhelper::check("test-api-fill.c:mi_malloc", !p.is_null());

        let p = alloc::mi_realloc(p, 128);
        let _ = original_testhelper::check("test-api-fill.c:mi_realloc", !p.is_null());
        if !p.is_null() {
            free::mi_free(p);
        }

        let heap_handle = heap::mi_heap_new();
        let _ = original_testhelper::check("test-api-fill.c:mi_heap_new", !heap_handle.is_null());
        if !heap_handle.is_null() {
            let hp = alloc::mi_heap_calloc(heap_handle, 4, 32);
            let hp_ok = !hp.is_null() && heap::mi_heap_contains(heap_handle, hp);
            let _ = original_testhelper::check("test-api-fill.c:mi_heap_calloc", hp_ok);
            if !hp.is_null() {
                free::mi_free(hp);
            }
            heap::mi_heap_delete(heap_handle);
        }

        let aligned = alloc_aligned::mi_malloc_aligned(256, 64);
        let aligned_ok = !aligned.is_null() && (aligned as usize).is_multiple_of(64);
        let _ = original_testhelper::check("test-api-fill.c:mi_malloc_aligned", aligned_ok);
        if !aligned.is_null() {
            free::mi_free(aligned);
        }

        let mut memptr = ptr::null_mut();
        let rc = alloc_posix::mi_posix_memalign(&mut memptr, 128, 512);
        let posix_ok = rc == 0 && !memptr.is_null() && (memptr as usize).is_multiple_of(128);
        let _ = original_testhelper::check("test-api-fill.c:mi_posix_memalign", posix_ok);
        if !memptr.is_null() {
            free::mi_free(memptr);
        }

        let hello = CString::new("fill").expect("CString literal");
        let dup = alloc::mi_strndup(hello.as_ptr(), 4);
        let dup_ok = !dup.is_null() && CStr::from_ptr(dup) == hello.as_c_str();
        let _ = original_testhelper::check("test-api-fill.c:mi_strndup", dup_ok);
        if !dup.is_null() {
            free::mi_free(dup.cast());
        }

        let mut json = vec![0_i8; 4096];
        let json_ptr = stats::mi_stats_get_json(json.len(), json.as_mut_ptr());
        let json_ok = !json_ptr.is_null() && CStr::from_ptr(json_ptr).to_bytes().starts_with(b"{");
        let _ = original_testhelper::check("test-api-fill.c:mi_stats_get_json", json_ok);
    }

    original_testhelper::print_test_summary()
}

#[test]
fn original_test_api_fill_harness() {
    assert_eq!(original_test_api_fill_main(), 0);
}
