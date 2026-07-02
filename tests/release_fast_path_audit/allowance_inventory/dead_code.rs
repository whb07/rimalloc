use super::*;

#[test]
fn phase6_dead_code_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let mut actual = BTreeMap::new();
    let mut module_actual = BTreeMap::new();
    for (rel, source) in &sources {
        let count = dead_code_allowance_count(source);
        if count > 0 {
            actual.insert(rel.as_str(), count);
        }
        let module_count = module_dead_code_allowance_count(source);
        if module_count > 0 {
            module_actual.insert(rel.as_str(), module_count);
        }
    }

    let expected = BTreeMap::from([
        ("src/allocator/alloc.rs", 1),
        ("src/allocator/aligned.rs", 1),
        ("src/raw/macros.rs", 1),
        ("src/allocator/arena.rs", 1),
        ("src/allocator/arena/debug.rs", 13),
        ("src/allocator/arena_meta.rs", 1),
        ("src/internal/atomic.rs", 1),
        ("src/internal/bitmap.rs", 1),
        ("src/types/freelist_wrappers.rs", 1),
        ("src/allocator/free.rs", 3),
        ("src/allocator/init.rs", 1),
        ("src/internal/mod.rs", 1),
        ("src/ffi/libc.rs", 1),
        ("src/allocator/options.rs", 1),
        ("src/allocator/page.rs", 1),
        ("src/platform/mod.rs", 1),
        ("src/platform/emscripten.rs", 1),
        ("src/platform/macos.rs", 1),
        ("src/platform/unix.rs", 1),
        ("src/platform/wasi.rs", 1),
        ("src/platform/windows.rs", 3),
        ("src/allocator/static_alloc.rs", 1),
        ("src/allocator/stats.rs", 1),
        ("src/allocator/thread_heap.rs", 1),
        ("src/allocator/tracking.rs", 6),
        ("src/types/constants.rs", 1),
        ("src/types/track.rs", 2),
    ]);
    let expected_module_level = BTreeMap::from([
        ("src/allocator/aligned.rs", 1),
        ("src/raw/macros.rs", 1),
        ("src/allocator/arena_meta.rs", 1),
        ("src/internal/atomic.rs", 1),
        ("src/internal/bitmap.rs", 1),
        ("src/allocator/init.rs", 1),
        ("src/internal/mod.rs", 1),
        ("src/ffi/libc.rs", 1),
        ("src/allocator/options.rs", 1),
        ("src/allocator/page.rs", 1),
        ("src/platform/mod.rs", 1),
        ("src/platform/emscripten.rs", 1),
        ("src/platform/macos.rs", 1),
        ("src/platform/unix.rs", 1),
        ("src/platform/wasi.rs", 1),
        ("src/platform/windows.rs", 1),
        ("src/allocator/static_alloc.rs", 1),
        ("src/allocator/stats.rs", 1),
        ("src/allocator/thread_heap.rs", 1),
        ("src/types/constants.rs", 1),
    ]);

    assert_eq!(
        actual, expected,
        "Phase 6 dead_code allowances should stay on the audited compatibility/scaffold allowlist"
    );
    assert_eq!(
        module_actual, expected_module_level,
        "broad module-level dead_code allowances should stay limited to permanent compatibility shims"
    );
    assert!(
        !actual.contains_key("src/lib.rs"),
        "crate-wide dead_code suppression should not return"
    );

    let safe_allowances: Vec<_> = actual
        .keys()
        .filter(|path| path.starts_with("src/safe"))
        .collect();
    assert!(
        safe_allowances.is_empty(),
        "safe facade modules should stay free of dead_code allowances: {safe_allowances:?}"
    );

    let core_type_allowances: Vec<_> = actual
        .keys()
        .filter(|path| path.starts_with("src/types/"))
        .copied()
        .collect();
    assert_eq!(
        core_type_allowances,
        vec![
            "src/types/constants.rs",
            "src/types/freelist_wrappers.rs",
            "src/types/track.rs"
        ],
        "core type dead_code allowances should remain item-level and explicitly justified"
    );
}
