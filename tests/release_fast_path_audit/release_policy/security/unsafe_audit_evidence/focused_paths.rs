pub(super) fn assert_focused_evidence_paths(evidence: &str) {
    const ALLOCATION_FREE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/allocation_free/marker_counts.rs";
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
        "allocation-realloc-thread-entry-boundary",
        "allocation-realloc-product-boundary",
        "allocation-realloc-free-on-failure-boundary",
        "allocation-realloc-zeroing-boundary",
        "allocation-realloc-public-boundary",
        "allocation-string-path-boundary",
        "allocation-cxx-new-handler-boundary",
        "allocation-cxx-new-try-boundary",
        "allocation-cxx-new-public-boundary",
        "allocation-cxx-new-product-boundary",
        "allocation-cxx-new-aligned-boundary",
        "allocation-cxx-new-realloc-boundary",
        "allocation-small-dispatch-boundary",
        "allocation-thread-heap-zero-boundary",
        "allocation-public-malloc-boundary",
        "allocation-zeroing-dispatch-boundary",
        "allocation-calloc-dispatch-boundary",
        "allocation-layout-request-boundary",
        "allocation-page-refill-boundary",
        "page-free-list-refill-boundary",
        "page-queue-full-transition-boundary",
        "page-queue-free-release-boundary",
        "page-queue-retire-boundary",
        "page-queue-abandon-boundary",
        "page-queue-reclaim-boundary",
        "page-queue-validation-boundary",
        "page-queue-direct-cache-boundary",
        "page-queue-link-mutation-boundary",
        "page-queue-append-boundary",
        "page-huge-allocation-boundary",
        "page-stats-public-boundary",
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
        "tests/release_fast_path_audit/unsafe_performance/os_memory/os_wrappers.rs";
    const UNIX_PRIMITIVE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/os_memory/platform_helpers.rs";
    const WINDOWS_PRIMITIVE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/windows.rs";
    const WASI_PRIMITIVE_FOCUSED_PATH: &str = "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/wasi.rs";
    const EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH: &str = "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/emscripten.rs";
    const RAW_CXX_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/raw_facade/cxx_compat.rs";
    const DATATYPE_FIXTURE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/datatype_acceptance/fixture_audit.rs";
    for audit_id in [
        "windows-virtual-memory-import-boundary",
        "windows-virtual-memory-config-boundary",
        "windows-virtual-memory-commit-boundary",
        "windows-virtual-memory-decommit-boundary",
        "windows-virtual-memory-release-boundary",
        "windows-virtual-memory-reset-boundary",
        "windows-virtual-memory-protect-boundary",
        "windows-virtual-memory-reserve-boundary",
        "windows-virtual-memory-retry-boundary",
        "windows-virtual-memory-allocation-export-boundary",
    ] {
        assert_focused_evidence(audit_id, WINDOWS_PRIMITIVE_FOCUSED_PATH);
    }
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
        ("unix-ffi-import-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-mmap-syscall-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-process-info-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-random-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-thread-tls-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-huge-page-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-numa-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        (
            "windows-process-info-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "windows-output-env-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        ("windows-tls-fls-boundary", WINDOWS_PRIMITIVE_FOCUSED_PATH),
        (
            "windows-large-page-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        ("windows-numa-boundary", WINDOWS_PRIMITIVE_FOCUSED_PATH),
        ("windows-etw-boundary", WINDOWS_PRIMITIVE_FOCUSED_PATH),
        ("windows-random-boundary", WINDOWS_PRIMITIVE_FOCUSED_PATH),
        (
            "windows-dynamic-import-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        ("wasi-memory-growth-boundary", WASI_PRIMITIVE_FOCUSED_PATH),
        ("wasi-range-primitive-boundary", WASI_PRIMITIVE_FOCUSED_PATH),
        ("wasi-io-query-boundary", WASI_PRIMITIVE_FOCUSED_PATH),
        ("wasi-thread-shim-boundary", WASI_PRIMITIVE_FOCUSED_PATH),
        (
            "emscripten-import-boundary",
            EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "emscripten-memory-boundary",
            EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "emscripten-clock-io-boundary",
            EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "emscripten-pthread-key-boundary",
            EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "emscripten-thread-shim-boundary",
            EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH,
        ),
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
    ] {
        assert_focused_evidence(audit_id, focused_path);
    }
}
