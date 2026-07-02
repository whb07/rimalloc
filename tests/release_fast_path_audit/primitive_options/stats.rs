pub(super) fn assert_stats_format_and_markers(unsafe_allowlist: &str, stats_rs: &str) {
    assert!(
        !stats_rs.contains("fn format_amount(mut n: i64, unit: i64) -> String")
            && !stats_rs.contains("format!(\"{n}   \"")
            && !stats_rs.contains("format!(\"{n}   {suffix:<3}\"")
            && !stats_rs.contains("format!(\"{magnitude}i{suffix}\"")
            && !stats_rs.contains("format!(\"{magnitude}{suffix}\"")
            && !stats_rs.contains("format!(\"{whole}.{frac1} {unitdesc:<3}\"")
            && !stats_rs.contains("let label = format!(\"bin{pagekind:>2}  {i:>3}\");")
            && !stats_rs.contains("String::new()")
            && !stats_rs.contains("to_string_lossy")
            && stats_rs.contains("fn format_amount(buf: &mut [c_char; 32], mut n: i64, unit: i64)")
            && stats_rs.contains("let mut buf = [0 as c_char; 32];")
            && stats_rs.contains("let mut label = [0 as c_char; 64];")
            && stats_rs.contains("format_amount(&mut buf, n, unit);")
            && stats_rs.contains("CStr::from_ptr(fmt) }.to_bytes()"),
        "stats amount formatting should stay on stack buffers like C"
    );
    assert!(
        !stats_rs.contains("&format!(\n            \"{prefix}{")
            && !stats_rs.contains("&format!(\"  \\\"{name}\\\": \")")
            && !stats_rs.contains("&format!(\"  \\\"{name}\\\": {val},\\n\")")
            && !stats_rs.contains("&format!(\n            \"    \\\"{name}\\\": {val}")
            && stats_rs.contains("fn c_char_buffer_str(buf: &[c_char]) -> &str")
            && stats_rs.contains("let mut buf = [0 as c_char; 128];")
            && stats_rs.contains("mi_json_buf_print(hbuf, c_char_buffer_str(&buf));"),
        "stats JSON formatting should use fixed stack buffers like C"
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
