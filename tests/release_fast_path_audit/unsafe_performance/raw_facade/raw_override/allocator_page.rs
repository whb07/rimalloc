use super::*;

pub(super) fn assert_allocator_page_no_exports(sources: &UnsafePerformanceSources) {
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let arena_ids_rs = sources.arena_ids_rs.as_str();
    let arena_purge_rs = sources.arena_purge_rs.as_str();
    let arena_meta_rs = sources.arena_meta_rs.as_str();
    let deferred_rs = sources.deferred_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let page_rs = sources.page_rs.as_str();

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
            "src/allocator/arena/purge.rs",
            arena_purge_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_collect",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_contain",
        ),
        (
            "src/allocator/arena/purge.rs",
            arena_purge_rs,
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
    ] {
        assert_internal_no_export(source_name, source, internal_name);
    }
}
