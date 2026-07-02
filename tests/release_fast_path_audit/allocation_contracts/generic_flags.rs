use std::fs;
use std::path::Path;

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
