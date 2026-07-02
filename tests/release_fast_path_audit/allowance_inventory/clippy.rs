use super::*;

#[test]
fn phase6_clippy_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let expected = BTreeMap::from([
        (("src/allocator/arena.rs", "too_many_arguments"), 3),
        (("src/allocator/arena/debug.rs", "too_many_arguments"), 1),
        (
            ("src/allocator/arena/os_memory.rs", "too_many_arguments"),
            1,
        ),
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
