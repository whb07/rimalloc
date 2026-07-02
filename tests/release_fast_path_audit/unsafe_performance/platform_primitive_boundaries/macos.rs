use super::*;

#[path = "macos/batch.rs"]
mod batch;
#[path = "macos/layout.rs"]
mod layout;

#[test]
fn macos_malloc_zone_override_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let macos_rs =
        fs::read_to_string(root.join("src/platform/macos.rs")).expect("read src/platform/macos.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/macos.rs\t")),
        "macOS malloc-zone override should stay off the unsafe migration allowlist"
    );
    assert!(
        !macos_rs.contains("SAFETY[AUDIT:macos-zone-override]"),
        "macOS malloc-zone override markers should stay split by operation"
    );
    for (audit_id, exact_count) in [
        ("macos-zone-layout-boundary", 5),
        ("macos-zone-lookup-boundary", 16),
        ("macos-zone-allocation-boundary", 14),
        ("macos-zone-size-query-boundary", 3),
        ("macos-zone-foreign-forwarding-boundary", 5),
        ("macos-zone-batch-boundary", 7),
        ("macos-zone-fork-boundary", 9),
        ("macos-zone-compat-boundary", 12),
        ("macos-zone-interpose-boundary", 1),
        ("macos-zone-init-boundary", 11),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            macos_rs.matches(&marker).count(),
            exact_count,
            "macOS malloc-zone marker {audit_id} should keep exact operation count"
        );
    }
    layout::assert_macos_zone_layout(&macos_rs);

    for (marker, expected) in [
        (
            "pub unsafe extern \"C\" fn mi_malloc_create_zone(",
            "unsafe { mi_get_default_zone() }",
        ),
        (
            "pub unsafe extern \"C\" fn mi_malloc_default_zone()",
            "unsafe { mi_get_default_zone() }",
        ),
        (
            "pub unsafe extern \"C\" fn mi_malloc_default_purgeable_zone()",
            "unsafe { mi_get_default_zone() }",
        ),
    ] {
        let item = source_item("src/platform/macos.rs", &macos_rs, marker);
        assert_no_rust_heap_constructs("src/platform/macos.rs", marker, item);
        assert!(
            item.contains(expected),
            "{marker} should continue returning the process-global mimalloc zone"
        );
    }

    let all_zones = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn mi_malloc_get_all_zones(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/macos.rs",
        "pub unsafe extern \"C\" fn mi_malloc_get_all_zones",
        all_zones,
    );
    assert!(
        all_zones.contains("if !addresses.is_null()")
            && all_zones.contains("*addresses = null_mut();")
            && all_zones.contains("if !count.is_null()")
            && all_zones.contains("*count = 0;")
            && all_zones.contains("KERN_SUCCESS"),
        "macOS all-zones shim should preserve nullable out-pointer writes and success result"
    );

    let zone_size = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_size(",
    );
    assert!(
        zone_size.contains("mi_any_heap_contains(p)")
            && zone_size.contains("crate::mimalloc::mi_usable_size(p)")
            && zone_size.contains("!unsafe { is_mimalloc_zone(zone) }")
            && zone_size.contains("match unsafe { (*zone).size }")
            && zone_size.contains("Some(size_fn) => unsafe { size_fn(zone, p) }"),
        "zone_size should preserve mimalloc ownership detection and foreign-zone forwarding"
    );

    let zone_free = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_free(",
    );
    assert!(
        zone_free.contains("mi_any_heap_contains(p.cast_const())")
            && zone_free.contains("crate::mimalloc::mi_free(p)")
            && zone_free.contains("!unsafe { is_mimalloc_zone(zone) }")
            && zone_free.contains("let Some(free_fn) = unsafe { (*zone).free }")
            && zone_free.contains("unsafe { free_fn(zone, p) };"),
        "zone_free should keep mimalloc local free and non-mimalloc zone forwarding"
    );

    let zone_realloc = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_realloc(",
    );
    assert!(
        zone_realloc.contains("if p.is_null() || unsafe { mi_any_heap_contains(p.cast_const()) }")
            && zone_realloc.contains("crate::mimalloc::mi_realloc(p, newsize)")
            && zone_realloc.contains("match unsafe { (*zone).realloc }")
            && zone_realloc.contains("Some(realloc_fn) => unsafe { realloc_fn(zone, p, newsize) }")
            && zone_realloc.contains("None => null_mut()"),
        "zone_realloc should preserve null/mimalloc routing and foreign-zone fallback"
    );

    batch::assert_macos_batch_boundaries(&macos_rs);

    for symbol in [
        "pub unsafe extern \"C\" fn _malloc_fork_prepare()",
        "pub unsafe extern \"C\" fn _malloc_fork_parent()",
        "pub unsafe extern \"C\" fn _malloc_fork_child()",
    ] {
        let item = source_item("src/platform/macos.rs", &macos_rs, symbol);
        assert!(
            item.contains("#[unsafe(no_mangle)]")
                || macos_rs[..macos_rs.find(symbol).expect("fork hook symbol present")]
                    .rsplit("\n\n")
                    .next()
                    .is_some_and(|prefix| prefix.contains("#[unsafe(no_mangle)]")),
            "{symbol} should keep the exported Darwin fork-hook symbol"
        );
    }

    let override_malloc = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn _mi_macos_override_malloc(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/macos.rs",
        "pub unsafe extern \"C\" fn _mi_macos_override_malloc",
        override_malloc,
    );
    assert!(
        override_malloc.contains("malloc_zone_register(addr_of_mut!(mi_malloc_zone))")
            && override_malloc
                .contains("malloc_get_all_zones(0, null_mut(), &mut zones, &mut count)")
            && override_malloc.contains("ret == KERN_SUCCESS && count > 0 && !zones.is_null()")
            && override_malloc.contains("unsafe { *zones }")
            && override_malloc.contains("unsafe { malloc_default_zone() }")
            && override_malloc.contains("malloc_zone_unregister(default_zone)")
            && override_malloc.contains("malloc_zone_register(default_zone)")
            && override_malloc.contains("if !purgeable_zone.is_null()")
            && override_malloc.contains("malloc_zone_unregister(purgeable_zone)")
            && override_malloc.contains("malloc_zone_register(purgeable_zone)"),
        "macOS override setup should preserve default and purgeable zone ordering"
    );
}
