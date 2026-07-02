pub(super) struct HeapPageVisitorSources<'a> {
    pub(super) arena_heap_pages_rs: &'a str,
    pub(super) heap_rs: &'a str,
}

pub(super) fn assert_heap_page_visitor_boundaries(sources: HeapPageVisitorSources<'_>) {
    let HeapPageVisitorSources {
        arena_heap_pages_rs,
        heap_rs,
    } = sources;

    let split_marker_minimums = [
        ("arena-heap-visit-callback-boundary", 5),
        ("arena-heap-block-visit-boundary", 6),
        ("arena-heap-page-iteration-boundary", 1),
        ("arena-heap-abandoned-visit-boundary", 5),
        ("arena-heap-delete-boundary", 8),
        ("arena-heap-transfer-boundary", 12),
        ("arena-heap-delete-dispatch-boundary", 9),
        ("arena-heap-move-boundary", 5),
        ("arena-heap-destroy-boundary", 5),
    ];
    assert!(
        !arena_heap_pages_rs.contains("SAFETY[AUDIT:arena-heap-page-visitors]"),
        "arena heap page visitors should stay split instead of reusing the old broad marker"
    );
    for (audit_id, minimum) in split_marker_minimums {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = arena_heap_pages_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "arena heap page visitor boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        arena_heap_pages_rs.contains("struct mi_heap_visit_info_t")
            && arena_heap_pages_rs.contains("struct mi_heap_delete_visit_info_t")
            && arena_heap_pages_rs.contains("visitor: BlockVisitCallback,")
            && arena_heap_pages_rs.contains("arg: BlockVisitCallbackArg,")
            && arena_heap_pages_rs.contains("BlockVisitCallback::new(Some(visitor))")
            && arena_heap_pages_rs.contains("BlockVisitCallbackArg::from_raw(arg)")
            && arena_heap_pages_rs
                .contains("unsafe fn mi_heap_visit_page_at_index(\n    slice_index: SliceIndex,")
            && arena_heap_pages_rs
                .contains("mi_heap_visit_page_at_index(SliceIndex::new(slice_index), arena, arg)",)
            && arena_heap_pages_rs
                .contains("mi_arena_page_at_slice(ArenaPtr::new_unchecked(arena), slice_index)"),
        "heap page visitor callbacks should stay in the split module and parse raw slice indexes once"
    );
    assert!(
        arena_heap_pages_rs.contains("let Some(typed_bin) = crate::core_types::Bin::new(bin as u16) else")
            && arena_heap_pages_rs.contains("crate::internal::mi_assert_internal!(false);\n                                break;")
            && arena_heap_pages_rs.contains("mi_heap_abandoned_count(heap, typed_bin).load_relaxed() > 0")
            && arena_heap_pages_rs.contains("mi_heap_abandoned_count(heap, bin).load_relaxed() == 0")
            && arena_heap_pages_rs
                .matches("mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire()")
                .count()
                == 1,
        "heap page visitor debug and abandoned-page reads should use typed Bin and ArenaIndex helpers"
    );
    assert!(
        arena_heap_pages_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_move_pages")
            && arena_heap_pages_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_destroy_pages",
            )
            && heap_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_force_destroy",
            )
            && heap_rs.contains("_mi_heap_move_pages(heap_raw, mi_heap_main());")
            && heap_rs.contains("_mi_heap_destroy_pages(heap.as_ptr());")
            && heap_rs.contains("unsafe { mi_heap_force_destroy_owned(heap) };"),
        "heap delete/destroy should keep C's heap page movement and force-destroy helper boundaries"
    );
}
