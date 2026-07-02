pub(super) fn assert_platform_primitive_focused_evidence(evidence: &str) {
    let assert_focused_evidence = |audit_id: &str, focused_path: &str| {
        let row_prefix = format!("{audit_id}\ttest\t{focused_path}\t");
        assert!(
            evidence.lines().any(|line| line.starts_with(&row_prefix)),
            "unsafe audit evidence should map {audit_id} to focused release-audit module {focused_path}"
        );
    };

    const OS_MEMORY_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/os_memory/os_wrappers.rs";
    const UNIX_PRIMITIVE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/os_memory/platform_helpers.rs";
    const WINDOWS_PRIMITIVE_FOCUSED_PATH: &str =
        "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/windows.rs";
    const WASI_PRIMITIVE_FOCUSED_PATH: &str = "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/wasi.rs";
    const EMSCRIPTEN_PRIMITIVE_FOCUSED_PATH: &str = "tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/emscripten.rs";

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

    for (audit_id, focused_path) in [
        ("os-memory-config-state-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-subproc-stats-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-page-size-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-aligned-hint-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-page-align-boundary", OS_MEMORY_FOCUSED_PATH),
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
        (
            "os-memory-retained-wrapper-boundary",
            OS_MEMORY_FOCUSED_PATH,
        ),
        ("os-memory-huge-page-boundary", OS_MEMORY_FOCUSED_PATH),
        ("os-memory-numa-boundary", OS_MEMORY_FOCUSED_PATH),
        ("unix-ffi-import-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-file-syscall-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-mmap-primitive-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-memory-alloc-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
        ("unix-memory-commit-boundary", UNIX_PRIMITIVE_FOCUSED_PATH),
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
        ("windows-fls-api-boundary", WINDOWS_PRIMITIVE_FOCUSED_PATH),
        (
            "windows-dll-lifecycle-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "windows-fls-thread-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "windows-teb-threadpool-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "windows-large-page-boundary",
            WINDOWS_PRIMITIVE_FOCUSED_PATH,
        ),
        (
            "windows-huge-page-allocation-boundary",
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
    ] {
        assert_focused_evidence(audit_id, focused_path);
    }
}
