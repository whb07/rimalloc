use super::*;

#[path = "marker_counts/aligned.rs"]
mod aligned;
#[path = "marker_counts/free.rs"]
mod free;

pub(super) fn assert_allocator_allocation_and_free_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let internal = sources.internal.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let alloc_boundary_rs = format!(
        "{alloc_rs}\n{}\n{}",
        sources.alloc_cxx_new_rs, sources.alloc_string_path_rs
    );
    let arena_rs = sources.arena_rs.as_str();
    let bitmap_rs = sources.bitmap_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let ptr_rs = sources.ptr_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_boundary_rs = format!("{page_rs}\n{}", sources.page_stats_bin_rs);
    let page_bins_rs = sources.page_bins_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let alloc_aligned_rs = sources.alloc_aligned_rs.as_str();

    free::assert_free_marker_counts(free_rs, bitmap_rs, unsafe_allowlist);
    aligned::assert_aligned_marker_counts(free_rs, alloc_aligned_rs, unsafe_allowlist);
    for (audit_id, min_count) in [
        ("allocation-platform-shim-boundary", 4),
        ("allocation-product-entry-boundary", 12),
        ("allocation-usable-size-entry-boundary", 9),
        ("allocation-small-dispatch-boundary", 7),
        ("allocation-thread-heap-zero-boundary", 8),
        ("allocation-public-malloc-boundary", 5),
        ("allocation-zeroing-dispatch-boundary", 9),
        ("allocation-calloc-dispatch-boundary", 5),
        ("allocation-layout-request-boundary", 10),
        ("allocation-realloc-thread-entry-boundary", 7),
        ("allocation-realloc-product-boundary", 6),
        ("allocation-realloc-free-on-failure-boundary", 6),
        ("allocation-realloc-zeroing-boundary", 12),
        ("allocation-realloc-public-boundary", 6),
        ("allocation-string-dup-boundary", 10),
        ("allocation-realpath-boundary", 9),
        ("allocation-cxx-new-handler-boundary", 4),
        ("allocation-cxx-new-try-boundary", 9),
        ("allocation-cxx-new-public-boundary", 4),
        ("allocation-cxx-new-product-boundary", 5),
        ("allocation-cxx-new-aligned-boundary", 5),
        ("allocation-cxx-new-realloc-boundary", 3),
        ("allocation-page-refill-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            alloc_boundary_rs.matches(&marker).count() >= min_count,
            "allocation unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    for retired_audit_id in [
        "allocation-heap-dispatch-boundary",
        "allocation-realloc-entry-boundary",
        "allocation-cxx-new-boundary",
    ] {
        assert!(
            !alloc_boundary_rs.contains(&format!("SAFETY[AUDIT:{retired_audit_id}]")),
            "allocation unsafe boundary {retired_audit_id} should stay split into focused operation markers"
        );
    }
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/alloc.rs\t"))
            && !alloc_rs.contains("SAFETY[AUDIT:allocation-entry-boundary]")
            && !alloc_rs.contains("SAFETY[AUDIT:allocation-abi-entry-boundary]")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_malloc(")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_calloc(")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_mallocn(")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_realloc(")
            && alloc_boundary_rs.contains("pub unsafe extern \"C\" fn mi_strdup(")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_new(")
            && alloc_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_malloc_zero(")
            && alloc_rs.contains("pub(crate) unsafe fn mi_theap_realloc_req(")
            && alloc_rs.contains("pub(crate) fn calloc_request_or_report(")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::PaddingCanary::from_page_block_keys("),
        "allocation unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
    for (audit_id, min_count) in [
        ("page-free-list-collect-boundary", 9),
        ("page-free-list-extend-boundary", 8),
        ("page-fresh-allocation-boundary", 9),
        ("page-queue-search-refill-boundary", 4),
        ("page-queue-full-transition-boundary", 5),
        ("page-queue-free-release-boundary", 4),
        ("page-queue-retire-boundary", 4),
        ("page-queue-abandon-boundary", 3),
        ("page-queue-reclaim-boundary", 4),
        ("page-queue-validation-boundary", 8),
        ("page-queue-direct-cache-boundary", 2),
        ("page-queue-link-mutation-boundary", 12),
        ("page-queue-append-boundary", 7),
        ("page-huge-allocation-boundary", 4),
        ("page-metadata-accessor-boundary", 7),
        ("page-stats-public-boundary", 12),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            page_boundary_rs.matches(&marker).count() >= min_count,
            "page unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !page_boundary_rs.contains("SAFETY[AUDIT:page-free-list-refill-boundary]")
            && !page_boundary_rs.contains("SAFETY[AUDIT:page-queue-transition-boundary]"),
        "page free-list and queue transitions should stay split into focused operation markers"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/page.rs\t"))
            && !page_rs.contains("SAFETY[AUDIT:page-allocation-boundary]")
            && page_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_malloc_generic(")
            && page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(")
            && page_rs.contains("unsafe fn mi_page_queue_find_free_ex(")
            && page_rs.contains("unsafe fn mi_page_queue_find_free_ex_typed(")
            && page_rs.contains("unsafe fn mi_find_free_page_typed(")
            && page_rs.contains("unsafe fn mi_page_extend_free_typed(")
            && page_rs.contains("pub unsafe extern \"C\" fn mi_page_fresh_alloc(")
            && page_rs.contains("unsafe fn mi_huge_page_alloc_typed(")
            && page_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_retire(")
            && page_boundary_rs.contains("pub unsafe extern \"C\" fn _mi_page_stats_bin(")
            && page_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_max_candidates)")
            && page_rs.contains("__mi_stat_counter_increase("),
        "page allocation unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );

    assert!(
        !alloc_aligned_rs.contains("fn _mi_theap_malloc_guarded(")
            && !alloc_aligned_rs.contains("fn _mi_theap_malloc_zero(")
            && !alloc_aligned_rs.contains("fn _mi_theap_malloc_zero_ex(")
            && !alloc_aligned_rs.contains("fn _mi_theap_realloc_zero(")
            && !alloc_aligned_rs.contains("fn _mi_padding_shrink("),
        "aligned allocation internals should call crate-local theap allocation helpers directly"
    );
    assert!(
        free_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && free_rs.contains("\".hidden _mi_padding_shrink\"")
            && free_rs.contains("\"_mi_padding_shrink:\"")
            && free_rs.contains("\"ret\"")
            && free_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_padding_shrink(")
            && free_rs.contains("if MI_PADDING == 0")
            && free_rs.contains("mi_page_decode_padding(page, block, &mut delta, &mut bsize)")
            && free_rs.contains(
                ".write_delta(\n            crate::core_types::PaddingDelta::for_usable("
            )
            && free_rs.contains("crate::core_types::PaddingDelta::new_unchecked(new_delta as u32)")
            && alloc_aligned_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && alloc_aligned_rs.contains("\"call _mi_padding_shrink\"")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call(")
            && alloc_aligned_rs.contains("unsafe { _mi_padding_shrink(page, block, min_size) };")
            && alloc_aligned_rs.contains(
                "unsafe { mi_padding_shrink_call(page, p.cast::<mi_block_t>(), adjust + size) };"
            ),
        "aligned allocation should keep C's _mi_padding_shrink helper boundary"
    );
    size_class_boundaries::assert_size_class_boundaries(size_class_boundaries::SizeClassSources {
        alloc_aligned_rs,
        alloc_rs,
        arena_rs,
        free_rs,
        internal,
        page_bins_rs,
        page_rs,
        page_stats_bin_rs: sources.page_stats_bin_rs.as_str(),
        ptr_rs,
        stats_rs,
    });
    typed_helpers::assert_typed_allocation_helpers(sources);
}
