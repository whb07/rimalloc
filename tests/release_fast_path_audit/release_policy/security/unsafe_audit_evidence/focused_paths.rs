#[path = "focused_paths/allocation_free.rs"]
mod allocation_free;
#[path = "focused_paths/datatype_fixtures.rs"]
mod datatype_fixtures;
#[path = "focused_paths/platform_primitives.rs"]
mod platform_primitives;

pub(super) fn assert_focused_evidence_paths(evidence: &str) {
    let assert_focused_evidence = |audit_id: &str, focused_path: &str| {
        let row_prefix = format!("{audit_id}\ttest\t{focused_path}\t");
        assert!(
            evidence.lines().any(|line| line.starts_with(&row_prefix)),
            "unsafe audit evidence should map {audit_id} to focused release-audit module {focused_path}"
        );
    };

    allocation_free::assert_allocation_free_focused_evidence(evidence);
    platform_primitives::assert_platform_primitive_focused_evidence(evidence);

    const LOCAL_BOUNDARIES_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs";
    const PAGE_ARENA_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs";
    const RAW_CXX_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/raw_facade/cxx_compat.rs";
    const STRICT_TYPE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs";
    datatype_fixtures::assert_datatype_fixture_focused_evidence(evidence);
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

    const RAW_FACADE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs";
    for audit_id in [
        "raw-macro-bootstrap-boundary",
        "raw-macro-fast-override-boundary",
        "raw-macro-macos-interpose-boundary",
        "raw-macro-windows-crt-boundary",
        "raw-macro-windows-aligned-boundary",
        "raw-macro-unix-core-boundary",
        "raw-macro-unix-string-boundary",
        "raw-macro-unix-realloc-boundary",
        "raw-macro-unix-aligned-posix-boundary",
        "raw-macro-unix-product-realloc-boundary",
        "raw-macro-wasi-libc-boundary",
        "raw-macro-linux-libc-boundary",
        "raw-macro-allocation-query-boundary",
    ] {
        assert_focused_evidence(audit_id, RAW_FACADE_FOCUSED_PATH);
    }
    for audit_id in [
        "raw-macro-fast-override-boundary",
        "raw-macro-macos-interpose-boundary",
    ] {
        assert_focused_evidence(
            audit_id,
            "tests/release_fast_path_audit/interposition_exports/linux_override.rs",
        );
    }
    for audit_id in [
        "raw-macro-macos-interpose-boundary",
        "raw-macro-windows-crt-boundary",
        "raw-macro-windows-aligned-boundary",
    ] {
        assert_focused_evidence(
            audit_id,
            "tests/release_fast_path_audit/release_policy/api_surface.rs",
        );
    }

    for (audit_id, focused_path) in [
        ("raw-macro-cxx-delete-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-array-delete-boundary", RAW_CXX_FOCUSED_PATH),
        (
            "raw-macro-cxx-aligned-delete-boundary",
            RAW_CXX_FOCUSED_PATH,
        ),
        ("raw-macro-cxx-aligned-new64-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-aligned-new32-boundary", RAW_CXX_FOCUSED_PATH),
        (
            "raw-macro-cxx-aligned-operator-boundary",
            RAW_CXX_FOCUSED_PATH,
        ),
        ("raw-macro-cxx-nothrow-boundary", RAW_CXX_FOCUSED_PATH),
        ("raw-macro-cxx-new-boundary", RAW_CXX_FOCUSED_PATH),
        (
            "raw-macro-header-forwarding-boundary",
            "tests/release_fast_path_audit/release_policy/api_surface.rs",
        ),
        (
            "stats-init-bin-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-update-boundary",
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
            "stats-print-export-boundary",
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
        ("typed-tld-accessors", STRICT_TYPE_FOCUSED_PATH),
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
