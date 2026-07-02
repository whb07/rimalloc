use super::*;

pub(super) fn assert_platform_tls_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let init_rs = sources.init_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let threadlocal_rs = sources.threadlocal_rs.as_str();

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/mod.rs\t")),
        "platform TLS primitive boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        !prim_rs.contains("SAFETY[AUDIT:platform-tls-primitives]"),
        "platform TLS primitive markers should stay split by operation"
    );
    assert!(
        !prim_rs.contains("SAFETY[AUDIT:platform-thread-heap-association-boundary]"),
        "platform thread-heap association markers should stay split by load/store and fallback responsibility"
    );
    for (audit_id, exact_count) in [
        ("platform-dynamic-tls-key-boundary", 3),
        ("platform-linux-tls-shim-boundary", 11),
        ("platform-thread-heap-default-load-boundary", 6),
        ("platform-thread-heap-cached-load-boundary", 5),
        ("platform-thread-heap-default-store-boundary", 2),
        ("platform-thread-heap-cached-store-boundary", 2),
        ("platform-thread-heap-empty-sentinel-boundary", 6),
        ("platform-thread-heap-association-fallback-boundary", 11),
        ("platform-allocator-shim-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            prim_rs.matches(&marker).count(),
            exact_count,
            "platform TLS primitive marker {audit_id} should keep exact operation count"
        );
    }
    assert!(
        prim_rs.contains("pub const MI_TLS_MODEL_THREAD_LOCAL")
            && prim_rs.contains("pub const MI_TLS_MODEL_FIXED_SLOT")
            && prim_rs.contains("pub const MI_TLS_MODEL_DYNAMIC_WIN32")
            && prim_rs.contains("pub const MI_TLS_MODEL_DYNAMIC_PTHREADS")
            && prim_rs.contains("pub const MI_THEAP_INITASNULL")
            && prim_rs.contains("pub const MI_TLS_MODEL_FIXED_SLOT_DEFAULT: usize = 108;")
            && prim_rs.contains("pub const MI_TLS_MODEL_FIXED_SLOT_CACHED: usize = 109;")
            && prim_rs.contains("pub const MI_TLS_EXPANSION_SLOT: usize")
            && prim_rs.contains("fn mi_prim_apple_tls_slot(slot: usize) -> *mut c_void")
            && prim_rs.contains("fn _mi_tls_theap_default_get() -> *mut mi_theap_t;")
            && prim_rs.contains("fn _mi_tls_theap_cached_get() -> *mut mi_theap_t;")
            && prim_rs.contains("__mi_theap_default@GOTTPOFF")
            && prim_rs.contains("__mi_theap_cached@GOTTPOFF")
            && prim_rs.contains("\"lea {empty}, [rip + _mi_theap_empty]\""),
        "platform TLS primitive layer should preserve model constants, Apple fixed-slot path, Linux C shim declarations, and x86_64 TLS assembly"
    );
    for marker in [
        "pub unsafe fn _mi_theap_default_raw(",
        "pub unsafe fn _mi_theap_default(",
        "pub unsafe fn _mi_theap_cached(",
        "pub(crate) fn mi_prim_thread_local_default_set(",
        "pub(crate) fn mi_prim_thread_local_cached_set(",
        "pub(crate) unsafe fn _mi_page_associated_theap_peek(",
        "pub(crate) unsafe extern \"C\" fn _mi_allocator_init(",
        "pub(crate) unsafe extern \"C\" fn _mi_allocator_done(",
    ] {
        let item = source_item("src/platform/mod.rs", prim_rs, marker);
        assert_no_rust_heap_constructs("src/platform/mod.rs", marker, item);
    }

    let default_raw = source_item(
        "src/platform/mod.rs",
        prim_rs,
        "pub unsafe fn _mi_theap_default_raw(",
    );
    assert!(
        default_raw.contains("unsafe { mi_linux_x64_theap_default_get() }")
            && default_raw.contains("unsafe { _mi_tls_theap_default_get() }")
            && default_raw.contains("mi_prim_tls_slot(MI_TLS_MODEL_FIXED_SLOT_DEFAULT)")
            && default_raw
                .contains("_mi_thread_local_get(_mi_theap_default_key) as *mut mi_theap_t"),
        "raw default thread-heap lookup should preserve Linux fast path, C shim fallback, fixed-slot path, and dynamic key path"
    );

    let cached = source_item(
        "src/platform/mod.rs",
        prim_rs,
        "pub unsafe fn _mi_theap_cached(",
    );
    assert!(
        cached.contains("mi_linux_x64_theap_cached_get()")
            && cached.contains("unsafe { _mi_tls_theap_cached_get() }")
            && cached.matches("unsafe { __mi_theap_empty() }").count() >= 4
            && cached.contains("mi_prim_tls_slot(MI_TLS_MODEL_FIXED_SLOT_CACHED)")
            && cached.contains("_mi_thread_local_get(_mi_theap_cached_key) as *mut mi_theap_t"),
        "cached thread-heap lookup should preserve Linux fast path, empty fallback, fixed-slot path, and dynamic key path"
    );

    let page_assoc = source_item(
        "src/platform/mod.rs",
        prim_rs,
        "pub(crate) unsafe fn _mi_page_associated_theap_peek(",
    );
    assert!(
        page_assoc.contains("let heap = unsafe { (*page).heap };")
            && page_assoc.contains("crate::init::mi_theap_main_tls_peek()")
            && page_assoc.contains("unsafe { _mi_heap_theap_peek(heap) }")
            && page_assoc.contains("ThreadHeapPtr::new(theap)"),
        "page-associated thread-heap lookup should keep non-initializing heap peeks and typed pointer wrapping"
    );

    super::super::super::threadlocal_boundaries::assert_dynamic_threadlocal_boundaries(
        threadlocal_rs,
    );
    assert!(
        init_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_cached_set(")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_default_safe("
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_main_safe("
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_empty_get("
            )
            && init_rs.contains("static MI_THEAP_CACHED_SET_KEEP")
            && init_rs.contains("static MI_THEAP_DEFAULT_SAFE_KEEP")
            && init_rs.contains("static MI_THEAP_MAIN_SAFE_KEEP")
            && init_rs.contains("static MI_THEAP_EMPTY_GET_KEEP"),
        "thread-heap accessors should keep C's local helper boundaries"
    );
}
