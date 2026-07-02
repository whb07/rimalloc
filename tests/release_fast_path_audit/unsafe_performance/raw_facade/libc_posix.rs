use super::*;

pub(super) fn assert_libc_and_posix_boundaries(sources: &UnsafePerformanceSources) {
    let libc_rs = sources.libc_rs.as_str();
    let raw_macros_rs = sources.raw_macros_rs.as_str();

    assert!(
        !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-libc-forwarding-boundary]"),
        "libc/POSIX raw facade forwarding should remain split into focused operation markers"
    );
    for (audit_id, minimum) in [
        ("raw-macro-unix-core-boundary", 4),
        ("raw-macro-unix-string-boundary", 4),
        ("raw-macro-unix-realloc-boundary", 2),
        ("raw-macro-unix-page-aligned-boundary", 6),
        ("raw-macro-unix-aligned-posix-boundary", 11),
        ("raw-macro-unix-cfree-boundary", 2),
        ("raw-macro-unix-product-realloc-boundary", 5),
        ("raw-macro-wasi-libc-boundary", 11),
        ("raw-macro-linux-libc-boundary", 10),
        ("raw-macro-linux-libc-aligned-boundary", 10),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = raw_macros_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "libc/POSIX raw facade boundary {audit_id} should retain at least {minimum} operation markers; found {count}"
        );
    }
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
