use super::PageArenaBehaviorSources;

pub(super) fn assert_arena_allocation_abi_wrappers(sources: &PageArenaBehaviorSources<'_>) {
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
