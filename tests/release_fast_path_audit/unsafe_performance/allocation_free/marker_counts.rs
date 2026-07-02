use super::*;

pub(super) fn assert_allocator_allocation_and_free_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let internal = sources.internal.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let bitmap_rs = sources.bitmap_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let ptr_rs = sources.ptr_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_bins_rs = sources.page_bins_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let alloc_aligned_rs = sources.alloc_aligned_rs.as_str();

    assert!(
        !free_rs.contains("clippy::if_same_then_else")
            && !bitmap_rs.contains("clippy::if_same_then_else"),
        "free.rs and bitmap.rs should not retain obsolete if_same_then_else allowances"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/free.rs\t"))
            && !free_rs.contains("SAFETY[AUDIT:free-path-boundary]")
            && free_rs.contains("core::arch::global_asm!(")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign(")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn mi_free_generic_local(")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn mi_free_generic_mt(")
            && free_rs.contains("unsafe fn mi_free_fast_try(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_free(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_usable_size(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_ufree(")
            && free_rs.contains("pub unsafe extern \"C\" fn mi_free_small("),
        "free-path unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    for (audit_id, expected_count) in [
        ("free-raw-export-boundary", 13),
        ("free-double-check-boundary", 3),
        ("free-pointer-lookup-boundary", 10),
        ("free-abi-validation-boundary", 5),
        ("free-validation-fixtures", 10),
        ("free-padding-boundary", 5),
        ("free-usable-size-boundary", 9),
        ("free-stats-boundary", 1),
        ("free-local-list-boundary", 1),
        ("free-remote-list-boundary", 1),
        ("free-route-dispatch-boundary", 10),
        ("free-abandoned-page-boundary", 1),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            free_rs.matches(&marker).count(),
            expected_count,
            "free unsafe boundary {audit_id} should retain exactly {expected_count} local operation markers"
        );
    }
    for (audit_id, expected_count) in [
        ("aligned-padding-boundary", 23),
        ("aligned-thread-heap-allocation-boundary", 11),
        ("aligned-page-allocation-boundary", 11),
        ("aligned-calloc-product-boundary", 14),
        ("aligned-global-allocation-export-boundary", 17),
        ("aligned-heap-allocation-export-boundary", 15),
        ("aligned-realloc-request-boundary", 15),
        ("aligned-realloc-copy-boundary", 4),
        ("aligned-recalloc-product-boundary", 8),
        ("aligned-realloc-global-export-boundary", 14),
        ("aligned-realloc-heap-realloc-boundary", 8),
        ("aligned-realloc-heap-rezalloc-boundary", 8),
        ("aligned-realloc-heap-recalloc-boundary", 10),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            alloc_aligned_rs.matches(&marker).count(),
            expected_count,
            "aligned allocation unsafe boundary {audit_id} should retain exactly {expected_count} local operation markers"
        );
    }
    assert!(
        !free_rs.contains("SAFETY[AUDIT:free-validation-boundary]")
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-realloc-heap-export-boundary]"),
        "free validation and heap aligned realloc markers should stay split into focused operation families"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/aligned.rs\t"))
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-allocation-boundary]")
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-heap-allocation-boundary]")
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-realloc-boundary]")
            && alloc_aligned_rs.contains("fn mi_malloc_is_naturally_aligned(")
            && alloc_aligned_rs.contains("fn aligned_layout_or_report(")
            && alloc_aligned_rs.contains("crate::core_types::LayoutRequest::aligned_at(")
            && alloc_aligned_rs.contains("pub unsafe extern \"C\" fn mi_malloc_aligned")
            && alloc_aligned_rs.contains("unsafe fn mi_theap_realloc_zero_aligned_at_req(")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call("),
        "aligned allocation unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
    for (audit_id, min_count) in [
        ("allocation-platform-shim-boundary", 4),
        ("allocation-product-entry-boundary", 20),
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
        ("allocation-string-path-boundary", 20),
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
            alloc_rs.matches(&marker).count() >= min_count,
            "allocation unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    for retired_audit_id in [
        "allocation-heap-dispatch-boundary",
        "allocation-realloc-entry-boundary",
        "allocation-cxx-new-boundary",
    ] {
        assert!(
            !alloc_rs.contains(&format!("SAFETY[AUDIT:{retired_audit_id}]")),
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
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_strdup(")
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
        ("page-stats-public-boundary", 19),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            page_rs.matches(&marker).count() >= min_count,
            "page unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !page_rs.contains("SAFETY[AUDIT:page-free-list-refill-boundary]")
            && !page_rs.contains("SAFETY[AUDIT:page-queue-transition-boundary]"),
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
            && page_rs.contains("pub unsafe extern \"C\" fn _mi_page_stats_bin(")
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
        ptr_rs,
        stats_rs,
    });
    typed_helpers::assert_typed_allocation_helpers(sources);
}
