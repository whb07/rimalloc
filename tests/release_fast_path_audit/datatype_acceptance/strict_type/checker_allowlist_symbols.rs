use super::super::DatatypeAcceptanceSources;

pub(super) fn assert_strict_checker_and_allowlist_symbols(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let strict_check_rs = audit_sources.strict_type_checker;
    let allowlist_rs = audit_sources.strict_type_allowlist;
    assert!(
        strict_check_rs.contains("class ItemContext:")
            && strict_check_rs.contains("TODO = ROOT / \"TODO.md\"")
            && strict_check_rs
                .contains("def item_contexts_by_line(text: str) -> dict[int, ItemContext]:")
            && strict_check_rs.contains("def wildcard_counts(entries: Iterable[AllowEntry])")
            && strict_check_rs.contains("def verify_wildcard_counts_documented")
            && strict_check_rs.contains("def run_self_tests() -> None:")
            && strict_check_rs.contains("symbol=context.symbols")
            && strict_check_rs.contains("f\"{message} in {context.primary_symbol}\"")
            && strict_check_rs.contains("assert contexts[3].symbols == (\"free_function\",)")
            && strict_check_rs.contains("assert contexts[8].symbols == (\"generic_function\",)")
            && strict_check_rs
                .contains("assert contexts[13].symbols == (\"AtomicField::load\", \"load\")")
            && strict_check_rs
                .contains("assert contexts[18].symbols == (\"AtomicField::MASK\", \"MASK\")")
            && strict_check_rs.contains("assert \"imported\" in contexts[23].symbols")
            && strict_check_rs.contains("assert contexts[28].symbols == (\"TABLE\",)")
            && strict_check_rs.contains(
                "assert bare_contexts[5].symbols == (\"LayoutRequest::malloc\", \"malloc\")"
            )
            && strict_check_rs.contains("\"LayoutRequest::malloc\"")
            && strict_check_rs.contains("wildcard_summary(wildcard_counts(entries))")
            && strict_check_rs.contains("Strict-type wildcard guard: total=")
            && strict_check_rs.contains("\"pointer-usize-roundtrip\"")
            && strict_check_rs.contains("\"generic_function\"")
            && strict_check_rs.contains("\"AtomicField::load\"")
            && strict_check_rs.contains("strict-type self-test failed"),
        "strict-type line-pattern checks should use item-aware symbols and self-test symbol-level line-pattern allowlist entries"
    );
    let wildcard_entries = allowlist_rs
        .lines()
        .filter(|line| {
            let mut fields = line.split('|');
            let kind = fields.next();
            let path = fields.next();
            let symbol = fields.next();
            kind.is_some() && path.is_some() && symbol == Some("*")
        })
        .collect::<Vec<_>>();
    assert!(
        wildcard_entries.is_empty(),
        "strict-type allowlist should not retain wildcard entries: {wildcard_entries:?}"
    );
    for platform_path in [
        "src/allocator/heap.rs",
        "src/allocator/threadlocal.rs",
        "src/platform/windows.rs",
        "src/platform/unix.rs",
        "src/platform/wasi.rs",
        "src/platform/os.rs",
        "src/platform/mod.rs",
        "src/platform/emscripten.rs",
        "src/platform/macos.rs",
        "src/allocator/aligned.rs",
        "src/allocator/arena.rs",
        "src/allocator/tracking.rs",
        "src/internal/mod.rs",
        "src/internal/atomic.rs",
        "src/allocator/stats.rs",
        "src/allocator/thread_heap.rs",
        "src/allocator/alloc.rs",
        "src/allocator/free.rs",
        "src/allocator/init.rs",
        "src/allocator/page_map.rs",
        "src/allocator/options.rs",
        "src/bitmap.rs",
        "src/internal/bitmap.rs",
        "src/internal/random.rs",
        "src/raw/macros.rs",
        "src/types/arena.rs",
        "src/types/atomic.rs",
        "src/types/atomic/tests.rs",
        "src/types/callbacks.rs",
        "src/types/freelist.rs",
        "src/types/guarded.rs",
        "src/types/layout.rs",
        "src/types/memid.rs",
        "src/types/page_queues.rs",
        "src/types/page_queues/tests.rs",
        "src/types/ptr.rs",
        "src/types/ptr/tests.rs",
        "src/types/scalar.rs",
        "src/ffi/libc.rs",
        "src/safe/mod.rs",
        "src/safe/allocation.rs",
        "src/safe/heap.rs",
    ] {
        assert!(
            !allowlist_rs.lines().any(|line| {
                let mut fields = line.split('|');
                let _kind = fields.next();
                let path = fields.next();
                let symbol = fields.next();
                path == Some(platform_path) && symbol == Some("*")
            }),
            "{platform_path} should not retain path-wide strict-type allowlist entries"
        );
    }
    for required in [
        "raw-pointer-parameter|src/platform/windows.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/platform/unix.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/platform/wasi.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/platform/os.rs|_mi_os_free_ex|mi_subproc_t|",
        "raw-pointer-parameter|src/allocator/heap.rs|mi_heap_destroy|mi_heap_t|",
        "raw-pointer-parameter|src/platform/mod.rs|_mi_heap_theap|mi_heap_t|",
        "raw-pointer-parameter|src/platform/emscripten.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/threadlocal.rs|mi_thread_local_claim_fun|mi_arena_t|",
        "metadata-reference|src/platform/unix.rs|_mi_prim_getenv|platform|",
        "metadata-reference|src/platform/windows.rs|McGenControlCallbackV2|platform|",
        "metadata-reference|src/internal/random.rs|_mi_random_next|random|",
        "metadata-reference|src/types/atomic.rs|AtomicArenaPage::load_acquire|atomic-field|",
        "pointer-usize-roundtrip|src/allocator/arena.rs|mi_manage_os_memory_ex2|arena-range|",
        "pointer-usize-roundtrip|src/platform/os.rs|mi_os_alloc_aligned_memid|os-memory|",
        "pointer-usize-roundtrip|src/platform/unix.rs|unix_mmap_prim_aligned|platform|",
        "pointer-usize-roundtrip|src/platform/wasi.rs|mi_prim_mem_grow|platform|",
        "pointer-usize-roundtrip|src/types/ptr/tests.rs|provenance_address_helpers_match_raw_pointer_math|provenance-test|",
        "pointer-usize-roundtrip|src/ffi/libc.rs|mi_outc|output-buffer|",
        "atomic-cast|src/internal/atomic.rs|atomic_usize|atomic-field|",
        "atomic-cast|src/types/atomic/tests.rs|atomic_thread_free_claims_and_releases_ownership_bit|atomic-field-test|",
        "heap-allocating-type|src/types/atomic/concurrency_models.rs|concurrent_remote_pushes_keep_both_nodes_reachable|loom-model|",
        "heap-allocating-type|src/types/atomic/concurrency_models.rs|collect_racing_with_push_does_not_drop_nodes|loom-model|",
        "heap-allocating-type|src/types/atomic/concurrency_models.rs|concurrent_claims_allow_exactly_one_owner_transition|loom-model|",
        "heap-allocating-type|src/types/atomic/concurrency_models.rs|release_ownership_racing_with_push_preserves_heads|loom-model|",
        "heap-allocating-type|src/platform/windows.rs|mi_getenv_value|String|",
        "bare-domain-usize|src/platform/windows.rs|win_virtual_alloc|usize|",
        "bare-domain-usize|src/platform/unix.rs|unix_mmap_prim_aligned|usize|",
        "bare-domain-usize|src/platform/wasi.rs|mi_wasi_try_alloc|usize|",
        "bare-domain-usize|src/platform/os.rs|mi_os_alloc_aligned_ex|usize|",
        "bare-domain-usize|src/raw/macros.rs|mi_unix_reallocarray|usize|",
        "bare-domain-usize|src/types/callbacks.rs|BlockVisitCallback::invoke|usize|",
        "bare-domain-usize|src/types/guarded.rs|GuardedConfig::decide|usize|",
        "bare-domain-usize|src/types/layout.rs|LayoutRequest::malloc|usize|",
        "bare-domain-usize|src/types/memid.rs|ReservedBytes::new|usize|",
        "bare-domain-usize|src/types/page_queues/tests.rs|with_queue|usize|",
        "bare-domain-usize|src/types/scalar.rs|RequestedSize::new|usize|",
        "bare-domain-usize|src/platform/macos.rs|malloc_zone_malloc|usize|",
        "bare-domain-usize|src/safe/allocation.rs|Allocation::with_size_align|usize|",
        "bare-domain-usize|src/safe/heap.rs|HeapAllocation::resize_with_size_align|usize|",
        "bare-domain-usize|src/allocator/tracking.rs|mi_track_malloc|usize|",
        "raw-pointer-parameter|src/allocator/aligned.rs|mi_padding_shrink_call|mi_block_t|",
        "raw-pointer-parameter|src/allocator/aligned.rs|mi_theap_malloc_zero_aligned_at|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/aligned.rs|mi_heap_realloc_aligned|mi_heap_t|",
        "pointer-usize-roundtrip|src/allocator/aligned.rs|mi_theap_realloc_zero_aligned_at_req|alignment/provenance|",
        "raw-pointer-parameter|src/internal/mod.rs|_mi_malloc_generic|mi_theap_t|",
        "raw-pointer-parameter|src/internal/mod.rs|mi_page_block_size|mi_page_t|",
        "raw-pointer-parameter|src/internal/mod.rs|mi_block_nextx|mi_block_t|",
        "pointer-usize-roundtrip|src/internal/mod.rs|mi_page_slice_offset_of|packed-pointer|",
        "raw-pointer-parameter|src/allocator/stats.rs|mi_stats_merge_theap_to_heap|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/stats.rs|mi_heap_stats_get_json|mi_heap_t|",
        "metadata-reference|src/allocator/stats.rs|__mi_stat_increase_mt|mi_heap_t|",
        "bare-domain-usize|src/allocator/stats.rs|mi_stats_get_bin_size_from_raw|usize|",
        "raw-pointer-parameter|src/allocator/thread_heap.rs|_mi_heap_area_init|mi_page_t|",
        "raw-pointer-parameter|src/allocator/thread_heap.rs|_mi_theap_create|mi_heap_t|",
        "raw-pointer-parameter|src/allocator/thread_heap.rs|mi_theap_collect_ex|mi_theap_t|",
        "pointer-usize-roundtrip|src/allocator/thread_heap.rs|mi_theap_area_visit_blocks_typed|area-visitor|",
        "raw-pointer-parameter|src/allocator/alloc.rs|mi_theap_malloc|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/alloc.rs|mi_heap_malloc|mi_heap_t|",
        "pointer-usize-roundtrip|src/allocator/alloc.rs|_mi_page_malloc_zero_typed|padding/provenance|",
        "bare-domain-usize|src/allocator/alloc.rs|malloc_layout_or_report|usize|",
        "raw-pointer-parameter|src/allocator/free.rs|_mi_free_generic|mi_page_t|",
        "metadata-reference|src/allocator/free.rs|mi_page_decode_padding|mi_page_t|",
        "pointer-usize-roundtrip|src/allocator/free.rs|mi_validate_pointer|validation|",
        "raw-pointer-parameter|src/allocator/init.rs|_mi_thread_done|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/init.rs|_mi_subproc_to_id|mi_subproc_t|",
        "raw-pointer-parameter|src/allocator/init.rs|_mi_is_heap_main|mi_heap_t|",
        "raw-pointer-parameter|src/allocator/page_map.rs|_mi_page_map_register|mi_page_t|",
        "raw-pointer-parameter|src/allocator/page_map.rs|_mi_page_map_unsafe_destroy|mi_subproc_t|",
        "raw-pointer-parameter|src/internal/bitmap.rs|_mi_bitmap_forall_set|mi_arena_t|",
        "metadata-reference|src/internal/bitmap.rs|mi_bitmap_setN|bitmap|",
        "bare-domain-usize|src/internal/bitmap.rs|mi_bitmap_setN|usize|",
        "raw-pointer-parameter|src/types/ptr.rs|new|mi_page_t|",
        "bare-domain-usize|src/types/ptr.rs|checked_byte_add|usize|",
        "metadata-reference|src/allocator/options.rs|with_options_table|options|",
        "bare-domain-usize|src/allocator/options.rs|option_value_load|usize|",
        "bare-domain-usize|src/types/arena.rs|new|usize|",
        "pointer-usize-roundtrip|src/types/arena.rs|SliceRange::from_raw|slice-index|",
    ] {
        assert!(
            allowlist_rs.contains(required),
            "strict-type allowlist should retain symbol-level entry {required}"
        );
    }
}
