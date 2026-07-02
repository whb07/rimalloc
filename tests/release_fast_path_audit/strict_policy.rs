use super::*;

#[path = "strict_policy/allowlist_governance.rs"]
mod allowlist_governance;

#[test]
fn c_memory_order_enum_stays_in_atomic_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    for (rel, source) in sources {
        if rel == "src/internal/atomic.rs" {
            continue;
        }
        assert!(
            !source.contains("mi_memory_order_e")
                && !source.contains("mi_memory_order::")
                && !source.contains("mi_memory_order_"),
            "{rel} should use core_types::MemoryOrder or Ordering, not C-compatible mi_memory_order_e"
        );
    }
}

#[test]
fn strict_type_modules_remain_no_std_no_heap_alloc_no_std_sync() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_types = root.join("src/types");
    let mut sources = Vec::new();
    collect_rs_sources(root, &core_types, &mut sources);

    let allowed_core_atomic_modules = BTreeSet::from([
        "src/types/arena.rs",
        "src/types/atomic.rs",
        "src/types/constants.rs",
        "src/types/ptr.rs",
    ]);

    for (rel, source) in sources {
        for forbidden in [
            "extern crate std",
            "use std",
            "std::",
            "extern crate alloc",
            "crate::alloc::",
            "::alloc::",
            "use alloc::",
            "Vec<",
            "Vec::",
            "Box<",
            "Box::",
            "String",
            "format!",
            "to_string(",
            "to_owned(",
            "mi_malloc",
            "mi_zalloc",
            "mi_calloc",
            "mi_realloc",
            "mi_heap_zalloc",
            "_mi_meta_alloc",
            "_mi_meta_zalloc",
            "_mi_arenas_alloc",
            "Mutex",
            "MutexGuard",
            "RwLock",
            "OnceLock",
            "LazyLock",
            "Condvar",
            "std::thread",
        ] {
            assert!(
                !source.contains(forbidden),
                "{rel} should not introduce std, heap allocation, formatting allocation, or std sync through {forbidden}"
            );
        }

        if source.contains("core::sync::atomic") {
            assert!(
                allowed_core_atomic_modules.contains(rel.as_str()),
                "{rel} should route strict-type atomic access through the dedicated atomic/cell wrappers"
            );
        }
    }
}

