use super::*;

pub(super) fn assert_raw_facade_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let page_map = sources.page_map.as_str();
    let internal = sources.internal.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let arena_debug_rs = sources.arena_debug_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let arena_ids_rs = sources.arena_ids_rs.as_str();
    let arena_meta_rs = sources.arena_meta_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let libc_rs = sources.libc_rs.as_str();
    let os_rs = sources.os_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let deferred_rs = sources.deferred_rs.as_str();
    let random_rs = sources.random_rs.as_str();
    let raw_macros_rs = sources.raw_macros_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let threadlocal_rs = sources.threadlocal_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let prim_unix_rs = sources.prim_unix_rs.as_str();

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/internal/mod.rs\t"))
            && internal
                .matches("SAFETY[AUDIT:internal-translation-boundary]")
                .count()
                >= 70
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
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/raw/macros.rs\t"))
            && raw_macros_rs
                .matches("SAFETY[AUDIT:raw-macro-allocation-boundary]")
                .count()
                >= 170
            && raw_macros_rs.contains("core::arch::global_asm!(")
            && raw_macros_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && raw_macros_rs.contains("pub unsafe fn mi_unix_malloc(")
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

    for (source_name, source, internal_name) in [
        (
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub unsafe extern \"C\" fn _mi_page_malloc_zero",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc_aligned",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_collect",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_contain",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_free",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_abandon",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_alloc",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_free",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_unabandon",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all",
        ),
        (
            "src/allocator/arena/heap_pages.rs",
            arena_heap_pages_rs,
            "pub unsafe extern \"C\" fn _mi_heap_visit_blocks",
        ),
        (
            "src/allocator/arena/heap_pages.rs",
            arena_heap_pages_rs,
            "pub unsafe extern \"C\" fn _mi_heap_move_pages",
        ),
        (
            "src/allocator/arena/heap_pages.rs",
            arena_heap_pages_rs,
            "pub unsafe extern \"C\" fn _mi_heap_destroy_pages",
        ),
        (
            "src/allocator/arena/ids.rs",
            arena_ids_rs,
            "pub unsafe extern \"C\" fn _mi_arena_id_none",
        ),
        (
            "src/allocator/arena/ids.rs",
            arena_ids_rs,
            "pub unsafe extern \"C\" fn _mi_arena_from_id",
        ),
        (
            "src/allocator/arena/ids.rs",
            arena_ids_rs,
            "pub unsafe extern \"C\" fn _mi_arena_memid_is_suitable",
        ),
        (
            "src/allocator/arena_meta.rs",
            arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_free",
        ),
        (
            "src/allocator/arena_meta.rs",
            arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_is_meta_page",
        ),
        (
            "src/allocator/arena_meta.rs",
            arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_zalloc",
        ),
        (
            "src/allocator/free.rs",
            free_rs,
            "pub unsafe extern \"C\" fn _mi_page_ptr_unalign",
        ),
        (
            "src/allocator/free.rs",
            free_rs,
            "pub unsafe extern \"C\" fn _mi_padding_shrink",
        ),
        (
            "src/allocator/page.rs",
            page_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_page_abandon",
        ),
        (
            "src/allocator/page.rs",
            page_rs,
            "pub unsafe extern \"C\" fn _mi_page_free",
        ),
        (
            "src/allocator/page.rs",
            page_rs,
            "pub unsafe extern \"C\" fn _mi_page_init",
        ),
        (
            "src/allocator/page/deferred.rs",
            deferred_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_deferred_free",
        ),
        ("src/ffi/libc.rs", libc_rs, "pub(crate) fn _mi_snprintf"),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_vsnprintf",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub extern \"C\" fn _mi_toupper",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnicmp",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_streq",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcpy",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcat",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlen",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnlen",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnstr",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_free",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_mem_init",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_commit",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reuse",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_decommit",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reset",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_protect",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_clock_now",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_process_info",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_random_buf",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_getenv",
        ),
        (
            "src/ffi/libc.rs",
            libc_rs,
            "pub(crate) extern \"C\" fn _mi_popcount_generic",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_alloc",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_zalloc",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free_ex",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_page_size",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_good_alloc_size",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_overcommit",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_virtual_reserve",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_virtual_address_bits",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_reuse",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit_ex",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_purge_ex",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) extern \"C\" fn _mi_os_random_weak",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_split",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_next",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init_weak",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak",
        ),
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
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_init",
        ),
        (
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_set",
        ),
        (
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_get",
        ),
        (
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_thread_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_free",
        ),
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
            "src/allocator/alloc.rs",
            alloc_rs,
            "pub(crate) unsafe extern \"C\" fn mi_theap_alloc_new",
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
        ("src/platform/mod.rs", prim_rs, "fn _mi_allocator_init"),
        ("src/platform/mod.rs", prim_rs, "fn _mi_allocator_done"),
        (
            "src/allocator/options.rs",
            options_rs,
            "fn _mi_warning_message",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "fn _mi_error_message",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "fn _mi_option_get_fast",
        ),
        ("src/allocator/options.rs", options_rs, "fn _mi_assert_fail"),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub unsafe extern \"C\" fn _mi_stats_merge_into",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub extern \"C\" fn _mi_stats_init",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "pub(crate) fn _mi_raw_message",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "pub(crate) fn _mi_message",
        ),
    ] {
        let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
        assert!(
            !source.contains(&no_mangle_export),
            "{internal_name} in {source_name} should stay crate-internal instead of a dynamic export"
        );
        assert_not_no_mangle_item(source_name, source, internal_name);
    }

    assert!(
        !internal.contains("pub static mut _mi_page_map:")
            && !internal.contains("pub fn _mi_page_map_init() -> bool;")
            && !internal.contains("pub fn _mi_arenas_alloc(")
            && !internal.contains("pub fn _mi_meta_zalloc(")
            && !internal.contains("pub fn _mi_page_free(")
            && !internal.contains("pub fn _mi_page_malloc_zero(")
            && !internal.contains("pub fn _mi_vsnprintf(")
            && !internal.contains("pub fn _mi_strlcpy(")
            && !internal.contains("pub fn _mi_os_random_weak(")
            && !internal.contains("pub fn _mi_random_init(")
            && !internal.contains("pub fn _mi_random_next(")
            && !internal.contains("pub fn _mi_theap_random_next(")
            && !internal.contains("pub fn _mi_os_alloc(")
            && !internal.contains("pub fn _mi_os_page_size(")
            && !internal.contains("pub fn _mi_os_commit(")
            && !internal.contains("pub fn _mi_os_purge_ex(")
            && !internal.contains("pub fn _mi_thread_done(")
            && !internal.contains("pub fn _mi_thread_id(")
            && !internal.contains("pub fn _mi_theap_guarded_init(")
            && !internal.contains("pub fn _mi_theap_options_init(")
            && !internal.contains("pub fn _mi_theap_default_set(")
            && !internal.contains("pub fn _mi_theap_cached_set(")
            && !internal.contains("pub fn _mi_theap_default_safe(")
            && !internal.contains("pub fn _mi_theap_main_safe(")
            && !internal.contains("pub fn _mi_theap_collect_retired(")
            && !internal.contains("pub fn _mi_theap_collect_abandon(")
            && !internal.contains("pub fn _mi_theap_init(")
            && !internal.contains("pub fn _mi_theap_free(")
            && !internal.contains("pub fn _mi_theap_incref(")
            && !internal.contains("pub fn _mi_theap_decref(")
            && !internal.contains("pub fn _mi_theap_area_visit_blocks(")
            && !internal.contains("pub fn _mi_theap_malloc_zero(")
            && !internal.contains("pub fn _mi_theap_malloc_zero_ex(")
            && !internal.contains("pub fn _mi_theap_realloc_zero(")
            && !internal.contains("pub fn _mi_theap_create(")
            && !internal.contains("pub fn _mi_heap_move_pages(")
            && !internal.contains("pub fn _mi_heap_destroy_pages(")
            && !internal.contains("pub fn _mi_heap_force_destroy(")
            && !internal.contains("pub fn _mi_padding_shrink(")
            && !internal.contains("pub fn _mi_is_heap_main(")
            && !internal.contains("pub fn _mi_is_theap_main(")
            && !internal.contains("pub fn _mi_warning_message(")
            && !internal.contains("pub fn _mi_verbose_message(")
            && !internal.contains("pub fn _mi_trace_message(")
            && !internal.contains("pub fn _mi_option_get_fast(")
            && !internal.contains("pub fn _mi_error_message(")
            && !internal.contains("pub fn _mi_auto_process_init(")
            && !internal.contains("pub fn _mi_auto_process_done(")
            && !internal.contains("pub fn _mi_allocator_init(")
            && !internal.contains("pub fn _mi_allocator_done(")
            && !internal.contains("pub fn _mi_preloading(")
            && !internal.contains("pub fn _mi_subproc(")
            && !internal.contains("pub fn _mi_subproc_main(")
            && !internal.contains("pub fn _mi_subproc_heap_main(")
            && !internal.contains("pub fn _mi_subproc_from_id(")
            && !internal.contains("pub fn _mi_deferred_free(")
            && !internal.contains("pub fn _mi_arena_id_none(")
            && !internal.contains("pub fn _mi_arena_from_id(")
            && !internal.contains("pub fn _mi_arena_memid_is_suitable(")
            && !internal.contains("pub static _mi_page_empty:")
            && !internal.contains("pub static _mi_theap_empty:")
            && !internal.contains("pub static _mi_theap_empty_wrong:")
            && internal.contains("pub(crate) use crate::page_map::{")
            && internal.contains("pub(crate) use crate::os::{"),
        "page-map internals should be imported through crate paths, not extern declarations"
    );
}
