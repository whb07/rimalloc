use super::*;

pub(super) fn assert_process_lifecycle_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let init_rs = sources.init_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let page_rs = sources.page_rs.as_str();

    init_markers::assert_init_lifecycle_markers(init_rs, prim_rs, unsafe_allowlist);
    assert!(
        !heap_rs.contains("SAFETY[AUDIT:heap-lifecycle-boundary]"),
        "heap lifecycle boundaries should remain split into focused operation markers"
    );
    for (audit_id, minimum) in [
        ("heap-thread-association-boundary", 14),
        ("heap-configuration-boundary", 6),
        ("heap-stats-merge-boundary", 5),
        ("heap-creation-boundary", 8),
        ("heap-nonmain-cleanup-boundary", 6),
        ("heap-nonmain-lifecycle-boundary", 10),
        ("heap-main-lifecycle-boundary", 4),
        ("heap-ownership-query-boundary", 23),
        ("heap-underutilized-boundary", 4),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = heap_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "heap unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc()")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_to_id")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_main")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_from_id"
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_heap_main"
            )
            && init_rs.contains("static mut subproc_main: MiSubprocPrefix")
            && init_rs.contains("static mut subprocs: *mut mi_subproc_t =")
            && init_rs.contains("core::arch::global_asm!(\".hidden subproc_main\")")
            && init_rs.contains("#[cfg_attr(target_os = \"linux\", unsafe(no_mangle))]")
            && init_rs.contains("unsafe fn mi_subproc_main_addr()")
            && init_rs.contains("unsafe fn mi_subproc_main_addr_typed() -> MainSubproc")
            && init_rs.contains(
                "MainSubproc::new_unchecked(SubprocPtr::new_unchecked(mi_subproc_main_addr()))"
            )
            && init_rs.contains(
                "unsafe fn mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> Option<HeapPtr>"
            )
            && init_rs.contains("fn _mi_subproc_to_id_typed(subproc: Option<SubprocPtr>)")
            && init_rs.contains("unsafe fn _mi_subproc_typed() -> BorrowedSubproc")
            && init_rs.contains("unsafe fn _mi_subproc_from_id_typed(subproc_id: mi_subproc_id_t)")
            && init_rs.contains(
                "unsafe fn _mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> MainHeap"
            )
            && init_rs.contains("\"lea {main}, [rip + subproc_main]\"")
            && !init_rs.contains("mi_subproc_main_ptr")
            && init_rs.contains("crate::prim::_mi_theap_default_raw()")
            && init_rs.contains("static MI_SUBPROC_TO_ID_KEEP")
            && init_rs.contains("static MI_SUBPROC_MAIN_KEEP")
            && init_rs.contains("static MI_SUBPROC_FROM_ID_KEEP")
            && init_rs.contains("static MI_SUBPROC_HEAP_MAIN_KEEP")
            && prim_rs.contains("pub unsafe fn _mi_theap_default_raw()"),
        "subproc helpers should stay as C's small local helper boundaries and inspect the raw default TLS cell"
    );
    subproc_fast_paths::assert_subproc_fast_paths(init_rs, heap_rs, free_rs, page_rs);
}
