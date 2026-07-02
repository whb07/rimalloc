use super::*;

#[test]
fn allocation_product_paths_use_typed_requests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let aligned_rs =
        fs::read_to_string(root.join("src/allocator/aligned.rs")).expect("read alloc_aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let posix_rs = fs::read_to_string(root.join("src/raw/posix.rs")).expect("read alloc_posix.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    let product_overflow_checks = alloc_rs.matches("mi_count_size_overflow").count()
        + aligned_rs.matches("mi_count_size_overflow").count()
        + posix_rs.matches("mi_count_size_overflow").count();

    assert!(
        product_overflow_checks == 1
            && !alloc_rs.contains("Temp compat bridge")
            && !aligned_rs.contains("mi_count_size_overflow")
            && !aligned_rs.contains("_mi_theap_realloc_zero(")
            && !posix_rs.contains("mi_count_size_overflow")
            && alloc_rs.contains(
                "fn product_size_or_report(\n    count: crate::core_types::ItemCount,\n    size: crate::core_types::RequestedSize,"
            )
            && alloc_rs.contains("pub(crate) fn calloc_request_or_report(")
            && alloc_rs.contains("fn alloc_product_or_report(")
            && alloc_rs
                .contains("crate::internal::mi_count_size_overflow(count.get(), size.get(), &mut total)")
            && alloc_rs.contains(
                "crate::core_types::CallocRequest::from_checked_total(count.get(), size.get(), total.get())"
            )
            && alloc_rs.contains(
                "crate::core_types::AllocProduct::from_checked_total(count.get(), size.get(), total.get())"
            )
            && !alloc_rs.contains("CallocRequest::new(count, size).ok()")
            && !alloc_rs.contains("AllocProduct::new(count, size).ok()")
            && alloc_rs.contains("unsafe fn mi_theap_calloc_req(")
            && alloc_rs.contains("unsafe fn mi_theap_mallocn_req(")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_req(")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_request(")
            && alloc_rs.contains("unsafe fn mi_theap_reallocf_req(")
            && alloc_rs.contains("unsafe fn mi_theap_reallocn_req(")
            && alloc_rs.contains("unsafe fn mi_theap_recalloc_req(")
            && alloc_rs.contains("alloc_product_or_report(newcount, size)")
            && alloc_rs.contains(
                "unsafe fn mi_theap_alloc_new_n(\n    theap: *mut mi_theap_t,\n    req: crate::core_types::AllocProduct,"
            )
            && alloc_rs.contains("realloc_req: crate::core_types::ReallocRequest,")
            && alloc_rs.contains("mi_theap_recalloc_req(theap, req, realloc_req)")
            && alloc_rs.contains("mi_theap_realloc_typed(theap, realloc_req)")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_null(")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_existing(")
            && alloc_rs.contains("zero_new_tail == crate::core_types::Zeroed::Yes")
            && alloc_rs.contains("req.zero_new_tail(),")
            && layout_rs.contains("pub(crate) struct AllocProduct")
            && layout_rs.contains("pub(crate) struct ReallocRequest")
            && layout_rs.contains("enum ReallocSource")
            && layout_rs.contains("pub(crate) fn zero_new_tail(&self) -> Zeroed")
            && layout_rs.contains("pub(crate) unsafe fn from_checked_total(")
            && aligned_rs.contains("unsafe fn mi_theap_realloc_zero_aligned_at_req(")
            && aligned_rs.contains("let offset = crate::core_types::AlignmentOffset::new(offset);")
            && aligned_rs.contains(
                "crate::core_types::LayoutRequest::aligned_at(newsize, alignment, offset, zeroed)"
            )
            && aligned_rs.contains("mi_theap_realloc_layout(theap, p, layout)")
            && aligned_rs.contains("calloc_request_or_report(count, size)")
            && aligned_rs.contains("calloc_request_or_report(newcount, size)")
            && posix_rs.contains("crate::core_types::CallocRequest::new(count, size)")
            && !posix_rs.contains("SAFETY[AUDIT:raw-posix-compat]")
            && posix_rs.matches("SAFETY[AUDIT:raw-posix-errno-boundary]").count() >= 5
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-query-free-boundary]")
                .count()
                >= 7
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-aligned-allocation-boundary]")
                .count()
                >= 13
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-realloc-boundary]")
                .count()
                >= 19
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-string-dup-boundary]")
                .count()
                >= 7
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-environment-boundary]")
                .count()
                >= 13
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-aligned-recalloc-boundary]")
                .count()
                >= 6
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_posix_memalign")
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_reallocarray")
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_reallocarr")
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_dupenv_s")
            && layout_rs.contains("pub(crate) fn layout(self) -> LayoutRequest")
            && layout_rs.contains("let total_size = count.checked_mul(size)"),
        "calloc product-size paths should use CallocRequest, non-calloc product paths should use AllocProduct, and both should share one C-compatible overflow reporting adapter"
    );
}

