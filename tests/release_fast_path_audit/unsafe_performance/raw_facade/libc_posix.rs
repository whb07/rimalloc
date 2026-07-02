use super::*;

pub(super) fn assert_libc_and_posix_boundaries(sources: &UnsafePerformanceSources) {
    let libc_rs = sources.libc_rs.as_str();
    let raw_macros_rs = sources.raw_macros_rs.as_str();

    assert!(
        raw_macros_rs
            .matches("SAFETY[AUDIT:raw-macro-libc-forwarding-boundary]")
            .count()
            >= 60,
        "libc/POSIX raw facade forwarding should keep operation-level audit markers"
    );
    for symbol in [
        "pub unsafe extern \"C\" fn mi_unix_malloc_export",
        "pub unsafe extern \"C\" fn mi_unix_calloc_export",
        "pub unsafe extern \"C\" fn mi_unix_realloc_export",
        "pub unsafe extern \"C\" fn mi_unix_free_export",
        "pub unsafe fn mi_unix_posix_memalign",
        "unsafe(export_name = \"posix_memalign\")",
        "pub unsafe extern \"C\" fn mi_wasi_libc_malloc_export",
        "pub unsafe extern \"C\" fn mi_linux_libc_malloc_export",
        "unsafe(export_name = \"__posix_memalign\")",
    ] {
        assert!(
            raw_macros_rs.contains(symbol),
            "libc/POSIX raw facade should retain `{symbol}`"
        );
    }
    for helper in [
        "pub(crate) fn _mi_snprintf",
        "pub(crate) unsafe extern \"C\" fn _mi_vsnprintf",
        "pub(crate) unsafe extern \"C\" fn _mi_strlcpy",
        "pub(crate) unsafe extern \"C\" fn _mi_strlcat",
        "pub(crate) unsafe extern \"C\" fn _mi_strnstr",
        "pub(crate) extern \"C\" fn _mi_popcount_generic",
    ] {
        assert!(
            libc_rs.contains(helper),
            "libc compatibility helper should stay crate-local: {helper}"
        );
    }
}
