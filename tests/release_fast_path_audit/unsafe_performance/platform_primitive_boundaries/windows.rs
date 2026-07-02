use super::*;

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
    for (marker, minimum_count) in [
        ("windows-virtual-memory-boundary", 32),
        ("windows-tls-fls-boundary", 27),
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

    let mem_init = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_mem_init(",
    );
    assert!(
        mem_init.contains("if config.is_null()")
            && mem_init.contains("GetSystemInfo(&mut si)")
            && mem_init.contains("config).has_virtual_reserve = true")
            && mem_init.contains("LoadLibraryA(c\"kernelbase.dll\".as_ptr())")
            && mem_init.contains("pVirtualAlloc2 = mi_get_proc")
            && mem_init.contains("pNtAllocateVirtualMemoryEx = mi_get_proc")
            && mem_init.contains("pGetLargePageMinimum = mi_get_proc")
            && mem_init.contains("win_enable_large_os_pages(&mut (*config).large_page_size)"),
        "Windows mem init should preserve system-info setup, dynamic primitive lookup, and large-page probing"
    );

    let alloc = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_alloc(",
    );
    assert!(
        alloc.contains("size > 0 && size.is_multiple_of")
            && alloc.contains("let mut flags = MEM_RESERVE;")
            && alloc.contains("if commit {\n        flags |= MEM_COMMIT;")
            && alloc.contains("win_virtual_alloc(")
            && alloc.contains("if !is_zero.is_null()")
            && alloc.contains("if !addr.is_null()"),
        "Windows primitive allocation should preserve flag construction and nullable out-pointer writes"
    );

    let alloc_once = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "unsafe fn win_virtual_alloc_prim_once(",
    );
    assert!(
        alloc_once.contains("_mi_os_get_aligned_hint(try_alignment, size)")
            && alloc_once.contains("VirtualAlloc(hint, size, flags, PAGE_READWRITE)")
            && alloc_once.contains("try_alignment > unsafe { win_allocation_granularity }")
            && alloc_once.contains("let Some(virtual_alloc2) = unsafe { pVirtualAlloc2 }")
            && alloc_once.contains(
                "MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterAddressRequirements"
            )
            && alloc_once.contains("virtual_alloc2(")
            && alloc_once.contains("VirtualAlloc(addr, size, flags, PAGE_READWRITE)"),
        "Windows virtual allocation helper should keep hinted allocation, VirtualAlloc2 alignment, and VirtualAlloc fallback"
    );

    for (marker, expected) in [
        (
            "pub unsafe extern \"C\" fn _mi_prim_commit(",
            "VirtualAlloc(addr, size, MEM_COMMIT, PAGE_READWRITE)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_decommit(",
            "VirtualFree(addr, size, MEM_DECOMMIT)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_free(",
            "VirtualFree(addr, 0, MEM_RELEASE)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_reset(",
            "VirtualAlloc(addr, size, MEM_RESET, PAGE_READWRITE)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_protect(",
            "VirtualProtect(",
        ),
    ] {
        let item = source_item("src/platform/windows.rs", &windows_rs, marker);
        assert!(
            item.contains(expected) && item.contains("GetLastError()"),
            "{marker} should preserve the Windows memory primitive and error code path"
        );
    }

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
