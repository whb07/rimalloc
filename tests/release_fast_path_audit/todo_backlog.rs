use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "release_policy.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: Unsafe migration allowlist still has small reviewable entries",
        "## Issue: Strict-type removable entries remain broad file-level allowances",
        "## Issue: Arena diagnostics still expose too many raw metadata reads",
        "## Issue: Release fast-path audit split is still uneven",
        "## Issue: Page module still mixes queue, count, and allocation policy",
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
        "58 entries: 44 production,\n  11 test, and 3 translation files",
        "strict-type-allowlist.txt",
        "96 entries; 6 are still marked `remove:`",
        "`src/allocator/arena.rs` is 4,613 lines",
        "`src/allocator/page.rs` is 3,442",
        "`src/allocator/arena/debug.rs`",
        "`tests/release_fast_path_audit.rs` is split",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,983 lines",
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
            line_count <= 3_200,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
