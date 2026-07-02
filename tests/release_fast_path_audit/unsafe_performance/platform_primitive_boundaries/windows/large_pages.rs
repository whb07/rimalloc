use super::*;

pub(super) fn assert_windows_large_page_primitives(windows_rs: &str) {
    let huge_once = source_item(
        "src/platform/windows.rs",
        windows_rs,
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
        windows_rs,
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
}
