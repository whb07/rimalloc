use super::PageArenaBehaviorSources;

pub(super) fn assert_arena_page_allocation_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources.arena_rs.contains(
            "unsafe fn mi_arena_try_alloc_at(\n    arena: ArenaPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources
            .arena_rs
            .contains(") -> Option<(*mut c_void, crate::core_types::ArenaAllocation)>")
            && sources.arena_types.contains(
                "pub(crate) struct ArenaAllocation {\n    pub(crate) arena: ArenaPtr,\n    pub(crate) range: SliceRange,\n    pub(crate) memid: crate::internal::mi_memid_t,\n}"
            )
            && sources
                .arena_rs
                .contains("let mut allocation = crate::core_types::ArenaAllocation {")
            && sources
                .arena_rs
                .contains("unsafe { *memid = allocation.assume_init().memid };")
            && sources
                .arena_rs
                .contains("mi_arena_try_alloc_at(arena, slice_count, commit, tseq)")
            && !sources.arena_rs.contains(
                "unsafe fn mi_arena_try_alloc_at(\n    arena: *mut mi_arena_t,\n    slice_count: crate::core_types::SliceCount,"
            )
            && !sources
                .arena_rs
                .contains("mi_arena_try_alloc_at(arena, slice_count.get() as usize"),
        "arena allocation helper should accept typed SliceCount and return ArenaAllocation"
    );
    assert!(
        sources.arena_rs.contains(
            "unsafe fn mi_arenas_try_alloc(\n    heap: BorrowedHeap,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources
            .arena_rs
            .contains("    alignment: crate::core_types::Alignment,")
            && sources
                .arena_rs
                .contains("    req_arena: Option<ArenaPtr>,")
            && sources
                .arena_rs
                .contains("    numa_node: crate::core_types::NumaNode,")
            && sources
                .arena_rs
                .contains("let Some(subproc) = (unsafe { heap.ptr().subproc() }) else")
            && sources.arena_rs.contains("heap: HeapPtr::new(heap),")
            && sources
                .arena_rs
                .contains("crate::core_types::NumaNode::new(numa_node),")
            && sources
                .arena_rs
                .contains("req_arena: ArenaPtr::new(req_arena),")
            && sources
                .arena_rs
                .contains("let Some(heap_ptr) = HeapPtr::new(heap) else")
            && sources
                .arena_rs
                .contains("BorrowedHeap::new_unchecked(heap_ptr),")
            && !sources.arena_rs.contains(
                "unsafe fn mi_arenas_try_alloc(\n    heap: *mut mi_heap_t,"
            ),
        "arena try-alloc should receive typed heap, requested arena, slice count, and NUMA node below raw entry points"
    );
    assert!(
        sources
            .arena_types
            .contains("#[repr(transparent)]\npub(crate) struct SharedArena(pub(crate) ArenaPtr);")
            && sources
                .arena_types
                .contains("#[repr(transparent)]\npub(crate) struct ExclusiveArena(pub(crate) ArenaPtr);")
            && sources
                .arena_types
                .contains("#[repr(transparent)]\npub(crate) struct ParentArena(pub(crate) ArenaPtr);")
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
                .arena_rs
                .contains("let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && sources.arena_rs.contains("mi_arena_initialize(\n                SubprocPtr::new_unchecked(subproc),\n                OsMemPtr::new_unchecked(start),\n                typed_slice_count,")
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
    assert!(
        sources.arena_rs.contains("struct ArenaAllocTypedArgs {\n    heap: Option<HeapPtr>,\n    layout: crate::core_types::LayoutRequest,\n    align_offset: crate::core_types::AlignmentOffset,")
            && sources
                .arena_rs
                .contains("unsafe fn mi_arenas_alloc_aligned_typed(args: ArenaAllocTypedArgs)")
            && sources.arena_rs.contains("let size = layout.requested();")
            && sources.arena_rs.contains("let alignment = layout.alignment();")
            && sources
                .arena_rs
                .contains("arena_slice_count: Option<crate::core_types::SliceCount>,")
            && sources.arena_rs.contains("fn mi_arena_slice_count_for_request(\n    size: crate::core_types::RequestedSize,")
            && sources
                .arena_rs
                .contains("crate::core_types::SliceCount::from_usize(mi_slice_count_of_size(size.get()))")
            && sources
                .arena_rs
                .contains("arena_slice_count: mi_arena_slice_count_for_request(layout.requested())")
            && sources
                .arena_rs
                .contains("crate::core_types::LayoutRequest::aligned_at(")
            && sources
                .arena_rs
                .contains("crate::core_types::AlignmentOffset::new(align_offset)")
            && sources.arena_rs.contains("struct ArenaAllocArgs {\n    heap: Option<HeapPtr>,\n    size: crate::core_types::RequestedSize,")
            && sources
                .arena_rs
                .contains("unsafe fn mi_arenas_alloc_typed(args: ArenaAllocArgs)")
            && sources
                .arena_rs
                .contains("crate::core_types::LayoutRequest::aligned(size.get(), alignment)")
            && sources
                .arena_rs
                .contains("let size = crate::core_types::RequestedSize::new(size);")
            && sources
                .arena_rs
                .contains("crate::core_types::AlignmentOffset::new(0)")
            && sources
                .arena_rs
                .contains("req_arena: ArenaPtr::new(req_arena)")
            && sources
                .arena_rs
                .contains("tseq: crate::core_types::ThreadSeq::new(tseq)")
            && sources
                .arena_rs
                .contains("numa_node: crate::core_types::NumaNode::new(numa_node)"),
        "arena allocation ABI wrappers should parse raw size/alignment into typed layout domains before using arena internals"
    );
}
