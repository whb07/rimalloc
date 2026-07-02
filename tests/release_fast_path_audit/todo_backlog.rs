use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "datatype_acceptance.rs",
    "release_policy.rs",
    "release_policy/security.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/arena_boundaries.rs",
    "unsafe_performance/bitmap_boundaries.rs",
    "unsafe_performance/heap_page_visitors.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
    "unsafe_performance/local_boundaries.rs",
    "unsafe_performance/page_lifecycle.rs",
    "unsafe_performance/platform_primitive_boundaries.rs",
    "unsafe_performance/size_class_boundaries.rs",
    "unsafe_performance/tail_guards.rs",
    "unsafe_performance/threadlocal_boundaries.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Strict-Type and Audit Granularity Backlog",
        "## Issue 1: Make strict-type line-pattern checks item-aware",
        "## Issue 2: Narrow platform strict-type wildcard allowances",
        "## Issue 3: Narrow allocator and internal strict-type wildcard allowances",
        "## Issue 4: Split the arena audit operation cluster",
        "## Issue 5: Split the initialization lifecycle audit cluster",
        "## Issue 6: Shrink release-audit review units and lower line caps",
        "## Suggested implementation order",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`da60fc0`",
        "`docs/unsafe-migration-allowlist.tsv` has 0 translation entries and no\n  production entries",
        "`ALLOWLIST_ENTRY_CAP` is 0",
        "covers 82 source files, 5,893 unsafe mentions,\n  and 86 operation IDs",
        "`strict-type-allowlist.txt` has 413 entries",
        "242 raw-pointer-parameter\n  entries, 123 bare-domain `usize` entries, 19 pointer/usize round-trip",
        "`REMOVE_ENTRY_CAP` remains 0",
        "45 strict-type entries still use symbol `*`",
        "14 raw-pointer-parameter\n  entries, 16 bare-domain `usize` entries, 7 pointer/usize round-trip",
        "`scripts/check-strict-types.py` now checks line-pattern findings with an\n  item-aware context",
        "deliberate wildcard\n  fallback entries for module-level or true ABI boundaries",
        "`src/allocator/alloc.rs`,\n  `src/allocator/free.rs`, and `src/allocator/init.rs`",
        "The targeted platform, allocator, and internal files no\n  longer have strict-type wildcard entries",
        "SAFETY[AUDIT:init-lifecycle-boundary]` has 277 markers",
        "SAFETY[AUDIT:raw-macro-allocation-boundary]` has 178 markers",
        "SAFETY[AUDIT:allocation-entry-boundary]` has 166 markers",
        "SAFETY[AUDIT:stats-reporting-boundary]` has 161 markers",
        "SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers",
        "SAFETY[AUDIT:aligned-allocation-boundary]` has 158 markers",
        "SAFETY[AUDIT:page-map-boundary]` has 153 markers",
        "SAFETY[AUDIT:os-memory-boundary]` has 136 markers",
        "SAFETY[AUDIT:windows-primitive-boundary]` has 114 markers",
        "The arena audit cluster is now split into `arena-page-lifecycle-boundary`\n  with 107 markers",
        "`arena-allocation-boundary` with 105 markers",
        "`arena-purge-boundary` with 38 markers",
        "`arena-registry-boundary` with\n  37 markers",
        "`arena-slice-boundary` with 31 markers",
        "`arena-abandoned-boundary` with 22 markers",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,544 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,150 lines",
        "`tests/release_fast_path_audit.rs` is 1,935 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,222 lines",
        "`tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries.rs`\n  is 895 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 831 lines",
        "1. [x] Make strict-type line-pattern checks item-aware.",
        "2. [x] Narrow platform strict-type wildcard allowances.",
        "3. [x] Narrow allocator and internal strict-type wildcard allowances.",
        "4. [x] Split the arena audit operation cluster.",
        "5. [ ] Split the initialization lifecycle audit cluster.",
        "6. [ ] Shrink release-audit review units and lower line caps.",
        "The strict-type checker can enforce symbol-level entries for line-pattern\n      findings",
        "The targeted platform, allocator, and internal strict-type wildcard\n      entries are replaced",
        "The arena and initialization operation IDs are split into narrower audit\n      IDs",
    ];

    for evidence in required_evidence {
        assert!(
            todo.contains(evidence),
            "TODO.md should keep measurable backlog evidence `{evidence}`"
        );
    }
}

#[test]
fn release_audit_harness_stays_split() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let audit_root = root.join("tests/release_fast_path_audit.rs");
    let module_dir = root.join("tests/release_fast_path_audit");
    let root_lines = fs::read_to_string(&audit_root)
        .expect("read release fast-path audit root")
        .lines()
        .count();

    assert!(
        root_lines <= 3_500,
        "release fast-path audit root should stay a small harness; found {root_lines} lines"
    );

    for module in RELEASE_AUDIT_MODULES {
        let path = module_dir.join(module);
        let line_count = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
            .lines()
            .count();

        assert!(
            line_count <= 3_000,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
