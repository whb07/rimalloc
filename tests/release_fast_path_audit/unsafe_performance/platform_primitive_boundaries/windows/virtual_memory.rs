use super::*;

pub(super) fn assert_windows_virtual_memory_primitives(windows_rs: &str) {
    let mem_init = source_item(
        "src/platform/windows.rs",
        windows_rs,
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
        windows_rs,
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
        windows_rs,
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
        let item = source_item("src/platform/windows.rs", windows_rs, marker);
        assert!(
            item.contains(expected) && item.contains("GetLastError()"),
            "{marker} should preserve the Windows memory primitive and error code path"
        );
    }
}
