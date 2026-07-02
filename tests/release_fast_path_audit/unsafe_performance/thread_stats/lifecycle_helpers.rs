pub(super) fn assert_lifecycle_helper_boundaries(init_rs: &str, options_rs: &str, theap_rs: &str) {
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_auto_process_init()")
            && init_rs.contains(
                "static MI_AUTO_PROCESS_INIT_KEEP: unsafe extern \"C\" fn() = _mi_auto_process_init;"
            )
            && init_rs.contains("#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_auto_process_done()")
            && init_rs.contains(
                "static MI_AUTO_PROCESS_DONE_KEEP: unsafe extern \"C\" fn() = _mi_auto_process_done;"
            ),
        "_mi_auto_process_init and _mi_auto_process_done should keep C's local lifecycle helper boundaries"
    );
    assert!(
        options_rs
            .contains("#[inline(never)]\npub(crate) fn _mi_message(args: fmt::Arguments<'_>)")
            && options_rs.contains(
                "mi_vfprintf_thread(None, ptr::null_mut(), c\"mimalloc: \".as_ptr(), args);"
            )
            && options_rs
                .contains("static MI_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_message;"),
        "_mi_message should keep C's local message helper boundary and mimalloc prefix path"
    );
    assert!(
        options_rs.contains("#[inline(never)]\nfn mi_recurse_enter_prim() -> bool")
            && options_rs.contains(
                "static MI_RECURSE_ENTER_PRIM_KEEP: fn() -> bool = mi_recurse_enter_prim;"
            )
            && options_rs.contains("#[inline(never)]\nfn mi_recurse_exit_prim()")
            && options_rs
                .contains("static MI_RECURSE_EXIT_PRIM_KEEP: fn() = mi_recurse_exit_prim;")
            && options_rs.contains("mi_recurse_enter_prim()")
            && options_rs.contains("mi_recurse_exit_prim();"),
        "option recursion guards should keep C's noinline primitive helper boundaries"
    );

    assert!(
        !init_rs.contains("fn _mi_theap_collect_abandon(")
            && !init_rs.contains("fn _mi_theap_create(")
            && !init_rs.contains("fn _mi_theap_free(")
            && !init_rs.contains("fn _mi_heap_force_destroy("),
        "thread initialization internals should call crate-local theap lifecycle helpers directly"
    );
    assert!(
        theap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_collect_abandon"
        ) && theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_free"),
        "thread teardown should keep C's heap collect/free call boundaries instead of inlining them into _mi_thread_done"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_init")
            && theap_rs.contains("let Some(theap) = ThreadHeapPtr::new(theap) else")
            && theap_rs.contains("let Some(heap) = HeapPtr::new(heap) else")
            && theap_rs.contains("let Some(tld) = TldPtr::new(tld) else")
            && theap_rs.contains(
                "unsafe fn _mi_theap_init_typed(theap: ThreadHeapPtr, heap: HeapPtr, tld: TldPtr)"
            )
            && theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_create")
            && theap_rs.contains("unsafe fn _mi_theap_create_typed(heap: HeapPtr, tld: TldPtr) -> Option<ThreadHeapPtr>")
            && theap_rs.contains("unsafe { _mi_theap_create_typed(heap, tld) }\n        .map(ThreadHeapPtr::as_ptr)")
            && theap_rs.contains("_mi_theap_init_typed(theap, heap, tld);\n        Some(theap)"),
        "_mi_theap_create and _mi_theap_init should parse raw metadata pointers once and use typed wrappers below the ABI shim"
    );
}
