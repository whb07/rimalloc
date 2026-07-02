use super::*;

pub(super) fn assert_windows_process_thread_and_etw_primitives(windows_rs: &str) {
    let process_info = source_item(
        "src/platform/windows.rs",
        windows_rs,
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
        windows_rs,
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
        windows_rs,
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
        windows_rs,
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
        windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
    );
    let thread_done = source_item(
        "src/platform/windows.rs",
        windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
    );
    let thread_assoc = source_item(
        "src/platform/windows.rs",
        windows_rs,
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
        windows_rs,
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
        windows_rs,
        "pub unsafe extern \"system\" fn McGenControlCallbackV2(",
    );
    let event_write = source_item(
        "src/platform/windows.rs",
        windows_rs,
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
