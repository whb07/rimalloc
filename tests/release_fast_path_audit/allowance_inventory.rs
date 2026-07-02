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

#[test]
fn phase6_clippy_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let expected = BTreeMap::from([
        (("src/allocator/arena.rs", "too_many_arguments"), 4),
        (("src/allocator/arena/debug.rs", "too_many_arguments"), 1),
        (("src/internal/bitmap.rs", "many_single_char_names"), 1),
        (("src/allocator/free.rs", "absurd_extreme_comparisons"), 1),
        (("src/ffi/libc.rs", "many_single_char_names"), 2),
        (("src/raw/macros.rs", "must_use_candidate"), 1),
        (("src/platform/os.rs", "absurd_extreme_comparisons"), 3),
        (("src/platform/os.rs", "too_many_arguments"), 1),
        (("src/platform/unix.rs", "struct_field_names"), 1),
        (("src/allocator/page.rs", "absurd_extreme_comparisons"), 1),
        (("src/internal/random.rs", "many_single_char_names"), 1),
    ]);

    assert_eq!(
        clippy_allowance_counts(&sources),
        expected,
        "localized Clippy allowances should remain limited to audited C-shaped arity and secure/debug constant comparisons"
    );

    let safe_rs = fs::read_to_string(root.join("src/safe/mod.rs")).expect("read safe facade");
    let callbacks_rs =
        fs::read_to_string(root.join("src/ffi/callbacks.rs")).expect("read callback guards");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read raw facade");
    let safe_boundary_script =
        fs::read_to_string(root.join("scripts/check-safe-boundary.py")).expect("read safe check");
    assert!(
        safe_rs.starts_with("#![deny(clippy::must_use_candidate)]"),
        "safe Rust facade should locally deny missing must-use markers even while translated internals keep the package allowance"
    );
    assert!(
        callbacks_rs.contains("#![deny(clippy::must_use_candidate)]")
            && raw_rs.contains("#![deny(clippy::must_use_candidate)]")
            && safe_boundary_script.contains("STRICT_LINT_POLICY_FILES = {")
            && safe_boundary_script.contains("\"src/safe/mod.rs\"")
            && safe_boundary_script.contains("\"src/ffi/callbacks.rs\"")
            && safe_boundary_script.contains("\"src/raw/mod.rs\"")
            && safe_boundary_script.contains("SAFE_FACING_LINT_FILES = [")
            && safe_boundary_script.contains("allow(clippy::"),
        "safe-facing callback and raw facade glue should locally deny must-use drift and CI should reject unaudited Clippy allowances"
    );
}

#[test]
fn package_clippy_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read manifest");
    let lints = package_clippy_lints(&manifest);

    let allowed = BTreeSet::from([
        "borrow_as_ptr",
        "bool_to_int_with_if",
        "branches_sharing_code",
        "cast_lossless",
        "cast_possible_truncation",
        "cast_possible_wrap",
        "cast_precision_loss",
        "cast_ptr_alignment",
        "cast_sign_loss",
        "checked_conversions",
        "doc_markdown",
        "elidable_lifetime_names",
        "explicit_iter_loop",
        "if_not_else",
        "inline_always",
        "items_after_statements",
        "iter_with_drain",
        "large_stack_arrays",
        "literal_string_with_formatting_args",
        "manual_let_else",
        "many_single_char_names",
        "map_unwrap_or",
        "match_same_arms",
        "match_wild_err_arm",
        "missing_const_for_fn",
        "missing_errors_doc",
        "must_use_candidate",
        "needless_continue",
        "needless_pass_by_value",
        "needless_raw_string_hashes",
        "no_effect_underscore_binding",
        "option_if_let_else",
        "option_option",
        "ptr_as_ptr",
        "ptr_cast_constness",
        "pub_underscore_fields",
        "redundant_closure_for_method_calls",
        "redundant_pub_crate",
        "ref_as_ptr",
        "semicolon_if_nothing_returned",
        "significant_drop_tightening",
        "similar_names",
        "single_match_else",
        "suboptimal_flops",
        "too_many_lines",
        "trivially_copy_pass_by_ref",
        "uninlined_format_args",
        "unnecessary_semicolon",
        "unreadable_literal",
        "use_self",
        "used_underscore_binding",
        "used_underscore_items",
        "useless_let_if_seq",
        "while_float",
        "wildcard_imports",
    ]);

    let actual_allowed: BTreeSet<_> = lints
        .iter()
        .filter_map(|(lint, level)| (*level == "\"allow\"").then_some(*lint))
        .collect();
    assert_eq!(
        actual_allowed, allowed,
        "package-level Clippy allowances should stay on the audited mechanical-translation allowlist"
    );

    for lint in ["all", "cargo", "nursery", "pedantic", "missing_safety_doc"] {
        assert!(
            !matches!(lints.get(lint), Some(&"\"allow\"")),
            "{lint} must not become a package-level allowance"
        );
    }
}

