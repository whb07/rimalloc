pub(super) fn assert_arena_helper_boundaries(
    arena_rs: &str,
    arena_debug_rs: &str,
    arena_types: &str,
) {
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
        ) && arena_rs.contains("unsafe fn mi_arena_try_claim_abandoned_at(\n    slice_index: SliceIndex,")
            && arena_debug_rs.contains("    slice_index: SliceIndex,\n    pbit_of_page:")
            && arena_rs.contains(
                "mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned)"
            )
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
}
