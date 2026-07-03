use super::DatatypeAcceptanceSources;

pub(super) fn assert_padding_and_atomic_reconciliation(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let types_spec = audit_sources.types_spec;
    let atomic_thread_free = audit_sources.source("src/types/atomic/thread_free.rs");
    let padding_rs = audit_sources.source("src/types/padding.rs");
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
        atomic_thread_free
            .contains("pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);")
            && !atomic_thread_free
                .contains("pub(crate) struct AtomicThreadFree(pub(crate) *mut usize);"),
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
}
