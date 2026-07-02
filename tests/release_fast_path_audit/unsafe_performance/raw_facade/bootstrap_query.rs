use super::*;

pub(super) fn assert_bootstrap_and_query_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let page_map = sources.page_map.as_str();
    let internal = sources.internal.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let arena_debug_rs = sources.arena_debug_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let arena_meta_rs = sources.arena_meta_rs.as_str();
    let deferred_rs = sources.deferred_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let raw_macros_rs = sources.raw_macros_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();

    let raw_macro_marker_minimums = [
        ("raw-macro-bootstrap-boundary", 15),
        ("raw-macro-fast-override-boundary", 3),
        ("raw-macro-macos-interpose-boundary", 5),
        ("raw-macro-windows-crt-boundary", 14),
        ("raw-macro-windows-aligned-boundary", 14),
        ("raw-macro-unix-core-boundary", 4),
        ("raw-macro-unix-string-boundary", 4),
        ("raw-macro-unix-realloc-boundary", 2),
        ("raw-macro-unix-page-aligned-boundary", 6),
        ("raw-macro-unix-aligned-posix-boundary", 11),
        ("raw-macro-unix-cfree-boundary", 2),
        ("raw-macro-unix-product-realloc-boundary", 5),
        ("raw-macro-wasi-libc-boundary", 11),
        ("raw-macro-linux-libc-boundary", 10),
        ("raw-macro-linux-libc-aligned-boundary", 10),
        ("raw-macro-allocation-query-boundary", 10),
        ("raw-macro-cxx-delete-boundary", 3),
        ("raw-macro-cxx-array-delete-boundary", 3),
        ("raw-macro-cxx-aligned-delete-boundary", 8),
        ("raw-macro-cxx-aligned-new64-boundary", 6),
        ("raw-macro-cxx-aligned-new32-boundary", 6),
        ("raw-macro-cxx-aligned-operator-boundary", 9),
        ("raw-macro-cxx-nothrow-boundary", 7),
        ("raw-macro-cxx-new-boundary", 4),
        ("raw-macro-header-forwarding-boundary", 6),
    ];

    for (audit_id, minimum) in raw_macro_marker_minimums {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = raw_macros_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "raw macro facade should retain at least {minimum} {audit_id} markers; found {count}"
        );
    }

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/internal/mod.rs\t"))
            && !internal.contains("SAFETY[AUDIT:internal-translation-boundary]")
            && internal.contains("unsafe extern \"C\" {")
            && internal.contains("pub unsafe fn _mi_ptr_page(p: *const c_void) -> *mut mi_page_t")
            && internal.contains("pub unsafe fn _mi_checked_ptr_page(p: *const c_void)")
            && internal.contains("pub unsafe fn _mi_unchecked_ptr_page(p: *const c_void)")
            && internal.contains("pub(crate) unsafe fn mi_page_block_size_typed(")
            && internal.contains("pub unsafe fn mi_page_start(page: *const mi_page_t)")
            && internal.contains("pub unsafe fn mi_page_slice_start(page: *const mi_page_t)")
            && internal.contains("pub unsafe fn mi_page_thread_free(page: *const mi_page_t)")
            && internal.contains("pub unsafe fn mi_page_is_owned(page: *const mi_page_t)")
            && internal.contains("pub(crate) unsafe fn mi_page_claim_ownership(")
            && internal.contains("pub unsafe fn mi_block_next(")
            && internal.contains("pub unsafe fn mi_block_set_next(")
            && internal.contains("pub(crate) fn mi_ptr_decode(")
            && internal.contains("pub(crate) fn mi_ptr_encode(")
            && internal.contains("crate::provenance::remember_encoded(word.get(), null, keys, p);")
            && internal.contains(
                "crate::core_types::AtomicPtrField(ptr::addr_of!((*theap).heap).cast_mut())"
            )
            && internal.contains(
                "crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField("
            )
            && internal.contains("unsafe fn mi_cpu_movsb_max() -> usize")
            && internal.contains("unsafe fn mi_cpu_stosb_max() -> usize")
            && internal.contains("core::arch::asm!("),
        "internal translation unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    for (audit_id, minimum) in [
        ("internal-runtime-shim-boundary", 1),
        ("internal-c-abi-shim-boundary", 1),
        ("internal-cpu-memory-boundary", 15),
        ("internal-thread-heap-shim-boundary", 5),
        ("internal-direct-bin-boundary", 1),
        ("internal-page-map-lookup-boundary", 5),
        ("internal-page-size-count-boundary", 13),
        ("internal-page-owner-heap-boundary", 3),
        ("internal-page-location-boundary", 7),
        ("internal-page-flag-owner-boundary", 8),
        ("internal-thread-free-boundary", 4),
        ("internal-block-link-boundary", 13),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = internal.matches(&marker).count();
        assert!(
            count >= minimum,
            "internal unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        !internal.contains("SAFETY[AUDIT:internal-page-metadata-boundary]"),
        "internal page metadata helpers should stay split into focused operation markers"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/raw/macros.rs\t"))
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-allocation-boundary]")
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-libc-forwarding-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-bootstrap-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-fast-override-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-macos-interpose-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-windows-crt-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-windows-aligned-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-core-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-string-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-realloc-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-cfree-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-wasi-libc-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-linux-libc-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-allocation-query-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-delete-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-new-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-header-forwarding-boundary]")
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-override-entry-boundary]")
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-boundary]")
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-boundary]")
            && raw_macros_rs.contains("core::arch::global_asm!(")
            && raw_macros_rs.contains("mod unix_bootstrap")
            && raw_macros_rs.contains("if crate::internal::_mi_preloading()")
            && raw_macros_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && raw_macros_rs.contains("pub unsafe extern \"C\" fn mi_cxx_delete")
            && raw_macros_rs.contains("pub unsafe fn mi_forward1")
            && raw_macros_rs.contains("pub unsafe fn mi_malloc_size_forward")
            && raw_macros_rs.contains("pub unsafe fn mi_unix_malloc(")
            && raw_macros_rs.contains("pub unsafe fn mi_unix_posix_memalign")
            && raw_macros_rs.contains("pub unsafe extern \"C\" fn mi_unix_malloc_export")
            && raw_macros_rs.contains("pub unsafe extern \"C\" fn mi_wasi_libc_malloc_export")
            && raw_macros_rs.contains("pub unsafe extern \"C\" fn mi_linux_libc_malloc_export"),
        "raw macro allocation facade unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );

    local_boundaries::assert_page_map_and_deferred_boundaries(local_boundaries::BoundarySources {
        page_map,
        unsafe_allowlist,
        alloc_rs,
        arena_rs,
        arena_debug_rs,
        arena_heap_pages_rs,
        arena_meta_rs,
        deferred_rs,
        free_rs,
        heap_rs,
        init_rs,
        internal,
        page_rs,
        theap_rs,
    });
}
