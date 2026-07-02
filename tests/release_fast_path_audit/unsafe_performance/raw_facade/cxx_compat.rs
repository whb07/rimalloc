use super::*;

pub(super) fn assert_cxx_compatibility_boundaries(sources: &UnsafePerformanceSources) {
    let alloc_rs = sources.alloc_rs.as_str();
    let raw_macros_rs = sources.raw_macros_rs.as_str();

    assert!(
        raw_macros_rs
            .matches("SAFETY[AUDIT:raw-macro-cxx-boundary]")
            .count()
            >= 45,
        "C++ raw facade compatibility should keep operation-level audit markers"
    );
    for symbol in [
        "pub static _mi_cxx_interposes: [mi_interpose_s; 8]",
        "unsafe(export_name = \"_ZdlPv\")",
        "unsafe(export_name = \"_Znwm\")",
        "pub unsafe extern \"C\" fn mi_cxx_delete",
        "pub unsafe extern \"C\" fn mi_cxx_delete_array",
        "pub unsafe extern \"C\" fn mi_cxx_new_64",
        "pub unsafe extern \"C\" fn mi_cxx_new_array_64",
        "pub unsafe fn mi_operator_new_array_nothrow",
        "pub unsafe fn mi_operator_delete_sized",
        "pub unsafe fn mi_operator_new_aligned",
    ] {
        assert!(
            raw_macros_rs.contains(symbol),
            "C++ raw facade should retain `{symbol}`"
        );
    }
    assert!(
        alloc_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn mi_theap_try_new")
            && alloc_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_try_new")
            && alloc_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_try_new")
            && alloc_rs.contains("static MI_THEAP_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_HEAP_TRY_NEW_KEEP"),
        "C++ new retry paths should keep separate thread-heap, default-heap, and explicit-heap helper boundaries"
    );
}
