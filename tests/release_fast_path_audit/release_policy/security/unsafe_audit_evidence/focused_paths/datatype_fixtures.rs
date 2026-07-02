pub(super) fn assert_datatype_fixture_focused_evidence(evidence: &str) {
    let assert_focused_evidence = |audit_id: &str, focused_path: &str| {
        let row_prefix = format!("{audit_id}\ttest\t{focused_path}\t");
        assert!(
            evidence.lines().any(|line| line.starts_with(&row_prefix)),
            "unsafe audit evidence should map {audit_id} to focused release-audit module {focused_path}"
        );
    };
    const DATATYPE_FIXTURE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs";
    for audit_id in [
        "typed-page-pointer-test-fixtures",
        "typed-block-pointer-test-fixtures",
        "typed-heap-pointer-test-fixtures",
        "typed-thread-heap-pointer-test-fixtures",
        "typed-tld-pointer-test-fixtures",
        "typed-pointer-address-test-fixtures",
        "typed-arena-subproc-pointer-test-fixtures",
        "typed-page-queue-pointer-test-fixtures",
        "typed-provenance-pointer-test-fixtures",
        "typed-main-heap-test-fixtures",
        "typed-owned-heap-test-fixtures",
        "typed-borrowed-heap-test-fixtures",
        "typed-thread-heap-state-test-fixtures",
        "typed-thread-local-heap-test-fixtures",
    ] {
        assert_focused_evidence(audit_id, DATATYPE_FIXTURE_FOCUSED_PATH);
    }
}
