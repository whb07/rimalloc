#[path = "original_testhelper.rs"]
mod original_testhelper;

use std::ffi::{CStr, CString};

use mechanical_mimalloc::{alloc, free, options};

fn original_main_override() -> i32 {
    original_testhelper::reset_test_summary();

    let p = unsafe { alloc::mi_malloc(128) };
    let _ = original_testhelper::check("main-override.c:mi_malloc", !p.is_null());
    if !p.is_null() {
        unsafe { free::mi_free(p) };
    }

    let hello = CString::new("override").expect("CString literal");
    let dup = unsafe { alloc::mi_strdup(hello.as_ptr()) };
    let dup_ok = if dup.is_null() {
        false
    } else {
        let value = unsafe { CStr::from_ptr(dup) } == hello.as_c_str();
        unsafe { free::mi_free(dup.cast()) };
        value
    };
    let _ = original_testhelper::check("main-override.c:mi_strdup", dup_ok);

    let _ = original_testhelper::check("main-override.c:mi_version", options::mi_version() > 0);
    original_testhelper::print_test_summary()
}

#[test]
fn original_main_override_harness() {
    assert_eq!(original_main_override(), 0);
}
