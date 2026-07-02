use super::*;
#[path = "unsafe_performance/bitmap_boundaries.rs"]
mod bitmap_boundaries;
#[path = "unsafe_performance/lifecycle_boundaries.rs"]
mod lifecycle_boundaries;
#[path = "unsafe_performance/local_boundaries.rs"]
mod local_boundaries;
#[path = "unsafe_performance/size_class_boundaries.rs"]
mod size_class_boundaries;
#[path = "unsafe_performance/tail_guards.rs"]
mod tail_guards;
#[path = "unsafe_performance/threadlocal_boundaries.rs"]
mod threadlocal_boundaries;
#[test]
fn free_path_internals_are_not_dynamic_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let page_map = fs::read_to_string(root.join("src/allocator/page_map.rs"))
        .expect("read src/allocator/page_map.rs");
    let internal =
        fs::read_to_string(root.join("src/internal/mod.rs")).expect("read src/internal/mod.rs");
    let internal_memid =
        fs::read_to_string(root.join("src/internal/memid.rs")).expect("read src/internal/memid.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");
    let arena_debug_rs = fs::read_to_string(root.join("src/allocator/arena/debug.rs"))
        .expect("read src/allocator/arena/debug.rs");
    let arena_ids_rs = fs::read_to_string(root.join("src/allocator/arena/ids.rs"))
        .expect("read src/allocator/arena/ids.rs");
    let arena_prefix_rs = fs::read_to_string(root.join("src/allocator/arena/prefix.rs"))
        .expect("read src/allocator/arena/prefix.rs");
    let arena_meta_rs = fs::read_to_string(root.join("src/allocator/arena_meta.rs"))
        .expect("read src/allocator/arena_meta.rs");
    let atomic_rs = fs::read_to_string(root.join("src/internal/atomic.rs"))
        .expect("read src/internal/atomic.rs");
    let bitmap_rs = fs::read_to_string(root.join("src/internal/bitmap.rs"))
        .expect("read src/internal/bitmap.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let libc_rs = fs::read_to_string(root.join("src/ffi/libc.rs")).expect("read src/ffi/libc.rs");
    let memid_rs =
        fs::read_to_string(root.join("src/types/memid.rs")).expect("read core_types/memid.rs");
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let freelist_types = fs::read_to_string(root.join("src/types/freelist_wrappers.rs"))
        .expect("read core_types/freelist_wrappers.rs");
    let arena_types =
        fs::read_to_string(root.join("src/types/arena.rs")).expect("read core_types/arena.rs");
    let page_types =
        fs::read_to_string(root.join("src/types/page.rs")).expect("read core_types/page.rs");
    let scalar_types =
        fs::read_to_string(root.join("src/types/scalar.rs")).expect("read core_types/scalar.rs");
    let atomic_types =
        fs::read_to_string(root.join("src/types/atomic.rs")).expect("read core_types/atomic.rs");
    let ptr_rs = fs::read_to_string(root.join("src/types/ptr.rs")).expect("read core_types/ptr.rs");
    let validation_types = fs::read_to_string(root.join("src/types/validation.rs"))
        .expect("read core_types/validation.rs");
    let page_states_rs = fs::read_to_string(root.join("src/types/page_states.rs"))
        .expect("read core_types/page_states.rs");
    let page_queues_rs = fs::read_to_string(root.join("src/types/page_queues.rs"))
        .expect("read core_types/page_queues.rs");
    let os_rs =
        fs::read_to_string(root.join("src/platform/os.rs")).expect("read src/platform/os.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let page_bins_rs = fs::read_to_string(root.join("src/allocator/page/bins.rs"))
        .expect("read src/allocator/page/bins.rs");
    let page_counts_rs = fs::read_to_string(root.join("src/allocator/page/counts.rs"))
        .expect("read src/allocator/page/counts.rs");
    let deferred_rs = fs::read_to_string(root.join("src/allocator/page/deferred.rs"))
        .expect("read src/allocator/page/deferred.rs");
    let page_flags_rs = fs::read_to_string(root.join("src/allocator/page/flags.rs"))
        .expect("read src/allocator/page/flags.rs");
    let page_validation_rs = fs::read_to_string(root.join("src/allocator/page/validation.rs"))
        .expect("read src/allocator/page/validation.rs");
    let random_rs = fs::read_to_string(root.join("src/internal/random.rs"))
        .expect("read src/internal/random.rs");
    let stats_rs = fs::read_to_string(root.join("src/allocator/stats.rs"))
        .expect("read src/allocator/stats.rs");
    let theap_rs = fs::read_to_string(root.join("src/allocator/thread_heap.rs"))
        .expect("read src/allocator/thread_heap.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");
    let alloc_aligned_rs = fs::read_to_string(root.join("src/allocator/aligned.rs"))
        .expect("read src/allocator/aligned.rs");
    let heap_rs =
        fs::read_to_string(root.join("src/allocator/heap.rs")).expect("read src/allocator/heap.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let prim_unix_rs =
        fs::read_to_string(root.join("src/platform/unix.rs")).expect("read src/platform/unix.rs");

    assert!(
        !free_rs.contains("clippy::if_same_then_else")
            && !bitmap_rs.contains("clippy::if_same_then_else"),
        "free.rs and bitmap.rs should not retain obsolete if_same_then_else allowances"
    );

    local_boundaries::assert_page_map_and_deferred_boundaries(local_boundaries::BoundarySources {
        page_map: &page_map,
        alloc_rs: &alloc_rs,
        arena_rs: &arena_rs,
        arena_debug_rs: &arena_debug_rs,
        arena_meta_rs: &arena_meta_rs,
        deferred_rs: &deferred_rs,
        free_rs: &free_rs,
        heap_rs: &heap_rs,
        init_rs: &init_rs,
        internal: &internal,
        page_rs: &page_rs,
        theap_rs: &theap_rs,
    });

    for (source_name, source, internal_name) in [
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_page_malloc_zero",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc_aligned",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_collect",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_contain",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_free",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_abandon",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_alloc",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_free",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_unabandon",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_visit_blocks",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_move_pages",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_destroy_pages",
        ),
        (
            "src/allocator/arena/ids.rs",
            &arena_ids_rs,
            "pub unsafe extern \"C\" fn _mi_arena_id_none",
        ),
        (
            "src/allocator/arena/ids.rs",
            &arena_ids_rs,
            "pub unsafe extern \"C\" fn _mi_arena_from_id",
        ),
        (
            "src/allocator/arena/ids.rs",
            &arena_ids_rs,
            "pub unsafe extern \"C\" fn _mi_arena_memid_is_suitable",
        ),
        (
            "src/allocator/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_free",
        ),
        (
            "src/allocator/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_is_meta_page",
        ),
        (
            "src/allocator/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_zalloc",
        ),
        (
            "src/allocator/free.rs",
            &free_rs,
            "pub unsafe extern \"C\" fn _mi_page_ptr_unalign",
        ),
        (
            "src/allocator/free.rs",
            &free_rs,
            "pub unsafe extern \"C\" fn _mi_padding_shrink",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_page_abandon",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_free",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_init",
        ),
        (
            "src/allocator/page/deferred.rs",
            &deferred_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_deferred_free",
        ),
        ("src/ffi/libc.rs", &libc_rs, "pub(crate) fn _mi_snprintf"),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_vsnprintf",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub extern \"C\" fn _mi_toupper",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnicmp",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_streq",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcpy",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcat",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlen",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnlen",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnstr",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_free",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_mem_init",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_commit",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reuse",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_decommit",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reset",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_protect",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_clock_now",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_process_info",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_random_buf",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_getenv",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) extern \"C\" fn _mi_popcount_generic",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_alloc",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_zalloc",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free_ex",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_page_size",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_good_alloc_size",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_overcommit",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_virtual_reserve",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_virtual_address_bits",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_reuse",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit_ex",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_purge_ex",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) extern \"C\" fn _mi_os_random_weak",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_split",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_next",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init_weak",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero_ex",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_realloc_zero",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_guarded",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub extern \"C\" fn _mi_thread_id",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_thread_done",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_guarded_init",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_options_init",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_set",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_cached_set",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_safe",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_main_safe",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_empty_get",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_is_heap_main",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_is_theap_main",
        ),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc_to_id"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_preloading"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc_main"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc_from_id"),
        (
            "src/allocator/init.rs",
            &init_rs,
            "fn _mi_subproc_heap_main",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "fn _mi_auto_process_init",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "fn _mi_auto_process_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_init",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_set",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_get",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_thread_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_free",
        ),
        (
            "src/allocator/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_force_destroy",
        ),
        (
            "src/allocator/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_peek",
        ),
        (
            "src/allocator/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_or_init",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_retired",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub(crate) unsafe extern \"C\" fn mi_theap_alloc_new",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_abandon",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_theap_init",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_create",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_incref",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_decref",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_free",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_delete",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_random_next",
        ),
        ("src/platform/mod.rs", &prim_rs, "fn _mi_allocator_init"),
        ("src/platform/mod.rs", &prim_rs, "fn _mi_allocator_done"),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_warning_message",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_error_message",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_option_get_fast",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_assert_fail",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn _mi_stats_merge_into",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub extern \"C\" fn _mi_stats_init",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "pub(crate) fn _mi_raw_message",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
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
    assert!(
        !alloc_aligned_rs.contains("fn _mi_theap_malloc_guarded(")
            && !alloc_aligned_rs.contains("fn _mi_theap_malloc_zero(")
            && !alloc_aligned_rs.contains("fn _mi_theap_malloc_zero_ex(")
            && !alloc_aligned_rs.contains("fn _mi_theap_realloc_zero(")
            && !alloc_aligned_rs.contains("fn _mi_padding_shrink("),
        "aligned allocation internals should call crate-local theap allocation helpers directly"
    );
    assert!(
        free_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && free_rs.contains("\".hidden _mi_padding_shrink\"")
            && free_rs.contains("\"_mi_padding_shrink:\"")
            && free_rs.contains("\"ret\"")
            && free_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_padding_shrink(")
            && free_rs.contains("if MI_PADDING == 0")
            && free_rs.contains("mi_page_decode_padding(page, block, &mut delta, &mut bsize)")
            && free_rs.contains(
                ".write_delta(\n            crate::core_types::PaddingDelta::for_usable("
            )
            && free_rs.contains("crate::core_types::PaddingDelta::new_unchecked(new_delta as u32)")
            && alloc_aligned_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && alloc_aligned_rs.contains("\"call _mi_padding_shrink\"")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call(")
            && alloc_aligned_rs.contains("unsafe { _mi_padding_shrink(page, block, min_size) };")
            && alloc_aligned_rs.contains(
                "unsafe { mi_padding_shrink_call(page, p.cast::<mi_block_t>(), adjust + size) };"
            ),
        "aligned allocation should keep C's _mi_padding_shrink helper boundary"
    );
    size_class_boundaries::assert_size_class_boundaries(size_class_boundaries::SizeClassSources {
        alloc_aligned_rs: &alloc_aligned_rs,
        alloc_rs: &alloc_rs,
        arena_rs: &arena_rs,
        free_rs: &free_rs,
        internal: &internal,
        page_bins_rs: &page_bins_rs,
        page_rs: &page_rs,
        ptr_rs: &ptr_rs,
        stats_rs: &stats_rs,
    });
    assert!(
        options_rs.contains(
            "#[inline(never)]\npub(crate) extern \"C\" fn _mi_option_get_fast(option: mi_option_t) -> c_long"
        ) && options_rs.contains("mi_option_get_fast_impl(option)")
            && options_rs.contains("option_value_load(idx)")
            && free_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_reclaim_on_free)")
            && free_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_cross_thread_max_reclaim)")
            && page_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_max_candidates)"),
        "fast option reads should keep C's local _mi_option_get_fast helper boundary on free/page policy paths"
    );
    assert!(
        heap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t"
        ) && heap_rs.contains("if heap.is_null() || _mi_is_heap_main(heap)")
            && heap_rs.contains("crate::init::_mi_theap_main_safe()")
            && heap_rs.contains("_mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()")
            && heap_rs.contains("let mut theap = unsafe { _mi_heap_theap_get_peek(heap) };")
            && prim_rs.contains("_mi_heap_theap_get_peek(heap)"),
        "heap/theap cache misses should keep C's local _mi_heap_theap_get_peek no-create helper boundary"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_is_main_thread() -> bool")
            && init_rs.contains("#[inline(always)]\nfn mi_is_main_thread_fast() -> bool")
            && init_rs.contains("let main_tid = unsafe { tld_main.thread_id };")
            && init_rs.contains("main_tid == 0 || main_tid == current_thread_id()")
            && init_rs.contains("pub extern \"C\" fn _mi_is_main_thread() -> bool {\n    mi_is_main_thread_fast()\n}")
            && !init_rs.contains("static MAIN_THREAD_ID")
            && !init_rs.contains("mi_mark_main_thread")
            && init_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_preloading() -> bool")
            && init_rs.contains("OS_PRELOADING.load(Ordering::Relaxed)")
            && prim_rs.contains("#[cfg(mi_c_malloc_fast)]\ncore::arch::global_asm!(")
            && prim_rs.contains("\".hidden _mi_is_redirected\"")
            && prim_rs.contains("\"_mi_is_redirected:\"")
            && prim_rs.contains("\"xor eax,eax\"")
            && prim_rs.contains("pub(crate) fn _mi_is_redirected() -> bool;")
            && prim_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]\npub unsafe extern \"C\" fn _mi_is_redirected() -> bool")
            && prim_rs.contains("false")
            && init_rs.contains("pub extern \"C\" fn mi_is_redirected() -> bool {\n    unsafe { crate::prim::_mi_is_redirected() }\n}"),
        "lifecycle predicates should keep C's _mi_is_main_thread/_mi_preloading/_mi_is_redirected helper boundaries"
    );
    assert!(
        prim_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_allocator_init(message: *mut *const c_char) -> bool"
        ) && prim_rs.contains("unsafe { *message = ptr::null() };")
            && prim_rs.contains("\".hidden _mi_allocator_done\"")
            && prim_rs.contains("\"_mi_allocator_done:\"")
            && prim_rs.contains("pub(crate) fn _mi_allocator_done();")
            && prim_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_allocator_done()")
            && options_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_options_init()")
            && options_rs.contains("for i in 0..MI_OPTION_COUNT")
            && options_rs.contains("mi_max_error_count.store(max_errors as isize, Ordering::Release);")
            && options_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_options_post_init()")
            && options_rs.contains("mi_add_stderr_output();")
            && options_rs.contains("mi_options_print();")
            && init_rs.contains("_mi_options_post_init();")
            && init_rs.contains("if unsafe { crate::prim::_mi_is_redirected() }")
            && init_rs.contains("_mi_verbose_message(format_args!(\"malloc is redirected.\\n\"));")
            && init_rs.contains("let mut msg: *const c_char = ptr::null();")
            && init_rs.contains("unsafe { _mi_allocator_init(ptr::addr_of_mut!(msg)) };")
            && init_rs.contains("mi_option_is_enabled(mi_option_t::mi_option_show_errors)")
            && init_rs.contains("unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };")
            && init_rs.contains("_mi_allocator_done();"),
        "process init/done should keep C's allocator/options helper boundaries and redirector hook sequence"
    );
    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_fputs(")
            && options_rs.contains("#[link_name = \"stdout\"]")
            && options_rs.contains("#[link_name = \"stderr\"]")
            && options_rs.contains("fn mi_output_is_c_file(out: mi_output_fun) -> bool")
            && options_rs.contains("p == unsafe { C_STDOUT } || p == unsafe { C_STDERR }")
            && options_rs.contains("let use_default = match out {")
            && options_rs.contains("None => true,")
            && options_rs.contains("Some(out) => mi_output_is_c_file(out),")
            && options_rs.contains(
                "fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback"
            )
            && options_rs.contains("let default_out = mi_out_get_default(Some(&mut default_arg));")
            && options_rs.contains("unsafe { default_out.invoke(message, default_arg) };")
            && options_rs.contains("let out = OutputCallback::new(Some(out));")
            && options_rs.contains("let arg = OutputCallbackArg::from_raw(arg);")
            && options_rs.contains("unsafe { out.invoke(message, arg) };")
            && stats_rs.contains("_mi_fputs(buf.out, buf.arg, ptr::null(), buf.buf);")
            && init_rs.contains("unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };"),
        "_mi_fputs should keep C's local helper boundary and stdout/stderr default-output compatibility"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_getenv(")
            && libc_rs.contains("if name.is_null() || result.is_null() || result_size < 64")
            && libc_rs.contains("crate::prim::_mi_prim_getenv(name, result, result_size)")
            && prim_unix_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_getenv(")
            && prim_unix_rs.contains("for i in 0..10000usize")
            && prim_unix_rs.contains("unsafe { _mi_strnicmp(name, s, len) == 0")
            && prim_unix_rs.contains("if _mi_preloading() {\n        return false;")
            && prim_unix_rs.contains("s = unsafe { getenv(buf.as_ptr()) };")
            && options_rs.contains("crate::libc::_mi_getenv("),
        "_mi_getenv and _mi_prim_getenv should keep C's environment helper boundaries and option parser dependency"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_popcount_generic(")
            && libc_rs.contains("if x <= 1 {\n        return x;")
            && libc_rs.contains("if !x == 0 {\n        return MI_SIZE_BITS;")
            && libc_rs.contains("mi_popcount_generic64(x as u64)")
            && libc_rs.contains(
                "static MI_POPCOUNT_GENERIC_KEEP: extern \"C\" fn(usize) -> usize = _mi_popcount_generic;"
            ),
        "_mi_popcount_generic should keep C's local noinline generic popcount boundary"
    );
    for marker in [
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_mem_init(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_alloc(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_commit(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_reset(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_protect(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_reuse(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_clock_now(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_process_info(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_random_buf(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield(",
        "static MI_PRIM_MEM_INIT_KEEP",
        "static MI_PRIM_ALLOC_KEEP",
        "static MI_PRIM_COMMIT_KEEP",
        "static MI_PRIM_RESET_KEEP",
        "static MI_PRIM_PROTECT_KEEP",
        "static MI_PRIM_REUSE_KEEP",
        "static MI_PRIM_ALLOC_HUGE_OS_PAGES_KEEP",
        "static MI_PRIM_NUMA_NODE_COUNT_KEEP",
        "static MI_PRIM_CLOCK_NOW_KEEP",
        "static MI_PRIM_PROCESS_INFO_KEEP",
        "static MI_PRIM_OUT_STDERR_KEEP",
        "static MI_PRIM_RANDOM_BUF_KEEP",
        "static MI_PRIM_THREAD_INIT_AUTO_DONE_KEEP",
        "static MI_PRIM_THREAD_DONE_AUTO_DONE_KEEP",
        "static MI_PRIM_THREAD_ASSOCIATE_DEFAULT_THEAP_KEEP",
        "static MI_PRIM_THREAD_IS_IN_THREADPOOL_KEEP",
        "static MI_PRIM_THREAD_YIELD_KEEP",
    ] {
        assert!(
            prim_unix_rs.contains(marker),
            "Unix primitive helper boundary marker missing: {marker}"
        );
    }
    for marker in [
        "#[inline(never)]\npub(crate) extern \"C\" fn _mi_os_random_weak(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_split(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_next(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_init(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_init_weak(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak(",
    ] {
        assert!(
            random_rs.contains(marker),
            "random helper boundary marker missing: {marker}"
        );
    }
    assert!(
        random_rs.contains("SAFETY[AUDIT:random-context-boundaries]"),
        "random context helper unsafe operations should retain operation-level audit markers"
    );
    assert!(
        !random_rs.contains("static MI_OS_RANDOM_WEAK_KEEP")
            && !random_rs.contains("static MI_RANDOM_SPLIT_KEEP")
            && !random_rs.contains("static MI_RANDOM_NEXT_KEEP")
            && !random_rs.contains("static MI_RANDOM_INIT_KEEP")
            && !random_rs.contains("static MI_RANDOM_INIT_WEAK_KEEP")
            && !random_rs.contains("static MI_RANDOM_REINIT_IF_WEAK_KEEP"),
        "random helper retention function pointers should stay absent; they force indirect calls in the release cdylib"
    );
    for marker in [
        "#[inline(never)]\npub(crate) extern \"C\" fn _mi_toupper(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\npub(crate) unsafe extern \"C\" fn _mi_strnicmp(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\npub(crate) unsafe extern \"C\" fn _mi_streq(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlcpy(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlcat(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnlen(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlen(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnstr(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_vsnprintf(",
        "#[inline(never)]\npub(crate) fn _mi_snprintf(",
        "static MI_TOUPPER_KEEP",
        "static MI_STRNICMP_KEEP",
        "static MI_STREQ_KEEP",
        "static MI_STRLCPY_KEEP",
        "static MI_STRLCAT_KEEP",
        "static MI_STRNLEN_KEEP",
        "static MI_STRLEN_KEEP",
        "static MI_STRNSTR_KEEP",
        "static MI_VSNPRINTF_KEEP",
        "static MI_SNPRINTF_KEEP",
    ] {
        assert!(
            libc_rs.contains(marker),
            "libc helper boundary marker missing: {marker}"
        );
    }
    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) fn _mi_raw_message(")
            && options_rs.contains("static MI_RAW_MESSAGE_KEEP"),
        "_mi_raw_message should keep C's local raw-message helper boundary"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_stats_init(")
            && stats_rs.contains("static MI_STATS_INIT_KEEP: extern \"C\" fn() = _mi_stats_init;"),
        "_mi_stats_init should keep C's local process statistics initialization boundary"
    );
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
        stats_rs.contains(
            "#[inline(never)]\nunsafe extern \"C\" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t)"
        ) && stats_rs.contains(
            "if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src)"
        ) && stats_rs.contains(
            "unsafe { mi_stats_add_nonnull(stats, src) };"
        ) && stats_rs.contains(
            "#[inline(never)]\nunsafe fn mi_stats_add_nonnull(stats: *mut mi_stats_t, src: *const mi_stats_t)"
        ) && stats_rs.contains(
            "mi_stat_count_add_mt(\n                    ptr::addr_of_mut!((*stats).$field),\n                    ptr::addr_of!((*src).$field),"
        ) && stats_rs.contains(
            "mi_stat_counter_add_mt(\n                    ptr::addr_of_mut!((*stats).$field),\n                    ptr::addr_of!((*src).$field),"
        ) && stats_rs.contains(
            "if MI_STAT > 1 {\n        let stats_malloc_bins ="
        )
            && stats_rs.contains("for i in 0..=MI_BIN_HUGE")
            && stats_rs.contains("let stats_page_bins =")
            && !stats_rs.contains("&mut stats.$field")
            && !stats_rs.contains("&src.$field")
            && !stats_rs.contains("iter_mut().zip(src.malloc_bins.iter())")
            && !stats_rs.contains("iter_mut().zip(src.page_bins.iter())")
            && !stats_rs.contains("._stat_reserved\n        .iter_mut()\n        .zip(src._stat_reserved.iter())")
            && !stats_rs.contains("._stat_counter_reserved\n        .iter_mut()\n        .zip(src._stat_counter_reserved.iter())")
            && !stats_rs.contains("stats.chunk_bins.iter_mut().zip(src.chunk_bins.iter())")
            && !stats_rs.contains("static MI_STATS_ADD_KEEP"),
        "mi_stats_add should keep C's raw-pointer aggregate helper boundary without a data-relocation keepalive, route checked callers to the non-null body, release bin-stat gates, and leave reserved extension arrays untouched"
    );
    let stats_add_nonnull_start = stats_rs
        .find("#[inline(never)]\nunsafe fn mi_stats_add_nonnull(")
        .expect("find mi_stats_add_nonnull");
    let stats_add_nonnull_end = stats_rs[stats_add_nonnull_start..]
        .find("\n}\n\nfn mi_stats_add_into")
        .map(|offset| stats_add_nonnull_start + offset)
        .expect("find end of mi_stats_add_nonnull");
    let stats_add_nonnull_body = &stats_rs[stats_add_nonnull_start..stats_add_nonnull_end];
    assert!(
        !stats_add_nonnull_body.contains("stats.is_null()")
            && !stats_add_nonnull_body.contains("src.is_null()")
            && !stats_add_nonnull_body.contains("ptr::eq(stats.cast_const(), src)"),
        "mi_stats_add_nonnull should keep the checked merge body free of duplicate entry guards"
    );
    let stats_add_into_start = stats_rs
        .find("fn mi_stats_add_into(to: &mut mi_stats_t, from: &mi_stats_t)")
        .expect("find mi_stats_add_into");
    let stats_add_into_end = stats_rs[stats_add_into_start..]
        .find("\n}\n\nfn mi_stats_copy")
        .map(|offset| stats_add_into_start + offset)
        .expect("find end of mi_stats_add_into");
    let stats_add_into_body = &stats_rs[stats_add_into_start..stats_add_into_end];
    assert!(
        stats_add_into_body
            .contains("unsafe { mi_stats_add(to as *mut mi_stats_t, from as *const mi_stats_t) };")
            && !stats_add_into_body.contains("mi_stats_add_nonnull("),
        "mi_stats_add_into should exercise C's checked raw mi_stats_add compatibility boundary"
    );
    let stats_merge_start = stats_rs
        .find("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into(")
        .expect("find _mi_stats_merge_into");
    let stats_merge_end = stats_rs[stats_merge_start..]
        .find("\n}\n\n#[repr(C)]")
        .map(|offset| stats_merge_start + offset)
        .expect("find end of _mi_stats_merge_into");
    let stats_merge_body = &stats_rs[stats_merge_start..stats_merge_end];
    assert!(
        stats_merge_body.contains("mi_stats_add_nonnull(to, from);")
            && stats_merge_body.contains("_mi_memzero(from.cast(), size_of::<mi_stats_t>());")
            && !stats_merge_body.contains("mi_stats_add_into("),
        "_mi_stats_merge_into should match C's direct checked stats-add body plus memzero sequence"
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
        os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_numa_node_count() -> i32")
            && os_rs.contains(
                "static MI_OS_NUMA_NODE_COUNT_KEEP: unsafe extern \"C\" fn() -> i32 = _mi_os_numa_node_count;"
            )
            && prim_unix_rs.contains("#[inline(never)]\nunsafe fn unix_mmap_prim_aligned(")
            && prim_unix_rs.contains("static UNIX_MMAP_PRIM_ALIGNED_KEEP: unsafe fn("),
        "NUMA count and aligned mmap helpers should keep C's local OS helper boundaries"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\nunsafe fn mi_out_num(")
            && libc_rs.contains("static MI_OUT_NUM_KEEP: unsafe fn(")
            && stats_rs.contains("#[inline(never)]\nfn mi_json_buf_print_value(")
            && stats_rs.contains(
                "static MI_JSON_BUF_PRINT_VALUE_KEEP: fn(&mut mi_json_buf_t, &str, i64) = mi_json_buf_print_value;"
            ),
        "number formatting and JSON scalar output should keep C's local formatting helper boundaries"
    );
    assert!(
        options_rs
            .contains("#[inline(never)]\npub(crate) fn _mi_trace_message(args: fmt::Arguments<'_>)")
            && options_rs.contains("mi_option_get(mi_option_t::mi_option_verbose) <= 1")
            && options_rs.contains(
                "static MI_TRACE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_trace_message;"
            )
            && options_rs.contains(
                "#[inline(never)]\npub(crate) fn _mi_verbose_message(args: fmt::Arguments<'_>)"
            )
            && options_rs.contains("!mi_option_is_enabled(mi_option_t::mi_option_verbose)")
            && options_rs.contains(
                "static MI_VERBOSE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_verbose_message;"
            )
            && !internal.contains("macro_rules! mi_trace_message"),
        "_mi_trace_message and _mi_verbose_message should keep C's local message helper boundaries without stale extern declarations or macro exports"
    );
    assert!(
        options_rs.contains(
            "#[inline(never)]\npub(crate) fn _mi_warning_message_fmt(args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_warning_message(fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_WARNING_MESSAGE_FMT_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_warning_message_fmt;"
        ) && options_rs.contains(
            "static MI_WARNING_MESSAGE_KEEP: unsafe extern \"C\" fn(*const c_char) = _mi_warning_message;"
        ) && options_rs.contains(
            "#[inline(never)]\npub(crate) fn _mi_error_message_fmt(err: Errno, args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_error_message(err: c_int, fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_FMT_KEEP: for<'a> fn(Errno, fmt::Arguments<'a>) = _mi_error_message_fmt;"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_KEEP: unsafe extern \"C\" fn(c_int, *const c_char) = _mi_error_message;"
        ) && !internal.contains("pub fn _mi_warning_message(")
            && !internal.contains("pub fn _mi_error_message("),
        "_mi_warning_message and _mi_error_message should keep C's local message helper boundaries without stale extern declarations"
    );
    assert!(
        os_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_has_overcommit()"
        ) && os_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_has_virtual_reserve()"
        ) && os_rs
            .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_large_page_size()")
            && os_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_minimal_purge_size()"
            )
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_guard_page_size()")
            && os_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_virtual_address_bits()"
            )
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_canuse_large_page(")
            && os_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_good_alloc_size("
            )
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_init()")
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_get_aligned_hint(")
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_size()"
            )
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_set_at("
            )
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_reset_at("
            )
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_commit(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_decommit(")
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_reuse(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_purge(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_protect(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_unprotect(")
            && os_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_free_ex(")
            && os_rs.contains("unsafe fn mi_os_current_subproc() -> Option<BorrowedSubproc>")
            && os_rs.contains(
                "unsafe fn mi_os_subproc_stats_mut(subproc: BorrowedSubproc) -> *mut mi_stats_t"
            )
            && os_rs.contains(
                "unsafe fn mi_os_prim_free(\n    memid: OsMemId,\n    commit_size: CommittedBytes,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains("let subproc = subproc.or_else(|| mi_os_current_subproc());")
            && os_rs.contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && os_rs.contains(
                "let subproc = SubprocPtr::new(subproc).map(|subproc| unsafe { BorrowedSubproc::new(subproc) });"
            )
            && os_rs.contains("let addr = OsMemPtr::new(addr);")
            && os_rs.contains("let size = ReservedBytes::new(size);")
            && os_rs.contains(
                "unsafe { _mi_os_free_ex_typed(addr, size, still_committed, memid, subproc) };"
            )
            && os_rs.contains(
                "unsafe fn _mi_os_free_ex_typed(\n    addr: Option<OsMemPtr>,\n    size: ReservedBytes,\n    still_committed: bool,\n    os: OsMemId,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains("if matches!(&os.kind, OsMemKind::Huge) {")
            && os_rs.contains(
                "unsafe {\n            mi_os_free_huge_os_pages(OsMemPtr::new(base), ReservedBytes::new(csize), subproc)\n        };"
            )
            && os_rs.contains("} else {\n        let committed = if still_committed {")
            && os_rs.contains("unsafe { mi_os_prim_free(memid, committed, subproc) };")
            && os_rs.contains(
                "unsafe fn mi_os_free_huge_os_pages(\n    p: Option<OsMemPtr>,\n    size: ReservedBytes,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains(
                "mi_os_prim_free(memid, CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE), subproc)"
            )
            && os_rs.contains(
                "mi_os_prim_free(memid, CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE), None);"
            )
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_zalloc(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_alloc_aligned(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_numa_node()")
            && os_rs.contains("static MI_OS_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_GUARD_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_ALLOC_KEEP")
            && os_rs.contains("static MI_OS_FREE_KEEP")
            && os_rs.contains("static MI_OS_REUSE_KEEP")
            && os_rs.contains("static MI_OS_PURGE_KEEP")
            && os_rs.contains("static MI_OS_PROTECT_KEEP")
            && os_rs.contains("static MI_OS_UNPROTECT_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_SET_AT_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_RESET_BEFORE_KEEP"),
        "OS helper wrappers should keep C's local helper boundaries and retained wrapper set"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_free(")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains(
                "crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new(active, queue)"
            )
            && page_rs.contains("let _ = _mi_page_free_active_internal(all_free, queued);")
            && page_rs.contains("let _ = queued.remove();")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage"
            )
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_internal<S>")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("let destroyed = _mi_arenas_page_free_typed(page_wrapper, owner);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_free_typed<S>(\n    page: crate::core_types::AllFreePage<S>,\n    owner: crate::core_types::ThreadHeapLocal,"
            )
            && arena_rs
                .contains("unsafe { _mi_arenas_page_free(page.page().as_ptr(), owner.ptr().as_ptr()) };")
            && arena_rs.contains("page.free_to_arena(owner)")
            && theap_rs.contains("_mi_page_free(page, pq);"),
        "page release should keep C's local _mi_page_free helper boundary while using typed all-free page witnesses internally"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_abandon(")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_abandon_internal(\n    page_wrapper: crate::core_types::PageForAbandon,\n    queue: crate::core_types::PageQueuePtr,"
            )
            && page_rs.contains(") -> crate::core_types::PageAbandonOutcome {")
            && page_rs.contains("_mi_page_free_collect(page, false);")
            && page_rs.contains("if mi_page_all_free(page) {")
            && page_rs.contains("let destroyed = match page_wrapper {")
            && page_rs.contains("let Some(all_free) = page.mark_all_free() else {")
            && page_rs.contains("_mi_page_free_full_internal(all_free, queue)")
            && page_rs.contains("_mi_page_free_active_internal(all_free, queued)")
            && page_rs.contains("QueuedPage::<crate::core_types::FullQueue>::new")
            && page_rs.contains("QueuedPage::<crate::core_types::NormalQueue>::new")
            && page_rs.contains("let _ = queued.remove();")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("_mi_arenas_page_abandon(page, theap);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && page_rs.contains("crate::core_types::PageAbandonOutcome::Abandoned")
            && page_rs.contains("crate::core_types::AbandonedPage::new_unchecked(page_ptr)")
            && page_rs.contains("let page_ptr = crate::core_types::PagePtr::new_unchecked(page);")
            && page_rs.contains("crate::core_types::FullPage::new_unchecked(page_ptr).for_abandon()")
            && page_rs.contains("crate::core_types::ActivePage::new_unchecked(page_ptr).for_abandon()")
            && theap_rs.contains("_mi_page_abandon(page, pq);"),
        "page abandon should keep C's local helper boundary while using typed active/full abandon witnesses internally"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect(")
            && page_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect_partly("
            )
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_unfull(")
            && page_rs.contains(
                "#[inline(never)]\npub(crate) unsafe fn _mi_page_unfull_internal(\n    page_wrapper: crate::core_types::FullPage,"
            )
            && page_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_retire(page: *mut mi_page_t)")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_retire_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,"
            )
            && page_rs.contains("crate::core_types::AllFreePage::active_unchecked")
            && page_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_init(")
            && page_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_queue_is_valid(")
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_stats_bin(")
            && page_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_queue_append(")
            && page_rs.contains("static MI_PAGE_QUEUE_IS_VALID_KEEP")
            && page_rs.contains("static MI_PAGE_QUEUE_APPEND_KEEP")
            && page_rs.contains("mi_page_thread_free_collect(page);")
            && page_rs.contains("let atomic_tf = crate::core_types::AtomicThreadFree(")
            && page_rs.contains("crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*page).xthread_free))")
            && page_rs.contains("let remote = atomic_tf.collect_acq_rel();")
            && page_rs.contains("if remote.is_empty() {")
            && page_rs.contains("atomic_tf.load_relaxed().head_raw() == head")
            && alloc_rs.contains("let block = allocated.block().as_ptr();")
            && !alloc_rs.contains("allocated.0.as_ptr()")
            && free_rs.contains("block_wrapper.into_free_block()")
            && !free_rs.contains("crate::core_types::FreeBlock::new_unchecked")
            && !free_rs.contains("crate::core_types::FreeBlock(")
            && freelist_types.contains("AllocatedBlock::new_unchecked(head)")
            && freelist_types.contains("pub(crate) unsafe fn pop(page: AvailablePage)")
            && freelist_types.contains("let block_raw = block.block().as_ptr();")
            && !freelist_types.contains("AllocatedBlock(head)")
            && !freelist_types.contains("block.0.as_ptr()")
            && page_rs.contains("let thread_free = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ))")
            && page_rs.contains("crate::core_types::ThreadFreeWord::from_parts(\n                    None,\n                    crate::core_types::ThreadFreeOwner::Owned")
            && !page_rs.contains("crate::internal::mi_tf_create(ptr::null_mut()")
            && !page_rs.contains("crate::internal::mi_tf_block(mi_atomic_load_explicit")
            && !page_rs.contains("(*page).xthread_free == 1")
            && freelist_types.contains("pub(crate) struct AlignedBlockTag(BlockTag);")
            && freelist_types.contains("pub(crate) struct GuardedBlockTag(BlockTag);")
            && freelist_types.contains("pub(crate) unsafe fn write_aligned_to(block: BlockPtr)")
            && freelist_types.contains("pub(crate) unsafe fn write_guarded_to(block: BlockPtr)")
            && freelist_types.contains("pub(crate) unsafe fn read_guarded_from_raw(")
            && freelist_types.contains(") -> Option<GuardedBlockTag>")
            && freelist_types.contains("pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_types.contains("Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })")
            && freelist_types.contains("pub(crate) unsafe fn write_to(self, block: BlockPtr)")
            && atomic_types.contains("pub(crate) struct RemoteFreeList(Option<BlockPtr>);")
            && atomic_types.contains("fn from_detached_thread_free_head(head: Option<BlockPtr>) -> Self")
            && atomic_types.contains("pub(crate) fn head(self) -> Option<BlockPtr>")
            && atomic_types.contains("pub(crate) fn is_empty(self) -> bool")
            && atomic_types.contains("pub(crate) unsafe fn collect_into_local(")
            && !freelist_types.contains("pub(crate) struct RemoteFreeList(pub(crate) Option<BlockPtr>)")
            && alloc_rs.contains("crate::core_types::BlockTag::write_aligned_to(")
            && alloc_rs.contains("crate::core_types::BlockTag::write_guarded_to(")
            && alloc_aligned_rs.contains("crate::core_types::BlockTag::write_aligned_to(block)")
            && internal.contains("crate::core_types::BlockLink::is_guarded_raw(block)")
            && free_rs.contains("crate::core_types::BlockLink::is_guarded_raw(block)")
            && !alloc_rs.contains("crate::core_types::BlockLink::tagged(")
            && !alloc_aligned_rs.contains("crate::core_types::BlockLink::tagged(")
            && !internal.contains("BlockLink::read_from_raw(block).is_guarded_tag()")
            && !free_rs.contains("BlockLink::read_from_raw(block).is_guarded_tag()")
            && !freelist_types.contains("pub(crate) const fn tagged(tag: BlockTag) -> Self")
            && !freelist_types.contains("pub(crate) const ALIGNED: Self")
            && !freelist_types.contains("pub(crate) const GUARDED: Self")
            && !internal.contains("(*block).next")
            && !alloc_rs.contains("(*block).next = 0")
            && !alloc_rs.contains("(*block).next = MI_BLOCK_TAG_GUARDED")
            && !alloc_aligned_rs.contains("(*block).next = MI_BLOCK_TAG_ALIGNED")
            && !free_rs.contains("(*block).next == MI_BLOCK_TAG_GUARDED")
            && !internal.contains("(*block.cast_mut()).next == MI_BLOCK_TAG_GUARDED")
            && page_rs
                .contains("crate::internal::mi_assert_internal!(!force || (*page).local_free.is_null());")
            && page_rs.contains("unsafe fn mi_page_move_to_full_typed(")
            && page_rs.contains("unsafe fn mi_page_unfull_typed(")
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from_raw(\n    to: crate::core_types::PageQueuePtr,\n    from: crate::core_types::PageQueuePtr,"
            )
            && page_rs.contains("unsafe { page.into_full(to) }")
            && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)")
            && page_rs.contains("unsafe { page.unfull(to) }")
            && page_states_rs.contains("pub(crate) unsafe fn to_full(")
            && page_queues_rs.contains("impl QueuedPage<FullQueue>")
            && page_queues_rs
                .contains("pub(crate) unsafe fn unfull(self, to: NormalPageQueue)")
            && !page_states_rs.contains("pub(crate) unsafe fn unfull(")
            && !page_rs.contains("pub unsafe fn mi_page_queue_enqueue_from_ex(")
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from_ex(\n    to: *mut mi_page_queue_t,"
            )
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from(\n    to: *mut mi_page_queue_t,"
            )
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from_full(\n    to: *mut mi_page_queue_t,"
            )
            && page_flags_rs.contains("match crate::core_types::MemId::view((*page).memid)")
            && page_flags_rs.contains("crate::core_types::MemIdView::Os(os) => os")
            && !page_flags_rs.contains("(*page).memid.mem.os.base")
            && page_rs.contains(
                "let _ = _mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked("
            )
            && page_rs.contains("QueuedPage::<crate::core_types::FullQueue>::new")
            && page_rs.contains("mi_page_unfull_typed(queued, to)")
            && page_rs
                .contains("mi_page_move_to_full_typed(\n                crate::core_types::ActivePage")
            && free_rs.contains("let Some(all_free) = active.mark_all_free() else {")
            && free_rs.contains("_mi_page_retire_internal(all_free);")
            && free_rs
                .contains("_mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked")
            && free_rs.contains("_mi_page_free_collect(page_raw, false);")
            && free_rs.contains("_mi_page_free_collect_partly(page_raw, mt_free.as_ptr());")
            && arena_rs.contains("crate::page::_mi_page_init(theap, page)")
            && arena_rs.contains("_mi_page_free_collect(page_raw, false)")
            && arena_rs.contains("_mi_page_stats_bin(page)")
            && theap_rs.contains("_mi_page_free_collect(page, true)")
            && page_rs.contains("mod counts;")
            && page_rs.contains("mod validation;")
            && page_counts_rs.contains("SAFETY[AUDIT:page-count-accessors]")
            && page_validation_rs.contains("SAFETY[AUDIT:page-validation-helpers]")
            && page_validation_rs.contains("pub unsafe fn mi_page_block_at(")
            && page_validation_rs.contains("pub unsafe fn _mi_page_is_valid(")
            && page_validation_rs.contains("pub(crate) unsafe fn mi_page_is_valid_init(")
            && page_validation_rs.contains("mi_page_queue_contains(pq, cpage)")
            && page_counts_rs.contains("pub(crate) unsafe fn mi_page_used_count(")
            && page_counts_rs.contains("pub(crate) unsafe fn mi_page_capacity_count(")
            && page_counts_rs.contains("pub(super) unsafe fn mi_page_queue_block_size_raw(")
            && page_rs.contains("unsafe fn mi_page_queue_is_valid_typed(")
            && page_rs.contains("pub(crate) unsafe fn mi_normal_page_queue_is_valid(")
            && page_rs.contains("pub(crate) unsafe fn mi_huge_page_queue_is_valid(")
            && page_rs.contains("pub(crate) unsafe fn mi_full_page_queue_is_valid(")
            && theap_rs.contains("mi_normal_page_queue_is_valid(")
            && theap_rs.contains("mi_huge_page_queue_is_valid(")
            && theap_rs.contains("mi_full_page_queue_is_valid(")
            && !theap_rs.contains("_mi_page_queue_is_valid("),
        "page lifecycle and page-queue helpers should keep C's local helper boundaries"
    );
    assert!(
        alloc_rs
            .contains("pub(crate) unsafe fn _mi_page_malloc_zero_typed(\n    theap: *mut mi_theap_t,\n    available: crate::core_types::AvailablePage,\n    padded: crate::core_types::PaddedSize,")
            && alloc_rs.contains("zeroed: crate::core_types::Zeroed,")
            && alloc_rs.contains("let page_ptr = available.page();")
            && alloc_rs.contains("let size = padded.get();")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::Zeroed::from_bool(zero)")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(")
            && alloc_rs.contains("let padded = crate::core_types::PaddedSize::new_unchecked(size);")
            && alloc_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_malloc_zero")
            && alloc_rs.contains("let Some(available) = active.available() else {")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(theap, available, padded, zeroed, ptr::null_mut())")
            && page_rs.contains(
                "crate::core_types::Zeroed::from_bool(zero),\n            ptr::null_mut(),"
            )
            && internal.contains("pub(crate) unsafe fn _mi_theap_get_free_small_page_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    padded: crate::core_types::PaddedSize,")
            && internal.contains("let theap = theap.ptr().as_ptr();")
            && internal.contains("let idx = mi_direct_bin_from_padded(padded).as_usize();")
            && alloc_rs.contains("let page = _mi_theap_get_free_small_page_typed(local_theap, padded);")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(theap, available, padded, zeroed, usable)")
            && alloc_aligned_rs.contains(
                "let page =\n            unsafe { crate::internal::_mi_theap_get_free_small_page_typed(local_theap, padded) };"
            )
            && alloc_aligned_rs.contains(
                "let available = unsafe { active.available().unwrap_unchecked() };"
            )
            && alloc_aligned_rs.contains(
                "_mi_page_malloc_zero_typed(\n                        theap,\n                        available,\n                        padded,"
            )
            && alloc_aligned_rs.contains("crate::core_types::Zeroed::from_bool(zero),")
            && alloc_aligned_rs.contains("let padded = layout.padded();")
            && !page_rs.contains("let p = _mi_page_malloc_zero(theap, page, size, zero);")
            && !alloc_aligned_rs
                .contains("let p = _mi_page_malloc_zero(theap, page, padsize, zero);")
            && !alloc_rs.contains("_mi_theap_get_free_small_page(theap, padded.get())")
            && !alloc_aligned_rs.contains("_mi_theap_get_free_small_page(theap, padsize)"),
        "page and aligned allocation should use typed padded-size helpers internally"
    );
    assert!(
        atomic_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_atomic_once_enter")
            && atomic_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_atomic_once_release")
            && atomic_types.contains("pub(crate) struct OnceGuard<'a>")
            && atomic_types
                .contains("pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self>")
            && atomic_types.contains("impl Drop for OnceGuard<'_>")
            && atomic_types.contains("crate::atomic::_mi_atomic_once_release(")
            && atomic_rs.contains("_once: crate::core_types::OnceGuard<'static>")
            && atomic_rs.contains("crate::core_types::OnceGuard::try_enter(state)")
            && !atomic_rs.contains("once: *mut mi_atomic_once_t,\n}")
            && !atomic_rs.contains("_mi_atomic_once_release(self.once);"),
        "atomic once should keep C's enter/release helper boundaries and release through typed OnceGuard Drop"
    );
    bitmap_boundaries::assert_bitmap_traversal_and_regular_boundaries(
        &bitmap_rs,
        &arena_rs,
        &callbacks_rs,
        &threadlocal_rs,
    );
    bitmap_boundaries::assert_bitmap_helper_boundaries(&bitmap_rs);
    for marker in [
        "#[inline(never)]\nunsafe fn mi_arena_slice_start(",
        "#[inline(never)]\nunsafe fn mi_arena_page_at_slice(",
        "#[inline(never)]\nunsafe fn mi_arenas_page_singleton_alloc(",
        "#[inline(never)]\nunsafe fn mi_arena_purge(",
        "static MI_ARENA_SLICE_START_KEEP",
        "static MI_ARENA_PAGE_AT_SLICE_KEEP",
        "static MI_ARENAS_PAGE_SINGLETON_ALLOC_KEEP",
        "static MI_ARENA_PURGE_KEEP",
    ] {
        assert!(
            arena_rs.contains(marker),
            "arena helper boundary marker missing: {marker}"
        );
    }
    for marker in [
        "#[inline(never)]\nunsafe fn mi_debug_show_chunks(",
        "static MI_DEBUG_SHOW_CHUNKS_KEEP",
    ] {
        assert!(
            arena_debug_rs.contains(marker),
            "arena debug helper boundary marker missing: {marker}"
        );
    }
    let claim_start = arena_rs
        .find("unsafe fn mi_arena_try_claim_abandoned_at(")
        .expect("find mi_arena_try_claim_abandoned_at");
    let claim_end = arena_rs[claim_start..]
        .find("\n}\n\nunsafe extern \"C\" fn mi_arena_try_claim_abandoned(")
        .map(|offset| claim_start + offset)
        .expect("find end of mi_arena_try_claim_abandoned_at");
    let claim_body = &arena_rs[claim_start..claim_end];
    assert!(
        !claim_body.contains("mi_arena_page_at_slice(")
            && claim_body.contains("pages_meta.add(slice_index)")
            && claim_body.contains("slice_index * MI_ARENA_SLICE_SIZE"),
        "abandoned arena page claiming should keep C's inlined page lookup shape"
    );
    assert!(
        arena_rs.contains(
            "unsafe { mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned) }"
        ),
        "abandoned arena page callback should parse raw slice indexes at the C ABI boundary"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_slice_start(\n    arena: ArenaPtr,\n    slice_index: crate::core_types::SliceIndex,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_page_at_slice(\n    arena: ArenaPtr,\n    slice_index: crate::core_types::SliceIndex,"
        ) && arena_rs.contains("unsafe fn mi_heap_visit_page_at_index(\n    slice_index: SliceIndex,")
            && arena_rs.contains("unsafe fn mi_arena_try_claim_abandoned_at(\n    slice_index: SliceIndex,")
            && arena_debug_rs.contains("    slice_index: SliceIndex,\n    pbit_of_page:")
            && arena_rs.contains("mi_heap_visit_page_at_index(SliceIndex::new(slice_index), arena, arg)")
            && arena_rs.contains(
                "mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned)"
            )
            && arena_rs
                .contains("mi_arena_page_at_slice(ArenaPtr::new_unchecked(arena), slice_index)")
            && arena_rs.contains("let page = mi_arena_page_at_slice(arena, slice_index);")
            && arena_rs.contains(
                "mi_arena_slice_start(arena_ptr, crate::core_types::SliceIndex::new(slice_index))"
            )
            && arena_rs.contains(
                "mi_arena_slice_start(ArenaPtr::new_unchecked(arena), range.start())\n            .as_ptr()\n            .cast::<c_void>()"
            )
            && !arena_rs.contains("unsafe fn mi_arena_page_at_slice(\n    arena: *mut mi_arena_t,")
            && !arena_rs.contains("unsafe fn mi_arena_slice_start(\n    arena: *mut mi_arena_t,"),
        "arena slice address/page helpers should receive typed ArenaPtr and SliceIndex values"
    );
    assert!(
        arena_types.contains(
            "pub(crate) fn single_at_offset(self, offset: usize) -> Result<Self, SliceError>"
        ) && arena_rs.contains("let Ok(single) = range.single_at_offset(i) else")
            && !arena_rs.contains("SliceRange::new(slice_index + i, 1"),
        "arena purge fallback should split SliceRange values without reopening raw slice arithmetic"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_schedule_purge(arena: ArenaPtr, range: crate::core_types::SliceRange)"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && !arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) -> bool"
        ) && !arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> bool"
        ) && arena_ids_rs.contains("match crate::core_types::MemId::view(memid)")
            && arena_ids_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && arena_ids_rs.contains("let range = arena_memid.range;")
            && arena_ids_rs.contains("fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && arena_ids_rs.contains("crate::core_types::MemIdView::Os(_)")
            && memid_rs.contains("pub(crate) struct ExternalMemId")
            && memid_rs.contains("External(ExternalMemId)")
            && memid_rs.contains("impl MemId<ExternalMem>")
            && memid_rs.contains("pub(crate) fn external(")
            && arena_rs.contains(
                "memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();"
            )
            && arena_rs
                .contains("crate::core_types::MemId::<crate::core_types::ExternalMem>::external")
            && arena_ids_rs.contains("crate::core_types::MemIdView::External(external)")
            && os_rs.contains("crate::core_types::MemIdView::External(external)")
            && arena_ids_rs.contains("let os_size = external.size.get();")
            && arena_debug_rs
                .contains("let page_slice_count = match crate::core_types::MemId::view(unsafe { (*raw).memid })")
            && arena_debug_rs.contains("crate::core_types::MemIdView::Arena(a) => a.range.count_usize() as isize")
            && os_rs.contains("external.size.get()")
            && os_rs.contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && memid_rs.contains("pub(crate) fn os_huge(")
            && memid_rs.contains("fn os_with_kind(")
            && internal_memid.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os(")
            && os_rs.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(")
            && !os_rs.contains("mi_memkind_is_os(memid.memkind)")
            && !os_rs.contains("(*memid).memkind = crate::internal::mi_memkind_t::MI_MEM_OS_HUGE")
            && !arena_rs.contains("memid.mem.os.")
            && !arena_rs.contains("memid.mem.arena.")
            && !arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_EXTERNAL")
            && !arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_NONE")
            && !os_rs.contains("memid.mem.os.")
            && !internal.contains("memid.mem.os.base = base")
            && !internal.contains("memid.mem.os.size = size")
            && !internal_memid.contains("memid.mem.os.base = base")
            && !internal_memid.contains("memid.mem.os.size = size")
            && page_map.contains("fn mi_memid_os_base_size(memid: mi_memid_t)")
            && page_map.contains("crate::core_types::MemIdView::Os(os)")
            && page_map.contains("crate::core_types::MemIdView::External(external)")
            && !page_map.contains("memid.mem.os.")
            && !page_map.contains("mi_page_map_memid.mem.os.")
            && theap_rs.contains("let allocated_size = match crate::core_types::MemId::view(memid)")
            && theap_rs.contains("crate::core_types::MemIdView::Arena(a)")
            && theap_rs.contains("a.range.count_usize() * crate::arena::MI_ARENA_SLICE_SIZE")
            && theap_rs.contains("fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool")
            && theap_rs.contains("crate::core_types::MemIdView::Unknown(code)")
            && theap_rs.contains("code.0 > crate::core_types::MemKindCode::STATIC.0")
            && theap_rs.contains("match crate::core_types::MemId::view(memid)")
            && theap_rs.contains("crate::core_types::MemIdView::HeapMain(_)")
            && theap_rs.contains("crate::core_types::MemIdView::Meta(_)")
            && theap_rs.contains("mi_memid_is_refcounted((*theap.as_ptr()).memid)")
            && atomic_types.contains(
                "pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);"
            )
            && atomic_types.contains("pub(crate) unsafe fn incref_acq_rel(self) -> usize")
            && atomic_types.contains("pub(crate) unsafe fn decref_acq_rel(self) -> usize")
            && ptr_rs.contains("pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount")
            && theap_rs.matches(".refcount_cell()").count() == 3
            && theap_rs.contains(".incref_acq_rel();")
            && theap_rs.contains(".decref_acq_rel()")
            && !theap_rs.contains("AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))")
            && !theap_rs.contains("mi_atomic_increment_acq_rel(ptr::addr_of_mut!((*theap).refcount))")
            && !theap_rs.contains("mi_atomic_fetch_sub_explicit(\n            ptr::addr_of_mut!((*theap).refcount),")
            && !theap_rs.contains("memid.mem.os.")
            && !theap_rs.contains("memid.memkind")
            && init_rs.contains("fn mi_memid_is_static(memid: mi_memid_t) -> bool")
            && init_rs.contains("crate::core_types::MemIdView::Static(_)")
            && memid_rs.contains("impl MemId<StaticMem>")
            && memid_rs.contains("pub(crate) const fn static_mem() -> Self")
            && init_rs.contains(
                "crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw()"
            )
            && init_rs.contains("!mi_memid_is_static((*main_prefix).memid)")
            && init_rs.contains("!mi_memid_is_static(theap_main.memid)")
            && !init_rs.contains(
                "crate::internal::_mi_memid_create(mi_memkind_t::MI_MEM_STATIC)"
            )
            && !init_rs.contains("(*main_prefix).memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && !init_rs.contains("theap_main.memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && internal.contains("pub(crate) mod memid;")
            && internal.contains("pub use memid::*;")
            && !internal.contains("pub enum mi_memkind_t")
            && internal_memid.contains("//! Raw memory-origin identifiers from mimalloc's internal header.")
            && internal_memid.contains("pub enum mi_memkind_t")
            && internal_memid.contains("pub fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && internal_memid.contains("pub fn mi_memid_needs_no_free(memid: mi_memid_t) -> bool")
            && internal.contains("pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool")
            && internal_memid.contains("crate::core_types::MemIdView::None(_)")
            && internal_memid.contains("crate::core_types::MemIdView::External(_)")
            && internal_memid.contains("crate::core_types::MemIdView::Static(_)")
            && internal.contains("crate::core_types::MemIdView::Arena(_)")
            && !internal.contains("pub unsafe fn _mi_memid_create(")
            && !internal_memid.contains("pub unsafe fn _mi_memid_create(")
            && !internal.contains("raw.memkind = memkind")
            && internal.contains("mi_page_is_in_arena(page)")
            && arena_meta_rs.contains("MemId::<MetaMem>::meta(")
            && arena_meta_rs.contains("NonZeroU32::new_unchecked(block_count as u32)")
            && memid_rs.contains("impl MemId<MetaMem>")
            && memid_rs.contains("is_pinned: true")
            && memid_rs.contains("initially_committed: true")
            && memid_rs.contains("initially_zero: true")
            && !internal.contains("crate::types::mi_memkind_is_os(memid.memkind)")
            && !internal.contains("crate::types::mi_memkind_needs_no_free(memid.memkind)")
            && !internal.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !internal.contains("memid.mem.meta.meta_page = mpage")
            && !internal.contains("memid.mem.meta.block_index = block_idx as u32")
            && !internal.contains("memid.mem.meta.block_count = block_count as u32")
            && !internal_memid.contains("memid.mem.meta.meta_page = mpage")
            && !internal_memid.contains("memid.mem.meta.block_index = block_idx as u32")
            && !internal_memid.contains("memid.mem.meta.block_count = block_count as u32")
            && arena_meta_rs.contains(
                "if let crate::core_types::MemIdView::Meta(meta) = crate::core_types::MemId::view(memid)"
            )
            && arena_meta_rs.contains("let block_count = meta.block_count.get() as usize;")
            && arena_meta_rs.contains("let block_idx = meta.block_index.0 as usize;")
            && !arena_meta_rs.contains("memid.mem.meta.")
            && !arena_meta_rs.contains("memid.memkind == crate::internal::mi_memkind_t::MI_MEM_META")
            && !arena_rs.contains("const fn mi_memkind_is_os(memkind: mi_memkind_t)")
            && !arena_rs.contains("mi_memkind_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.memkind })")
            && !arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            )
            && !arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange)"
            )
            && !arena_rs.contains(
                "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            ),
        "arena free/purge path should dispatch through MemIdView and carry SliceRange"
    );
    assert!(
        page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    submap: *mut mi_submap_t,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_set_range_prim(\n    page: PagePtr,\n    mut idx: crate::core_types::PageMapIndex,\n    mut sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_clear_range_prim("
        ) && page_map.contains(
            "unsafe fn mi_page_map_set_range(\n    page: PagePtr,\n    idx: crate::core_types::PageMapIndex,\n    sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_clear_range("
        ) && page_map.contains("unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool")
            && page_map.contains("unsafe fn mi_page_map_unregister_typed(page: PagePtr)")
            && page_map.contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && page_map.contains("unsafe { mi_page_map_register_typed(page) }")
            && page_map.contains("unsafe { mi_page_map_unregister_typed(page) };")
            && !page_map.contains("unsafe fn mi_page_map_ensure_committed(idx: usize, slice_count: usize)")
            && !page_map.contains("unsafe fn mi_page_map_ensure_committed(idx: usize, submap:")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    mut slice_count: usize,")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    slice_count: crate::core_types::SliceCount,")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: crate::core_types::PageMapIndex,")
            && !page_map.contains("unsafe fn mi_page_map_set_range(\n    page: *mut mi_page_t,\n    idx: usize,\n    sub_idx: usize,\n    slice_count: usize,")
            && !page_map.contains("fn mi_page_map_get_idx(\n    page: *mut mi_page_t,"),
        "page-map range helpers should validate raw indexes/counts once and carry typed indexes plus SliceCount internally"
    );
    assert!(
        arena_rs.contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && memid_rs.contains("slice_index: range.start_u32()")
            && memid_rs.contains("slice_count: range.count_u32()")
            && page_map.contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && !arena_rs.contains(" as u32")
            && !page_map.contains(" as u32")
            && !memid_rs.contains(" as u32")
            && !arena_rs.contains("SliceRange::from_raw(slice_index as")
            && !arena_rs.contains("SliceRange::from_raw(slice_index, slice_count as")
            && !page_map.contains("SliceCount::from_usize(slice_count as"),
        "arena and page-map slice fields should use checked typed conversions instead of lossy casts"
    );
    assert!(
        page_types.contains("pub(crate) struct RegisteredPageRange {\n    page: PagePtr,\n    index: PageMapIndex,\n    sub_index: PageMapSubIndex,\n    slice_count: SliceCount,\n}")
            && page_types.contains("pub(crate) unsafe fn new_unchecked(\n        page: PagePtr,\n        index: PageMapIndex,\n        sub_index: PageMapSubIndex,\n        slice_count: SliceCount,\n    ) -> Self")
            && page_types.contains("pub(crate) fn page(self) -> PagePtr")
            && page_types.contains("pub(crate) fn index(self) -> PageMapIndex")
            && page_types.contains("pub(crate) fn sub_index(self) -> PageMapSubIndex")
            && page_types.contains("pub(crate) fn slice_count(self) -> SliceCount")
            && page_map.contains("crate::core_types::RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)\n    };")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.index() == idx);")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.sub_index() == sub_idx);")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.slice_count() == slice_count);")
            && page_map.contains("Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })"),
        "RegisteredPageRange should be an opaque page-map registration witness over PagePtr, PageMapIndex, PageMapSubIndex, and SliceCount"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && atomic_types
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_types.contains("expected: &mut *mut T,")
            && page_map.contains(
                "crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });"
            )
            && page_map.contains("atomic_submap.load(crate::core_types::MemoryOrder::Acquire)")
            && page_map.contains("atomic_submap.compare_exchange_acq_rel(&mut expect, new_sub)")
            && !page_map.contains("mi_atomic_load_ptr_acquire(_mi_page_map.add(idx))")
            && !page_map.contains(
                "mi_atomic_cas_strong_acq_rel(\n                        _mi_page_map.add(idx).cast::<usize>()"
            ),
        "page-map submap slot atomics should use AtomicPtrField"
    );
    assert!(
        internal.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of!((*theap).heap).cast_mut())\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && internal.contains(
            "crate::core_types::AtomicPtrField(_mi_page_map_base().add(idx))\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && !internal.contains("crate::atomic::mi_atomic_load_ptr_acquire(&(*theap).heap)")
            && !internal.contains("crate::atomic::mi_atomic_load_ptr_acquire(_mi_page_map_base().add(idx))"),
        "internal thread-heap and page-map pointer loads should use AtomicPtrField"
    );
    assert!(
        arena_ids_rs.contains(
            "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> mi_memid_t"
        ) && arena_rs.contains("memid: unsafe { mi_memid_create_arena(arena, range) },")
            && arena_ids_rs.contains("let Some((arena, range)) = (unsafe { mi_arena_range_from_memid(memid) }) else")
            && arena_ids_rs.contains("unsafe { *slice_index = range.start_usize() };")
            && arena_ids_rs.contains("unsafe { *slice_count = range.count_usize() };")
            && scalar_types.contains("pub(crate) fn as_u32(self) -> Option<u32>")
            && scalar_types.contains("pub(crate) const fn from_usize_const(count: usize) -> Option<Self>")
            && arena_types.contains("pub(crate) fn start_u32(self) -> u32")
            && arena_types.contains("pub(crate) const fn count_u32(self) -> u32")
            && memid_rs.contains("slice_index: range.start_u32()")
            && memid_rs.contains("slice_count: range.count_u32()")
            && arena_rs.contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && !arena_rs.contains("mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) as u32")
            && !arena_rs.contains("mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) as u32")
            && !arena_rs.contains("mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) as u32")
            && !memid_rs.contains("range.start().0 as u32")
            && !arena_ids_rs.contains(
                "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    slice_index: usize,\n    slice_count: usize,"
            )
            && !arena_ids_rs.contains(
                "SliceRange::from_raw(slice_index as u32, slice_count as u32)"
            ),
        "arena memid construction should receive an already-typed SliceRange"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_try_alloc_at(\n    arena: ArenaPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(") -> Option<(*mut c_void, crate::core_types::ArenaAllocation)>")
            && arena_types.contains(
                "pub(crate) struct ArenaAllocation {\n    pub(crate) arena: ArenaPtr,\n    pub(crate) range: SliceRange,\n    pub(crate) memid: crate::internal::mi_memid_t,\n}"
            )
            && arena_rs.contains("let mut allocation = crate::core_types::ArenaAllocation {")
            && arena_rs.contains("unsafe { *memid = allocation.assume_init().memid };")
            && arena_rs.contains("mi_arena_try_alloc_at(arena, slice_count, commit, tseq)")
            && !arena_rs.contains(
                "unsafe fn mi_arena_try_alloc_at(\n    arena: *mut mi_arena_t,\n    slice_count: crate::core_types::SliceCount,"
            )
            && !arena_rs.contains("mi_arena_try_alloc_at(arena, slice_count.get() as usize"),
        "arena allocation helper should accept typed SliceCount and return ArenaAllocation"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arenas_try_alloc(\n    heap: BorrowedHeap,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("    alignment: crate::core_types::Alignment,")
            && arena_rs.contains("    req_arena: Option<ArenaPtr>,")
            && arena_rs.contains("    numa_node: crate::core_types::NumaNode,")
            && arena_rs.contains("let Some(subproc) = (unsafe { heap.ptr().subproc() }) else")
            && arena_rs.contains("heap: HeapPtr::new(heap),")
            && arena_rs.contains("crate::core_types::NumaNode::new(numa_node),")
            && arena_rs.contains("req_arena: ArenaPtr::new(req_arena),")
            && arena_rs.contains("let Some(heap_ptr) = HeapPtr::new(heap) else")
            && arena_rs.contains("BorrowedHeap::new_unchecked(heap_ptr),")
            && !arena_rs.contains(
                "unsafe fn mi_arenas_try_alloc(\n    heap: *mut mi_heap_t,"
            ),
        "arena try-alloc should receive typed heap, requested arena, slice count, and NUMA node below raw entry points"
    );
    assert!(
        arena_types.contains("#[repr(transparent)]\npub(crate) struct SharedArena(pub(crate) ArenaPtr);")
            && arena_types.contains("#[repr(transparent)]\npub(crate) struct ExclusiveArena(pub(crate) ArenaPtr);")
            && arena_types.contains("#[repr(transparent)]\npub(crate) struct ParentArena(pub(crate) ArenaPtr);")
            && arena_types.contains("impl SharedArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {\n        if unsafe { arena.is_exclusive() } {")
            && arena_types.contains("impl ExclusiveArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {\n        if unsafe { arena.is_exclusive() } {")
            && arena_types.contains("impl ParentArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {\n        if unsafe { arena.parent().is_none() } {"),
        "arena role wrappers should have checked constructors based on raw arena state"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_pages_size(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_info_slices_needed(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_bitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_bbitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_initialize(\n    subproc: SubprocPtr,\n    start: OsMemPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("let slice_count_usize = slice_count.get() as usize;")
            && arena_rs.contains("let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && arena_rs.contains("mi_arena_initialize(\n                SubprocPtr::new_unchecked(subproc),\n                OsMemPtr::new_unchecked(start),\n                typed_slice_count,")
            && !arena_rs.contains("unsafe fn mi_arena_pages_size(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_info_slices_needed(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_bitmap_init(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_bbitmap_init(slice_count: usize"),
        "arena metadata sizing and initialization should receive typed SliceCount"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && arena_rs.contains("let block_size_typed = unsafe { crate::core_types::BlockSize::new_unchecked(block_size) };")
            && arena_rs.contains(
                "let block_alignment_typed =\n        unsafe { crate::core_types::MaybeAlignment::new_unchecked(block_alignment) };",
            )
            && arena_rs.contains("const MI_SMALL_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("const MI_MEDIUM_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("const MI_LARGE_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_SMALL_PAGE_SLICE_COUNT, block_size_typed)")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_MEDIUM_PAGE_SLICE_COUNT, block_size_typed)")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_LARGE_PAGE_SLICE_COUNT, block_size_typed)")
            && !arena_rs.contains("unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: usize,"),
        "arena page allocation helpers should carry typed SliceCount after raw entry-point validation"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_now()")
            && stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_start()")
            && stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_end(")
            && stats_rs.contains("let t0 = _mi_clock_now();")
            && stats_rs.contains("let diff = _mi_clock_now() - t0;")
            && stats_rs.contains("_mi_clock_now() - start - CLOCK_DIFF.load")
            && os_rs.contains("let start_t = _mi_clock_start();")
            && os_rs.contains("let mut elapsed = _mi_clock_end(start_t);")
            && arena_rs.contains("let expire = crate::stats::_mi_clock_now() + delay as i64;")
            && arena_rs.contains("let now = crate::stats::_mi_clock_now();"),
        "clock helpers should keep C's timing helper boundaries and main callers"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool")
            && arena_rs.matches("crate::core_types::AtomicI64Field(").count() == 7
            && arena_rs
                .matches(".compare_exchange_acq_rel(&mut expire0, expire)")
                .count()
                == 2
            && arena_rs.contains(".load(crate::core_types::MemoryOrder::Relaxed)")
            && arena_rs.contains(".load(crate::core_types::MemoryOrder::Acquire)")
            && arena_rs
                .matches(".store(0, crate::core_types::MemoryOrder::Release)")
                .count()
                == 2
            && arena_rs.contains(
                "now + ((delay as mi_msecs_t) / 10),\n                    crate::core_types::MemoryOrder::Release,"
            )
            && !arena_rs.contains("mi_atomic_casi64_strong_acq_rel")
            && !arena_rs.contains("mi_atomic_loadi64_explicit")
            && !arena_rs.contains("mi_atomic_loadi64_acquire")
            && !arena_rs.contains("mi_atomic_storei64_release")
            && !arena_rs.contains("mi_memory_order_e::mi_memory_order_relaxed"),
        "arena purge expiry should use typed atomic i64 field wrappers"
    );
    assert!(
            atomic_types.contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && atomic_types.contains("desired: usize,")
            && atomic_types.contains(
                "atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)"
            )
            && arena_rs
                .matches("ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)")
                .count()
                == 2
            && arena_rs.contains(".compare_exchange_acq_rel(&mut expected, 0)")
            && arena_rs.contains("let next_count = count + 1;")
            && arena_rs.contains(".compare_exchange_acq_rel(&mut count, next_count)")
            && !arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            )
            && !arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            ),
        "arena_count compare-exchange should use AtomicUsizeField"
    );
    assert!(
        atomic_types
            .contains("pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);")
            && atomic_types.contains("impl AtomicAbandonedCount")
            && atomic_types.contains("pub(crate) unsafe fn load_relaxed(self) -> usize")
            && atomic_types
                .contains("pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize")
            && atomic_types
                .contains("pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize")
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_heap_abandoned_count(\n    heap: *mut mi_heap_t,\n    bin: crate::core_types::Bin,"
            )
            && arena_prefix_rs.contains("abandoned_count[bin.as_usize()]")
            && arena_rs.contains("let Some(typed_bin) = crate::core_types::Bin::new(bin as u16) else")
            && arena_rs.contains("crate::internal::mi_assert_internal!(false);\n                                break;")
            && arena_rs.contains("mi_heap_abandoned_count(heap, typed_bin).load_relaxed() > 0")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).load_relaxed() == 0")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1)")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1)")
            && !arena_rs.contains(".abandoned_count[bin].load(Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[bin].fetch_add(1, Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[bin].fetch_sub(1, Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[i].load(Ordering::Relaxed)"),
        "arena abandoned-count atomics should go through AtomicAbandonedCount with typed Bin indexes"
    );
    assert!(
        atomic_types
            .contains("pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);")
            && atomic_types.contains("impl AtomicArenaPage")
            && atomic_types.contains(
                "pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t"
            )
            && atomic_types.contains(
                "pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t)"
            )
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_arena_index(arena: *mut mi_arena_t) -> crate::core_types::ArenaIndex"
            )
            && !arena_rs.contains(
                "unsafe fn mi_heap_arena_page_cell(\n    heap: *mut mi_heap_t,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_heap_arena_page_cell(\n    heap: BorrowedHeap,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && arena_rs.contains(
                "unsafe fn mi_heap_ensure_arena_pages(\n    heap: BorrowedHeap,\n    arena: ArenaPtr,\n    arena_idx: ArenaIndex,"
            )
            && arena_prefix_rs.contains("arena_pages[arena_idx.get() as usize]")
            && arena_rs.matches("mi_heap_arena_page_cell(heap, arena_idx).load_acquire()").count()
                == 1
            && arena_rs
                .matches("mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire()")
                .count()
                == 2
            && arena_rs
                .contains("mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast())")
            && !arena_rs.contains(".arena_pages[arena_idx].load(Ordering::Acquire)")
            && !arena_rs.contains(".arena_pages[arena_idx]\n                    .store")
            && !arena_rs.contains(".arena_pages[i].load(Ordering::Acquire)"),
        "heap arena page-table atomics should use AtomicArenaPage with typed ArenaIndex values"
    );
    assert!(
        atomic_types
            .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_release(")
            && arena_prefix_rs
                .matches("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(")
                .count()
                == 1
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_subproc_arena_cell(\n    subproc: *mut mi_subproc_t,\n    arena_idx: ArenaIndex,"
            )
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_arena_from_index_typed(\n    subproc: SubprocPtr,\n    arena_idx: ArenaIndex,"
            )
            && arena_prefix_rs
                .matches("(*subproc.cast::<MiSubprocPrefix>()).arenas[")
                .count()
                == 1
            && arena_rs.contains("let Some(arena_idx) = ArenaIndex::from_usize(idx)")
            && arena_rs
                .matches("mi_arena_from_index_typed(")
                .count()
                >= 5
            && arena_rs
                .matches(".compare_exchange_release(&mut expected, arena)")
                .count()
                == 2
            && arena_rs
                .contains(".store(ptr::null_mut(), crate::core_types::MemoryOrder::Release);")
            && !arena_rs.contains("mi_arena_from_index(subproc, i)")
            && !arena_rs.contains("mi_atomic_cas_ptr_strong_release")
            && !arena_rs.contains("mi_atomic_store_ptr_release")
            && !arena_rs.contains("mi_atomic_load_ptr_acquire"),
        "arena table pointer atomics should use AtomicPtrField with typed ArenaIndex values"
    );
    assert!(
        free_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign")
            && free_rs.contains(
                "crate::internal::mi_assert!(\n                _mi_page_ptr_unalign(page_raw, user_raw) == user_raw.cast()\n            );"
            )
            && free_rs.contains(
                "pub(crate) unsafe fn mi_validate_block_from_ptr(\n    page: crate::core_types::PagePtr,\n    user: crate::core_types::UserPtr,\n) -> crate::core_types::PointerLookup"
            )
            && free_rs.contains("let location = mi_pointer_location(page, user, MI_SECURE > 0, true);")
            && free_rs.contains("unsafe fn mi_lookup_pointer_page(")
            && free_rs.contains("user: crate::core_types::UserPtr,")
            && free_rs.contains(") -> Option<crate::core_types::PagePtr>")
            && free_rs.contains("let registered_page = mi_lookup_pointer_page(user_ptr);")
            && free_rs.contains("unsafe fn mi_pointer_location(")
            && free_rs.contains("page: crate::core_types::PagePtr,")
            && free_rs.contains("user: crate::core_types::UserPtr,")
            && free_rs.contains(") -> crate::core_types::PointerLocation")
            && free_rs.contains("let page_raw = page.as_ptr();")
            && free_rs.contains("let user_raw = user.as_ptr();")
            && free_rs.contains("let block = if mi_page_has_interior_pointers(page_raw) || force_unalign")
            && free_rs.contains("PointerLocation::Guarded")
            && validation_types.contains("pub(crate) struct LocatedBlock")
            && validation_types.contains("pub(crate) struct GuardedBlock(BlockPtr);")
            && validation_types.contains("pub(crate) fn after_guarded_detection(block: BlockPtr) -> Self")
            && validation_types.contains("impl PointerLocation")
            && validation_types.contains("pub(crate) fn located_block(self) -> LocatedBlock")
            && validation_types.contains("Valid(ValidPointerLookup)")
            && validation_types.contains("pub(crate) struct ValidPointerLookup {\n    user: UserPtr,\n    page: PagePtr,\n    location: PointerLocation,")
            && validation_types.contains("pub(crate) fn valid(user: UserPtr, page: PagePtr, location: PointerLocation) -> Self")
            && validation_types.contains("pub(crate) struct BlockForFree {\n    block: BlockPtr,\n    was_guarded: WasGuarded,")
            && validation_types.contains("pub(crate) fn is_guarded(self) -> bool")
            && validation_types
                .contains("pub(crate) fn into_free_block(self) -> crate::core_types::FreeBlock")
            && validation_types.contains("pub(crate) struct ValidatedFree {\n    user: UserPtr,\n    page: PagePtr,\n    block: BlockForFree,")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("ValidatedFree { user, page, block }")
            && validation_types.contains("pub(crate) fn page(self) -> PagePtr")
            && validation_types.contains("pub(crate) fn block(self) -> BlockForFree")
            && free_rs.contains("/// # Safety\n/// `lookup` must come from validating the same page/user pair that is about to\n/// be freed by the generic free path.")
            && free_rs.contains("/// the block; this boundary consumes the lookup and performs any required\n/// guarded-block unprotection before returning the freeable block witness.")
            && free_rs.contains("unsafe fn mi_block_for_free_from_lookup(")
            && free_rs.contains("let validated = lookup.prepare_free()?;")
            && free_rs.contains("unsafe fn mi_prepare_validated_free(")
            && free_rs.contains("mi_prepare_validated_free(lookup).map(crate::core_types::ValidatedFree::block)")
            && free_rs.contains("let block = block_wrapper.block().as_ptr();")
            && free_rs.contains("let was_guarded = block_wrapper.is_guarded();")
            && free_rs.contains("let page = validated.page().as_ptr();")
            && free_rs.contains("validated.block()")
            && free_rs.contains("let path = page_tid.classify_free_path(current_tid);")
            && free_rs.contains("let owner = validated.page().classify_for_free(current_tid);")
            && free_rs.contains("crate::core_types::PageFreeOwner::Local(local_page)")
            && free_rs.contains("owner: crate::core_types::PageFreeOwner,")
            && free_rs.contains("crate::core_types::PageFreeOwner::Foreign(foreign_page)")
            && free_rs.contains("mi_free_block_local(\n                    local_page,")
            && free_rs.contains("mi_free_block_mt(foreign_page, validated.block());")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());")
            && !free_rs.contains("let delta = page_tid.xor_current(current_tid);")
            && !source_item(
                "src/allocator/free.rs",
                &free_rs,
                "pub(crate) unsafe fn mi_free_ex_slow("
            )
            .contains("LocalPage::new_unchecked")
            && !source_item(
                "src/allocator/free.rs",
                &free_rs,
                "pub(crate) unsafe fn mi_free_ex_slow("
            )
            .contains("ForeignPage::new_unchecked")
            && free_rs.contains("GuardedBlock::after_guarded_detection(block)")
            && free_rs.contains("GuardedBlock::after_guarded_detection(block_ptr)")
            && free_rs.contains("if let Some(validated) = mi_prepare_validated_free(mi_validate_pointer(p, msg))")
            && free_rs.contains("let location = mi_pointer_location(page_ptr, user_ptr, false, false);")
            && free_rs.contains("PointerLookup::valid(user_ptr, page_ptr, location)")
            && free_rs.matches("mi_block_check_unguard(validated.page(), validated.block(), validated.user())").count() == 1
            && !free_rs.contains("let Some((validated, located))")
            && !free_rs.contains("let block_raw = if mi_page_has_interior_pointers(page_raw)")
            && alloc_rs.contains("let location = valid.location();")
            && alloc_rs.contains("let block = location.block();")
            && free_rs.matches("let page = crate::core_types::PagePtr::new_unchecked(page);").count() == 2
            && free_rs.matches("let user = crate::core_types::UserPtr::new_unchecked(p);").count() == 2
            && free_rs
                .matches("mi_prepare_validated_free(mi_validate_block_from_ptr(page, user))")
                .count()
                == 2
            && free_rs.contains("LocalPage::new_unchecked(validated.page())")
            && free_rs.contains("ForeignPage::new_unchecked(validated.page())")
            && !free_rs.contains("mi_block_for_free_from_lookup(mi_validate_block_from_ptr(page, user))")
            && !free_rs.contains("mi_block_for_free_from_lookup(page, p, mi_validate_block_from_ptr(page, p))")
            && !validation_types.contains("Valid {\n        user: UserPtr")
            && !validation_types.contains("pub(crate) block: BlockPtr")
            && !validation_types.contains("pub(crate) page: PagePtr")
            && !validation_types.contains("pub(crate) struct GuardedBlock(pub(crate) BlockPtr);")
            && !validation_types.contains("pub(crate) fn new(user: UserPtr")
            && !validation_types.contains("pub(crate) fn new(block: BlockPtr")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new")
            && !free_rs.contains("validated.block,")
            && !free_rs.contains("PointerLocation::BlockStart(b) => (b, false)")
            && !free_rs.contains("pub unsafe fn mi_validate_block_from_ptr(\n    page: *const mi_page_t,\n    p: *mut c_void,\n) -> *mut mi_block_t"),
        "free validation should keep _mi_page_ptr_unalign while returning PointerLookup and BlockForFree"
    );
    assert!(
        !heap_rs.contains("fn _mi_theap_cached_set(")
            && !heap_rs.contains("fn _mi_theap_create(")
            && !heap_rs.contains("fn _mi_theap_free(")
            && !heap_rs.contains("fn _mi_heap_move_pages(")
            && !heap_rs.contains("fn _mi_heap_destroy_pages("),
        "heap internals should call crate-local theap helpers directly"
    );
    assert!(
        arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_move_pages")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_destroy_pages"
            )
            && heap_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_force_destroy"
            )
            && heap_rs.contains("_mi_heap_move_pages(heap_raw, mi_heap_main());")
            && heap_rs.contains("_mi_heap_destroy_pages(heap.as_ptr());")
            && heap_rs.contains("unsafe { mi_heap_force_destroy_owned(heap) };"),
        "heap delete/destroy should keep C's heap page movement and force-destroy helper boundaries"
    );
    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc_aligned"
        ) && arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc")
            && arena_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_contain")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all"
            )
            && arena_rs.contains("mi_arenas_try_alloc(")
            && arena_rs.contains("mi_arena_os_alloc_aligned(")
            && arena_rs.contains("_mi_arenas_alloc_aligned(")
            && arena_rs.contains("mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), None)")
            && arena_rs.contains("static MI_ARENAS_CONTAIN_KEEP")
            && arena_rs.contains("mi_arenas_unsafe_destroy(subproc)")
            && theap_rs.contains("let theap = _mi_arenas_alloc(")
            && init_rs.contains("_mi_arenas_unsafe_destroy_all(subproc)"),
        "arena allocation, containment, and unsafe destroy helpers should keep C's local call boundaries"
    );
    assert!(
        arena_rs.contains("struct ArenaAllocTypedArgs {\n    heap: Option<HeapPtr>,\n    layout: crate::core_types::LayoutRequest,\n    align_offset: crate::core_types::AlignmentOffset,")
            && arena_rs.contains("unsafe fn mi_arenas_alloc_aligned_typed(args: ArenaAllocTypedArgs)")
            && arena_rs.contains("let size = layout.requested();")
            && arena_rs.contains("let alignment = layout.alignment();")
            && arena_rs.contains("arena_slice_count: Option<crate::core_types::SliceCount>,")
            && arena_rs.contains("fn mi_arena_slice_count_for_request(\n    size: crate::core_types::RequestedSize,")
            && arena_rs.contains("crate::core_types::SliceCount::from_usize(mi_slice_count_of_size(size.get()))")
            && arena_rs.contains("arena_slice_count: mi_arena_slice_count_for_request(layout.requested())")
            && arena_rs.contains("crate::core_types::LayoutRequest::aligned_at(")
            && arena_rs.contains("crate::core_types::AlignmentOffset::new(align_offset)")
            && arena_rs.contains("struct ArenaAllocArgs {\n    heap: Option<HeapPtr>,\n    size: crate::core_types::RequestedSize,")
            && arena_rs.contains("unsafe fn mi_arenas_alloc_typed(args: ArenaAllocArgs)")
            && arena_rs.contains("crate::core_types::LayoutRequest::aligned(size.get(), alignment)")
            && arena_rs.contains("let size = crate::core_types::RequestedSize::new(size);")
            && arena_rs.contains("crate::core_types::AlignmentOffset::new(0)")
            && arena_rs.contains("req_arena: ArenaPtr::new(req_arena)")
            && arena_rs.contains("tseq: crate::core_types::ThreadSeq::new(tseq)")
            && arena_rs.contains("numa_node: crate::core_types::NumaNode::new(numa_node)"),
        "arena allocation ABI wrappers should parse raw size/alignment into typed layout domains before using arena internals"
    );
    let reabandon_arena = source_item(
        "src/allocator/arena.rs",
        &arena_rs,
        "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena",
    );
    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped"
        )
            && arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(\n    page: crate::core_types::OwnedAbandonedPage,\n    _arena_memid: crate::core_types::ArenaMemId,"
            )
            && arena_ids_rs
                .contains("unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool")
            && arena_ids_rs.contains("crate::core_types::MemId::view(unsafe { (*page).memid })")
            && arena_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !arena_rs.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !arena_rs.contains("(*page).memid.memkind != mi_memkind_t::MI_MEM_ARENA")
            && arena_rs.contains("_mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page))")
            && arena_rs.contains("_mi_arenas_page_abandon(page, theap)")
            && !reabandon_arena.contains("mi_page_memid_is_arena(page)")
            && !reabandon_arena.contains("MemId::view")
            && free_rs.contains("crate::core_types::MemId::view((*page).memid)")
            && free_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !free_rs.contains(
                "(*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA"
            )
            && free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped_arena(")
            && !free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped(page)"),
        "free path should validate arena memids and carry the witness into the crate-local reabandon helper"
    );
    assert!(
        arena_ids_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arena_id_none")
            && arena_ids_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arena_from_id")
            && arena_ids_rs
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_memid_is_suitable")
            && arena_ids_rs.contains("static MI_ARENA_ID_NONE_KEEP")
            && arena_ids_rs.contains("static MI_ARENA_FROM_ID_KEEP")
            && arena_ids_rs.contains("unsafe { _mi_arena_id_none() }")
            && arena_rs.contains("let arena = unsafe { _mi_arena_from_id(arena_id) };")
            && heap_rs.contains("(*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);")
            && free_rs.contains("_mi_arena_memid_is_suitable("),
        "arena id conversion and memid suitability helpers should keep C's local call boundaries"
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
    assert!(
        theap_rs.contains("_mi_memcpy_aligned(\n            theap_raw.cast(),")
            && internal.contains("unsafe fn mi_cpu_movsb_max() -> usize")
            && internal.contains("\"mov {max}, qword ptr [rip + _mi_cpu_movsb_max]\"")
            && internal.contains("if n <= mi_cpu_movsb_max()")
            && internal.contains("\"rep movsb\"")
            && internal.contains("let movsb_max = mi_cpu_movsb_max();")
            && internal.contains("if n > movsb_max && n.is_multiple_of(size_of::<usize>())")
            && internal.contains("if n <= movsb_max")
            && internal.contains("\"rep movsq\"")
            && internal.contains("pub unsafe fn _mi_memcpy_aligned"),
        "_mi_theap_init should copy the empty template through C's _mi_memcpy_aligned path"
    );
    let theap_init_start = theap_rs
        .find("pub(crate) unsafe extern \"C\" fn _mi_theap_init")
        .expect("find _mi_theap_init");
    let theap_init_end = theap_rs[theap_init_start..]
        .find("pub(crate) unsafe extern \"C\" fn _mi_theap_random_next")
        .map(|offset| theap_init_start + offset)
        .expect("find _mi_theap_random_next after _mi_theap_init");
    let theap_init_body = &theap_rs[theap_init_start..theap_init_end];
    assert!(
        theap_init_body.contains(
            "(*theap_raw).cookie = _mi_random_next(ptr::addr_of_mut!((*theap_raw).random)) | 1;"
        ) && !theap_init_body.contains("_mi_theap_random_next(theap)"),
        "_mi_theap_init should match C's optimized artifact by calling _mi_random_next directly for the cookie"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize"
        ) && atomic_types.contains(
            "pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize"
        ) && atomic_types.contains("pub(crate) unsafe fn store_seq_cst(self, value: usize)")
            && theap_init_body.contains("theap.refcount_cell().store_seq_cst(1);")
            && theap_init_body.contains("theap.set_tld(Some(tld));")
            && theap_init_body.contains("theap.set_heap(Some(heap));")
            && !theap_init_body.contains(
                "mi_atomic_store_explicit(\n        ptr::addr_of_mut!((*theap).refcount),"
            )
            && !theap_init_body.contains("(*theap).refcount = 1;"),
        "_mi_theap_init should initialize C's thread-heap metadata through typed wrapper accessors"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicGuard<'a>")
            && atomic_types.contains(
                "pub(crate) unsafe fn try_acquire(field: AtomicUsizeField) -> Option<Self>"
            )
            && atomic_types.contains("impl Drop for AtomicGuard<'_>")
            && atomic_types.contains("self.field.store(0, Ordering::Release)")
            && atomic_rs.contains("_guard: crate::core_types::AtomicGuard<'static>")
            && atomic_rs.contains("crate::core_types::AtomicGuard::try_acquire(field)")
            && !atomic_rs.contains("guard: *mut mi_atomic_guard_t,\n}")
            && !atomic_rs.contains("mi_atomic_store_release(self.guard.cast::<usize>(), 0);"),
        "atomic guard scopes should acquire and release through typed AtomicGuard Drop"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>)"
        ) && atomic_types.contains("pub(crate) struct MiLockGuard<'a>")
            && atomic_types.contains("pub(crate) unsafe fn acquire(lock: MiLockPtr) -> Self")
            && atomic_types.contains("crate::atomic::mi_lock_acquire(lock.0.as_ptr())")
            && atomic_types.contains("impl Drop for MiLockGuard<'_>")
            && atomic_types.contains("crate::atomic::mi_lock_release(self.lock.0.as_ptr())")
            && ptr_rs.contains("pub(crate) unsafe fn arena_reserve_lock(self) -> MiLockPtr")
            && ptr_rs.contains("pub(crate) unsafe fn heaps_lock(self) -> MiLockPtr")
            && arena_rs.contains("MiLockGuard::acquire(subproc.arena_reserve_lock())")
            && init_rs.contains(
                "let lock = MiLockPtr(NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock)));"
            )
            && init_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && init_rs.contains(
                "let heaps_lock = unsafe { MiLockGuard::acquire(subproc.heaps_lock()) };"
            )
            && init_rs.contains("drop(heaps_lock);")
            && options_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && options_rs.contains(
                "Manual lock handling keeps this translated callback buffer path close to"
            )
            && options_rs
                .contains("Intentionally holds the output buffer lock while invoking `out`"),
        "subproc and options locks should acquire and release through typed MiLockGuard Drop"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T"
        ) && theap_rs.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of_mut!((*theap).heap))\n            .exchange_acq_rel(ptr::null_mut())"
        ) && !theap_rs.contains("mi_atomic_exchange_ptr_acq_rel"),
        "_mi_theap_free should clear the heap pointer through AtomicPtrField"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into"),
        "heap cleanup should keep C's stats merge call boundary instead of inlining it into _mi_theap_free"
    );
    assert!(
        stats_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt")
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt"
            ),
        "theap lifecycle should keep C's multithreaded stats update helper boundaries as crate-local calls"
    );
    assert!(
        stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt"
        ) && stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt"
        ) && stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt"
        ),
        "multithreaded stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_increase")
            && stats_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_decrease")
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease"
            )
            && stats_rs.contains("static MI_STAT_ADJUST_INCREASE_KEEP"),
        "thread-local stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        internal.contains("pub(crate) use crate::stats::{"),
        "internal stats helper re-exports should stay crate-local so hot call sites are direct local calls"
    );
}
