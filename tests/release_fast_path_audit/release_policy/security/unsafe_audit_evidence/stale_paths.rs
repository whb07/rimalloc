pub(super) fn assert_no_stale_aggregate_paths(evidence: &str) {
    let stale_paths = [
        "tests/release_fast_path_audit.rs",
        "tests/release_fast_path_audit/unsafe_performance.rs",
        "tests/release_fast_path_audit/release_policy.rs",
        "tests/release_fast_path_audit/datatype_acceptance.rs",
        "tests/release_fast_path_audit/interposition_exports.rs",
        "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        "tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs",
        "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries.rs",
        "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs",
    ];

    for line in evidence.lines().skip(1) {
        let mut fields = line.split('\t');
        let audit_id = fields.next().expect("evidence row audit id");
        let _kind = fields.next().expect("evidence row kind");
        let path = fields.next().expect("evidence row path");
        let detail = fields.next().expect("evidence row detail");
        assert!(
            fields.next().is_none() && !detail.is_empty(),
            "evidence row should stay four tab-separated fields: {line}"
        );
        assert!(
            !stale_paths.contains(&path),
            "{audit_id} evidence should cite a focused release-audit module instead of stale aggregate path {path}"
        );
    }
}
