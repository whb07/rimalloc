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
        "# TODO: Fresh-Eyes Audit Cap, Harness, and Strict-Type Backlog",
        "## Issue 1: Lower the source audit marker cap to 22",
        "## Issue 2: Split release-audit modules over 260 lines",
        "## Issue 3: Guard strict-type permanent reason and file/domain hotspots",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`7af25fa`",
        "previous backlog is implemented and pushed: source audit marker\ncap 24",
        "release-audit module cap 280",
        "strict-type allowlist hotspot\ngovernance",
        "now sitting at the 22-marker source cap",
        "Issues 1 and 2 of this backlog have been\nimplemented and validated locally",
        "strict-type allowlist reason/domain hotspots",
        "covers 82 source files, 5,899 unsafe\n  mentions, and 492 operation IDs",
        "covers those 492 operation IDs with\n  1,052 automated evidence rows",
        "No source audit operation ID has more than 22 source markers",
        "old\n  over-cap IDs are split into responsibility-level marker families",
        "Eight IDs now sit at the 22-marker cap",
        "`macos-zone-allocation-boundary`",
        "`windows-large-page-boundary`",
        "`arena-abandoned-boundary`",
        "`arena-page-fresh-boundary`",
        "`internal-atomic-fetch-boundary`",
        "`page-count-accessors`",
        "`typed-callback-test-fixtures`",
        "`typed-page-queue-test-fixtures`",
        "has 634 active entries: 332\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
        "It has zero wildcard entries and\n  zero removable entries",
        "largest strict-type permanent reason groups are allocation compatibility\n  boundary with 47 entries",
        "typed-domain constructor boundary with 42 entries",
        "internal translated C helper boundary with 41 entries",
        "C ABI parity or\n  translated-C source shape with 33 entries",
        "C/platform ABI forwarding boundary\n  with 32 entries",
        "aligned allocation compatibility boundary with 30\n  entries",
        "largest strict-type raw-pointer file/domain hotspots are\n  `src/internal/mod.rs`/`mi_page_t` with 33 entries",
        "`src/allocator/alloc.rs`/`mi_theap_t` with 30",
        "`src/allocator/page.rs`/`mi_page_t` with 25",
        "`src/allocator/thread_heap.rs`/`mi_theap_t`",
        "`src/allocator/free.rs`/`mi_page_t` with 19",
        "`src/allocator/page.rs`/`mi_theap_t` with 19",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 131 module files recursively and\n  enforces a 260-line cap",
        "`strict_policy.rs` at 260 lines",
        "`release_policy/callbacks.rs` at 255 lines",
        "`datatype_acceptance/domain_wrappers.rs` at 253 lines",
        "`unsafe_performance/allocation_free/marker_counts.rs` at 251 lines",
        "`unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs` at 248 lines",
        "`release_policy/security/unsafe_audit_evidence/focused_paths.rs` at 247 lines",
        "Completed in this pass. The current 22-marker cap is enforced",
        "[x] Split the 24-marker IDs by responsibility",
        "[x] Split the 23-marker IDs by operation family",
        "no source audit operation ID exceeds 22 source\n      markers",
        "Completed in this pass. The release-audit cap is now 260 lines",
        "`strict_policy.rs` sits exactly at that target",
        "Split `interposition_exports/linux_override.rs` into Linux TLS shim",
        "Split `primitive_options.rs` into option storage",
        "Lower the recursive release-audit module cap from 280 to 260",
        "strict-type allowlist now has active-entry, kind/file, domain, wildcard, and\nremovable-entry governance",
        "top permanent disposition reason groups",
        "top raw-pointer file/domain pairs",
        "permanent ABI/source-shape edges or future typed-helper rewrite",
        "1. [x] Lower the source audit marker cap to 22.",
        "2. [x] Split release-audit modules over 260 lines.",
        "3. [ ] Guard strict-type permanent reason and file/domain hotspots.",
        "[x] No source audit operation ID has more than 22 source markers",
        "[x] Release-audit modules are split enough to lower the recursive module cap",
        "recursive module cap\n      to 260 lines",
        "Strict-type permanent reason and file/domain hotspot governance is\n      measured",
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
            line_count <= 260,
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
