#![allow(non_snake_case)]

#[path = "original_testhelper.rs"]
mod original_testhelper;

use std::ffi::c_void;

use rimalloc::raw;

struct Static {
    p: *mut c_void,
}

impl Static {
    fn new() -> Self {
        eprintln!("static-dep: static constructor");
        let p = unsafe { raw::mi_malloc(64) };
        assert!(!p.is_null());
        Self { p }
    }
}

impl Drop for Static {
    fn drop(&mut self) {
        unsafe {
            raw::mi_free(self.p);
        }
        eprintln!("static-dep: static destructor");
    }
}

fn Test() {
    let guard = Static::new();
    assert!(!guard.p.is_null());
}

#[cfg(windows)]
type Hinstance = *mut c_void;
#[cfg(windows)]
type Dword = u32;
#[cfg(windows)]
type Lpvoid = *mut c_void;
#[cfg(windows)]
type Bool = i32;

#[cfg(windows)]
unsafe extern "system" fn DllMain(_module: Hinstance, _reason: Dword, _reserved: Lpvoid) -> Bool {
    Test();
    1
}

#[cfg(not(windows))]
fn DllMain() -> bool {
    Test();
    true
}

#[test]
fn original_main_static_dep_harness() {
    original_testhelper::reset_test_summary();
    let _ = original_testhelper::check(
        "main-static-dep.cpp:Test",
        std::panic::catch_unwind(Test).is_ok(),
    );
    let dll_ok = {
        #[cfg(windows)]
        {
            unsafe { DllMain(std::ptr::null_mut(), 1, std::ptr::null_mut()) != 0 }
        }
        #[cfg(not(windows))]
        {
            DllMain()
        }
    };
    let _ = original_testhelper::check("main-static-dep.cpp:DllMain", dll_ok);
    assert_eq!(original_testhelper::print_test_summary(), 0);
}
