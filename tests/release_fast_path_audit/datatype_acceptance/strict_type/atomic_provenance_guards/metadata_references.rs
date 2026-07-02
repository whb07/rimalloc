use super::DatatypeAcceptanceSources;

pub(super) fn assert_metadata_reference_boundaries(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let strict_check_rs = audit_sources.strict_type_checker;
    let allowlist_rs = audit_sources.strict_type_allowlist;
    let ptr_core_rs = audit_sources.source("src/types/ptr.rs");

    assert!(
        strict_check_rs.contains("METADATA_REF_RE = re.compile")
            && strict_check_rs.contains("kind=\"metadata-reference\"")
            && strict_check_rs.contains("domain=\"metadata\"")
            && strict_check_rs
                .contains("message=\"broad raw metadata reference outside typed field helpers\"")
            && !allowlist_rs.contains("metadata-reference|src/bitmap.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/internal/bitmap.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/free.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/options.rs|*|")
            && allowlist_rs
                .contains("metadata-reference|src/internal/bitmap.rs|mi_bitmap_setN|bitmap|")
            && allowlist_rs.contains(
                "metadata-reference|src/allocator/free.rs|mi_page_decode_padding|mi_page_t|"
            )
            && allowlist_rs.contains(
                "metadata-reference|src/allocator/options.rs|with_options_table|options|"
            )
            && !allowlist_rs.contains("metadata-reference|src/allocator/stats.rs|*|")
            && allowlist_rs
                .contains("metadata-reference|src/allocator/stats.rs|mi_heap_stats_get|mi_heap_t|")
            && !allowlist_rs.contains("metadata-reference|src/types/atomic.rs|*|atomic-field|")
            && allowlist_rs.contains(
                "metadata-reference|src/types/atomic.rs|AtomicArenaPage::load_acquire|atomic-field|"
            )
            && allowlist_rs.contains(
                "metadata-reference|src/types/atomic.rs|AtomicArenaPage::store_release|atomic-field|"
            )
            && ptr_core_rs.contains("let raw = self.as_ptr();")
            && ptr_core_rs.contains("ptr::read(ptr::addr_of!((*raw).block_size))")
            && ptr_core_rs
                .contains("ptr::write(ptr::addr_of_mut!((*raw).block_size), block_size.get())")
            && ptr_core_rs
                .contains("AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).free), head)")
            && ptr_core_rs
                .contains("LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).local_free), head)")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_id_cell(self) -> AtomicPageThreadId")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_free_cell(self) -> AtomicThreadFree")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_cell(self) -> AtomicPtrField<mi_heap_t>")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count_cell(self) -> AtomicUsizeField")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count_cell(self) -> AtomicUsizeField"),
        "aliasable allocator metadata should be accessed through pointer wrapper field helpers and guarded against broad Rust references"
    );
}
