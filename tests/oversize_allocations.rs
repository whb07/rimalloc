use std::os::raw::c_long;

use mechanical_mimalloc::raw::mimalloc::{
    mi_free, mi_malloc, mi_option_get, mi_option_set, mi_option_t,
};

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

#[test]
fn max_alloc_size_minus_one_returns_null_without_abort() {
    let _quiet_expected_error = unsafe { ShowErrorsGuard::disabled() };
    let p = unsafe { mi_malloc(isize::MAX as usize - 1) };
    assert!(p.is_null());
    unsafe { mi_free(p) };
}
