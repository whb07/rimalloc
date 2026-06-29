// Raw/unsafe options API example. Prefer `mechanical_mimalloc::GlobalOptions`
// unless you need the C-shaped compatibility surface.
fn main() {
    let option = mechanical_mimalloc::raw::mi_option_t::mi_option_show_errors;

    unsafe {
        let previous = mechanical_mimalloc::raw::mi_option_is_enabled(option);
        mechanical_mimalloc::raw::mi_option_set_enabled(option, !previous);
        assert_eq!(
            mechanical_mimalloc::raw::mi_option_is_enabled(option),
            !previous
        );

        mechanical_mimalloc::raw::mi_option_set_enabled(option, previous);
        assert_eq!(
            mechanical_mimalloc::raw::mi_option_is_enabled(option),
            previous
        );
    }
}
