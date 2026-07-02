pub(super) fn assert_stats_and_strict_type_focused_evidence(evidence: &str) {
    let assert_focused_evidence = |audit_id: &str, focused_path: &str| {
        let row_prefix = format!("{audit_id}\ttest\t{focused_path}\t");
        assert!(
            evidence.lines().any(|line| line.starts_with(&row_prefix)),
            "unsafe audit evidence should map {audit_id} to focused release-audit module {focused_path}"
        );
    };

    const STRICT_TYPE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs";
    for (audit_id, focused_path) in [
        (
            "stats-init-bin-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-mt-update-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-local-update-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-adjust-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-accumulate-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-merge-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-clock-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-libc-file-symbol-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-recurse-guard-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-delayed-output-buffer-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-stderr-output-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-output-callback-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-format-output-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "options-print-output-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "stats-heap-merge-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-public-merge-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-heap-visitor-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-public-export-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-formatting-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-buffered-output-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-print-format-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-print-export-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-process-info-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-json-buffer-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-json-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "typed-pointer-constructor-boundary",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-page-scalar-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-page-free-list-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-page-atomic-accessors", STRICT_TYPE_FOCUSED_PATH),
        (
            "typed-page-fixture-touch-accessors",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-block-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-heap-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-thread-heap-accessors", STRICT_TYPE_FOCUSED_PATH),
        (
            "typed-tld-thread-identity-accessors",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-tld-subproc-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-tld-metadata-accessors", STRICT_TYPE_FOCUSED_PATH),
        (
            "typed-tld-fixture-touch-accessors",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-arena-identity-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-arena-linkage-accessors", STRICT_TYPE_FOCUSED_PATH),
        (
            "typed-arena-fixture-touch-accessors",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-subproc-identity-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-subproc-arena-accessors", STRICT_TYPE_FOCUSED_PATH),
        (
            "typed-subproc-heap-link-accessors",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-subproc-count-accessors", STRICT_TYPE_FOCUSED_PATH),
        ("typed-subproc-metadata-accessors", STRICT_TYPE_FOCUSED_PATH),
        (
            "typed-subproc-fixture-touch-accessors",
            STRICT_TYPE_FOCUSED_PATH,
        ),
        ("typed-page-queue-accessors", STRICT_TYPE_FOCUSED_PATH),
    ] {
        assert_focused_evidence(audit_id, focused_path);
    }
}
