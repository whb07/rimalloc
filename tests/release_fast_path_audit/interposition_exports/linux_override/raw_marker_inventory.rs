pub(super) fn assert_raw_marker_inventory(unsafe_allowlist: &str, override_rs: &str) {
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/raw/macros.rs\t"))
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-allocation-boundary]")
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-bootstrap-boundary]")
                .count()
                >= 15
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-fast-override-boundary]")
                .count()
                >= 3
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-macos-interpose-boundary]")
                .count()
                >= 5
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-windows-crt-boundary]")
                .count()
                >= 14
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-windows-aligned-boundary]")
                .count()
                >= 14
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-libc-forwarding-boundary]")
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-core-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-string-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-realloc-boundary]")
                .count()
                >= 2
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]")
                .count()
                >= 20
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-wasi-libc-boundary]")
                .count()
                >= 11
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-linux-libc-boundary]")
                .count()
                >= 20
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-allocation-query-boundary]")
                .count()
                >= 10
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-cxx-boundary]")
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-delete-boundary]")
                .count()
                >= 3
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]")
                .count()
                >= 3
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]")
                .count()
                >= 8
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]")
                .count()
                >= 6
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]")
                .count()
                >= 6
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]")
                .count()
                >= 9
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]")
                .count()
                >= 7
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-new-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-header-forwarding-boundary]")
                .count()
                >= 6
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-override-entry-boundary]")
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-boundary]")
            && override_rs.contains("core::arch::global_asm!(")
            && override_rs.contains("mod unix_bootstrap")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && override_rs.contains("pub unsafe fn mi_forward1")
            && override_rs.contains("pub unsafe fn mi_malloc_size_forward")
            && override_rs.contains("pub unsafe fn mi_unix_malloc(")
            && override_rs.contains("pub unsafe fn mi_unix_posix_memalign")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_unix_malloc_export")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_wasi_libc_malloc_export")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_linux_libc_malloc_export"),
        "raw macro allocation facade unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
}
