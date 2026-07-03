pub(super) fn assert_arena_marker_inventory(arena_region: &str, unsafe_allowlist: &str) {
    let arena_main_rs = include_str!("../../../../src/allocator/arena.rs");
    let os_abandoned_rs = include_str!("../../../../src/allocator/arena/os_abandoned.rs");
    let focused_markers = [
        "SAFETY[AUDIT:arena-slice-geometry-boundary]",
        "SAFETY[AUDIT:arena-slice-containment-boundary]",
        "SAFETY[AUDIT:arena-slice-layout-boundary]",
        "SAFETY[AUDIT:arena-registry-count-index-boundary]",
        "SAFETY[AUDIT:arena-heap-page-table-boundary]",
        "SAFETY[AUDIT:arena-registry-start-index-boundary]",
        "SAFETY[AUDIT:arena-registry-suitability-boundary]",
        "SAFETY[AUDIT:arena-registry-iteration-boundary]",
        "SAFETY[AUDIT:arena-registry-destroy-boundary]",
        "SAFETY[AUDIT:arena-registry-add-boundary]",
        "SAFETY[AUDIT:arena-slice-commit-boundary]",
        "SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]",
        "SAFETY[AUDIT:arena-slice-commit-accounting-boundary]",
        "SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]",
        "SAFETY[AUDIT:arena-initialization-boundary]",
        "SAFETY[AUDIT:arena-initialization-page-table-boundary]",
        "SAFETY[AUDIT:arena-reserve-growth-boundary]",
        "SAFETY[AUDIT:arena-os-fallback-boundary]",
        "SAFETY[AUDIT:arena-public-reserve-entry-boundary]",
        "SAFETY[AUDIT:arena-managed-os-memory-boundary]",
        "SAFETY[AUDIT:arena-huge-reservation-boundary]",
        "SAFETY[AUDIT:arena-abandoned-boundary]",
        "SAFETY[AUDIT:arena-abandoned-page-table-boundary]",
        "SAFETY[AUDIT:arena-page-fresh-boundary]",
        "SAFETY[AUDIT:arena-page-fresh-metadata-boundary]",
        "SAFETY[AUDIT:arena-page-regular-boundary]",
        "SAFETY[AUDIT:arena-page-singleton-boundary]",
        "SAFETY[AUDIT:arena-page-free-entry-boundary]",
        "SAFETY[AUDIT:arena-page-free-validation-boundary]",
        "SAFETY[AUDIT:arena-page-free-stats-boundary]",
        "SAFETY[AUDIT:arena-page-free-release-boundary]",
        "SAFETY[AUDIT:arena-page-abandon-boundary]",
        "SAFETY[AUDIT:arena-page-abandon-os-list-boundary]",
        "SAFETY[AUDIT:arena-os-abandoned-list-boundary]",
        "SAFETY[AUDIT:arena-page-reabandon-boundary]",
        "SAFETY[AUDIT:arena-page-unabandon-entry-boundary]",
        "SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]",
        "SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]",
        "SAFETY[AUDIT:arena-heap-page-publication-boundary]",
        "SAFETY[AUDIT:arena-page-ownership-transition-boundary]",
        "SAFETY[AUDIT:arena-purge-schedule-boundary]",
        "SAFETY[AUDIT:arena-purge-free-dispatch-boundary]",
        "SAFETY[AUDIT:arena-purge-direct-boundary]",
        "SAFETY[AUDIT:arena-purge-range-boundary]",
        "SAFETY[AUDIT:arena-purge-visitor-boundary]",
        "SAFETY[AUDIT:arena-purge-expire-policy-boundary]",
        "SAFETY[AUDIT:arena-purge-collect-guard-boundary]",
    ];
    let marker_total = focused_markers
        .into_iter()
        .map(|marker| arena_region.matches(marker).count())
        .sum::<usize>();
    for (audit_id, min_count) in [
        ("arena-slice-geometry-boundary", 11),
        ("arena-slice-containment-boundary", 13),
        ("arena-slice-layout-boundary", 7),
        ("arena-registry-count-index-boundary", 6),
        ("arena-heap-page-table-boundary", 5),
        ("arena-registry-start-index-boundary", 3),
        ("arena-registry-suitability-boundary", 3),
        ("arena-registry-iteration-boundary", 4),
        ("arena-registry-destroy-boundary", 6),
        ("arena-registry-add-boundary", 10),
        ("arena-slice-commit-boundary", 3),
        ("arena-slice-allocation-bitmaps-boundary", 7),
        ("arena-slice-commit-accounting-boundary", 14),
        ("arena-slice-allocation-dispatch-boundary", 6),
        ("arena-initialization-boundary", 13),
        ("arena-initialization-page-table-boundary", 4),
        ("arena-reserve-growth-boundary", 11),
        ("arena-os-fallback-boundary", 15),
        ("arena-public-reserve-entry-boundary", 8),
        ("arena-managed-os-memory-boundary", 10),
        ("arena-huge-reservation-boundary", 14),
        ("arena-abandoned-boundary", 16),
        ("arena-abandoned-page-table-boundary", 6),
        ("arena-page-fresh-boundary", 8),
        ("arena-page-fresh-metadata-boundary", 14),
        ("arena-page-regular-boundary", 11),
        ("arena-page-singleton-boundary", 2),
        ("arena-page-free-entry-boundary", 4),
        ("arena-page-free-validation-boundary", 9),
        ("arena-page-free-stats-boundary", 4),
        ("arena-page-free-release-boundary", 6),
        ("arena-page-abandon-boundary", 15),
        ("arena-page-abandon-os-list-boundary", 1),
        ("arena-os-abandoned-list-boundary", 11),
        ("arena-page-reabandon-boundary", 8),
        ("arena-page-unabandon-entry-boundary", 6),
        ("arena-page-unabandon-mapped-boundary", 5),
        ("arena-page-unabandon-os-list-boundary", 3),
        ("arena-heap-page-publication-boundary", 3),
        ("arena-page-ownership-transition-boundary", 6),
        ("arena-purge-schedule-boundary", 6),
        ("arena-purge-free-dispatch-boundary", 6),
        ("arena-purge-direct-boundary", 5),
        ("arena-purge-range-boundary", 5),
        ("arena-purge-visitor-boundary", 3),
        ("arena-purge-expire-policy-boundary", 5),
        ("arena-purge-collect-guard-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            arena_region.matches(&marker).count() >= min_count,
            "arena unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/arena"))
            && !arena_region.contains("SAFETY[AUDIT:arena-region-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-allocation-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-page-lifecycle-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-registry-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-purge-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-slice-boundary]")
            && !arena_region.contains("SAFETY[AUDIT:arena-slice-allocation-boundary]")
            && marker_total >= 340
            && focused_markers
                .into_iter()
                .all(|marker| arena_region.contains(marker)),
        "arena unsafe boundaries should stay split into focused operation markers without a migration allowlist entry"
    );
    assert!(
        arena_main_rs.contains("os_abandoned::push_front(")
            && arena_main_rs.contains("os_abandoned::remove(")
            && !arena_main_rs.contains("os_abandoned_pages")
            && !arena_main_rs.contains("os_abandoned_pages_lock")
            && os_abandoned_rs.contains("pub(super) unsafe fn push_front(")
            && os_abandoned_rs.contains("pub(super) unsafe fn remove(")
            && os_abandoned_rs.contains("pub(super) unsafe fn first_snapshot(")
            && os_abandoned_rs.contains("pub(super) unsafe fn is_empty("),
        "heap OS abandoned-page list head and link mutation should stay in the private helper"
    );
}