#[test]
fn strict_type_wrappers_do_not_allocate_on_alloc_free_hot_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let core_types = root.join("src/types");
    let mut strict_type_sources = Vec::new();
    collect_rs_sources(root, &core_types, &mut strict_type_sources);

    for (rel, source) in strict_type_sources {
        assert_no_rust_heap_constructs(&rel, "strict type module", &source);
    }

    for (source_name, source, marker) in [
        (
            "src/allocator/alloc.rs",
            alloc_rs.as_str(),
            "pub(crate) unsafe fn _mi_page_malloc_zero_typed(",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs.as_str(),
            "unsafe fn mi_theap_malloc_generic(",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs.as_str(),
            "unsafe fn mi_theap_malloc_zero_req(",
        ),
        (
            "src/allocator/page.rs",
            page_rs.as_str(),
            "pub(crate) unsafe fn _mi_malloc_generic_typed(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn mi_free_generic_local(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn mi_free_generic_mt(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "unsafe fn mi_prepare_validated_free(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "unsafe fn mi_free_validated(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "unsafe fn mi_free_fast_try(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "pub(crate) unsafe fn mi_route_free(",
        ),
    ] {
        let item = source_item(source_name, source, marker);
        assert_no_rust_heap_constructs(source_name, marker, item);
    }

    for marker in [
        "pub(crate) unsafe extern \"C\" fn mi_free_generic_local(",
        "pub(crate) unsafe extern \"C\" fn mi_free_generic_mt(",
        "unsafe fn mi_prepare_validated_free(",
        "unsafe fn mi_free_validated(",
        "unsafe fn mi_free_fast_try(",
        "pub(crate) unsafe fn mi_route_free(",
    ] {
        let item = source_item("src/allocator/free.rs", &free_rs, marker);
        for forbidden in [
            "mi_malloc(",
            "mi_zalloc(",
            "mi_calloc(",
            "mi_realloc(",
            "mi_heap_zalloc(",
            "_mi_meta_alloc",
            "_mi_meta_zalloc",
            "_mi_arenas_alloc",
        ] {
            assert!(
                !item.contains(forbidden),
                "{marker} in src/allocator/free.rs should not allocate on the free hot path through {forbidden}"
            );
        }
    }

    assert!(
        alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::MallocGenericFlags::new(")
            && alloc_rs.contains("crate::core_types::RequestedSize::new(size).padded_unchecked()")
            && alloc_rs.contains(
                "unsafe fn mi_theap_malloc_small_zero_nonnull(\n    theap: ThreadHeapLocal,\n    size: crate::core_types::RequestedSize,"
            )
            && alloc_rs.contains(
                "unsafe fn mi_heap_try_new_typed(\n    heap: BorrowedHeap,\n    size: crate::core_types::RequestedSize,"
            )
            && alloc_rs.contains(
                "unsafe fn mi_heap_alloc_new_typed(\n    heap: BorrowedHeap,\n    size: crate::core_types::RequestedSize,"
            )
            && page_rs.contains("layout: crate::core_types::LayoutRequest")
            && page_rs.contains("flags: crate::core_types::MallocGenericFlags")
            && free_rs.contains(".prepare_free()")
            && free_rs.contains("validated: crate::core_types::ValidatedFree")
            && free_rs.contains(
                "unsafe fn mi_free_validated(validated: crate::core_types::ValidatedFree,"
            )
            && free_rs.contains("let owner = validated.page().classify_for_free(current_tid);")
            && free_rs.contains("owner: crate::core_types::PageFreeOwner,"),
        "allocation/free hot paths should continue to consume strict-type wrappers under the no-allocation source guard"
    );

    assert!(
        free_rs.contains("fn mi_free_fast_path_enabled(usable: *mut usize) -> bool")
            && free_rs.contains("&& !cfg!(debug_assertions)")
            && free_rs.contains("&& MI_DEBUG == 0")
            && free_rs.contains("&& MI_SECURE == 0")
            && free_rs.contains("&& MI_GUARDED == 0")
            && free_rs.contains("&& MI_PADDING == 0")
            && free_rs.contains("if mi_free_fast_try(p, usable) {\n            return;\n        }")
            && free_rs.contains("crate::core_types::FreePathKind::LocalFast =>")
            && free_rs.contains("crate::core_types::FreePathKind::RemotePlain")
            && free_rs.contains("if !page_tid.is_abandoned() && !page_tid.is_abandoned_mapped()")
            && source_item(
                "src/allocator/free.rs",
                &free_rs,
                "pub unsafe extern \"C\" fn mi_ufree("
            )
            .contains("mi_route_free(p, c\"mi_ufree\".as_ptr(), usable);"),
        "mi_free should keep the release-only no-usable fast route while mi_ufree keeps the usable-size route"
    );
}

#[test]
fn datatype_acceptance_criteria_have_cross_cutting_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut rust_sources = Vec::new();
    collect_rs_sources(root, &src, &mut rust_sources);

    let types_spec = fs::read_to_string(root.join("TYPES_SPEC.md")).expect("read TYPES_SPEC.md");
    let unsafe_migration_allowlist =
        fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
            .expect("read unsafe migration allowlist");
    let strict_type_allowlist = fs::read_to_string(root.join("strict-type-allowlist.txt"))
        .expect("read strict-type allowlist");
    let strict_type_checker = fs::read_to_string(root.join("scripts/check-strict-types.py"))
        .expect("read strict-type checker");
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");

    datatype_acceptance::assert_datatype_acceptance_criteria_have_cross_cutting_guards(
        datatype_acceptance::DatatypeAcceptanceSources {
            rust_sources: &rust_sources,
            types_spec: &types_spec,
            unsafe_migration_allowlist: &unsafe_migration_allowlist,
            strict_type_allowlist: &strict_type_allowlist,
            strict_type_checker: &strict_type_checker,
            todo: &todo,
        },
    );
}
