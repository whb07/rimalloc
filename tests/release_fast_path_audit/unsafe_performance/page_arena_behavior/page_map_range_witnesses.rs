use super::PageArenaBehaviorSources;

pub(super) fn assert_page_map_range_witnesses(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources.page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    submap: *mut mi_submap_t,"
        ) && sources.page_map.contains(
            "unsafe fn mi_page_map_set_range_prim(\n    page: PagePtr,\n    mut idx: crate::core_types::PageMapIndex,\n    mut sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources
            .page_map
            .contains("unsafe fn mi_page_map_clear_range_prim(")
            && sources.page_map.contains(
                "unsafe fn mi_page_map_set_range(\n    page: PagePtr,\n    idx: crate::core_types::PageMapIndex,\n    sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
            )
            && sources
                .page_map
                .contains("unsafe fn mi_page_map_clear_range(")
            && sources
                .page_map
                .contains("unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool")
            && sources
                .page_map
                .contains("unsafe fn mi_page_map_unregister_typed(page: PagePtr)")
            && sources
                .page_map
                .contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && sources
                .page_map
                .contains("unsafe { mi_page_map_register_typed(page) }")
            && sources
                .page_map
                .contains("unsafe { mi_page_map_unregister_typed(page) };")
            && !sources
                .page_map
                .contains("unsafe fn mi_page_map_ensure_committed(idx: usize, slice_count: usize)")
            && !sources
                .page_map
                .contains("unsafe fn mi_page_map_ensure_committed(idx: usize, submap:")
            && !sources.page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    mut slice_count: usize,")
            && !sources.page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    slice_count: crate::core_types::SliceCount,")
            && !sources.page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: crate::core_types::PageMapIndex,")
            && !sources.page_map.contains("unsafe fn mi_page_map_set_range(\n    page: *mut mi_page_t,\n    idx: usize,\n    sub_idx: usize,\n    slice_count: usize,")
            && !sources
                .page_map
                .contains("fn mi_page_map_get_idx(\n    page: *mut mi_page_t,"),
        "page-map range helpers should validate raw indexes/counts once and carry typed indexes plus SliceCount internally"
    );
    assert!(
        sources
            .arena_rs
            .contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && sources.memid_rs.contains("slice_index: range.start_u32()")
            && sources.memid_rs.contains("slice_count: range.count_u32()")
            && sources
                .page_map
                .contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && !sources.arena_rs.contains(" as u32")
            && !sources.page_map.contains(" as u32")
            && !sources.memid_rs.contains(" as u32")
            && !sources
                .arena_rs
                .contains("SliceRange::from_raw(slice_index as")
            && !sources
                .arena_rs
                .contains("SliceRange::from_raw(slice_index, slice_count as")
            && !sources
                .page_map
                .contains("SliceCount::from_usize(slice_count as"),
        "arena and page-map slice fields should use checked typed conversions instead of lossy casts"
    );
    assert!(
        sources.page_types.contains("pub(crate) struct RegisteredPageRange {\n    page: PagePtr,\n    index: PageMapIndex,\n    sub_index: PageMapSubIndex,\n    slice_count: SliceCount,\n}")
            && sources.page_types.contains("pub(crate) unsafe fn new_unchecked(\n        page: PagePtr,\n        index: PageMapIndex,\n        sub_index: PageMapSubIndex,\n        slice_count: SliceCount,\n    ) -> Self")
            && sources
                .page_types
                .contains("pub(crate) fn page(self) -> PagePtr")
            && sources
                .page_types
                .contains("pub(crate) fn index(self) -> PageMapIndex")
            && sources
                .page_types
                .contains("pub(crate) fn sub_index(self) -> PageMapSubIndex")
            && sources
                .page_types
                .contains("pub(crate) fn slice_count(self) -> SliceCount")
            && sources.page_map.contains("crate::core_types::RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)\n    };")
            && sources
                .page_map
                .contains("crate::internal::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());")
            && sources
                .page_map
                .contains("crate::internal::mi_debug_assert!(registered.index() == idx);")
            && sources
                .page_map
                .contains("crate::internal::mi_debug_assert!(registered.sub_index() == sub_idx);")
            && sources
                .page_map
                .contains("crate::internal::mi_debug_assert!(registered.slice_count() == slice_count);")
            && sources
                .page_map
                .contains("Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })"),
        "RegisteredPageRange should be an opaque page-map registration witness over PagePtr, PageMapIndex, PageMapSubIndex, and SliceCount"
    );
}
