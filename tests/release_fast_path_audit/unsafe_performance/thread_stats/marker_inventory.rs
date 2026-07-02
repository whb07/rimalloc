pub(super) fn assert_stats_marker_inventory(
    unsafe_allowlist: &str,
    options_rs: &str,
    stats_rs: &str,
) {
    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) fn _mi_raw_message(")
            && options_rs.contains("static MI_RAW_MESSAGE_KEEP"),
        "_mi_raw_message should keep C's local raw-message helper boundary"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_stats_init(")
            && stats_rs.contains("static MI_STATS_INIT_KEEP: extern \"C\" fn() = _mi_stats_init;"),
        "_mi_stats_init should keep C's local process statistics initialization boundary"
    );
    for (audit_id, min_count) in [
        ("stats-init-bin-boundary", 4),
        ("stats-counter-mt-update-boundary", 7),
        ("stats-counter-local-update-boundary", 6),
        ("stats-counter-adjust-boundary", 10),
        ("stats-counter-accumulate-boundary", 3),
        ("stats-counter-merge-boundary", 14),
        ("stats-clock-boundary", 1),
        ("stats-heap-merge-boundary", 14),
        ("stats-public-merge-boundary", 16),
        ("stats-heap-visitor-boundary", 16),
        ("stats-public-export-boundary", 14),
        ("stats-formatting-boundary", 6),
        ("stats-buffered-output-boundary", 3),
        ("stats-print-format-boundary", 3),
        ("stats-print-export-boundary", 16),
        ("stats-process-info-boundary", 6),
        ("stats-json-buffer-boundary", 4),
        ("stats-json-boundary", 16),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            stats_rs.matches(&marker).count() >= min_count,
            "stats unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/stats.rs\t"))
            && !stats_rs.contains("SAFETY[AUDIT:stats-reporting-boundary]")
            && !stats_rs.contains("SAFETY[AUDIT:stats-counter-boundary]"),
        "stats unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
    assert!(
        !stats_rs.contains("SAFETY[AUDIT:stats-merge-boundary]")
            && !stats_rs.contains("SAFETY[AUDIT:stats-output-boundary]"),
        "stats merge and output boundaries should remain split into focused operation markers"
    );
}
