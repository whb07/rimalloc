pub(super) fn assert_free_marker_counts(free_rs: &str, bitmap_rs: &str, unsafe_allowlist: &str) {
    assert!(
        !free_rs.contains("clippy::if_same_then_else")
            && !bitmap_rs.contains("clippy::if_same_then_else"),
        "free.rs and bitmap.rs should not retain obsolete if_same_then_else allowances"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/free.rs\t"))
            && !free_rs.contains("SAFETY[AUDIT:free-path-boundary]")
            && free_rs.contains("core::arch::global_asm!(")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign(")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn mi_free_generic_local(")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn mi_free_generic_mt(")
            && free_rs.contains("unsafe fn mi_free_fast_try(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_free(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_usable_size(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_ufree(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_free_small("),
        "free-path unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    for (audit_id, expected_count) in [
        ("free-raw-export-boundary", 13),
        ("free-double-check-boundary", 3),
        ("free-pointer-lookup-boundary", 10),
        ("free-abi-validation-boundary", 5),
        ("free-validation-fixtures", 10),
        ("free-padding-boundary", 5),
        ("free-usable-size-boundary", 9),
        ("free-stats-boundary", 1),
        ("free-local-list-boundary", 1),
        ("free-remote-list-boundary", 1),
        ("free-route-dispatch-boundary", 10),
        ("free-abandoned-page-boundary", 1),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            free_rs.matches(&marker).count(),
            expected_count,
            "free unsafe boundary {audit_id} should retain exactly {expected_count} local operation markers"
        );
    }
}
