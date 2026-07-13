pub(super) fn assert_page_state_domain_wrappers(core_mod: &str, page_states_rs: &str) {
    for module in [
        "arena",
        "atomic",
        "free_actions",
        "freelist",
        "freelist_wrappers",
        "heap",
        "layout",
        "memid",
        "page",
        "page_queues",
        "page_states",
        "ptr",
        "scalar",
        "validation",
        "validation_extra",
    ] {
        assert!(
            core_mod.contains(&format!("mod {module};")),
            "core_types should own the {module} semantic wrapper module"
        );
    }

    for witness in [
        "LocalPage",
        "ForeignPage",
        "AbandonedPageType",
        "AbandonedMappedPageType",
        "OwnedAbandonedPage",
        "ActivePage",
        "AvailablePage",
        "FullPage",
        "RetiredPage",
        "DestroyedPage",
    ] {
        assert!(
            page_states_rs.contains(&format!("pub(crate) struct {witness}(PagePtr);")),
            "{witness} should keep its page pointer field private"
        );
        assert!(
            !page_states_rs.contains(&format!("pub(crate) struct {witness}(pub(crate) PagePtr);")),
            "{witness} should not expose tuple construction outside page_states"
        );
    }
    assert!(
        page_states_rs
            .contains("pub(in crate::types) unsafe fn new_unchecked(page: PagePtr) -> Self")
            && page_states_rs.contains("pub(crate) fn page(&self) -> PagePtr"),
        "page-state witnesses should expose explicit unchecked constructors and page accessors"
    );
    assert!(
        page_states_rs.contains("pub(crate) struct ThreadHeapLocal(ThreadHeapPtr);")
            && page_states_rs
                .contains("pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self")
            && page_states_rs.contains("pub(crate) fn ptr(self) -> ThreadHeapPtr")
            && page_states_rs
                .contains("pub(crate) fn borrowed(self) -> crate::core_types::ThreadHeapBorrowed")
            && page_states_rs
                .contains("pub(crate) unsafe fn state(self) -> crate::core_types::ThreadHeapState")
            && !page_states_rs
                .contains("pub(crate) struct ThreadHeapLocal(pub(crate) ThreadHeapPtr);"),
        "ThreadHeapLocal should keep its thread-heap pointer private behind explicit construction"
    );
}
