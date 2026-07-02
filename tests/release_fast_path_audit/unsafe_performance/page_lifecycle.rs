#[path = "page_lifecycle/release_abandon.rs"]
mod release_abandon;

pub(super) struct BoundarySources<'a> {
    pub(super) alloc_rs: &'a str,
    pub(super) alloc_aligned_rs: &'a str,
    pub(super) arena_rs: &'a str,
    pub(super) atomic_types: &'a str,
    pub(super) free_rs: &'a str,
    pub(super) freelist_types: &'a str,
    pub(super) internal: &'a str,
    pub(super) page_counts_rs: &'a str,
    pub(super) page_flags_rs: &'a str,
    pub(super) page_queues_rs: &'a str,
    pub(super) page_rs: &'a str,
    pub(super) page_states_rs: &'a str,
    pub(super) page_validation_rs: &'a str,
    pub(super) theap_rs: &'a str,
}

pub(super) fn assert_page_lifecycle_and_queue_boundaries(sources: BoundarySources<'_>) {
    let BoundarySources {
        alloc_rs,
        alloc_aligned_rs,
        arena_rs,
        atomic_types,
        free_rs,
        freelist_types,
        internal,
        page_counts_rs,
        page_flags_rs,
        page_queues_rs,
        page_rs,
        page_states_rs,
        page_validation_rs,
        theap_rs,
    } = sources;

    release_abandon::assert_page_release_and_abandon_boundaries(
        page_rs,
        page_queues_rs,
        arena_rs,
        theap_rs,
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect(")
            && page_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect_partly("
            )
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_unfull(")
            && page_rs.contains(
                "#[inline(never)]\npub(crate) unsafe fn _mi_page_unfull_internal(\n    page_wrapper: crate::core_types::FullPage,"
            )
            && page_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_retire(page: *mut mi_page_t)")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_retire_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,"
            )
            && page_rs.contains("crate::core_types::AllFreePage::active_unchecked")
            && page_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_init(")
            && page_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_queue_is_valid(")
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_stats_bin(")
            && page_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_queue_append(")
            && page_rs.contains("static MI_PAGE_QUEUE_IS_VALID_KEEP")
            && page_rs.contains("static MI_PAGE_QUEUE_APPEND_KEEP")
            && page_rs.contains("mi_page_thread_free_collect(page);")
            && page_rs.contains("let atomic_tf = crate::core_types::AtomicThreadFree(")
            && page_rs.contains("crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*page).xthread_free))")
            && page_rs.contains("let remote = atomic_tf.collect_acq_rel();")
            && page_rs.contains("if remote.is_empty() {")
            && page_rs.contains("atomic_tf.load_relaxed().head_raw() == head")
            && alloc_rs.contains("let block = allocated.block().as_ptr();")
            && !alloc_rs.contains("allocated.0.as_ptr()")
            && free_rs.contains("block_wrapper.into_free_block()")
            && !free_rs.contains("crate::core_types::FreeBlock::new_unchecked")
            && !free_rs.contains("crate::core_types::FreeBlock(")
            && freelist_types.contains("AllocatedBlock::new_unchecked(head)")
            && freelist_types.contains("pub(crate) unsafe fn pop(page: AvailablePage)")
            && freelist_types.contains("let block_raw = block.block().as_ptr();")
            && !freelist_types.contains("AllocatedBlock(head)")
            && !freelist_types.contains("block.0.as_ptr()")
            && page_rs.contains("let thread_free = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ))")
            && page_rs.contains("crate::core_types::ThreadFreeWord::from_parts(\n                    None,\n                    crate::core_types::ThreadFreeOwner::Owned")
            && !page_rs.contains("crate::internal::mi_tf_create(ptr::null_mut()")
            && !page_rs.contains("crate::internal::mi_tf_block(mi_atomic_load_explicit")
            && !page_rs.contains("(*page).xthread_free == 1")
            && freelist_types.contains("pub(crate) struct AlignedBlockTag(BlockTag);")
            && freelist_types.contains("pub(crate) struct GuardedBlockTag(BlockTag);")
            && freelist_types.contains("pub(crate) unsafe fn write_aligned_to(block: BlockPtr)")
            && freelist_types.contains("pub(crate) unsafe fn write_guarded_to(block: BlockPtr)")
            && freelist_types.contains("pub(crate) unsafe fn read_guarded_from_raw(")
            && freelist_types.contains(") -> Option<GuardedBlockTag>")
            && freelist_types.contains("pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_types.contains("Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })")
            && freelist_types.contains("pub(crate) unsafe fn write_to(self, block: BlockPtr)")
            && atomic_types.contains("pub(crate) struct RemoteFreeList(Option<BlockPtr>);")
            && atomic_types.contains("fn from_detached_thread_free_head(head: Option<BlockPtr>) -> Self")
            && atomic_types.contains("pub(crate) fn head(self) -> Option<BlockPtr>")
            && atomic_types.contains("pub(crate) fn is_empty(self) -> bool")
            && atomic_types.contains("pub(crate) unsafe fn collect_into_local(")
            && !freelist_types.contains("pub(crate) struct RemoteFreeList(pub(crate) Option<BlockPtr>)")
            && alloc_rs.contains("crate::core_types::BlockTag::write_aligned_to(")
            && alloc_rs.contains("crate::core_types::BlockTag::write_guarded_to(")
            && alloc_aligned_rs.contains("crate::core_types::BlockTag::write_aligned_to(block)")
            && internal.contains("crate::core_types::BlockLink::is_guarded_raw(block)")
            && free_rs.contains("crate::core_types::BlockLink::is_guarded_raw(block)")
            && !alloc_rs.contains("crate::core_types::BlockLink::tagged(")
            && !alloc_aligned_rs.contains("crate::core_types::BlockLink::tagged(")
            && !internal.contains("BlockLink::read_from_raw(block).is_guarded_tag()")
            && !free_rs.contains("BlockLink::read_from_raw(block).is_guarded_tag()")
            && !freelist_types.contains("pub(crate) const fn tagged(tag: BlockTag) -> Self")
            && !freelist_types.contains("pub(crate) const ALIGNED: Self")
            && !freelist_types.contains("pub(crate) const GUARDED: Self")
            && !internal.contains("(*block).next")
            && !alloc_rs.contains("(*block).next = 0")
            && !alloc_rs.contains("(*block).next = MI_BLOCK_TAG_GUARDED")
            && !alloc_aligned_rs.contains("(*block).next = MI_BLOCK_TAG_ALIGNED")
            && !free_rs.contains("(*block).next == MI_BLOCK_TAG_GUARDED")
            && !internal.contains("(*block.cast_mut()).next == MI_BLOCK_TAG_GUARDED")
            && page_rs
                .contains("crate::internal::mi_assert_internal!(!force || (*page).local_free.is_null());")
            && page_rs.contains("unsafe fn mi_page_move_to_full_typed(")
            && page_rs.contains("unsafe fn mi_page_unfull_typed(")
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from_raw(\n    to: crate::core_types::PageQueuePtr,\n    from: crate::core_types::PageQueuePtr,"
            )
            && page_rs.contains("unsafe { page.into_full(to) }")
            && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)")
            && page_rs.contains("unsafe { page.unfull(to) }")
            && page_states_rs.contains("pub(crate) unsafe fn to_full(")
            && page_queues_rs.contains("impl QueuedPage<FullQueue>")
            && page_queues_rs
                .contains("pub(crate) unsafe fn unfull(self, to: NormalPageQueue)")
            && !page_states_rs.contains("pub(crate) unsafe fn unfull(")
            && !page_rs.contains("pub unsafe fn mi_page_queue_enqueue_from_ex(")
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from_ex(\n    to: *mut mi_page_queue_t,"
            )
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from(\n    to: *mut mi_page_queue_t,"
            )
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from_full(\n    to: *mut mi_page_queue_t,"
            )
            && page_flags_rs.contains("match crate::core_types::MemId::view((*page).memid)")
            && page_flags_rs.contains("crate::core_types::MemIdView::Os(os) => os")
            && !page_flags_rs.contains("(*page).memid.mem.os.base")
            && page_rs.contains(
                "let _ = _mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked("
            )
            && page_rs.contains("QueuedPage::<crate::core_types::FullQueue>::new")
            && page_rs.contains("mi_page_unfull_typed(queued, to)")
            && page_rs
                .contains("mi_page_move_to_full_typed(\n                crate::core_types::ActivePage")
            && free_rs.contains("let Some(all_free) = active.mark_all_free() else {")
            && free_rs.contains("_mi_page_retire_internal(all_free);")
            && free_rs
                .contains("_mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked")
            && free_rs.contains("_mi_page_free_collect(page_raw, false);")
            && free_rs.contains("_mi_page_free_collect_partly(page_raw, mt_free.as_ptr());")
            && arena_rs.contains("crate::page::_mi_page_init(theap, page)")
            && arena_rs.contains("_mi_page_free_collect(page_raw, false)")
            && arena_rs.contains("_mi_page_stats_bin(page)")
            && theap_rs.contains("_mi_page_free_collect(page, true)")
            && page_rs.contains("mod counts;")
            && page_rs.contains("mod validation;")
            && page_counts_rs.contains("SAFETY[AUDIT:page-count-accessors]")
            && page_counts_rs.contains("SAFETY[AUDIT:page-queue-count-accessors]")
            && page_validation_rs.contains("SAFETY[AUDIT:page-validation-helpers]")
            && page_validation_rs.contains("pub unsafe fn mi_page_block_at(")
            && page_validation_rs.contains("pub unsafe fn _mi_page_is_valid(")
            && page_validation_rs.contains("pub(crate) unsafe fn mi_page_is_valid_init(")
            && page_validation_rs.contains("mi_page_queue_contains(pq, cpage)")
            && page_counts_rs.contains("pub(crate) unsafe fn mi_page_used_count(")
            && page_counts_rs.contains("pub(crate) unsafe fn mi_page_capacity_count(")
            && page_counts_rs.contains("pub(super) unsafe fn mi_page_queue_block_size_raw(")
            && page_rs.contains("unsafe fn mi_page_queue_is_valid_typed(")
            && page_rs.contains("pub(crate) unsafe fn mi_normal_page_queue_is_valid(")
            && page_rs.contains("pub(crate) unsafe fn mi_huge_page_queue_is_valid(")
            && page_rs.contains("pub(crate) unsafe fn mi_full_page_queue_is_valid(")
            && theap_rs.contains("mi_normal_page_queue_is_valid(")
            && theap_rs.contains("mi_huge_page_queue_is_valid(")
            && theap_rs.contains("mi_full_page_queue_is_valid(")
            && !theap_rs.contains("_mi_page_queue_is_valid("),
        "page lifecycle and page-queue helpers should keep C's local helper boundaries"
    );
}
