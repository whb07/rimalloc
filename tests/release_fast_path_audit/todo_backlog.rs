use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "release_policy.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/bitmap_boundaries.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
    "unsafe_performance/local_boundaries.rs",
    "unsafe_performance/tail_guards.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: Small test fixtures still sit on the unsafe migration allowlist",
        "## Issue: Small production files are still covered by broad unsafe allowlist entries",
        "## Issue: The page strict-type allowance is still file-wide after the bin split",
        "## Issue: Release fast-path audit has one large assertion module left",
        "## Issue: Large allocator translation units still need cohesive boundaries",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "docs/unsafe-migration-allowlist.tsv",
        "53 entries: 43 production, 7 test,\n  and 3 translation files",
        "`src/types/page_states/tests.rs` is 151 lines",
        "`src/types/page_queues/tests.rs` is 160 lines",
        "`src/types/heap/tests.rs` is 212 lines",
        "SAFETY[AUDIT:typed-freelist-wrapper-test-fixtures]",
        "SAFETY[AUDIT:typed-layout-test-fixtures]",
        "`ALLOWLIST_ENTRY_CAP` is now\n    54",
        "`src/types/padding.rs` is 143 lines",
        "`src/allocator/tracking.rs` is 185 lines",
        "`src/types/page.rs` is 258 lines",
        "SAFETY[AUDIT:provenance-miri-registry]",
        "`ALLOWLIST_ENTRY_CAP` is now\n    53",
        "strict-type-allowlist.txt",
        "104 entries; 4 are still marked `remove:`",
        "for `alloc`,\n  `aligned`, `bitmap`, and `internal`",
        "`bare-domain-usize|src/allocator/page.rs|*|usize` allowance was replaced",
        "`mi_page_block_at`, `mi_page_free_list_extend`, `mi_huge_page_alloc`",
        "`mi_find_page`, `mi_bin_impl`, `_mi_page_stats_bin`",
        "`REMOVE_ENTRY_CAP` is now 4",
        "`src/allocator/arena.rs` is 4,628 lines",
        "`src/allocator/page.rs` is 3,109",
        "`src/allocator/page/bins.rs` is 128 lines",
        "SAFETY[AUDIT:page-bin-size-classes]",
        "SAFETY[AUDIT:arena-debug-render]` with 34 markers",
        "`SAFETY[AUDIT:page-count-accessors]` with 22\n  markers",
        "SAFETY[AUDIT:owned-allocation]` with 17 markers",
        "`tests/release_fast_path_audit.rs` is 2,864 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,756 lines",
        "`tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is\n  77 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,291 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 700 lines",
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
