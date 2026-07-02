use super::*;

fn collect_release_audit_modules(dir: &Path, modules: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("read release-audit module directory") {
        let entry = entry.expect("read release-audit module entry");
        let path = entry.path();
        if path.is_dir() {
            collect_release_audit_modules(&path, modules);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            modules.push(path);
        }
    }
}

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh-Eyes Evidence, Source-Audit, and Harness Backlog",
        "## Issue 1: Repoint unsafe audit evidence to focused modules",
        "## Issue 2: Split remaining source audit IDs over 32 markers",
        "## Issue 3: Lower the source audit marker cap to 28",
        "## Issue 4: Split release-audit modules over 320 lines",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`18ed54a`",
        "previous backlog is implemented, validated, committed, and\npushed",
        "covers 82 source files, 5,899 unsafe\n  mentions, and 391 operation IDs",
        "covers those 391 operation IDs with\n  842 automated evidence rows",
        "One source audit operation ID is still over 32 source markers, and 12 IDs are\n  over 28 markers",
        "largest remaining source ID is\n  `typed-heap-thread-tld-pointer-test-fixtures` with 35 markers",
        "`platform-thread-heap-association-boundary`,\n  `windows-virtual-memory-boundary`, and `init-subproc-registry-boundary` with\n  32 markers each",
        "`arena-slice-boundary`, `owned-allocation`,\n  `typed-subproc-accessors`, and `typed-scalar-test-fixtures` have 31 markers\n  each",
        "Issue 1 retargeted 184 dispatcher evidence rows",
        "`docs/unsafe-audit-evidence.tsv`\n  no longer cites the targeted aggregate or parent release-audit modules",
        "root `release_fast_path_audit.rs`, `release_policy.rs`,\n  `datatype_acceptance.rs`, `interposition_exports.rs`",
        "`unsafe_performance.rs`, `allocation_free.rs`, `lifecycle_boundaries.rs`,\n  `os_memory.rs`, `platform_primitive_boundaries.rs`",
        "`platform_primitive_boundaries/unix_wasi_emscripten.rs` all have zero\n  evidence rows",
        "has 634 active entries: 332\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
        "It has no retained wildcard\n  entries",
        "Strict-type wildcard guard: total=0; none.",
        "largest strict-type allowlist file hotspots are `src/internal/mod.rs` with\n  57 active entries",
        "`src/allocator/alloc.rs` with 53",
        "`src/allocator/page.rs` with 51",
        "`src/allocator/aligned.rs` with 50",
        "`src/internal/bitmap.rs` with 45",
        "`src/raw/macros.rs` with 32",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 107 module files recursively and\n  enforces a 400-line cap",
        "largest release-audit modules are:\n  `allowance_inventory.rs` at 398 lines",
        "`unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 390 lines",
        "`release_policy/security/unsafe_audit_evidence/focused_paths.rs` at 385",
        "`interposition_exports/linux_override.rs` at 371 lines",
        "`unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs` at 367 lines",
        "`primitive_options.rs` at 323 lines",
        "Full validation for the previous backlog passed after Issue 6",
        "Move evidence rows for split parent modules to the child module that owns\n      the assertion",
        "Tighten the release-audit evidence guard so parent dispatcher paths are\n      accepted only for genuinely harness-wide policy checks",
        "Split `typed-heap-thread-tld-pointer-test-fixtures` into separate heap",
        "Split `platform-thread-heap-association-boundary` by default heap TLS",
        "Split `windows-virtual-memory-boundary` and\n      `init-subproc-registry-boundary` by operation",
        "no source audit\n      operation ID exceeds 28 source markers",
        "Split `allowance_inventory.rs` by allowance family",
        "Split `thread_tls.rs`, `interposition_exports/linux_override.rs`, and\n      `page_arena_behavior/memid_arena_dispatch.rs`",
        "lower the recursive release-audit\n      module cap from 400 to 320",
        "1. [x] Repoint unsafe audit evidence to focused modules.",
        "2. [ ] Split remaining source audit IDs over 32 markers.",
        "3. [ ] Lower the source audit marker cap to 28.",
        "4. [ ] Split release-audit modules over 320 lines.",
        "[x] Evidence rows for operation-specific audit IDs cite focused child modules",
        "No source audit operation ID has more than 28 source markers",
        "recursive module cap\n      to 320 lines",
        "Strict-type wildcard coverage remains at zero retained wildcards",
        "`TODO.md` and its release-audit guard reflect the current measured counts",
        "For every completed item, run `cargo fmt --all`",
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
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let root_lines = fs::read_to_string(&audit_root)
        .expect("read release fast-path audit root")
        .lines()
        .count();

    assert!(
        root_lines <= 300,
        "release fast-path audit root should stay a small harness; found {root_lines} lines"
    );

    let mut modules = Vec::new();
    collect_release_audit_modules(&module_dir, &mut modules);
    modules.sort();
    assert!(
        modules.len() >= 80,
        "release fast-path audit should discover split modules recursively; found {} modules",
        modules.len()
    );
    assert!(
        todo.contains(&format!(
            "discovers {} module files recursively",
            modules.len()
        )),
        "TODO.md should document the measured release-audit module count"
    );

    let mut line_counts = Vec::new();
    for path in modules {
        let line_count = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
            .lines()
            .count();

        assert!(
            line_count <= 400,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
        let rel = path
            .strip_prefix(&module_dir)
            .expect("module path under release-audit module directory")
            .to_string_lossy()
            .replace('\\', "/");
        line_counts.push((rel, line_count));
    }

    line_counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (rel, line_count) in line_counts.iter().take(6) {
        assert!(
            todo.contains(&format!("`{rel}` at {line_count} lines")),
            "TODO.md should document measured release-audit module size for {rel}"
        );
    }
}
