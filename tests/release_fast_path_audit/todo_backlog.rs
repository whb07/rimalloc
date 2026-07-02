use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "release_policy.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: Unsafe migration allowlist should keep shrinking",
        "## Issue: Strict-type allowlist still has removable file-level entries",
        "## Issue: Production module-level allowances hide local intent",
        "## Issue: Safe facade tests are mixed into the facade module",
        "## Issue: Release fast-path audit is too large and brittle",
        "## Issue: Large translated modules still need smaller review boundaries",
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
        "62 entries: 44 production,\n  15 test, and 3 translation files",
        "strict-type-allowlist.txt",
        "95 entries; 8 are still marked `remove:`",
        "`src/safe/mod.rs` is now 40 lines",
        "`tests/release_fast_path_audit.rs` is now split into",
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
            line_count <= 4_200,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
