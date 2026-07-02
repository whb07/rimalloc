use super::DatatypeAcceptanceSources;

pub(super) fn assert_type_spec_reconciliation(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let page_states_rs = audit_sources.source("src/types/page_states.rs");
    let page_queues_rs = audit_sources.source("src/types/page_queues.rs");

    let types_spec = audit_sources.types_spec;
    let atomic = audit_sources.source("src/types/atomic.rs");
    let padding_rs = audit_sources.source("src/types/padding.rs");
    let validation_types = audit_sources.source("src/types/validation.rs");
    let alloc_rs = audit_sources.source("src/allocator/alloc.rs");
    let free_rs = audit_sources.source("src/allocator/free.rs");
    assert!(
        padding_rs.contains(
            "pub(crate) fn from_page_block_keys(page: PagePtr, block: BlockPtr, keys: PageKeys) -> Self"
        )
            && padding_rs.contains("page.as_ptr().cast()")
            && padding_rs.contains("block.as_ptr().cast()")
            && !padding_rs.contains("page: *const c_void")
            && !padding_rs.contains("block: *const c_void")
            && padding_rs.contains(
                "pub(crate) fn for_usable(delta: usize, usable_size: UsableSize) -> Option<Self>"
            )
            && padding_rs.contains("u32::try_from(delta).ok().map(Self)")
            && alloc_rs.contains("PaddingCanary::from_page_block_keys")
            && alloc_rs.contains("PagePtr::new_unchecked(page)")
            && alloc_rs.contains("PaddingDelta::for_usable(delta, usable_size)")
            && alloc_rs.contains("crate::core_types::PaddingDelta::new_unchecked(delta as u32)")
            && free_rs.contains("PaddingCanary::from_page_block_keys")
            && free_rs.contains("let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());")
            && free_rs.contains("let block_ptr = crate::core_types::BlockPtr::new_unchecked(block.cast_mut());")
            && free_rs.contains("PaddingDelta::for_usable("),
        "padding metadata should construct canaries from typed PagePtr/BlockPtr plus PageKeys and deltas with the usable-size bound"
    );
    assert!(
        atomic.contains("pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);")
            && !atomic.contains("pub(crate) struct AtomicThreadFree(pub(crate) *mut usize);"),
        "AtomicThreadFree should be backed by AtomicUsizeField, matching TYPES_SPEC.md"
    );
    assert!(
        types_spec.contains("The thread-free atomic cell is `AtomicThreadFree(AtomicUsizeField)`")
            && types_spec
                .contains("Earlier\nplans used the placeholder name `ThreadFreeCell(*mut usize)`")
            && !types_spec.contains("pub(crate) struct ThreadFreeCell(*mut usize);")
            && types_spec.contains("pub(crate) fn push_remote_acq_rel(")
            && types_spec.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool;"
            ),
        "TYPES_SPEC.md should document AtomicThreadFree as the reconciled thread-free cell"
    );
    assert!(
        types_spec.contains(
            "`AbandonedCountCell` is the staged arena-domain name for the same storage now operated on by `AtomicAbandonedCount(*const AtomicUsize)`"
        ) && types_spec.contains(
            "`ArenaPageCell` is the staged arena-domain name for the same storage now operated on by `AtomicArenaPage(*const AtomicPtr<c_void>)`"
        ) && types_spec.contains(
            "`ArenaPageTablePtr` remains the non-atomic page-table pointer witness"
        ),
        "TYPES_SPEC.md should reconcile staged arena page-table cell names with live atomic wrappers"
    );

    let page_rs = audit_sources.source("src/allocator/page.rs");
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
            && page_rs.contains("_mi_theap_page_reclaim(wrapper, theap_wrapper);")
            && page_states_rs.contains("pub(crate) fn reclaim(self, _target: ThreadHeapLocal) -> ActivePage")
            && free_rs.contains("unsafe fn mi_abandoned_page_try_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,")
            && free_rs.contains("_mi_theap_page_reclaim(\n            page,\n            crate::core_types::ThreadHeapLocal::new_unchecked")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: *mut mi_page_t")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    theap: *mut mi_theap_t")
            && !free_rs.contains("unsafe fn mi_abandoned_page_try_reclaim(\n    page: *mut mi_page_t"),
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
            && free_rs.contains("mi_free_block_local(\n                    local_page,\n                    validated.block(),")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new"),
        "mi_free_block_local should accept only LocalPage plus validation-created BlockForFree"
    );
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        )
            && free_rs.contains("mi_free_block_mt(foreign_page, validated.block());")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_rs.contains(
                "unsafe fn mi_free_try_collect_mt_typed(\n    page: crate::core_types::OwnedAbandonedPage,\n    remote_free: crate::core_types::RemoteFreeList,"
            )
            && free_rs.contains(
                "pub(crate) unsafe extern \"C\" fn mi_free_try_collect_mt(\n    page: *mut mi_page_t,\n    mt_free: *mut mi_block_t,"
            )
            && free_rs.contains("crate::core_types::OwnedAbandonedPage::new_unchecked")
            && !free_rs.contains("pub(crate) unsafe fn mi_free_block_mt(\n    page: *mut mi_page_t")
            && !free_rs.contains("unsafe fn mi_free_try_collect_mt_typed(\n    page: *mut mi_page_t"),
        "remote free should enter through ForeignPage or abandoned-page witnesses before touching remote-free internals"
    );
    assert!(
        page_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
        ) && page_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
        ) && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)"),
        "page queue transitions should require typed queue/page witnesses"
    );
    assert!(
        page_rs.contains(
            "unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::FullPageQueue,\n    append: crate::core_types::FullPageQueue,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains("crate::core_types::FullPage::new_unchecked(page_wrapper.page())")
            && page_rs.contains(
                "crate::core_types::QueuedPage::<crate::core_types::FullQueue>::new(full, queue)"
            )
            && page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,\n    to: crate::core_types::NormalPageQueue,"
            )
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: FullPageQueue,"
            )
            && page_queues_rs.contains("crate::page::mi_page_is_in_full(page.page().as_ptr())")
            && page_queues_rs.contains("queue: PageQueue::full(queue)")
            && !page_rs.contains("unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::FullPage"
        ),
        "full-queue operations should accept only FullPage witnesses and FullPageQueue handles"
    );
    assert!(
        page_queues_rs.contains(
            "pub(crate) unsafe fn new(\n        page: crate::core_types::ActivePage,\n        queue: NormalPageQueue,"
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
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_move_to_front_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_append_normal_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    append: crate::core_types::NormalPageQueue,"
            )
            && !page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: NormalPageQueue,"
            )
            && !page_rs.contains("unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::PagePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::PagePtr"),
        "normal queue operations should accept only active non-full pages matching NormalPageQueue or the huge-page sentinel queue"
    );
}
