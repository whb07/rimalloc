use super::*;

pub(super) fn assert_header_forwarding_boundaries(sources: &UnsafePerformanceSources) {
    let raw_macros_rs = sources.raw_macros_rs.as_str();

    assert!(
        raw_macros_rs
            .matches("SAFETY[AUDIT:raw-macro-header-forwarding-boundary]")
            .count()
            >= 6,
        "header forwarding raw facade should keep operation-level audit markers"
    );
    for marker in [
        "pub unsafe fn mi_forward1",
        "pub unsafe fn mi_forward3",
        "pub unsafe fn mi_forward02",
        "pub unsafe fn mi_vfree_forward",
        "pub unsafe fn mi_aligned_alloc_forward",
    ] {
        let item = source_item("src/raw/macros.rs", raw_macros_rs, marker);
        assert_no_rust_heap_constructs("src/raw/macros.rs", marker, item);
    }
    assert!(
        raw_macros_rs.contains("pub unsafe fn mi_malloc_good_size_forward")
            && raw_macros_rs.contains("pub unsafe fn mi_malloc_size_forward")
            && raw_macros_rs.contains("crate::mimalloc::mi_malloc_good_size(size)")
            && raw_macros_rs.contains("crate::mimalloc::mi_usable_size(p)"),
        "header/query forwarding should keep tiny direct calls into the public mimalloc facade"
    );
}
