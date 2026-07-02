use super::*;

#[path = "windows/large_pages.rs"]
mod large_pages;
#[path = "windows/process_thread.rs"]
mod process_thread;
#[path = "windows/virtual_memory.rs"]
mod virtual_memory;

#[test]
fn windows_primitive_backend_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let windows_rs = fs::read_to_string(root.join("src/platform/windows.rs"))
        .expect("read src/platform/windows.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/windows.rs\t")),
        "Windows primitive backend should stay off the unsafe migration allowlist"
    );
    assert!(
        !windows_rs.contains("SAFETY[AUDIT:windows-primitive-boundary]"),
        "Windows primitive backend aggregate marker should stay split"
    );
    assert!(
        !windows_rs.contains("SAFETY[AUDIT:windows-virtual-memory-boundary]"),
        "Windows virtual-memory markers should stay split by operation"
    );
    for (marker, minimum_count) in [
        ("windows-virtual-memory-import-boundary", 4),
        ("windows-virtual-memory-config-boundary", 1),
        ("windows-virtual-memory-commit-boundary", 2),
        ("windows-virtual-memory-decommit-boundary", 3),
        ("windows-virtual-memory-release-boundary", 3),
        ("windows-virtual-memory-reset-boundary", 3),
        ("windows-virtual-memory-protect-boundary", 2),
        ("windows-virtual-memory-reserve-boundary", 6),
        ("windows-virtual-memory-retry-boundary", 2),
        ("windows-virtual-memory-allocation-export-boundary", 6),
        ("windows-fls-api-boundary", 1),
        ("windows-dll-lifecycle-boundary", 5),
        ("windows-thread-yield-boundary", 1),
        ("windows-dll-entry-boundary", 11),
        ("windows-allocator-lifecycle-boundary", 2),
        ("windows-fls-thread-boundary", 5),
        ("windows-teb-threadpool-boundary", 2),
        ("windows-large-page-boundary", 11),
        ("windows-huge-page-allocation-boundary", 11),
        ("windows-process-info-boundary", 11),
        ("windows-numa-boundary", 8),
        ("windows-output-env-boundary", 7),
        ("windows-etw-boundary", 4),
        ("windows-random-boundary", 2),
        ("windows-dynamic-import-boundary", 1),
    ] {
        let needle = format!("SAFETY[AUDIT:{marker}]");
        let count = windows_rs.matches(needle.as_str()).count();
        assert!(
            count >= minimum_count,
            "Windows primitive audit marker `{marker}` should have at least {minimum_count} source markers, found {count}"
        );
    }
    assert!(
        !windows_rs.contains("SAFETY[AUDIT:windows-tls-fls-boundary]"),
        "Windows TLS/FLS markers should stay split by FLS API, DLL lifecycle, FLS thread, and TEB probe responsibility"
    );
    for import in [
        "pub fn VirtualAlloc(",
        "pub fn VirtualFree(lpAddress: PVOID, dwSize: SIZE_T, dwFreeType: DWORD) -> BOOL;",
        "pub fn VirtualProtect(",
        "pub fn VirtualQuery(",
        "pub fn GetProcessTimes(",
        "pub fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;",
        "pub fn WriteConsoleA(",
        "pub fn WriteFile(",
        "pub fn LoadLibraryA(lpLibFileName: LPCSTR) -> HINSTANCE;",
        "pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> FARPROC;",
        "fn atexit(func: Option<unsafe extern \"C\" fn()>) -> c_int;",
        "fn NtCurrentTeb() -> *mut c_void;",
        "pub fn EventWriteTransfer(",
        "pub fn EventRegister(",
        "pub fn EventUnregister(RegHandle: REGHANDLE) -> ULONG;",
    ] {
        assert!(
            windows_rs.contains(import),
            "Windows primitive import missing: {import}"
        );
    }

    virtual_memory::assert_windows_virtual_memory_primitives(&windows_rs);
    large_pages::assert_windows_large_page_primitives(&windows_rs);
    process_thread::assert_windows_process_thread_and_etw_primitives(&windows_rs);
}
