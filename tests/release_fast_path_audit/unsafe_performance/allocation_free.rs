use super::*;

pub(super) fn assert_allocator_allocation_and_free_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let internal = sources.internal.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let atomic_rs = sources.atomic_rs.as_str();
    let bitmap_rs = sources.bitmap_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let ptr_rs = sources.ptr_rs.as_str();
    let validation_types = sources.validation_types.as_str();
    let page_states_rs = sources.page_states_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_bins_rs = sources.page_bins_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let alloc_aligned_rs = sources.alloc_aligned_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();

    assert!(
        !free_rs.contains("clippy::if_same_then_else")
            && !bitmap_rs.contains("clippy::if_same_then_else"),
        "free.rs and bitmap.rs should not retain obsolete if_same_then_else allowances"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/free.rs\t"))
            && free_rs.matches("SAFETY[AUDIT:free-path-boundary]").count() >= 60
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
    for (audit_id, min_count) in [
        ("aligned-padding-boundary", 20),
        ("aligned-heap-allocation-boundary", 60),
        ("aligned-realloc-boundary", 60),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            alloc_aligned_rs.matches(&marker).count() >= min_count,
            "aligned allocation unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/aligned.rs\t"))
            && !alloc_aligned_rs.contains("SAFETY[AUDIT:aligned-allocation-boundary]")
            && alloc_aligned_rs.contains("fn mi_malloc_is_naturally_aligned(")
            && alloc_aligned_rs.contains("fn aligned_layout_or_report(")
            && alloc_aligned_rs.contains("crate::core_types::LayoutRequest::aligned_at(")
            && alloc_aligned_rs.contains("pub unsafe extern \"C\" fn mi_malloc_aligned")
            && alloc_aligned_rs.contains("unsafe fn mi_theap_realloc_zero_aligned_at_req(")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call("),
        "aligned allocation unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
    for (audit_id, min_count) in [
        ("allocation-abi-entry-boundary", 100),
        ("allocation-heap-dispatch-boundary", 30),
        ("allocation-layout-request-boundary", 15),
        ("allocation-page-refill-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            alloc_rs.matches(&marker).count() >= min_count,
            "allocation unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/alloc.rs\t"))
            && !alloc_rs.contains("SAFETY[AUDIT:allocation-entry-boundary]")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_malloc(")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_calloc(")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_realloc(")
            && alloc_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_malloc_zero(")
            && alloc_rs.contains("pub(crate) unsafe fn mi_theap_realloc_req(")
            && alloc_rs.contains("pub(crate) fn calloc_request_or_report(")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::PaddingCanary::from_page_block_keys("),
        "allocation unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
    for (audit_id, min_count) in [
        ("page-free-list-refill-boundary", 30),
        ("page-queue-transition-boundary", 49),
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
    assert!(
        options_rs.contains(
            "#[inline(never)]\npub(crate) extern \"C\" fn _mi_option_get_fast(option: mi_option_t) -> c_long"
        ) && options_rs.contains("mi_option_get_fast_impl(option)")
            && options_rs.contains("option_value_load(idx)")
            && free_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_reclaim_on_free)")
            && free_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_cross_thread_max_reclaim)")
            && page_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_max_candidates)"),
        "fast option reads should keep C's local _mi_option_get_fast helper boundary on free/page policy paths"
    );
    assert!(
        heap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t"
        ) && heap_rs.contains("if heap.is_null() || _mi_is_heap_main(heap)")
            && heap_rs.contains("crate::init::_mi_theap_main_safe()")
            && heap_rs.contains("_mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()")
            && heap_rs.contains("let mut theap = unsafe { _mi_heap_theap_get_peek(heap) };")
            && prim_rs.contains("_mi_heap_theap_get_peek(heap)"),
        "heap/theap cache misses should keep C's local _mi_heap_theap_get_peek no-create helper boundary"
    );

    assert!(
        alloc_rs
            .contains("pub(crate) unsafe fn _mi_page_malloc_zero_typed(\n    theap: *mut mi_theap_t,\n    available: crate::core_types::AvailablePage,\n    padded: crate::core_types::PaddedSize,")
            && alloc_rs.contains("zeroed: crate::core_types::Zeroed,")
            && alloc_rs.contains("let page_ptr = available.page();")
            && alloc_rs.contains("let size = padded.get();")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::Zeroed::from_bool(zero)")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(")
            && alloc_rs.contains("let padded = crate::core_types::PaddedSize::new_unchecked(size);")
            && alloc_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_malloc_zero")
            && alloc_rs.contains("let Some(available) = active.available() else {")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(theap, available, padded, zeroed, ptr::null_mut())")
            && page_rs.contains(
                "crate::core_types::Zeroed::from_bool(zero),\n            ptr::null_mut(),"
            )
            && internal.contains("pub(crate) unsafe fn _mi_theap_get_free_small_page_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    padded: crate::core_types::PaddedSize,")
            && internal.contains("let theap = theap.ptr().as_ptr();")
            && internal.contains("let idx = mi_direct_bin_from_padded(padded).as_usize();")
            && alloc_rs.contains("let page = _mi_theap_get_free_small_page_typed(local_theap, padded);")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(theap, available, padded, zeroed, usable)")
            && alloc_aligned_rs.contains(
                "let page =\n            unsafe { crate::internal::_mi_theap_get_free_small_page_typed(local_theap, padded) };"
            )
            && alloc_aligned_rs.contains(
                "let available = unsafe { active.available().unwrap_unchecked() };"
            )
            && alloc_aligned_rs.contains(
                "_mi_page_malloc_zero_typed(\n                        theap,\n                        available,\n                        padded,"
            )
            && alloc_aligned_rs.contains("crate::core_types::Zeroed::from_bool(zero),")
            && alloc_aligned_rs.contains("let padded = layout.padded();")
            && !page_rs.contains("let p = _mi_page_malloc_zero(theap, page, size, zero);")
            && !alloc_aligned_rs
                .contains("let p = _mi_page_malloc_zero(theap, page, padsize, zero);")
            && !alloc_rs.contains("_mi_theap_get_free_small_page(theap, padded.get())")
            && !alloc_aligned_rs.contains("_mi_theap_get_free_small_page(theap, padsize)"),
        "page and aligned allocation should use typed padded-size helpers internally"
    );
    assert!(
        source_has_inline_never_function(
            atomic_rs,
            "pub unsafe extern \"C\" fn _mi_atomic_once_enter",
        ) && source_has_inline_never_function(
            atomic_rs,
            "pub unsafe extern \"C\" fn _mi_atomic_once_release",
        ) && atomic_types.contains("pub(crate) struct OnceGuard<'a>")
            && atomic_types
                .contains("pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self>")
            && atomic_types.contains("impl Drop for OnceGuard<'_>")
            && atomic_types.contains("crate::atomic::_mi_atomic_once_release(")
            && atomic_rs.contains("_once: crate::core_types::OnceGuard<'static>")
            && atomic_rs.contains("crate::core_types::OnceGuard::try_enter(state)")
            && !atomic_rs.contains("once: *mut mi_atomic_once_t,\n}")
            && !atomic_rs.contains("_mi_atomic_once_release(self.once);"),
        "atomic once should keep C's enter/release helper boundaries and release through typed OnceGuard Drop"
    );

    assert!(
        free_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign")
            && free_rs.contains(
                "crate::internal::mi_assert!(\n                _mi_page_ptr_unalign(page_raw, user_raw) == user_raw.cast()\n            );"
            )
            && free_rs.contains(
                "pub(crate) unsafe fn mi_validate_block_from_ptr(\n    page: crate::core_types::PagePtr,\n    user: crate::core_types::UserPtr,\n) -> crate::core_types::PointerLookup"
            )
            && free_rs.contains("let location = mi_pointer_location(page, user, MI_SECURE > 0, true);")
            && free_rs.contains("unsafe fn mi_lookup_pointer_page(")
            && free_rs.contains("user: crate::core_types::UserPtr,")
            && free_rs.contains(") -> Option<crate::core_types::PagePtr>")
            && free_rs.contains("let registered_page = mi_lookup_pointer_page(user_ptr);")
            && free_rs.contains("unsafe fn mi_pointer_location(")
            && free_rs.contains("page: crate::core_types::PagePtr,")
            && free_rs.contains("user: crate::core_types::UserPtr,")
            && free_rs.contains(") -> crate::core_types::PointerLocation")
            && free_rs.contains("let page_raw = page.as_ptr();")
            && free_rs.contains("let user_raw = user.as_ptr();")
            && free_rs.contains("let block = if mi_page_has_interior_pointers(page_raw) || force_unalign")
            && free_rs.contains("PointerLocation::Guarded")
            && validation_types.contains("pub(crate) struct LocatedBlock")
            && validation_types.contains("pub(crate) struct GuardedBlock(BlockPtr);")
            && validation_types.contains("pub(crate) fn after_guarded_detection(block: BlockPtr) -> Self")
            && validation_types.contains("impl PointerLocation")
            && validation_types.contains("pub(crate) fn located_block(self) -> LocatedBlock")
            && validation_types.contains("Valid(ValidPointerLookup)")
            && validation_types.contains("pub(crate) struct ValidPointerLookup {\n    user: UserPtr,\n    page: PagePtr,\n    location: PointerLocation,")
            && validation_types.contains("pub(crate) fn valid(user: UserPtr, page: PagePtr, location: PointerLocation) -> Self")
            && validation_types.contains("pub(crate) struct BlockForFree {\n    block: BlockPtr,\n    was_guarded: WasGuarded,")
            && validation_types.contains("pub(crate) fn is_guarded(self) -> bool")
            && validation_types
                .contains("pub(crate) fn into_free_block(self) -> crate::core_types::FreeBlock")
            && validation_types.contains("pub(crate) struct ValidatedFree {\n    user: UserPtr,\n    page: PagePtr,\n    block: BlockForFree,")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("ValidatedFree { user, page, block }")
            && validation_types.contains("pub(crate) fn page(self) -> PagePtr")
            && validation_types.contains("pub(crate) fn block(self) -> BlockForFree")
            && free_rs.contains("/// # Safety\n/// `lookup` must come from validating the same page/user pair that is about to\n/// be freed by the generic free path.")
            && free_rs.contains("/// the block; this boundary consumes the lookup and performs any required\n/// guarded-block unprotection before returning the freeable block witness.")
            && free_rs.contains("unsafe fn mi_block_for_free_from_lookup(")
            && free_rs.contains("let validated = lookup.prepare_free()?;")
            && free_rs.contains("unsafe fn mi_prepare_validated_free(")
            && free_rs.contains("mi_prepare_validated_free(lookup).map(crate::core_types::ValidatedFree::block)")
            && free_rs.contains("let block = block_wrapper.block().as_ptr();")
            && free_rs.contains("let was_guarded = block_wrapper.is_guarded();")
            && free_rs.contains("let page = validated.page().as_ptr();")
            && free_rs.contains("validated.block()")
            && free_rs.contains("let path = page_tid.classify_free_path(current_tid);")
            && free_rs.contains("let owner = validated.page().classify_for_free(current_tid);")
            && free_rs.contains("crate::core_types::PageFreeOwner::Local(local_page)")
            && free_rs.contains("owner: crate::core_types::PageFreeOwner,")
            && free_rs.contains("crate::core_types::PageFreeOwner::Foreign(foreign_page)")
            && free_rs.contains("mi_free_block_local(\n                    local_page,")
            && free_rs.contains("mi_free_block_mt(foreign_page, validated.block());")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());")
            && !free_rs.contains("let delta = page_tid.xor_current(current_tid);")
            && !source_item(
                "src/allocator/free.rs",
                free_rs,
                "pub(crate) unsafe fn mi_free_ex_slow("
            )
            .contains("LocalPage::new_unchecked")
            && !source_item(
                "src/allocator/free.rs",
                free_rs,
                "pub(crate) unsafe fn mi_free_ex_slow("
            )
            .contains("ForeignPage::new_unchecked")
            && free_rs.contains("GuardedBlock::after_guarded_detection(block)")
            && free_rs.contains("GuardedBlock::after_guarded_detection(block_ptr)")
            && free_rs.contains("if let Some(validated) = mi_prepare_validated_free(mi_validate_pointer(p, msg))")
            && free_rs.contains("let location = mi_pointer_location(page_ptr, user_ptr, false, false);")
            && free_rs.contains("PointerLookup::valid(user_ptr, page_ptr, location)")
            && free_rs.matches("mi_block_check_unguard(validated.page(), validated.block(), validated.user())").count() == 1
            && !free_rs.contains("let Some((validated, located))")
            && !free_rs.contains("let block_raw = if mi_page_has_interior_pointers(page_raw)")
            && alloc_rs.contains("let location = valid.location();")
            && alloc_rs.contains("let block = location.block();")
            && free_rs.matches("let page = crate::core_types::PagePtr::new_unchecked(page);").count() == 2
            && free_rs.matches("let user = crate::core_types::UserPtr::new_unchecked(p);").count() == 2
            && free_rs
                .matches("mi_prepare_validated_free(mi_validate_block_from_ptr(page, user))")
                .count()
                == 2
            && free_rs.contains("LocalPage::new_unchecked(validated.page())")
            && free_rs.contains("ForeignPage::new_unchecked(validated.page())")
            && !free_rs.contains("mi_block_for_free_from_lookup(mi_validate_block_from_ptr(page, user))")
            && !free_rs.contains("mi_block_for_free_from_lookup(page, p, mi_validate_block_from_ptr(page, p))")
            && !validation_types.contains("Valid {\n        user: UserPtr")
            && !validation_types.contains("pub(crate) block: BlockPtr")
            && !validation_types.contains("pub(crate) page: PagePtr")
            && !validation_types.contains("pub(crate) struct GuardedBlock(pub(crate) BlockPtr);")
            && !validation_types.contains("pub(crate) fn new(user: UserPtr")
            && !validation_types.contains("pub(crate) fn new(block: BlockPtr")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new")
            && !free_rs.contains("validated.block,")
            && !free_rs.contains("PointerLocation::BlockStart(b) => (b, false)")
            && !free_rs.contains("pub unsafe fn mi_validate_block_from_ptr(\n    page: *const mi_page_t,\n    p: *mut c_void,\n) -> *mut mi_block_t"),
        "free validation should keep _mi_page_ptr_unalign while returning PointerLookup and BlockForFree"
    );
    assert!(
        !heap_rs.contains("fn _mi_theap_cached_set(")
            && !heap_rs.contains("fn _mi_theap_create(")
            && !heap_rs.contains("fn _mi_theap_free(")
            && !heap_rs.contains("fn _mi_heap_move_pages(")
            && !heap_rs.contains("fn _mi_heap_destroy_pages("),
        "heap internals should call crate-local theap helpers directly"
    );
}
