use super::*;

#[path = "runtime_heap/threadlocal_runtime.rs"]
mod threadlocal_runtime;

pub(super) fn assert_runtime_heap_no_exports(sources: &UnsafePerformanceSources) {
    let alloc_rs = sources.alloc_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();
    let threadlocal_rs = sources.threadlocal_rs.as_str();

    threadlocal_runtime::assert_threadlocal_runtime_no_exports(threadlocal_rs);

    for (source_name, source, internal_name) in [
        (
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero_ex",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_realloc_zero",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_guarded",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub(crate) unsafe extern \"C\" fn mi_theap_alloc_new",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub extern \"C\" fn _mi_thread_id",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_thread_done",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_guarded_init",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_options_init",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_set",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_cached_set",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_safe",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_main_safe",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_empty_get",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_is_heap_main",
        ),
        (
            "src/allocator/init.rs",
            init_rs,
            "pub unsafe extern \"C\" fn _mi_is_theap_main",
        ),
        ("src/allocator/init.rs", init_rs, "fn _mi_subproc_to_id"),
        ("src/allocator/init.rs", init_rs, "fn _mi_preloading"),
        ("src/allocator/init.rs", init_rs, "fn _mi_subproc_main"),
        ("src/allocator/init.rs", init_rs, "fn _mi_subproc"),
        ("src/allocator/init.rs", init_rs, "fn _mi_subproc_from_id"),
        ("src/allocator/init.rs", init_rs, "fn _mi_subproc_heap_main"),
        ("src/allocator/init.rs", init_rs, "fn _mi_auto_process_init"),
        ("src/allocator/init.rs", init_rs, "fn _mi_auto_process_done"),
        (
            "src/allocator/heap.rs",
            heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_force_destroy",
        ),
        (
            "src/allocator/heap.rs",
            heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_peek",
        ),
        (
            "src/allocator/heap.rs",
            heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_or_init",
        ),
        (
            "src/allocator/page.rs",
            page_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_retired",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_abandon",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_theap_init",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_create",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_incref",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_decref",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_free",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_delete",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_random_next",
        ),
    ] {
        assert_internal_no_export(source_name, source, internal_name);
    }
}
