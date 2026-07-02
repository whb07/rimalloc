use super::*;

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
        ("windows-dll-lifecycle-boundary", 19),
        ("windows-fls-thread-boundary", 5),
        ("windows-teb-threadpool-boundary", 2),
        ("windows-large-page-boundary", 22),
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

    let huge_once = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "unsafe fn win_enable_large_os_pages_once(",
    );
    assert!(
        huge_once.contains("OpenProcessToken(")
            && huge_once.contains("LookupPrivilegeValueA(")
            && huge_once.contains("AdjustTokenPrivileges(")
            && huge_once.contains("pGetLargePageMinimum")
            && huge_once.contains("CloseHandle(token)"),
        "Windows large-page enablement should preserve token privilege and page-size probing"
    );

    let huge_alloc = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "unsafe fn _mi_prim_alloc_huge_os_pagesx(",
    );
    assert!(
        huge_alloc.contains("MEM_LARGE_PAGES | MEM_COMMIT | MEM_RESERVE")
            && huge_alloc.contains("pNtAllocateVirtualMemoryEx")
            && huge_alloc.contains("MI_MEM_EXTENDED_PARAMETER_NONPAGED_HUGE")
            && huge_alloc
                .contains("MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterNumaNode")
            && huge_alloc.contains("pVirtualAlloc2")
            && huge_alloc.contains("VirtualAlloc(hint_addr, size, flags, PAGE_READWRITE)"),
        "Windows huge-page allocation should preserve huge-page, NUMA, VirtualAlloc2, and fallback paths"
    );

    let process_info = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_process_info(",
    );
    assert!(
        process_info.contains("if pinfo.is_null()")
            && process_info.contains(
                "GetProcessTimes(GetCurrentProcess(), &mut ct, &mut et, &mut st, &mut ut)"
            )
            && process_info.contains("pGetProcessMemoryInfo = mi_get_proc")
            && process_info.contains("(*pinfo).current_rss = info.WorkingSetSize")
            && process_info.contains("(*pinfo).page_faults = info.PageFaultCount as usize"),
        "Windows process info should preserve nullable output, process times, and dynamic PSAPI counters"
    );

    let stderr = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_out_stderr(",
    );
    assert!(
        stderr.contains("if msg.is_null() || _mi_preloading()")
            && stderr.contains("GetStdHandle(STD_ERROR_HANDLE)")
            && stderr.contains("GetConsoleScreenBufferInfo(mi_stderr_handle, &mut sbi)")
            && stderr.contains("WriteConsoleA(")
            && stderr.contains("WriteFile(")
            && stderr.contains("std::io::stderr().lock()"),
        "Windows stderr primitive should preserve console, file, and host fallback output"
    );

    let getenv = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_getenv(",
    );
    assert!(
        getenv.contains("if result.is_null() || result_size == 0")
            && getenv.contains("unsafe { *result = 0 };")
            && getenv.contains("if name.is_null()")
            && getenv.contains("CStr::from_ptr(name)")
            && getenv.contains("mi_getenv_value(&key)")
            && getenv.contains(
                "copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), result, bytes.len())"
            )
            && getenv.contains("*result.add(bytes.len()) = 0;"),
        "Windows getenv primitive should preserve null checks, case-insensitive lookup, and bounded copy"
    );

    let fls_done = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"system\" fn mi_fls_done(",
    );
    assert!(
        fls_done.contains("if !theap.is_null()")
            && fls_done.contains("crate::internal::_mi_thread_done(theap)")
            && fls_done.contains("FlsSetValue(mi_fls_key, null_mut())"),
        "Windows FLS destructor should null-check and clear thread-local heap state"
    );

    let thread_init = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
    );
    let thread_done = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
    );
    let thread_assoc = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
    );
    assert!(
        thread_init.contains("FlsAlloc(Some(mi_fls_done))")
            && thread_done.contains("FlsFree(mi_fls_key)")
            && thread_done.contains("mi_fls_key = FLS_OUT_OF_INDEXES")
            && thread_assoc.contains("FlsSetValue(mi_fls_key, theap.cast::<c_void>())"),
        "Windows thread lifecycle should preserve FLS key creation, deletion, and heap association"
    );

    let threadpool = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool(",
    );
    assert!(
        threadpool.contains("NtCurrentTeb()")
            && threadpool.contains("if MI_SIZE_BITS == 32 { 0x0F90 } else { 0x1778 }")
            && threadpool.contains("return !pool_data.is_null();"),
        "Windows thread-pool detection should preserve TEB probing offsets"
    );

    let control = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"system\" fn McGenControlCallbackV2(",
    );
    let event_write = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe fn McGenEventWrite(",
    );
    assert!(
        control.contains("EVENT_CONTROL_CODE_ENABLE_PROVIDER")
            && control.contains("EVENT_CONTROL_CODE_DISABLE_PROVIDER")
            && control.contains("write_bytes(ctx_ref.EnableBitMask, 0, word_count)")
            && event_write.contains("EventWriteTransfer(")
            && windows_rs.contains("pub unsafe fn EventWriteETW_MI_ALLOC(")
            && windows_rs.contains("pub unsafe fn EventWriteETW_MI_FREE("),
        "Windows ETW helpers should preserve provider enablement, disable clearing, and event write forwarding"
    );
}
