pub(super) fn assert_aligned_marker_counts(
    free_rs: &str,
    alloc_aligned_rs: &str,
    unsafe_allowlist: &str,
) {
    for (audit_id, expected_count) in [
        ("aligned-padding-shrink-boundary", 3),
        ("aligned-guarded-allocation-boundary", 3),
        ("aligned-no-guard-allocation-boundary", 7),
        ("aligned-overalloc-adjust-boundary", 10),
        ("aligned-thread-heap-allocation-boundary", 11),
        ("aligned-page-allocation-boundary", 11),
        ("aligned-calloc-product-boundary", 14),
        ("aligned-global-allocation-export-boundary", 13),
        ("aligned-global-calloc-export-boundary", 4),
        ("aligned-heap-allocation-export-boundary", 15),
        ("aligned-realloc-request-boundary", 15),
        ("aligned-realloc-copy-boundary", 4),
        ("aligned-recalloc-product-boundary", 8),
        ("aligned-realloc-global-export-boundary", 14),
        ("aligned-realloc-heap-realloc-boundary", 8),
        ("aligned-realloc-heap-rezalloc-boundary", 8),
        ("aligned-realloc-heap-recalloc-boundary", 10),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            alloc_aligned_rs.matches(&marker).count(),
            expected_count,
            "aligned allocation unsafe boundary {audit_id} should retain exactly {expected_count} local operation markers"
        );
    }
    assert!(
        !free_rs.contains("SAFETY[AUDIT:free-validation-boundary]")
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-realloc-heap-export-boundary]"),
        "free validation and heap aligned realloc markers should stay split into focused operation families"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/aligned.rs\t"))
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-allocation-boundary]")
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-heap-allocation-boundary]")
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-realloc-boundary]")
            && alloc_aligned_rs.contains("fn mi_malloc_is_naturally_aligned(")
            && alloc_aligned_rs.contains("fn aligned_layout_or_report(")
            && alloc_aligned_rs.contains("crate::core_types::LayoutRequest::aligned_at(")
            && alloc_aligned_rs.contains("pub unsafe extern \"C\" fn mi_malloc_aligned")
            && alloc_aligned_rs.contains("unsafe fn mi_theap_realloc_zero_aligned_at_req(")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call("),
        "aligned allocation unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
}
