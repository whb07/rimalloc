use std::fs;
use std::path::Path;

#[test]
fn unsafe_audit_evidence_prefers_focused_release_audit_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = fs::read_to_string(root.join("docs/unsafe-audit-evidence.tsv"))
        .expect("read unsafe audit evidence");
    let unsafe_audit = fs::read_to_string(root.join("UNSAFE_AUDIT.md")).expect("read unsafe audit");
    let unsafe_audit_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit doc");

    const ALLOCATION_FREE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs";
    let assert_focused_evidence = |audit_id: &str, focused_path: &str| {
        let row_prefix = format!("{audit_id}\ttest\t{focused_path}\t");
        assert!(
            evidence.lines().any(|line| line.starts_with(&row_prefix)),
            "unsafe audit evidence should map {audit_id} to focused release-audit module {focused_path}"
        );
    };

    for audit_id in [
        "allocation-platform-shim-boundary",
        "allocation-product-entry-boundary",
        "allocation-realloc-entry-boundary",
        "allocation-string-path-boundary",
        "allocation-cxx-new-boundary",
        "allocation-heap-dispatch-boundary",
        "allocation-layout-request-boundary",
        "allocation-page-refill-boundary",
        "free-raw-export-boundary",
        "free-validation-boundary",
        "free-padding-boundary",
        "free-usable-size-boundary",
        "free-stats-boundary",
        "free-local-list-boundary",
        "free-remote-list-boundary",
        "free-route-dispatch-boundary",
        "free-abandoned-page-boundary",
        "aligned-padding-boundary",
        "aligned-thread-heap-allocation-boundary",
        "aligned-page-allocation-boundary",
        "aligned-calloc-product-boundary",
        "aligned-global-allocation-export-boundary",
        "aligned-heap-allocation-export-boundary",
        "aligned-realloc-request-boundary",
        "aligned-realloc-copy-boundary",
        "aligned-recalloc-product-boundary",
        "aligned-realloc-global-export-boundary",
        "aligned-realloc-heap-export-boundary",
    ] {
        assert_focused_evidence(audit_id, ALLOCATION_FREE_FOCUSED_PATH);
    }

    const LOCAL_BOUNDARIES_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs";
    const PAGE_ARENA_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs";
    const OS_MEMORY_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/os_memory.rs";
    const RAW_CXX_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/raw_facade/cxx_compat.rs";
    for audit_id in [
        "page-map-global-boundary",
        "page-map-32bit-commit-bitmap-boundary",
        "page-map-32bit-bootstrap-boundary",
        "page-map-top-level-lazy-commit-boundary",
        "page-map-global-bootstrap-boundary",
        "page-map-initial-submap-boundary",
        "page-map-null-sentinel-boundary",
        "page-map-teardown-boundary",
        "page-map-commit-boundary",
        "page-map-page-register-boundary",
        "page-map-range-register-boundary",
        "page-map-page-unregister-boundary",
        "page-map-range-unregister-boundary",
        "page-map-submap-publication-boundary",
        "page-map-range-primitive-boundary",
        "page-map-lookup-boundary",
    ] {
        assert_focused_evidence(audit_id, LOCAL_BOUNDARIES_FOCUSED_PATH);
        assert_focused_evidence(audit_id, PAGE_ARENA_FOCUSED_PATH);
    }

    for (audit_id, focused_path) in [
        (
            "raw-macro-bootstrap-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-override-entry-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-override-entry-boundary",
            "tests/release_fast_path_audit/release_policy/api_surface.rs",
        ),
        (
            "raw-macro-unix-core-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-unix-string-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-unix-realloc-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-unix-aligned-posix-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-unix-product-realloc-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-wasi-libc-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-linux-libc-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-allocation-query-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        ("raw-macro-cxx-delete-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-array-delete-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-aligned-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-nothrow-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-new-boundary", RAW_CXX_FOCUSED_PATH),
        (
            "raw-macro-header-forwarding-boundary",
            "tests/release_fast_path_audit/release_policy/api_surface.rs",
        ),
        ("os-memory-config-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-guard-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-commit-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-free-boundary", OS_MEMORY_FOCUSED_PATH),
        (
            "os-memory-primitive-allocation-boundary",
            OS_MEMORY_FOCUSED_PATH,
        ),
        (
            "os-memory-aligned-allocation-boundary",
            OS_MEMORY_FOCUSED_PATH,
        ),
        (
            "os-memory-memid-provenance-boundary",
            OS_MEMORY_FOCUSED_PATH,
        ),
        ("os-memory-zeroing-boundary", OS_MEMORY_FOCUSED_PATH),
        (
            "os-memory-request-allocation-boundary",
            OS_MEMORY_FOCUSED_PATH,
        ),
        ("os-memory-public-export-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-huge-page-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-numa-boundary", OS_MEMORY_FOCUSED_PATH),
        (
            "stats-counter-boundary",
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
            "stats-output-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-process-info-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-json-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "typed-pointer-constructor-boundary",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-page-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-block-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-heap-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-thread-heap-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-tld-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-arena-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-subproc-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-page-queue-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-page-pointer-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-block-pointer-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-heap-thread-tld-pointer-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-arena-subproc-pointer-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-page-queue-pointer-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-provenance-pointer-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-main-heap-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-owned-heap-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-borrowed-heap-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-thread-heap-state-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
        (
            "typed-thread-local-heap-test-fixtures",
            "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs",
        ),
    ] {
        assert_focused_evidence(audit_id, focused_path);
    }

    let tracked_ids = [
        "allocation-platform-shim-boundary",
        "allocation-product-entry-boundary",
        "allocation-heap-dispatch-boundary",
        "allocation-layout-request-boundary",
        "allocation-realloc-entry-boundary",
        "allocation-string-path-boundary",
        "allocation-cxx-new-boundary",
        "allocation-page-refill-boundary",
        "free-raw-export-boundary",
        "free-validation-boundary",
        "free-padding-boundary",
        "free-usable-size-boundary",
        "free-stats-boundary",
        "free-local-list-boundary",
        "free-remote-list-boundary",
        "free-route-dispatch-boundary",
        "free-abandoned-page-boundary",
        "aligned-padding-boundary",
        "aligned-thread-heap-allocation-boundary",
        "aligned-page-allocation-boundary",
        "aligned-calloc-product-boundary",
        "aligned-global-allocation-export-boundary",
        "aligned-heap-allocation-export-boundary",
        "aligned-realloc-request-boundary",
        "aligned-realloc-copy-boundary",
        "aligned-recalloc-product-boundary",
        "aligned-realloc-global-export-boundary",
        "aligned-realloc-heap-export-boundary",
        "raw-macro-bootstrap-boundary",
        "raw-macro-override-entry-boundary",
        "raw-macro-unix-core-boundary",
        "raw-macro-unix-string-boundary",
        "raw-macro-unix-realloc-boundary",
        "raw-macro-unix-aligned-posix-boundary",
        "raw-macro-unix-product-realloc-boundary",
        "raw-macro-wasi-libc-boundary",
        "raw-macro-linux-libc-boundary",
        "raw-macro-allocation-query-boundary",
        "raw-macro-cxx-delete-boundary",
        "raw-macro-cxx-array-delete-boundary",
        "raw-macro-cxx-aligned-boundary",
        "raw-macro-cxx-nothrow-boundary",
        "raw-macro-cxx-new-boundary",
        "raw-macro-header-forwarding-boundary",
        "os-memory-config-boundary",
        "os-memory-guard-boundary",
        "os-memory-commit-boundary",
        "os-memory-free-boundary",
        "os-memory-primitive-allocation-boundary",
        "os-memory-aligned-allocation-boundary",
        "os-memory-memid-provenance-boundary",
        "os-memory-zeroing-boundary",
        "os-memory-request-allocation-boundary",
        "os-memory-public-export-boundary",
        "os-memory-huge-page-boundary",
        "os-memory-numa-boundary",
        "page-map-global-boundary",
        "page-map-32bit-commit-bitmap-boundary",
        "page-map-32bit-bootstrap-boundary",
        "page-map-top-level-lazy-commit-boundary",
        "page-map-global-bootstrap-boundary",
        "page-map-initial-submap-boundary",
        "page-map-null-sentinel-boundary",
        "page-map-teardown-boundary",
        "page-map-commit-boundary",
        "page-map-page-register-boundary",
        "page-map-range-register-boundary",
        "page-map-page-unregister-boundary",
        "page-map-range-unregister-boundary",
        "page-map-submap-publication-boundary",
        "page-map-range-primitive-boundary",
        "page-map-lookup-boundary",
        "stats-counter-boundary",
        "stats-counter-merge-boundary",
        "stats-clock-boundary",
        "options-libc-file-symbol-boundary",
        "options-recurse-guard-boundary",
        "options-delayed-output-buffer-boundary",
        "options-stderr-output-boundary",
        "options-output-callback-boundary",
        "options-format-output-boundary",
        "options-print-output-boundary",
        "stats-heap-merge-boundary",
        "stats-public-merge-boundary",
        "stats-heap-visitor-boundary",
        "stats-public-export-boundary",
        "stats-output-boundary",
        "stats-process-info-boundary",
        "stats-json-boundary",
        "typed-pointer-constructor-boundary",
        "typed-page-accessors",
        "typed-block-accessors",
        "typed-heap-accessors",
        "typed-thread-heap-accessors",
        "typed-tld-accessors",
        "typed-arena-accessors",
        "typed-subproc-accessors",
        "typed-page-queue-accessors",
        "typed-page-pointer-test-fixtures",
        "typed-block-pointer-test-fixtures",
        "typed-heap-thread-tld-pointer-test-fixtures",
        "typed-arena-subproc-pointer-test-fixtures",
        "typed-page-queue-pointer-test-fixtures",
        "typed-provenance-pointer-test-fixtures",
        "typed-main-heap-test-fixtures",
        "typed-owned-heap-test-fixtures",
        "typed-borrowed-heap-test-fixtures",
        "typed-thread-heap-state-test-fixtures",
        "typed-thread-local-heap-test-fixtures",
    ];
    let stale_paths = [
        "tests/release_fast_path_audit.rs",
        "tests/release_fast_path_audit/unsafe_performance.rs",
        "tests/release_fast_path_audit/release_policy.rs",
        "tests/release_fast_path_audit/datatype_acceptance.rs",
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
            !tracked_ids.contains(&audit_id) || !stale_paths.contains(&path),
            "{audit_id} evidence should cite a focused release-audit module instead of stale aggregate path {path}"
        );
    }

    assert!(
        unsafe_audit.contains("the evidence row should name that focused\nmodule")
            && unsafe_audit_doc
                .contains("Release-audit evidence rows should cite the focused module"),
        "unsafe audit docs should describe focused release-audit evidence ownership"
    );
}
