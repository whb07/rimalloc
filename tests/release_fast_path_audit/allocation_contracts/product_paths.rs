use std::fs;
use std::path::Path;

#[test]
fn allocation_product_paths_use_typed_requests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let aligned_rs =
        fs::read_to_string(root.join("src/allocator/aligned.rs")).expect("read alloc_aligned.rs");
    let aligned_thread_heap_rs =
        fs::read_to_string(root.join("src/allocator/aligned/thread_heap.rs"))
            .expect("read src/allocator/aligned/thread_heap.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let alloc_cxx_new_rs = fs::read_to_string(root.join("src/allocator/alloc/cxx_new.rs"))
        .expect("read src/allocator/alloc/cxx_new.rs");
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
            && alloc_cxx_new_rs.contains("alloc_product_or_report(newcount, size)")
            && alloc_cxx_new_rs.contains(
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
            && aligned_thread_heap_rs.contains("calloc_request_or_report(count, size)")
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
                .matches("SAFETY[AUDIT:raw-posix-reallocarray-boundary]")
                .count()
                >= 5
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-reallocarr-boundary]")
                .count()
                >= 11
            && posix_rs
                .matches("SAFETY[AUDIT:raw-posix-expand-boundary]")
                .count()
                >= 3
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
