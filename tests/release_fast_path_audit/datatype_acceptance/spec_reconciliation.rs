use super::DatatypeAcceptanceSources;

#[path = "spec_reconciliation/padding_atomic.rs"]
mod padding_atomic;

pub(super) fn assert_type_spec_reconciliation(audit_sources: &DatatypeAcceptanceSources<'_>) {
    padding_atomic::assert_padding_and_atomic_reconciliation(audit_sources);

    let page_states_rs = audit_sources.source("src/types/page_states.rs");
    let page_queues_rs = audit_sources.source("src/types/page_queues.rs");

    let validation_types = audit_sources.source("src/types/validation.rs");
    let free_rs = audit_sources.source("src/allocator/free.rs");
    let free_abandoned_rs = audit_sources.source("src/allocator/free/abandoned.rs");

    let page_rs = audit_sources.source("src/allocator/page.rs");
    let page_runtime_queues_rs = audit_sources.source("src/allocator/page/queues.rs");
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_local(\n    page_wrapper: crate::core_types::LocalPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        ) && free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,"
        ) && page_rs.contains(
            "pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,"
        ),
        "typed free/reclaim entry points should reject raw page ownership states"
    );
    assert!(
        page_rs.contains(
            "pub(crate) unsafe fn try_reclaim_abandoned(\n    page_wrapper: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
            )
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_page_reclaim_raw")
            && page_rs.contains("let _ = _mi_theap_page_reclaim(wrapper, theap_wrapper);")
            && page_states_rs.contains("pub(crate) fn into_reclaimed(self, _target: ThreadHeapLocal) -> ActivePage")
            && free_abandoned_rs.contains("unsafe fn try_reclaim_abandoned(\n    page: crate::core_types::OwnedAbandonedPage,")
            && free_abandoned_rs.contains("let active = _mi_theap_page_reclaim(\n            page,\n            crate::core_types::ThreadHeapLocal::new_unchecked")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: *mut mi_page_t")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    theap: *mut mi_theap_t")
            && !free_abandoned_rs.contains("unsafe fn try_reclaim_abandoned(\n    page: *mut mi_page_t"),
        "reclaim should accept only OwnedAbandonedPage and ThreadHeapLocal below the raw shim"
    );
    assert!(
        validation_types.contains("impl BlockForFree {\n    fn new(block: BlockPtr, was_guarded: WasGuarded) -> Self")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("let block = BlockForFree::new(located.block(), located.was_guarded());")
            && validation_types.contains("Some(ValidatedFree { user, page, block })")
            && free_rs.contains(
                "pub(crate) unsafe fn mi_free_block_local(\n    page_wrapper: crate::core_types::LocalPage,\n    block_wrapper: crate::core_types::BlockForFree,"
            )
            && free_rs.contains("let parts = validated.into_parts();")
            && free_rs.contains("mi_free_block_local(\n                    local_page,\n                    parts.block,")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new"),
        "mi_free_block_local should accept only LocalPage plus validation-created BlockForFree"
    );
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        )
            && free_rs.contains("mi_free_block_mt(foreign_page, block);")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), block);")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_abandoned_rs.contains(
                "pub(super) unsafe fn collect_after_remote_free(\n    page: crate::core_types::OwnedAbandonedPage,\n    published: crate::core_types::PublishedRemoteHead,"
            )
            && free_rs.contains(
                "pub(crate) unsafe extern \"C\" fn mi_free_try_collect_mt(\n    page: *mut mi_page_t,\n    mt_free: *mut mi_block_t,"
            )
            && free_rs.contains("crate::core_types::assume_owned_abandoned_page")
            && !free_rs.contains("pub(crate) unsafe fn mi_free_block_mt(\n    page: *mut mi_page_t")
            && !free_abandoned_rs.contains("collect_after_remote_free(\n    page: *mut mi_page_t"),
        "remote free should enter through ForeignPage or abandoned-page witnesses before touching remote-free internals"
    );
    assert!(
        page_runtime_queues_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
        ) && page_runtime_queues_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
        ) && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)"),
        "page queue transitions should require typed queue/page witnesses"
    );
    assert!(
        page_runtime_queues_rs.contains(
            "unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::FullPageQueue,\n    append: crate::core_types::FullPageQueue,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains("crate::core_types::assume_full_page(page_wrapper.page())")
            && page_rs.contains(
                "crate::core_types::QueuedPage::<crate::core_types::FullQueue>::try_queue_full(\n                full, queue,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,\n    to: crate::core_types::NormalPageQueue,"
            )
            && page_queues_rs.contains(
                "pub(crate) unsafe fn try_queue_full(\n        page: crate::core_types::FullPage,\n        queue: FullPageQueue,"
            )
            && page_queues_rs.contains("crate::page::mi_page_is_in_full(page.page().as_ptr())")
            && page_queues_rs.contains("queue: PageQueue::full(queue)")
            && !page_runtime_queues_rs.contains("unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::FullPage"
        ),
        "full-queue operations should accept only FullPage witnesses and FullPageQueue handles"
    );
    assert!(
        page_queues_rs.contains(
            "pub(crate) unsafe fn try_queue(\n        page: crate::core_types::ActivePage,\n        queue: NormalPageQueue,"
        )
            && page_queues_rs.contains("let queue_ptr = queue.ptr().as_ptr();")
            && page_queues_rs.contains("let page_ptr = page.page().as_ptr();")
            && page_queues_rs.contains("&& !crate::page::mi_page_is_in_full(page_ptr)")
            && page_queues_rs.contains(
                ".is_some_and(|block_size| page.page().block_size().get() == block_size.get())"
            )
            && page_queues_rs.contains(
                "crate::page::mi_page_is_huge(page_ptr)\n                        && crate::page::mi_page_queue_is_huge(queue_ptr)"
            )
            && page_queues_rs.contains("queue: PageQueue::normal(queue)")
            && page_runtime_queues_rs.contains(
                "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_runtime_queues_rs.contains(
                "unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_runtime_queues_rs.contains(
                "unsafe fn mi_page_queue_move_to_front_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queue: crate::core_types::NormalPageQueue,"
            )
            && page_runtime_queues_rs.contains(
                "unsafe fn mi_page_queue_append_normal_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    append: crate::core_types::NormalPageQueue,"
            )
            && !page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: NormalPageQueue,"
            )
            && !page_runtime_queues_rs.contains("unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::PagePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queue: crate::core_types::PagePtr"),
        "normal queue operations should accept only active non-full pages matching NormalPageQueue or the huge-page sentinel queue"
    );
}
