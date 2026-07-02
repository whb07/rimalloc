use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "release_policy.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
    "unsafe_performance/local_boundaries.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: Unsafe migration allowlist still has small test fixtures",
        "## Issue: Page count accessors repeat the same unsafe proof too often",
        "## Issue: Strict-type removable entries are still broad",
        "## Issue: Release fast-path audit still has large child modules",
        "## Issue: Large allocator translation units still need boundary splits",
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
        "56 entries: 44 production,\n  9 test, and 3 translation files",
        "SAFETY[AUDIT:typed-arena-test-fixtures]",
        "SAFETY[AUDIT:typed-callback-test-fixtures]",
        "strict-type-allowlist.txt",
        "94 entries; 5 are still marked `remove:`",
        "`mi_arenas_try_find_free` now keeps\n    `crate::core_types::Alignment`",
        "`mi_arena_os_alloc_aligned` now takes a private `ArenaOsAllocRequest`",
        "`REMOVE_ENTRY_CAP` is now 5",
        "`src/allocator/arena.rs` is 4,613 lines",
        "`src/allocator/page.rs` is 3,220",
        "`src/allocator/page/counts.rs` is 254 lines",
        "22\n  `SAFETY[AUDIT:page-count-accessors]` markers",
        "use typed `used`, `reserved`, `capacity`, `set_used`,\n    `set_reserved`, and `set_capacity` accessors",
        "`src/allocator/arena/debug.rs`",
        "34\n  `SAFETY[AUDIT:arena-debug-render]` markers",
        "`tests/release_fast_path_audit.rs` is 2,864 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,877 lines",
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