#[test]
fn phase6_unsafe_op_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let actual = sources
        .iter()
        .filter_map(|(rel, source)| {
            source
                .contains("unsafe_op_in_unsafe_fn")
                .then_some(rel.as_str())
        })
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::<&str>::new();

    assert_eq!(
        actual, expected,
        "broad unsafe_op_in_unsafe_fn allowances should not be reintroduced"
    );
    assert!(
        !actual.contains("src/allocator/aligned.rs")
            && !actual.contains("src/allocator/alloc.rs")
            && !actual.contains("src/raw/posix.rs")
            && !actual.contains("src/raw/macros.rs")
            && !actual.contains("src/allocator/arena_meta.rs")
            && !actual.contains("src/internal/atomic.rs")
            && !actual.contains("src/internal/bitmap.rs")
            && !actual.contains("src/types/mod.rs")
            && !actual.contains("src/allocator/free.rs")
            && !actual.contains("src/allocator/heap.rs")
            && !actual.contains("src/internal/mod.rs")
            && !actual.contains("src/ffi/libc.rs")
            && !actual.contains("src/allocator/options.rs")
            && !actual.contains("src/platform/os.rs")
            && !actual.contains("src/allocator/page.rs")
            && !actual.contains("src/platform/mod.rs")
            && !actual.contains("src/platform/unix.rs")
            && !actual.contains("src/internal/random.rs")
            && !actual.contains("src/allocator/thread_heap.rs")
            && !actual.contains("src/allocator/threadlocal.rs")
            && !actual.contains("src/allocator/tracking.rs"),
        "allocation helpers, aligned allocation helpers, allocation override shims, POSIX allocation shims, arena metadata, atomic/lock helpers, bitmap helpers, core types, free-path helpers, heap helpers, internal helpers, libc helpers, options helpers, OS boundary helpers, page helpers, Unix/Emscripten/WASI/macOS primitive helpers, primitive TLS helpers, random helpers, thread-heap helpers, thread-local helpers, and tracking shims should not regain unsafe_op_in_unsafe_fn after explicit unsafe cleanup"
    );
}

