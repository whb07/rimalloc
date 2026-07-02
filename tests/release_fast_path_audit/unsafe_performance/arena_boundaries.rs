pub(super) struct BoundarySources<'a> {
    pub(super) arena_rs: &'a str,
    pub(super) arena_os_memory_rs: &'a str,
    pub(super) arena_purge_rs: &'a str,
    pub(super) unsafe_allowlist: &'a str,
}

pub(super) fn assert_arena_region_boundaries(sources: BoundarySources<'_>) {
    let BoundarySources {
        arena_rs,
        arena_os_memory_rs,
        arena_purge_rs,
        unsafe_allowlist,
    } = sources;
    let arena_region = format!("{arena_rs}\n{arena_os_memory_rs}\n{arena_purge_rs}");
    let arena_allocation_region = format!("{arena_rs}\n{arena_os_memory_rs}");

    let focused_markers = [
        "SAFETY[AUDIT:arena-slice-geometry-boundary]",
        "SAFETY[AUDIT:arena-slice-containment-boundary]",
        "SAFETY[AUDIT:arena-slice-layout-boundary]",
        "SAFETY[AUDIT:arena-registry-count-index-boundary]",
        "SAFETY[AUDIT:arena-heap-page-table-boundary]",
        "SAFETY[AUDIT:arena-registry-start-index-boundary]",
        "SAFETY[AUDIT:arena-registry-suitability-boundary]",
        "SAFETY[AUDIT:arena-registry-iteration-boundary]",
        "SAFETY[AUDIT:arena-registry-destroy-boundary]",
        "SAFETY[AUDIT:arena-registry-add-boundary]",
        "SAFETY[AUDIT:arena-slice-commit-boundary]",
        "SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]",
        "SAFETY[AUDIT:arena-slice-commit-accounting-boundary]",
        "SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]",
        "SAFETY[AUDIT:arena-initialization-boundary]",
        "SAFETY[AUDIT:arena-reserve-growth-boundary]",
        "SAFETY[AUDIT:arena-os-fallback-boundary]",
        "SAFETY[AUDIT:arena-public-reserve-boundary]",
        "SAFETY[AUDIT:arena-huge-reservation-boundary]",
        "SAFETY[AUDIT:arena-abandoned-boundary]",
        "SAFETY[AUDIT:arena-abandoned-page-table-boundary]",
        "SAFETY[AUDIT:arena-page-fresh-boundary]",
        "SAFETY[AUDIT:arena-page-fresh-metadata-boundary]",
        "SAFETY[AUDIT:arena-page-regular-boundary]",
        "SAFETY[AUDIT:arena-page-singleton-boundary]",
        "SAFETY[AUDIT:arena-page-free-entry-boundary]",
        "SAFETY[AUDIT:arena-page-free-validation-boundary]",
        "SAFETY[AUDIT:arena-page-free-stats-boundary]",
        "SAFETY[AUDIT:arena-page-free-release-boundary]",
        "SAFETY[AUDIT:arena-page-abandon-boundary]",
        "SAFETY[AUDIT:arena-page-reabandon-boundary]",
        "SAFETY[AUDIT:arena-page-unabandon-entry-boundary]",
        "SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]",
        "SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]",
        "SAFETY[AUDIT:arena-heap-page-publication-boundary]",
        "SAFETY[AUDIT:arena-page-ownership-transition-boundary]",
        "SAFETY[AUDIT:arena-purge-schedule-boundary]",
        "SAFETY[AUDIT:arena-purge-free-dispatch-boundary]",
        "SAFETY[AUDIT:arena-purge-direct-boundary]",
        "SAFETY[AUDIT:arena-purge-range-boundary]",
        "SAFETY[AUDIT:arena-purge-visitor-boundary]",
        "SAFETY[AUDIT:arena-purge-expire-policy-boundary]",
        "SAFETY[AUDIT:arena-purge-collect-guard-boundary]",
    ];
    let marker_total = focused_markers
        .into_iter()
        .map(|marker| arena_region.matches(marker).count())
        .sum::<usize>();
    for (audit_id, min_count) in [
        ("arena-slice-geometry-boundary", 11),
        ("arena-slice-containment-boundary", 13),
        ("arena-slice-layout-boundary", 7),
        ("arena-registry-count-index-boundary", 6),
        ("arena-heap-page-table-boundary", 5),
        ("arena-registry-start-index-boundary", 3),
        ("arena-registry-suitability-boundary", 3),
        ("arena-registry-iteration-boundary", 4),
        ("arena-registry-destroy-boundary", 6),
        ("arena-registry-add-boundary", 10),
        ("arena-slice-commit-boundary", 3),
        ("arena-slice-allocation-bitmaps-boundary", 7),
        ("arena-slice-commit-accounting-boundary", 14),
        ("arena-slice-allocation-dispatch-boundary", 6),
        ("arena-initialization-boundary", 17),
        ("arena-reserve-growth-boundary", 11),
        ("arena-os-fallback-boundary", 15),
        ("arena-public-reserve-boundary", 18),
        ("arena-huge-reservation-boundary", 14),
        ("arena-abandoned-boundary", 16),
        ("arena-abandoned-page-table-boundary", 6),
        ("arena-page-fresh-boundary", 8),
        ("arena-page-fresh-metadata-boundary", 14),
        ("arena-page-regular-boundary", 11),
        ("arena-page-singleton-boundary", 2),
        ("arena-page-free-entry-boundary", 4),
        ("arena-page-free-validation-boundary", 9),
        ("arena-page-free-stats-boundary", 4),
        ("arena-page-free-release-boundary", 6),
        ("arena-page-abandon-boundary", 17),
        ("arena-page-reabandon-boundary", 8),
        ("arena-page-unabandon-entry-boundary", 6),
        ("arena-page-unabandon-mapped-boundary", 5),
        ("arena-page-unabandon-os-list-boundary", 4),
        ("arena-heap-page-publication-boundary", 3),
        ("arena-page-ownership-transition-boundary", 6),
        ("arena-purge-schedule-boundary", 6),
        ("arena-purge-free-dispatch-boundary", 6),
        ("arena-purge-direct-boundary", 5),
        ("arena-purge-range-boundary", 5),
        ("arena-purge-visitor-boundary", 3),
        ("arena-purge-expire-policy-boundary", 5),
        ("arena-purge-collect-guard-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            arena_region.matches(&marker).count() >= min_count,
            "arena unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/arena"))
            && !arena_region.contains("SAFETY[AUDIT:arena-region-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-allocation-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-page-lifecycle-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-registry-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-purge-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-slice-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-slice-allocation-boundary]")
            && marker_total >= 340
            && focused_markers
                .into_iter()
                .all(|marker| arena_region.contains(marker)),
        "arena unsafe boundaries should stay split into focused operation markers without a migration allowlist entry"
    );

    assert!(
        arena_rs.contains("unsafe fn mi_arena_slice_start(")
            && arena_rs.contains("unsafe fn mi_arena_page_at_slice(")
            && arena_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_contain")
            && arena_rs.contains("SliceRange::new(0, slice_count.get() as usize, arena)")
            && arena_rs.contains("ArenaIndex::from_usize")
            && arena_rs.contains("unsafe fn mi_arenas_add(")
            && arena_rs.contains("pub unsafe extern \"C\" fn mi_arena_from_index")
            && arena_rs.contains("unsafe fn mi_heap_arena_pages(")
            && arena_rs.contains("unsafe fn mi_forall_arenas<F>")
            && arena_rs.contains("mi_subproc_arena_cell(subproc, arena_idx)")
            && arena_rs.contains("crate::core_types::NumaNode::new"),
        "arena slice projection and registry/subproc table boundaries should stay typed and index-checked"
    );

    assert!(
        arena_allocation_region.contains("unsafe fn mi_arena_try_alloc_at(")
            && arena_allocation_region.contains("unsafe fn mi_arenas_try_alloc(")
            && arena_os_memory_rs.contains("pub(super) unsafe fn mi_arena_reserve(")
            && arena_allocation_region
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_alloc_aligned")
            && arena_allocation_region
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_alloc")
            && arena_os_memory_rs.contains("unsafe fn mi_reserve_os_memory_ex2(")
            && arena_os_memory_rs
                .contains("pub unsafe extern \"C\" fn mi_reserve_huge_os_pages_at_ex")
            && arena_allocation_region.contains("mi_memid_create_arena(arena, range)")
            && arena_allocation_region.contains("mi_bbitmap_try_find_and_clearN(")
            && arena_allocation_region.contains("mi_bitmap_setN(")
            && arena_allocation_region.contains("_mi_os_commit(")
            && arena_allocation_region.contains("_mi_os_alloc_aligned(")
            && arena_allocation_region
                .contains("MiLockGuard::acquire(subproc.arena_reserve_lock())"),
        "arena allocation and OS reservation boundaries should preserve memid, bitmap, commit, and lock invariants"
    );

    assert!(
        arena_rs.contains("unsafe fn mi_abandoned_page_unown(")
            && arena_rs.contains("unsafe fn mi_arena_try_claim_abandoned_at(")
            && arena_rs.contains("unsafe extern \"C\" fn mi_arena_try_claim_abandoned")
            && arena_rs.contains("unsafe fn mi_heap_ensure_arena_pages(")
            && arena_rs.contains("unsafe fn mi_arenas_page_try_find_abandoned(")
            && arena_rs.contains("ThreadLocalClaimCallback::new(mi_arena_try_claim_abandoned)")
            && arena_rs.contains("crate::core_types::AbandonedPage::new_unchecked")
            && arena_rs.contains("mi_page_claim_ownership(abandoned)")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).load_relaxed()"),
        "arena abandoned-page boundaries should preserve claim/reclaim ownership and callback ABI shape"
    );

    assert!(
        arena_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_page_alloc")
            && arena_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_page_free")
            && arena_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_page_abandon")
            && arena_rs.contains(
                "pub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped",
            )
            && arena_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_page_unabandon")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1)")
            && arena_rs.contains("mi_heap_arena_page_cell(heap, arena_idx)")
            && arena_rs.contains(".store_release(arena_pages.cast())")
            && arena_rs.contains("page.free_to_arena(owner)")
            && arena_rs.contains("crate::core_types::AtomicPageThreadId")
            && arena_rs.contains("crate::core_types::PageOwnerId::AbandonedMapped")
            && arena_rs.contains("mi_bitmap_set((*arena_pages).pages_abandoned[bin.as_usize()]"),
        "arena page lifecycle handoff should preserve page state, memid ownership, and C ABI shims"
    );

    assert!(
        arena_purge_rs.contains("unsafe fn mi_arena_schedule_purge(")
            && arena_purge_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_free")
            && arena_purge_rs.contains("unsafe fn mi_arena_purge(")
            && arena_purge_rs.contains("unsafe fn mi_arena_try_purge_range(")
            && arena_purge_rs.contains("unsafe extern \"C\" fn mi_arena_try_purge_visitor")
            && arena_purge_rs.contains("unsafe fn mi_arenas_try_purge(")
            && arena_purge_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arenas_collect")
            && arena_purge_rs.contains("_mi_bitmap_forall_setc_rangesn(")
            && arena_purge_rs.contains("purge_expire")
            && arena_purge_rs.contains("_mi_os_purge_ex("),
        "arena purge and collection boundaries should preserve purge bitmap, timing, stats, and OS decommit behavior"
    );
}
