use super::super::DatatypeAcceptanceSources;

pub(super) fn assert_pointer_and_freelist_wrappers(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let unsafe_migration_allowlist = audit_sources.unsafe_migration_allowlist;
    let page_states_rs = audit_sources.source("src/types/page_states.rs");
    let page_queues_rs = audit_sources.source("src/types/page_queues.rs");
    let validation_extra_rs = audit_sources.source("src/types/validation_extra.rs");
    let ptr_core_rs = audit_sources.source("src/types/ptr.rs");
    let core_types_layout_tests_rs = audit_sources.source("src/types/layout_tests.rs");
    let freelist_wrappers_rs = audit_sources.source("src/types/freelist_wrappers.rs");
    let allowlist_rs = audit_sources.strict_type_allowlist;
    let strict_check_rs = audit_sources.strict_type_checker;
    assert!(
        ptr_core_rs.contains("#[repr(transparent)]\n        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n        pub(crate) struct $name(NonNull<$raw_type>);")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) fn new(ptr: *mut $raw_type) -> Option<Self>")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) fn as_ptr(self) -> *mut $raw_type")
            && ptr_core_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\npub(crate) struct PageConstPtr(NonNull<mi_page_t>);")
            && page_states_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct LocalPage(PagePtr);")
            && page_states_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct OwnedAbandonedPage(PagePtr);")
            && page_queues_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct NormalPageQueue(PageQueuePtr);")
            && page_queues_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct FullPageQueue(PageQueuePtr);")
            && validation_extra_rs.contains("#[repr(transparent)]\n#[must_use = \"allocator state capabilities must be consumed or explicitly returned\"]\npub(crate) struct FreeBlock(BlockPtr);")
            && freelist_wrappers_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug)]\npub(crate) struct AvailableFreeList(Option<BlockPtr>);")
            && freelist_wrappers_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug)]\npub(crate) struct LocalFreeList(Option<BlockPtr>);")
            && core_types_layout_tests_rs.contains("fn pointer_wrappers_match_nonnull_layout()")
            && core_types_layout_tests_rs.contains("fn optional_pointer_wrappers_are_pointer_sized()")
            && core_types_layout_tests_rs.contains("fn witness_wrappers_match_pointer_layout()")
            && core_types_layout_tests_rs
                .contains("fn packed_and_atomic_wrappers_match_raw_layout()")
            && !unsafe_migration_allowlist.contains("src/types/ptr.rs"),
        "hot strict-type wrappers should stay transparent, linear for state capabilities, copyable for identities, inlineable, and covered by layout/niche tests"
    );
    for (audit_id, min_count) in [
        ("typed-pointer-constructor-boundary", 5),
        ("typed-page-scalar-accessors", 14),
        ("typed-page-free-list-accessors", 4),
        ("typed-page-queue-link-accessors", 4),
        ("typed-page-atomic-accessors", 6),
        ("typed-page-fixture-touch-accessors", 1),
        ("typed-block-accessors", 6),
        ("typed-heap-accessors", 12),
        ("typed-thread-heap-accessors", 10),
        ("typed-tld-thread-identity-accessors", 7),
        ("typed-tld-subproc-accessors", 6),
        ("typed-tld-metadata-accessors", 8),
        ("typed-tld-fixture-touch-accessors", 3),
        ("typed-arena-identity-accessors", 13),
        ("typed-arena-linkage-accessors", 10),
        ("typed-arena-fixture-touch-accessors", 4),
        ("typed-subproc-identity-accessors", 2),
        ("typed-subproc-arena-accessors", 7),
        ("typed-subproc-heap-link-accessors", 5),
        ("typed-subproc-count-accessors", 12),
        ("typed-subproc-metadata-accessors", 2),
        ("typed-subproc-fixture-touch-accessors", 3),
        ("typed-page-queue-accessors", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            ptr_core_rs.matches(&marker).count() >= min_count,
            "typed pointer boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !ptr_core_rs.contains("SAFETY[AUDIT:typed-pointer-accessors]")
            && !ptr_core_rs.contains("SAFETY[AUDIT:typed-page-accessors]")
            && !ptr_core_rs.contains("SAFETY[AUDIT:typed-arena-accessors]")
            && !ptr_core_rs.contains("SAFETY[AUDIT:typed-subproc-accessors]"),
        "typed pointer unsafe markers should stay split by pointer family and page/arena accessor responsibility"
    );
    let from_raw_allowlist_entry =
        "raw-pointer-parameter|src/types/freelist_wrappers.rs|from_raw|mi_block_t|";
    assert!(
        freelist_wrappers_rs.contains("pub(crate) struct EncodedPtr<T> {")
            && freelist_wrappers_rs.contains("word: EncodedWord,")
            && freelist_wrappers_rs.contains(
                "pub(crate) unsafe fn read_word_from_raw(block: *const mi_block_t) -> Self"
            )
            && freelist_wrappers_rs
                .contains("word: EncodedWord::new(unsafe { (*block.cast_mut()).next })")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn write_word_to_raw(self, block: BlockPtr)")
            && freelist_wrappers_rs
                .contains("unsafe { (*block.as_ptr()).next = self.word.get() };")
            && freelist_wrappers_rs
                .contains("unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_wrappers_rs.contains("Self(unsafe { (*block.cast_mut()).next })")
            && freelist_wrappers_rs.contains("unsafe fn write_to(self, block: BlockPtr)")
            && freelist_wrappers_rs.contains("unsafe { (*block.as_ptr()).next = self.0 };")
            && freelist_wrappers_rs.contains("pub(crate) enum BlockLink {")
            && freelist_wrappers_rs.contains("FreeList(EncodedPtr<crate::types::mi_block_t>),")
            && freelist_wrappers_rs.contains("Guarded(GuardedBlockTag),")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_wrappers_rs
                .contains("Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn write_to(self, block: BlockPtr)")
            && freelist_wrappers_rs
                .contains("Self::FreeList(link) => unsafe { link.write_word_to_raw(block) }")
            && freelist_wrappers_rs
                .contains("Self::Guarded(tag) => unsafe { tag.write_to(block) }")
            && freelist_wrappers_rs.contains("pub(crate) struct PageKeys([usize; 2]);")
            && freelist_wrappers_rs.contains("pub(crate) const fn as_raw(&self) -> &[usize; 2]")
            && freelist_wrappers_rs
                .contains("pub(crate) struct AvailableFreeList(Option<BlockPtr>);")
            && freelist_wrappers_rs.contains("pub(crate) fn from_head(head: BlockPtr) -> Self")
            && freelist_wrappers_rs.contains("pub(crate) fn head(self) -> Option<BlockPtr>")
            && freelist_wrappers_rs
                .contains("pub(crate) struct LocalFreeList(Option<BlockPtr>);")
            && freelist_wrappers_rs.contains(
                "pub(crate) fn into_available(self) -> AvailableFreeList",
            )
            && freelist_wrappers_rs
                .contains("let next_raw = unsafe { mi_block_next(raw_page, head_raw) };")
            && freelist_wrappers_rs
                .contains("unsafe { mi_block_set_next(raw_page, block_raw, prev_raw) };")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn encoded_link(self) -> EncodedPtr<mi_block_t>")
            && ptr_core_rs.contains("unsafe { EncodedPtr::read_word_from_raw(self.as_ptr()) }")
            && ptr_core_rs.contains("pub(crate) unsafe fn write_link(self, link: BlockLink)")
            && ptr_core_rs.contains("unsafe { link.write_to(self) };")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn next(self, page: PagePtr) -> Option<BlockPtr>")
            && ptr_core_rs.contains(
                "pub(crate) unsafe fn set_next(self, page: PagePtr, next: Option<BlockPtr>)"
            )
            && ptr_core_rs.contains("unsafe { self.write_link(BlockLink::FreeList(link)) };")
            && strict_check_rs.contains("ALLOWED_BLOCK_NEXT = {")
            && strict_check_rs.contains("\"src/types/freelist_wrappers.rs\"")
            && strict_check_rs.contains("kind=\"block-next-access\"")
            && strict_check_rs
                .contains("message=\"direct mi_block_s.next access outside free-list wrappers\"")
            && !allowlist_rs
                .contains("raw-pointer-parameter|src/types/freelist_wrappers.rs|*|mi_block_t|")
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|read_word_from_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|from_unencoded_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|encode_block_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|read_guarded_from_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|read_from_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|is_guarded_raw|mi_block_t|"
            )
            && allowlist_rs.contains(from_raw_allowlist_entry),
        "mi_block_s.next reads/writes should stay behind EncodedPtr, BlockLink, BlockPtr, or free-list wrappers and strict checker coverage"
    );
}