#[test]
fn phase6_c_compat_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    const LINTS: &[&str] = &[
        "non_camel_case_types",
        "non_upper_case_globals",
        "non_snake_case",
        "unused_macros",
        "unused_imports",
        "improper_ctypes_definitions",
    ];
    let expected = BTreeMap::from([
        (("src/ffi/abi.rs", "non_camel_case_types"), 1),
        (("src/ffi/abi.rs", "non_upper_case_globals"), 1),
        (("src/allocator/alloc.rs", "non_snake_case"), 1),
        (("src/allocator/aligned.rs", "non_camel_case_types"), 1),
        (("src/allocator/aligned.rs", "non_upper_case_globals"), 1),
        (("src/raw/macros.rs", "improper_ctypes_definitions"), 1),
        (("src/raw/macros.rs", "non_camel_case_types"), 1),
        (("src/raw/macros.rs", "non_snake_case"), 1),
        (("src/raw/macros.rs", "non_upper_case_globals"), 1),
        (("src/allocator/arena.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena.rs", "non_upper_case_globals"), 1),
        (("src/allocator/arena/debug.rs", "non_camel_case_types"), 2),
        (
            ("src/allocator/arena/heap_pages.rs", "non_camel_case_types"),
            2,
        ),
        (("src/allocator/arena/prefix.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena_meta.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena_meta.rs", "non_upper_case_globals"), 1),
        (("src/internal/atomic.rs", "non_camel_case_types"), 1),
        (("src/internal/atomic.rs", "non_upper_case_globals"), 1),
        (("src/internal/bitmap.rs", "non_camel_case_types"), 1),
        (("src/internal/bitmap.rs", "non_snake_case"), 1),
        (("src/internal/bitmap.rs", "non_upper_case_globals"), 1),
        (("src/internal/bits.rs", "non_camel_case_types"), 1),
        (("src/internal/bits.rs", "non_upper_case_globals"), 1),
        (("src/ffi/headers.rs", "non_upper_case_globals"), 14),
        (("src/allocator/init.rs", "non_camel_case_types"), 1),
        (("src/allocator/init.rs", "non_snake_case"), 7),
        (("src/allocator/init.rs", "non_upper_case_globals"), 1),
        (("src/internal/mod.rs", "non_camel_case_types"), 1),
        (("src/internal/memid.rs", "non_camel_case_types"), 1),
        (("src/internal/mod.rs", "non_snake_case"), 1),
        (("src/internal/mod.rs", "non_upper_case_globals"), 1),
        (("src/internal/mod.rs", "unused_imports"), 1),
        (("src/lib.rs", "unused_imports"), 2),
        (("src/ffi/libc.rs", "non_camel_case_types"), 1),
        (("src/ffi/libc.rs", "non_upper_case_globals"), 1),
        (("src/allocator/options.rs", "non_camel_case_types"), 1),
        (("src/allocator/options.rs", "non_upper_case_globals"), 1),
        (("src/platform/os.rs", "non_camel_case_types"), 1),
        (("src/platform/os.rs", "non_upper_case_globals"), 1),
        (("src/allocator/page.rs", "non_camel_case_types"), 1),
        (("src/allocator/page.rs", "non_upper_case_globals"), 1),
        (("src/allocator/page_map.rs", "non_camel_case_types"), 1),
        (("src/allocator/page_map.rs", "non_upper_case_globals"), 1),
        (("src/platform/mod.rs", "non_camel_case_types"), 1),
        (("src/platform/mod.rs", "non_upper_case_globals"), 1),
        (("src/platform/emscripten.rs", "non_camel_case_types"), 1),
        (("src/platform/emscripten.rs", "non_upper_case_globals"), 1),
        (("src/platform/macos.rs", "non_camel_case_types"), 1),
        (("src/platform/macos.rs", "non_snake_case"), 1),
        (("src/platform/macos.rs", "non_upper_case_globals"), 1),
        (("src/platform/macos.rs", "unused_imports"), 1),
        (("src/platform/macos.rs", "unused_macros"), 1),
        (("src/platform/unix.rs", "non_camel_case_types"), 1),
        (("src/platform/unix.rs", "non_upper_case_globals"), 1),
        (("src/platform/wasi.rs", "non_camel_case_types"), 1),
        (("src/platform/wasi.rs", "non_upper_case_globals"), 1),
        (("src/platform/windows.rs", "non_camel_case_types"), 1),
        (("src/platform/windows.rs", "non_snake_case"), 1),
        (("src/platform/windows.rs", "non_upper_case_globals"), 1),
        (("src/platform/windows.rs", "unused_macros"), 1),
        (("src/internal/random.rs", "non_camel_case_types"), 1),
        (("src/internal/random.rs", "non_upper_case_globals"), 1),
        (("src/raw/mod.rs", "unused_imports"), 1),
        (("src/allocator/stats.rs", "non_camel_case_types"), 1),
        (("src/allocator/stats.rs", "non_upper_case_globals"), 1),
        (("src/allocator/thread_heap.rs", "non_camel_case_types"), 1),
        (("src/allocator/threadlocal.rs", "non_camel_case_types"), 1),
        (
            ("src/allocator/threadlocal.rs", "non_upper_case_globals"),
            1,
        ),
        (("src/types/constants.rs", "non_camel_case_types"), 1),
        (("src/types/constants.rs", "non_upper_case_globals"), 1),
    ]);

    assert_eq!(
        rust_allowance_counts(&sources, LINTS),
        expected,
        "C compatibility allowances should stay limited to audited ABI, re-export, and mechanical-translation surfaces"
    );
    for cleaned in ["src/allocator/static_alloc.rs", "src/allocator/tracking.rs"] {
        assert!(
            !rust_allowance_counts(&sources, LINTS)
                .keys()
                .any(|(rel, _)| *rel == cleaned),
            "{cleaned} should not regain C naming allowances"
        );
    }
}
