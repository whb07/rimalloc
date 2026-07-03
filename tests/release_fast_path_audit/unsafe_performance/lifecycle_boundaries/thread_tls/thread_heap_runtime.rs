pub(super) fn assert_thread_heap_runtime_boundaries(
    unsafe_allowlist: &str,
    stats_rs: &str,
    theap_rs: &str,
) {
    let theap_lists_rs = include_str!("../../../../../src/allocator/thread_heap/lists.rs");
    let theap_lifecycle_rs = format!("{theap_rs}\n{theap_lists_rs}");
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
        && theap_rs.contains("mi_theap_decref_inline(theap_ptr);\n        true")
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
        ("thread-heap-list-boundary", 12),
        ("thread-heap-random-boundary", 2),
        ("thread-heap-refcount-boundary", 9),
        ("thread-heap-delete-boundary", 7),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = theap_lifecycle_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "thread-heap unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        theap_rs.contains("let tld_theaps_head = lists::push_tld_front(tld, theap);")
            && theap_rs.contains("lists::push_heap_front(heap, theap);")
            && theap_rs.contains("lists::remove_from_heap(")
            && theap_rs.contains("lists::remove_from_tld(")
            && !theap_rs.contains("(*theap).hnext")
            && !theap_rs.contains("(*theap).hprev")
            && !theap_rs.contains("(*theap).tnext")
            && !theap_rs.contains("(*theap).tprev")
            && theap_lists_rs.contains("pub(super) unsafe fn push_tld_front(")
            && theap_lists_rs.contains("pub(super) unsafe fn push_heap_front(")
            && theap_lists_rs.contains("pub(super) unsafe fn remove_from_heap(")
            && theap_lists_rs.contains("pub(super) unsafe fn remove_from_tld(")
            && theap_lists_rs.contains("(*theap.as_ptr()).hnext")
            && theap_lists_rs.contains("(*theap.as_ptr()).hprev")
            && theap_lists_rs.contains("(*theap.as_ptr()).tnext")
            && theap_lists_rs.contains("(*theap.as_ptr()).tprev"),
        "thread-heap heap/TLD list links should stay in the private list helper"
    );
    assert!(
        !theap_rs.contains("SAFETY[AUDIT:thread-heap-visit-boundary]"),
        "thread-heap visitation markers should stay split by callback, area/block, and page-walk responsibility"
    );
}
