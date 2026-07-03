pub(super) fn assert_page_map_marker_inventory(page_map: &str, unsafe_allowlist: &str) {
    for internal_name in [
        "pub static mut _mi_page_map",
        "pub unsafe extern \"C\" fn _mi_page_map_init",
        "pub unsafe extern \"C\" fn _mi_page_map_register",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister_range",
        "pub unsafe extern \"C\" fn _mi_page_map_unsafe_destroy",
        "pub unsafe extern \"C\" fn _mi_safe_ptr_page",
    ] {
        let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
        assert!(
            !page_map.contains(&no_mangle_export),
            "{internal_name} should stay crate-internal instead of a dynamic export"
        );
    }
    for (audit_id, expected_count) in [
        ("page-map-global-boundary", 12),
        ("page-map-32bit-commit-bitmap-boundary", 8),
        ("page-map-32bit-bootstrap-boundary", 9),
        ("page-map-top-level-lazy-commit-boundary", 4),
        ("page-map-global-bootstrap-boundary", 12),
        ("page-map-initial-submap-boundary", 4),
        ("page-map-null-sentinel-boundary", 8),
        ("page-map-teardown-boundary", 13),
        ("page-map-commit-boundary", 5),
        ("page-map-page-register-boundary", 14),
        ("page-map-range-register-boundary", 6),
        ("page-map-page-unregister-boundary", 15),
        ("page-map-range-unregister-boundary", 10),
        ("page-map-submap-publication-boundary", 9),
        ("page-map-range-primitive-boundary", 5),
        ("page-map-lookup32-boundary", 9),
        ("page-map-lookup64-boundary", 10),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            page_map.matches(&marker).count(),
            expected_count,
            "page-map unsafe boundary {audit_id} should retain exactly {expected_count} local operation markers"
        );
    }
    assert!(
        !unsafe_allowlist.contains("src/allocator/page_map.rs")
            && !page_map.contains("SAFETY[AUDIT:page-map-boundary]")
            && !page_map.contains("SAFETY[AUDIT:page-map-register-boundary]")
            && !page_map.contains("SAFETY[AUDIT:page-map-init-boundary]")
            && page_map.contains("pub(crate) static mut _mi_page_map")
            && page_map.contains("static mut mi_page_map_max_address")
            && page_map.contains("static mut mi_page_map_memid")
            && page_map.contains("static mut mi_page_map_lock")
            && page_map.contains(
                "crate::core_types::AtomicPtrField::from_raw(unsafe { _mi_page_map.add(idx.get()) });"
            )
            && page_map
                .contains("RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)")
            && page_map.contains("pub unsafe extern \"C\" fn mi_is_in_heap_region"),
        "page-map unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
}
