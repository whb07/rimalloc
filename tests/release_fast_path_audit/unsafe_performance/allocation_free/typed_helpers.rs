use super::*;

#[path = "typed_helpers/atomic_once.rs"]
mod atomic_once;
#[path = "typed_helpers/local_helpers.rs"]
mod local_helpers;

pub(super) fn assert_typed_allocation_helpers(sources: &UnsafePerformanceSources) {
    let internal = sources.internal.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let atomic_rs = sources.atomic_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let validation_types = sources.validation_types.as_str();
    let page_states_rs = sources.page_states_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let alloc_aligned_rs = sources.alloc_aligned_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();

    local_helpers::assert_local_helper_boundaries(options_rs, free_rs, page_rs, heap_rs, prim_rs);
    atomic_once::assert_atomic_once_boundaries(atomic_rs, atomic_types);

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
        free_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign")
            && free_rs.contains(
                "pub(crate) unsafe fn mi_validate_block_from_ptr(\n    page: crate::core_types::PagePtr,\n    user: crate::core_types::UserPtr,\n) -> crate::core_types::PointerLookup"
            )
            && free_rs.contains(") -> crate::core_types::PointerLocation")
            && free_rs.contains("PointerLocation::Guarded")
            && validation_types.contains("pub(crate) struct GuardedBlock(BlockPtr);")
            && validation_types.contains("pub(crate) fn located_block(self) -> LocatedBlock")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("pub(crate) fn block(&self) -> &BlockForFree")
            && validation_types.contains("pub(crate) fn into_parts(self) -> ValidatedFreeParts")
            && validation_types
                .contains("pub(crate) fn into_free_block(self) -> crate::core_types::FreeBlock")
            && free_rs.contains("let validated = lookup.prepare_free()?;")
            && free_rs.contains("mi_block_check_unguard(validated.page(), validated.block(), validated.user())")
            && free_rs.contains("let parts = validated.into_parts();")
            && free_rs.contains("mi_free_block_mt(foreign_page, block);")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), block);")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && alloc_rs.contains("let (user, page, location) = valid.into_parts();")
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
            && !validation_types.contains("pub(crate) struct GuardedBlock(pub(crate) BlockPtr);")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new"),
        "free validation should preserve typed lookup and consume linear free capabilities exactly once"
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
