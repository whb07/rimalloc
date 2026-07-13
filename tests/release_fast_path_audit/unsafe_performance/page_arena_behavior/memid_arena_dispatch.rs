use super::PageArenaBehaviorSources;

#[path = "memid_arena_dispatch/memid_view.rs"]
mod memid_view;

pub(super) fn assert_memid_and_arena_dispatch_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    memid_view::assert_memid_view_dispatch(sources);
    assert!(
        sources.arena_ids_rs.contains(
            "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> mi_memid_t"
        ) && sources
            .arena_rs
            .contains("let mut allocation = crate::core_types::ArenaAllocation::new(")
            && sources
                .arena_rs
                .contains("unsafe { mi_memid_create_arena(arena, range) }")
            && sources
                .arena_ids_rs
                .contains("let Some((arena, range)) = (unsafe { mi_arena_range_from_memid(memid) }) else")
            && sources
                .arena_ids_rs
                .contains("unsafe { *slice_index = range.start_usize() };")
            && sources
                .arena_ids_rs
                .contains("unsafe { *slice_count = range.count_usize() };")
            && sources
                .scalar_types
                .contains("pub(crate) fn as_u32(self) -> Option<u32>")
            && sources
                .scalar_types
                .contains("pub(crate) const fn from_usize_const(count: usize) -> Option<Self>")
            && sources.arena_types.contains("pub(crate) fn start_u32(self) -> u32")
            && sources
                .arena_types
                .contains("pub(crate) const fn count_u32(self) -> u32")
            && sources.memid_rs.contains("slice_index: range.start_u32()")
            && sources.memid_rs.contains("slice_count: range.count_u32()")
            && sources
                .arena_rs
                .contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && !sources
                .arena_rs
                .contains("mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) as u32")
            && !sources
                .arena_rs
                .contains("mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) as u32")
            && !sources
                .arena_rs
                .contains("mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) as u32")
            && !sources.memid_rs.contains("range.start().0 as u32")
            && !sources.arena_ids_rs.contains(
                "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    slice_index: usize,\n    slice_count: usize,"
            )
            && !sources.arena_ids_rs.contains(
                "SliceRange::from_raw(slice_index as u32, slice_count as u32)"
            ),
        "arena memid construction should receive an already-typed SliceRange"
    );

    let reabandon_arena = crate::source_item(
        "src/allocator/arena.rs",
        sources.arena_rs,
        "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena",
    );
    assert!(
        sources.arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped"
        )
            && sources.arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(\n    page: crate::core_types::OwnedAbandonedPage,\n    _arena_memid: crate::core_types::ArenaMemId,"
            )
            && sources
                .arena_ids_rs
                .contains("unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool")
            && sources
                .arena_ids_rs
                .contains("crate::core_types::MemId::view(unsafe { (*page).memid })")
            && sources
                .arena_rs
                .contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !sources.arena_rs.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !sources.arena_rs.contains("(*page).memid.memkind != mi_memkind_t::MI_MEM_ARENA")
            && sources.arena_rs.contains("_mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page_raw))")
            && sources.arena_rs.contains("_mi_arenas_page_abandon(page_raw, theap)")
            && !reabandon_arena.contains("mi_page_memid_is_arena(page_raw)")
            && !reabandon_arena.contains("MemId::view")
            && sources.free_rs.contains("crate::core_types::MemId::view((*page_raw).memid)")
            && sources
                .free_rs
                .contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !sources.free_rs.contains(
                "(*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA"
            )
            && sources
                .free_rs
                .contains("_mi_arenas_page_try_reabandon_to_mapped_arena(")
            && !sources
                .free_rs
                .contains("_mi_arenas_page_try_reabandon_to_mapped(page)"),
        "free path should validate arena memids and carry the witness into the crate-local reabandon helper"
    );
    assert!(
        sources
            .arena_ids_rs
            .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_id_none")
            && sources
                .arena_ids_rs
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_from_id")
            && sources
                .arena_ids_rs
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_memid_is_suitable")
            && sources
                .arena_ids_rs
                .contains("static MI_ARENA_ID_NONE_KEEP")
            && sources
                .arena_ids_rs
                .contains("static MI_ARENA_FROM_ID_KEEP")
            && sources
                .arena_ids_rs
                .contains("unsafe { _mi_arena_id_none() }")
            && sources
                .arena_rs
                .contains("let arena = unsafe { _mi_arena_from_id(arena_id) };")
            && sources
                .heap_rs
                .contains("(*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);")
            && sources.free_rs.contains("_mi_arena_memid_is_suitable("),
        "arena id conversion and memid suitability helpers should keep C's local call boundaries"
    );
}