#[test]
fn realloc_validation_uses_pointer_lookup_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let aligned_rs =
        fs::read_to_string(root.join("src/allocator/aligned.rs")).expect("read alloc_aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    assert!(
        alloc_rs.contains(
            "unsafe fn mi_validate_ptr_page(\n    p: *const c_void,\n    msg: *const c_char,\n) -> crate::core_types::PointerLookup"
        ) && alloc_rs.contains("crate::free::mi_validate_pointer(p.cast_mut(), msg)")
            && alloc_rs.contains("unsafe fn mi_realloc_old_allocation(")
            && alloc_rs.contains("crate::core_types::PointerLookup::Valid(valid)")
            && alloc_rs.contains("let user = valid.user();")
            && alloc_rs.contains("let page = valid.page();")
            && alloc_rs.contains("let location = valid.location();")
            && alloc_rs.contains("let block = location.block();")
            && alloc_rs.contains("Some(crate::core_types::OldAllocation::from_validated_parts(")
            && alloc_rs.contains("unsafe fn mi_realloc_request_from_ptr(")
            && alloc_rs
                .contains("let Some(old) = mi_realloc_old_allocation(p, c\"mi_realloc\".as_ptr())")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_validated_core(")
            && alloc_rs.contains("zero_new_tail: crate::core_types::Zeroed,")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_null(")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_existing(")
            && alloc_rs.contains("req.old(),")
            && alloc_rs.contains("let page = old.page().as_ptr();")
            && alloc_rs.contains("let size = old.usable();")
            && alloc_rs
                .contains("crate::core_types::PreserveBytes::new(old.usable(), new_requested)")
            && alloc_rs.contains("let preserved = preserve.map_or(0, |preserve| preserve.get());")
            && alloc_rs.contains("let Some(preserve) = preserve else")
            && alloc_rs.contains("let copysize = preserve.get();")
            && !alloc_rs.contains("let copysize = if newsize > size { size } else { newsize };")
            && aligned_rs.contains("let preserve = crate::core_types::PreserveBytes::new(")
            && aligned_rs.contains("crate::core_types::UsableSize::new(size)")
            && aligned_rs.contains("_mi_memcpy_aligned(newp, p, preserve.get())")
            && !aligned_rs.contains("usize::min(newsize, size)")
            && alloc_rs.contains("crate::core_types::PointerLookup::Invalid { .. }")
            && layout_rs.contains("pub(crate) struct OldAllocation")
            && layout_rs.contains("pub(crate) struct PreserveBytes(usize);")
            && layout_rs.contains("Self(usize::min(old.get(), new.get()))")
            && layout_rs.contains("unsafe fn new(")
            && !layout_rs.contains("pub(crate) unsafe fn new(")
            && layout_rs.contains("pub(crate) unsafe fn from_validated_parts(")
            && layout_rs.contains("unsafe { Self::new(ptr, usable, page, block) }")
            && layout_rs.contains("pub(crate) fn usable(self) -> UsableSize")
            && layout_rs.contains("pub(crate) fn page(self) -> PagePtr")
            && layout_rs.contains("enum ReallocSource")
            && layout_rs.contains("ReallocSource::Null")
            && layout_rs.contains("ReallocSource::Existing(old)")
            && layout_rs.contains("pub(crate) fn from_null(")
            && layout_rs.contains("pub(crate) fn from_existing(")
            && layout_rs.contains("pub(crate) fn old(&self) -> Option<OldAllocation>")
            && !layout_rs.contains("old: Option<OldAllocation>")
            && !layout_rs.contains("with_old")
            && !alloc_rs.contains("unsafe fn mi_validate_ptr_page(p: *const c_void, msg: *const c_char) -> *mut mi_page_t")
            && !alloc_rs.contains("_mi_safe_ptr_page"),
        "realloc must use PointerLookup and construct OldAllocation only after validation"
    );
}

