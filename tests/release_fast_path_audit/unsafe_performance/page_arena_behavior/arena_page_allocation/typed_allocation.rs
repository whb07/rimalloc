use super::PageArenaBehaviorSources;

pub(super) fn assert_typed_arena_allocation_helpers(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources.arena_rs.contains(
            "unsafe fn mi_arena_try_alloc_at(\n    arena: ArenaPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && sources
            .arena_rs
            .contains(") -> Option<(*mut c_void, crate::core_types::ArenaAllocation)>")
            && sources.arena_types.contains(
                "pub(crate) struct ArenaAllocation {\n    arena: ArenaPtr,\n    range: SliceRange,\n    memid: crate::internal::mi_memid_t,\n}"
            )
            && sources.arena_types.contains("impl ArenaAllocation")
            && sources.arena_types.contains("pub(crate) const fn new(")
            && sources.arena_types.contains("pub(crate) fn arena(self) -> ArenaPtr")
            && sources.arena_types.contains("pub(crate) fn range(self) -> SliceRange")
            && sources
                .arena_types
                .contains("pub(crate) fn into_memid(self) -> crate::internal::mi_memid_t")
            && !sources.arena_types.contains("pub(crate) arena: ArenaPtr")
            && !sources.arena_types.contains("pub(crate) range: SliceRange")
            && !sources
                .arena_types
                .contains("pub(crate) memid: crate::internal::mi_memid_t")
            && sources
                .arena_rs
                .contains("let mut allocation = crate::core_types::ArenaAllocation::new(")
            && sources
                .arena_rs
                .contains("unsafe { *memid = allocation.assume_init().into_memid() };")
            && !sources.arena_rs.contains("allocation.range.start_usize()")
            && !sources.arena_rs.contains("allocation.memid.initially_committed")
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
}
