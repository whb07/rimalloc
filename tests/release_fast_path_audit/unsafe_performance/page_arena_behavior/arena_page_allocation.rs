use super::PageArenaBehaviorSources;

#[path = "arena_page_allocation/abi_wrappers.rs"]
mod abi_wrappers;
#[path = "arena_page_allocation/typed_allocation.rs"]
mod typed_allocation;

pub(super) fn assert_arena_page_allocation_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    typed_allocation::assert_typed_arena_allocation_helpers(sources);
    assert!(
        sources
            .arena_types
            .contains("#[repr(transparent)]\npub(crate) struct SharedArena(ArenaPtr);")
            && sources
                .arena_types
                .contains("#[repr(transparent)]\npub(crate) struct ExclusiveArena(ArenaPtr);")
            && sources
                .arena_types
                .contains("#[repr(transparent)]\npub(crate) struct ParentArena(ArenaPtr);")
            && !sources.arena_types.contains("SharedArena(pub(crate)")
            && !sources.arena_types.contains("ExclusiveArena(pub(crate)")
            && !sources.arena_types.contains("ParentArena(pub(crate)")
            && sources
                .arena_types
                .matches("SAFETY[AUDIT:typed-arena-domain]")
                .count()
                >= 7
            && sources.arena_types.contains("impl SharedArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self>")
            && sources.arena_types.contains(
                "before packaging the shared-arena witness.\n        if unsafe { arena.is_exclusive() } {\n            None"
            )
            && sources.arena_types.contains("impl ExclusiveArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self>")
            && sources.arena_types.contains(
                "before packaging the exclusive-arena witness.\n        if unsafe { arena.is_exclusive() } {\n            Some(Self(arena))"
            )
            && sources.arena_types.contains("impl ParentArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self>")
            && sources.arena_types.contains(
                "packaging a root parent-arena witness.\n        if unsafe { arena.parent().is_none() } {\n            Some(Self(arena))"
            ),
        "arena role wrappers should have checked constructors based on raw arena state"
    );
    assert!(
        sources.arena_rs.contains(
            "unsafe fn mi_arena_pages_size(\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arena_info_slices_needed(\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arena_bitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arena_bbitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arena_initialize(\n    subproc: SubprocPtr,\n    start: OsMemPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources
            .arena_rs
            .contains("let slice_count_usize = slice_count.get() as usize;")
            && sources
                .arena_os_memory_rs
                .contains("let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && sources.arena_os_memory_rs.contains("mi_arena_initialize(\n                SubprocPtr::new_unchecked(subproc),\n                OsMemPtr::new_unchecked(start),\n                typed_slice_count,")
            && !sources
                .arena_rs
                .contains("unsafe fn mi_arena_pages_size(slice_count: usize")
            && !sources
                .arena_rs
                .contains("unsafe fn mi_arena_info_slices_needed(slice_count: usize")
            && !sources
                .arena_rs
                .contains("unsafe fn mi_arena_bitmap_init(slice_count: usize")
            && !sources
                .arena_rs
                .contains("unsafe fn mi_arena_bbitmap_init(slice_count: usize"),
        "arena metadata sizing and initialization should receive typed SliceCount"
    );
    assert!(
        sources.arena_rs.contains(
            "unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources
            .arena_rs
            .contains("let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && sources
                .arena_rs
                .contains("let block_size_typed = unsafe { crate::core_types::BlockSize::new_unchecked(block_size) };")
            && sources.arena_rs.contains(
                "let block_alignment_typed =\n        unsafe { crate::core_types::MaybeAlignment::new_unchecked(block_alignment) };",
            )
            && sources
                .arena_rs
                .contains("const MI_SMALL_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && sources
                .arena_rs
                .contains("const MI_MEDIUM_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && sources
                .arena_rs
                .contains("const MI_LARGE_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && sources
                .arena_rs
                .contains("mi_arenas_page_regular_alloc(theap, MI_SMALL_PAGE_SLICE_COUNT, block_size_typed)")
            && sources
                .arena_rs
                .contains("mi_arenas_page_regular_alloc(theap, MI_MEDIUM_PAGE_SLICE_COUNT, block_size_typed)")
            && sources
                .arena_rs
                .contains("mi_arenas_page_regular_alloc(theap, MI_LARGE_PAGE_SLICE_COUNT, block_size_typed)")
            && !sources.arena_rs.contains("unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !sources.arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !sources.arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !sources.arena_rs.contains("unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: usize,"),
        "arena page allocation helpers should carry typed SliceCount after raw entry-point validation"
    );
    assert!(
        sources.arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc_aligned"
        ) && sources
            .arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc")
            && sources
                .arena_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_contain")
            && sources.arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all"
            )
            && sources.arena_rs.contains("mi_arenas_try_alloc(")
            && sources.arena_rs.contains("mi_arena_os_alloc_aligned(")
            && sources.arena_rs.contains("_mi_arenas_alloc_aligned(")
            && sources
                .arena_rs
                .contains("mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), None)")
            && sources.arena_rs.contains("static MI_ARENAS_CONTAIN_KEEP")
            && sources
                .arena_rs
                .contains("mi_arenas_unsafe_destroy(subproc)")
            && sources.theap_rs.contains("let theap = _mi_arenas_alloc(")
            && sources
                .init_rs
                .contains("_mi_arenas_unsafe_destroy_all(subproc)"),
        "arena allocation, containment, and unsafe destroy helpers should keep C's local call boundaries"
    );
    abi_wrappers::assert_arena_allocation_abi_wrappers(sources);
}
