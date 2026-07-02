use super::*;

#[path = "thread_tls/page_reclaim.rs"]
mod page_reclaim;
#[path = "thread_tls/platform_tls.rs"]
mod platform_tls;

pub(super) fn assert_thread_tls_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let alloc_cxx_new_rs = sources.alloc_cxx_new_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let callbacks_rs = sources.callbacks_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let internal = sources.internal.as_str();
    let internal_page_flags_rs = sources.internal_page_flags_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_flags_rs = sources.page_flags_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();

    platform_tls::assert_platform_tls_boundaries(sources);
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_thread_id(")
            && init_rs.contains("static MI_THREAD_ID_KEEP"),
        "_mi_thread_id should keep C's local helper boundary"
    );
    assert!(
        alloc_rs.contains(
            "#[inline(always)]\npub(crate) unsafe extern \"C\" fn _mi_theap_malloc_zero_ex"
        ) && alloc_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_malloc_zero")
            && alloc_rs.contains("static MI_THEAP_MALLOC_ZERO_EX_KEEP")
            && alloc_rs.contains("static MI_THEAP_MALLOC_ZERO_KEEP"),
        "_mi_theap_malloc_zero_ex should keep C's force-inline body and both allocation helpers should retain local definitions"
    );
    assert!(
        alloc_cxx_new_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn mi_theap_try_new")
            && alloc_cxx_new_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_try_new")
            && alloc_cxx_new_rs
                .contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_try_new")
            && alloc_cxx_new_rs.contains("mi_heap_try_new_typed(heap, size, nothrow)")
            && alloc_cxx_new_rs.contains("mi_try_new(size, true)")
            && alloc_cxx_new_rs.contains("static MI_THEAP_TRY_NEW_KEEP")
            && alloc_cxx_new_rs.contains("static MI_TRY_NEW_KEEP")
            && alloc_cxx_new_rs.contains("static MI_HEAP_TRY_NEW_KEEP"),
        "C++ new retry paths should keep C's separate theap/default-heap/explicit-heap helper boundaries"
    );
    page_reclaim::assert_page_reclaim_atomic_paths(
        page_rs,
        atomic_types,
        internal,
        internal_page_flags_rs,
        page_flags_rs,
        arena_rs,
        free_rs,
    );
    assert!(
        page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_page_free_list_extend")
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_page_extend_free")
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_page_fresh_alloc")
            && page_rs.contains("static MI_PAGE_FREE_LIST_EXTEND_KEEP")
            && page_rs.contains("static MI_PAGE_EXTEND_FREE_KEEP")
            && page_rs.contains("static MI_PAGE_FRESH_ALLOC_KEEP")
            && page_rs.contains("mi_page_free_list_extend(page, bsize, extend);")
            && page_rs.contains("if !mi_page_extend_free(theap, page)")
            && page_rs.contains("let block_size = mi_page_queue_block_size_raw(pq);")
            && page_rs.contains("let page = mi_page_fresh_alloc(theap, pq, block_size, 0);")
            && page_rs.contains(
                "crate::internal::mi_assert_internal!(block_size == mi_page_block_size(page));"
            )
            && page_rs
                .contains("let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);"),
        "fresh-page allocation and page free-list extension should keep C's local helper boundaries"
    );
    assert!(
    free_rs.contains(
        "unsafe fn mi_abandoned_page_try_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,"
    )
        && free_rs.contains(
            "#[inline(never)]\npub unsafe extern \"C\" fn mi_page_usable_aligned_size_of"
        )
        && free_rs.contains("static MI_ABANDONED_PAGE_TRY_RECLAIM_KEEP")
        && free_rs.contains("static MI_PAGE_USABLE_ALIGNED_SIZE_OF_KEEP")
        && free_rs.contains("&& mi_abandoned_page_try_reclaim(page, reclaim_on_free)")
        && free_rs.contains("_mi_theap_page_reclaim(\n            page,")
        && free_rs.contains("crate::core_types::OwnedAbandonedPage::new_unchecked")
        && free_rs.contains("unsafe fn mi_free_try_collect_mt_typed")
        && !free_rs.contains("_mi_theap_page_reclaim(theap, page)")
        && free_rs.contains("mi_page_usable_aligned_size_of_typed(page, p)"),
    "abandoned-page reclaim should route internal abandoned-page collection through typed page/heap witnesses"
);
    assert!(
    theap_rs.contains(
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_area_visit_blocks"
    ) && theap_rs.contains("static MI_THEAP_AREA_VISIT_BLOCKS_KEEP")
        && theap_rs.contains("visitor: BlockVisitCallback")
        && theap_rs.contains("arg: BlockVisitCallbackArg")
        && theap_rs.contains("let heap = HeapPtr::new_unchecked(mi_page_heap(page));")
        && callbacks_rs
            .contains("pub(crate) unsafe fn invoke(self, heap: HeapPtr, arg: HeapVisitCallbackArg) -> bool")
        && callbacks_rs.contains("heap: HeapPtr,\n        area: *const mi_heap_area_t,")
        && callbacks_rs.contains("fun(heap.as_ptr(), area, block, block_size, arg.as_ptr())")
        && init_rs.contains("ok = visitor.invoke(HeapPtr::new_unchecked(heap), arg);")
        && theap_rs.contains("mi_get_fast_divisor(bsize, &mut magic, &mut shift);")
        && arena_heap_pages_rs.contains(
            "_mi_theap_area_visit_blocks(ptr::addr_of!(area), page, (*vinfo).visitor, (*vinfo).arg)"
        )
        && !arena_heap_pages_rs.contains("unsafe fn mi_theap_area_visit_blocks("),
    "heap and arena block visitation should share C's theap area block visitor helper"
);
    assert!(
    theap_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_area_visitor")
        && theap_rs.contains(
            "static MI_THEAP_AREA_VISITOR_KEEP: mi_theap_area_visit_fun = mi_theap_area_visitor;"
        )
        && theap_rs.contains("Some(mi_theap_area_visitor)"),
    "mi_theap_area_visitor should keep C's local theap area visitor callback boundary"
);
    assert!(
        arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_alloc")
            && arena_rs.contains("static MI_ARENAS_PAGE_ALLOC_KEEP")
            && page_rs.contains(
                "let page = _mi_arenas_page_alloc(theap, block_size, page_alignment.get());",
            ),
        "arena page allocation should keep C's _mi_arenas_page_alloc local helper boundary"
    );
    assert!(
    stats_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_print_visitor")
        && stats_rs
            .contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_aggregate_visitor")
        && stats_rs.contains("static MI_HEAP_PRINT_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_print_visitor;")
        && stats_rs.contains(
            "static MI_HEAP_AGGREGATE_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_aggregate_visitor;"
        )
        && stats_rs.contains("Some(mi_heap_print_visitor as mi_heap_visit_fun)")
        && stats_rs.contains("Some(mi_heap_aggregate_visitor as mi_heap_visit_fun)"),
    "heap stats visitors should keep C's local callback helper boundaries"
);
    assert!(
    theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_incref")
        && theap_rs.contains("unsafe fn _mi_theap_incref_typed(theap: ThreadHeapPtr)")
        && theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_decref")
        && theap_rs.contains("#[inline(always)]\nunsafe fn mi_theap_decref_inline(theap: ThreadHeapPtr)")
        && theap_rs.contains("mi_theap_decref_inline(ThreadHeapPtr::new_unchecked(theap));\n        true")
        && theap_rs.contains("unsafe fn _mi_theap_delete_typed(theap: ThreadHeapPtr, acquire_tld_theaps_lock: bool)")
        && theap_rs.contains("match theap.state()")
        && theap_rs.contains("ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap)")
        && theap_rs.contains("ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return")
        && !theap_rs.contains("    _mi_theap_decref(theap);\n    true"),
    "_mi_theap_free should inline C's decref tail while incref/decref/delete use typed thread-heap wrappers below their raw shims"
);
    assert!(
        theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_random_next")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_delete")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_create")
            && !theap_rs.contains("static MI_THEAP_RANDOM_NEXT_KEEP")
            && theap_rs.contains("static MI_THEAP_CREATE_KEEP")
            && theap_rs.contains("static MI_THEAP_DELETE_KEEP"),
        "theap random/create/delete helpers should keep C's local helper boundaries without a random-next function pointer anchor"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/thread_heap.rs\t"))
            && !theap_rs.contains("SAFETY[AUDIT:thread-heap-lifecycle]"),
        "thread-heap lifecycle unsafe boundaries should stay split without a migration allowlist entry"
    );
    for (audit_id, minimum) in [
        ("thread-heap-visit-callback-boundary", 5),
        ("thread-heap-area-block-boundary", 10),
        ("thread-heap-page-visit-boundary", 13),
        ("thread-heap-collect-boundary", 15),
        ("thread-heap-default-boundary", 3),
        ("thread-heap-create-boundary", 7),
        ("thread-heap-random-boundary", 2),
        ("thread-heap-refcount-boundary", 9),
        ("thread-heap-delete-boundary", 7),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = theap_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "thread-heap unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        !theap_rs.contains("SAFETY[AUDIT:thread-heap-visit-boundary]"),
        "thread-heap visitation markers should stay split by callback, area/block, and page-walk responsibility"
    );
}
