pub(super) fn assert_page_release_and_abandon_boundaries(
    page_rs: &str,
    page_queues_rs: &str,
    arena_rs: &str,
    theap_rs: &str,
) {
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_free(")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queue: crate::core_types::NormalPageQueue,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains("mi_page_queue_contains(queue.ptr().as_ptr(), page)")
            && page_rs.contains("let _ = _mi_page_free_active_internal(all_free, queue);")
            && page_rs.contains("mi_page_queue_remove_raw(queue.ptr().as_ptr(), page);")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage"
            )
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_internal<S>")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("let destroyed = _mi_arenas_page_free_typed(page_wrapper, owner);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_free_typed<S>(\n    page: crate::core_types::AllFreePage<S>,\n    owner: crate::core_types::ThreadHeapLocal,"
            )
            && arena_rs
                .contains("unsafe { _mi_arenas_page_free(page.page().as_ptr(), owner.ptr().as_ptr()) };")
            && arena_rs.contains("page.free_to_arena(owner)")
            && theap_rs.contains("_mi_page_free(page, pq);"),
        "page release should keep C's local _mi_page_free helper boundary while using typed all-free page witnesses internally"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_abandon(")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_abandon_internal(\n    page_wrapper: crate::core_types::PageForAbandon,\n    queue: crate::core_types::PageQueuePtr,"
            )
            && page_rs.contains(") -> crate::core_types::PageAbandonOutcome {")
            && page_rs.contains("_mi_page_free_collect(page, false);")
            && page_rs.contains("if mi_page_all_free(page) {")
            && page_rs.contains("let destroyed = match page_wrapper {")
            && page_rs.contains("let Ok(all_free) = page.try_mark_all_free() else {")
            && page_rs.contains("_mi_page_free_full_internal(all_free, queue)")
            && page_rs.contains("_mi_page_free_active_internal(all_free, queue)")
            && page_rs.contains("QueuedPage::<crate::core_types::FullQueue>::try_queue_full")
            && page_rs.contains(">::try_queue(page, queue)")
            && page_rs.contains("let _ = queued.remove();")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("_mi_arenas_page_abandon(page, theap);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && page_rs.contains("crate::core_types::PageAbandonOutcome::Abandoned")
            && page_rs.contains("crate::core_types::assume_abandoned_page(page_ptr)")
            && page_rs.contains("let page_ptr = crate::core_types::PagePtr::new_unchecked(page);")
            && page_rs.contains("crate::core_types::assume_full_page(page_ptr)")
            && page_rs.contains("crate::core_types::assume_active_page(page_ptr)")
            && page_rs.contains(".into_abandon_candidate()")
            && theap_rs.contains("_mi_page_abandon(page, pq);"),
        "page abandon should keep C's local helper boundary while using typed active/full abandon witnesses internally"
    );
}