#[test]
fn generic_allocation_flags_use_typed_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    assert!(
        !alloc_rs.contains("usize::from(zero) | huge_alignment")
            && !page_rs.contains("zero_huge_alignment & !1")
            && !page_rs.contains("zero_huge_alignment & 1")
            && alloc_rs.contains("crate::core_types::MallocGenericFlags::new(")
            && alloc_rs.contains("fn maybe_alignment_or_report(alignment: usize)")
            && alloc_rs.contains("crate::core_types::MaybeAlignment::new(alignment)")
            && alloc_rs.contains("crate::core_types::MaybeHugeAlignment::new(huge_alignment.get())")
            && alloc_rs.contains("fn no_alignment() -> crate::core_types::MaybeAlignment")
            && alloc_rs.contains("unsafe fn mi_new_aligned_typed(")
            && alloc_rs.contains("unsafe fn mi_new_aligned_nothrow_typed(")
            && alloc_rs.contains("alignment: crate::core_types::Alignment,")
            && page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(")
            && page_rs.contains("theap: crate::core_types::ThreadHeapPtr")
            && page_rs.contains("pub(crate) unsafe fn _mi_theap_or_default_typed(")
            && page_rs.contains("Some(crate::core_types::ThreadHeapPtr::new_unchecked(theap))")
            && page_rs.contains("layout: crate::core_types::LayoutRequest")
            && page_rs.contains("let size = layout.padded().get();")
            && page_rs.contains("let flags = crate::core_types::MallocGenericFlags::from_raw(zero_huge_alignment)")
            && layout_rs.contains("pub(crate) unsafe fn from_raw(raw: usize) -> Self")
            && page_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("fn malloc_layout_or_report(size: usize, zero: bool)")
            && alloc_rs.contains("crate::core_types::LayoutRequest::zalloc(size)")
            && alloc_rs.contains("crate::core_types::LayoutRequest::malloc(size)")
            && alloc_rs.contains("let p = _mi_malloc_generic_typed(theap.ptr(), layout, flags, usable)")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("unsafe fn mi_theap_malloc_generic(\n    theap: ThreadHeapLocal,\n    layout: crate::core_types::LayoutRequest,\n    flags: crate::core_types::MallocGenericFlags,")
            && alloc_rs.contains("unsafe fn mi_theap_malloc_zero_req(\n    theap: *mut mi_theap_t,\n    layout: crate::core_types::LayoutRequest,\n    huge_alignment: crate::core_types::MaybeAlignment,")
            && alloc_rs.contains("let Some(layout) = malloc_layout_or_report(size, zero)")
            && !alloc_rs.contains("mi_theap_malloc_generic(theap, size, zero, huge_alignment, usable)")
            && layout_rs.contains("pub(crate) unsafe fn from_padded_unchecked(")
            && layout_rs.contains("requested_size = padded.get().wrapping_sub(crate::types::MI_PADDING_SIZE)")
            && page_rs.contains("flags.zeroed() == crate::core_types::Zeroed::Yes")
            && page_rs.contains("flags.huge_alignment().get()")
            && layout_rs.contains("pub(crate) struct MaybeHugeAlignment(usize);")
            && layout_rs.contains("pub(crate) struct MallocGenericFlags(usize);")
            && !page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(\n    mut theap: *mut mi_theap_t,")
            && !page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(\n    mut theap: *mut mi_theap_t,\n    size: usize,"),
        "generic allocation should use MallocGenericFlags instead of manually packing zero and huge alignment bits"
    );
}
