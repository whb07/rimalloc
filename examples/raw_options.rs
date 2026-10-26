// Raw/unsafe options API example. Prefer `rimalloc::GlobalOptions`
// unless you need the C-shaped compatibility surface.
fn main() {
    let option = rimalloc::raw::mi_option_t::mi_option_show_errors;

    unsafe {
        let previous = rimalloc::raw::mi_option_is_enabled(option);
        rimalloc::raw::mi_option_set_enabled(option, !previous);
        assert_eq!(rimalloc::raw::mi_option_is_enabled(option), !previous);

        rimalloc::raw::mi_option_set_enabled(option, previous);
        assert_eq!(rimalloc::raw::mi_option_is_enabled(option), previous);
    }
}
