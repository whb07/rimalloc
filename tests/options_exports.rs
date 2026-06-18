use core::ffi::c_void;

use mechanical_mimalloc::raw::mimalloc::*;

#[test]
fn option_and_callback_exports_link_and_mutate_state() {
    unsafe {
        let option = mi_option_t::mi_option_verbose;
        let original = mi_option_get(option);

        mi_option_disable(option);
        assert!(!mi_option_is_enabled(option));
        assert_eq!(mi_option_get_clamp(option, 3, 7), 3);

        mi_option_enable(option);
        assert!(mi_option_is_enabled(option));

        mi_option_set(option, 5);
        assert_eq!(mi_option_get(option), 5);
        assert_eq!(mi_option_get_clamp(option, 3, 7), 5);
        assert_eq!(mi_option_get_clamp(option, 6, 7), 6);
        assert_eq!(mi_option_get_size(option), 5);

        mi_option_set_enabled(option, false);
        assert_eq!(mi_option_get(option), 0);
        mi_option_set_enabled_default(option, true);
        mi_option_set_default(option, original);
        mi_option_set(option, original);

        let register_output: unsafe extern "C" fn(Option<mi_output_fun>, *mut c_void) =
            mi_register_output;
        let register_error: unsafe extern "C" fn(Option<mi_error_fun>, *mut c_void) =
            mi_register_error;
        let print_default: unsafe extern "C" fn() = mi_options_print;
        let print_out: unsafe extern "C" fn(Option<mi_output_fun>, *mut c_void) =
            mi_options_print_out;
        core::hint::black_box(register_output);
        core::hint::black_box(register_error);
        core::hint::black_box(print_default);
        core::hint::black_box(print_out);

        assert_eq!(mi_option_get(option), original);
    }
}
