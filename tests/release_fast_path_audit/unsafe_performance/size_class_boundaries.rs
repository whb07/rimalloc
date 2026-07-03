pub(super) struct SizeClassSources<'a> {
    pub(super) alloc_aligned_rs: &'a str,
    pub(super) alloc_rs: &'a str,
    pub(super) arena_rs: &'a str,
    pub(super) free_rs: &'a str,
    pub(super) internal: &'a str,
    pub(super) page_bins_rs: &'a str,
    pub(super) page_rs: &'a str,
    pub(super) page_stats_bin_rs: &'a str,
    pub(super) ptr_rs: &'a str,
    pub(super) stats_rs: &'a str,
}

pub(super) fn assert_size_class_boundaries(sources: SizeClassSources<'_>) {
    let SizeClassSources {
        alloc_aligned_rs,
        alloc_rs,
        arena_rs,
        free_rs,
        internal,
        page_bins_rs,
        page_rs,
        page_stats_bin_rs,
        ptr_rs,
        stats_rs,
    } = sources;

    assert!(
        page_rs.contains("mod bins;")
            && page_bins_rs
                .contains("#[inline(never)]\npub extern \"C\" fn mi_bin(size: usize) -> usize")
            && page_bins_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_bin(size: usize) -> usize")
            && page_bins_rs.contains(
                "#[inline(never)]\npub extern \"C\" fn _mi_bin_size(bin: usize) -> usize"
            )
            && page_bins_rs.contains("pub(crate) fn mi_bin_impl(size: usize) -> usize")
            && page_bins_rs
                .contains("fn mi_bin_from_raw_size(size: usize) -> crate::core_types::Bin")
            && page_bins_rs.contains(
                "pub(crate) fn mi_bin_from_padded(padded: crate::core_types::PaddedSize)"
            )
            && page_bins_rs.contains(
                "pub(crate) fn mi_bin_from_block_size(\n    block_size: crate::core_types::BlockSize,"
            )
            && page_bins_rs.contains(
                "pub(crate) fn mi_bin_from_usable(usable: crate::core_types::UsableSize)"
            )
            && page_bins_rs.contains("mi_bin_impl(size)")
            && page_bins_rs.contains("_mi_theap_empty")
            && page_bins_rs.contains(".pages[bin].block_size")
            && page_bins_rs.contains("_mi_bin_size(mi_bin_from_padded(padded).as_usize())")
            && stats_rs.contains("_mi_bin_size(bin)"),
        "bin lookup helpers should keep C's mi_bin/_mi_bin/_mi_bin_size boundaries and empty-theap table dependency"
    );
    assert!(
        alloc_rs.contains("let usable_size = mi_page_usable_block_size(page);")
            && alloc_rs.contains("let bin = mi_bin_from_usable(usable_size).as_usize();")
            && free_rs.contains("mi_bin_from_usable(bsize).as_usize()")
            && arena_rs.contains("let bin = mi_bin_from_block_size(block_size);")
            && internal.contains("pub(crate) unsafe fn mi_page_block_size_typed(")
            && internal.contains(") -> crate::core_types::BlockSize")
            && internal.contains("mi_page_block_size_typed(page).get()")
            && internal.contains("mi_page_block_size_typed(page).usable()")
            && arena_rs.contains("mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) })")
            && internal.contains(
                "block_size: crate::core_types::BlockSize,\n) -> *mut mi_page_queue_t {\n    unsafe {\n        let size = block_size.get();"
            )
            && internal.contains("crate::page::mi_bin_from_block_size(block_size).as_usize()")
            && page_stats_bin_rs.contains("pub(crate) unsafe fn mi_page_bin_typed(")
            && page_stats_bin_rs.contains(") -> crate::core_types::Bin")
            && page_stats_bin_rs.contains("pub(crate) unsafe fn _mi_page_stats_bin_typed(")
            && page_stats_bin_rs.contains(") -> crate::core_types::StatsBin")
            && page_rs.contains("let bin = mi_page_bin_typed(page).as_usize();")
            && page_stats_bin_rs.contains("let bin = _mi_page_stats_bin_typed(page).as_usize();")
            && page_rs.contains("let bin = mi_bin_from_block_size(block_size).as_usize();")
            && page_rs.contains(
                "== mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked("
            )
            && page_rs.contains("mi_page_queue_block_size_raw(prev),")
            && !page_rs.contains("pub(crate) fn _mi_bin_typed(size: usize)")
            && !alloc_rs.contains("_mi_bin_typed(")
            && !free_rs.contains("_mi_bin_typed(")
            && !arena_rs.contains("_mi_bin_typed(")
            && !internal.contains("_mi_bin_typed("),
        "internal bin users should consume typed Bin wrappers before raw array indexing"
    );
    assert!(
        free_rs.contains("pub(crate) unsafe fn mi_page_usable_size_of_typed(")
            && free_rs.contains(") -> crate::core_types::UsableSize")
            && free_rs.contains("pub(crate) unsafe fn mi_route_usable_size_typed(")
            && free_rs.contains("pub(crate) unsafe fn _mi_usable_size_typed(")
            && free_rs.contains("_mi_usable_size_typed(p, page).get()")
            && alloc_rs.contains("use crate::free::_mi_usable_size_typed;")
            && alloc_rs.contains("let size = _mi_usable_size_typed(p, page);")
            && !alloc_rs.contains("use crate::free::_mi_usable_size;"),
        "internal usable-size routing should carry UsableSize until raw ABI/comparison boundaries"
    );
    assert!(
        page_bins_rs.contains("pub(crate) fn mi_good_size_typed(")
            && page_bins_rs.contains(") -> crate::core_types::GoodSize")
            && page_bins_rs
                .contains("mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()")
            && alloc_rs
                .contains("mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()")
            && alloc_aligned_rs.contains(
                "fn mi_malloc_is_naturally_aligned(\n    size: crate::core_types::RequestedSize,\n    alignment: crate::core_types::Alignment,"
            )
            && alloc_aligned_rs.contains("mi_good_size_typed(requested).get()")
            && stats_rs.contains("mi_good_size_typed(RequestedSize::new(12 * 1024)).get()")
            && !stats_rs.contains("use crate::mimalloc::{mi_good_size")
            && !alloc_rs.contains("use crate::page::{_mi_bin_typed, _mi_malloc_generic_typed, MI_MAX_ALLOC_SIZE, mi_good_size};")
            && !alloc_aligned_rs.contains("mi_good_size(size)"),
        "internal good-size routing should carry GoodSize until raw ABI/comparison boundaries"
    );
    assert!(
        internal.contains(
            "pub(crate) fn mi_direct_bin_from_size(size: usize) -> crate::core_types::DirectBin"
        ) && internal.contains(
            "pub(crate) fn mi_direct_bin_from_padded(\n    padded: crate::core_types::PaddedSize,"
        ) && internal.contains("mi_direct_bin_from_size(padded.get())")
            && internal.contains("let idx = crate::core_types::WordCount::from_size(size).get();")
            && internal.contains("let idx = mi_direct_bin_from_padded(padded).as_usize();")
            && page_rs.contains("let direct_idx = mi_direct_bin_from_size(size);")
            && page_rs.contains("let idx = direct_idx.as_usize();")
            && page_rs.contains(".direct_page(direct_idx)")
            && page_rs.contains("theap_ptr.fill_direct_pages(")
            && ptr_rs.contains(
                "pub(crate) unsafe fn fill_direct_pages(\n        self,\n        start: DirectBin,\n        end: DirectBin,"
            )
            && !internal.contains(
                "let idx = _mi_wsize_from_size(size);\n    (*theap).pages_free_direct[idx]"
            )
            && !page_rs.contains("pages_free_direct"),
        "direct page-cache indexes should pass through DirectBin before pages_free_direct indexing"
    );
}
