use super::*;

pub(super) fn assert_libc_helper_no_exports(sources: &UnsafePerformanceSources) {
    let libc_rs = sources.libc_rs.as_str();

    for (source_name, source, internal_name) in [
        ("src/ffi/libc.rs", libc_rs, "pub(crate) fn _mi_snprintf"),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_vsnprintf",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub extern \"C\" fn _mi_toupper",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnicmp",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_streq",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcpy",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcat",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlen",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnlen",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnstr",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) extern \"C\" fn _mi_popcount_generic",
        ),
    ] {
        assert_internal_no_export(source_name, source, internal_name);
    }
}
