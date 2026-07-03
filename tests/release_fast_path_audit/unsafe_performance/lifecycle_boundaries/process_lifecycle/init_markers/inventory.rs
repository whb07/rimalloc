pub(super) fn assert_init_marker_inventory(init_rs: &str, unsafe_allowlist: &str) {
    let marker_total = [
        "SAFETY[AUDIT:init-main-subproc-lifecycle-boundary]",
        "SAFETY[AUDIT:init-platform-tls-boundary]",
        "SAFETY[AUDIT:init-platform-tls-windows-slot-boundary]",
        "SAFETY[AUDIT:init-process-boundary]",
        "SAFETY[AUDIT:init-process-validation-boundary]",
        "SAFETY[AUDIT:init-subproc-prefix-boundary]",
        "SAFETY[AUDIT:init-subproc-main-heap-load-boundary]",
        "SAFETY[AUDIT:init-subproc-main-heap-export-boundary]",
        "SAFETY[AUDIT:init-subproc-current-main-heap-boundary]",
        "SAFETY[AUDIT:init-subproc-main-heap-predicate-boundary]",
        "SAFETY[AUDIT:init-subproc-heap-visit-boundary]",
        "SAFETY[AUDIT:init-subproc-identity-boundary]",
        "SAFETY[AUDIT:init-subproc-create-boundary]",
        "SAFETY[AUDIT:init-subproc-metadata-destroy-boundary]",
        "SAFETY[AUDIT:init-subproc-registry-list-boundary]",
        "SAFETY[AUDIT:init-subproc-destroy-loop-boundary]",
        "SAFETY[AUDIT:init-subproc-registry-teardown-boundary]",
        "SAFETY[AUDIT:init-subproc-public-destroy-boundary]",
        "SAFETY[AUDIT:init-subproc-stats-count-boundary]",
        "SAFETY[AUDIT:init-thread-identity-boundary]",
        "SAFETY[AUDIT:init-thread-done-entry-boundary]",
        "SAFETY[AUDIT:init-thread-done-dispatch-boundary]",
        "SAFETY[AUDIT:init-thread-init-default-heap-boundary]",
        "SAFETY[AUDIT:init-thread-done-cleanup-boundary]",
        "SAFETY[AUDIT:init-thread-done-tls-reset-boundary]",
        "SAFETY[AUDIT:init-thread-init-entry-boundary]",
        "SAFETY[AUDIT:init-thread-heap-empty-sentinel-boundary]",
        "SAFETY[AUDIT:init-thread-heap-main-tls-boundary]",
        "SAFETY[AUDIT:init-thread-heap-main-safe-boundary]",
        "SAFETY[AUDIT:init-thread-heap-main-static-boundary]",
        "SAFETY[AUDIT:init-thread-default-heap-association-boundary]",
        "SAFETY[AUDIT:init-thread-default-heap-publication-boundary]",
        "SAFETY[AUDIT:init-thread-cached-heap-association-boundary]",
        "SAFETY[AUDIT:init-thread-heap-safe-access-boundary]",
        "SAFETY[AUDIT:init-options-guarded-boundary]",
        "SAFETY[AUDIT:init-random-cpu-boundary]",
        "SAFETY[AUDIT:init-tld-lifecycle-boundary]",
        "SAFETY[AUDIT:init-platform-hooks-boundary]",
    ]
    .into_iter()
    .map(|marker| init_rs.matches(marker).count())
    .sum::<usize>();
    let split_marker_minimums = [
        ("init-main-subproc-lifecycle-boundary", 6),
        ("init-platform-tls-boundary", 7),
        ("init-platform-tls-windows-slot-boundary", 10),
        ("init-subproc-prefix-boundary", 1),
        ("init-subproc-main-heap-load-boundary", 5),
        ("init-subproc-main-heap-export-boundary", 8),
        ("init-subproc-current-main-heap-boundary", 7),
        ("init-subproc-main-heap-predicate-boundary", 11),
        ("init-subproc-heap-visit-boundary", 4),
        ("init-subproc-identity-boundary", 16),
        ("init-subproc-create-boundary", 6),
        ("init-subproc-metadata-destroy-boundary", 3),
        ("init-subproc-registry-list-boundary", 5),
        ("init-subproc-destroy-loop-boundary", 13),
        ("init-subproc-registry-teardown-boundary", 2),
        ("init-subproc-public-destroy-boundary", 6),
        ("init-subproc-stats-count-boundary", 12),
        ("init-thread-identity-boundary", 1),
        ("init-thread-done-entry-boundary", 6),
        ("init-thread-done-dispatch-boundary", 10),
        ("init-thread-init-default-heap-boundary", 4),
        ("init-thread-done-cleanup-boundary", 16),
        ("init-thread-done-tls-reset-boundary", 5),
        ("init-thread-init-entry-boundary", 6),
        ("init-thread-heap-empty-sentinel-boundary", 3),
        ("init-thread-heap-main-tls-boundary", 9),
        ("init-thread-heap-main-safe-boundary", 8),
        ("init-thread-heap-main-static-boundary", 1),
        ("init-thread-default-heap-association-boundary", 14),
        ("init-thread-default-heap-publication-boundary", 3),
        ("init-thread-cached-heap-association-boundary", 11),
        ("init-thread-heap-safe-access-boundary", 7),
        ("init-tld-lifecycle-boundary", 7),
    ];

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/init.rs\t"))
            && !init_rs.contains("SAFETY[AUDIT:init-lifecycle-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-heap-association-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-registry-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-tls-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-heap-tls-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-lifecycle-boundary]")
            && marker_total >= 277,
        "init lifecycle unsafe boundaries should stay split into focused operation markers without a migration allowlist entry"
    );
    for (audit_id, minimum) in split_marker_minimums {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = init_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "init unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
}
