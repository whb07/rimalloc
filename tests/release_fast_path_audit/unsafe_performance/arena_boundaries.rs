#[path = "arena_boundaries/marker_inventory.rs"]
mod marker_inventory;

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

    marker_inventory::assert_arena_marker_inventory(&arena_region, unsafe_allowlist);

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
            && arena_rs.contains("unsafe { PagePtr::new_unchecked(page).thread_id_cell() };")
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
