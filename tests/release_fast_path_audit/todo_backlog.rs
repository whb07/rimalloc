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
        "## Issue 1: Lower the source audit marker cap to 24",
        "## Issue 2: Split release-audit modules over 280 lines",
        "## Issue 3: Tighten strict-type allowlist hotspot governance",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`8ade753`",
        "lowering the source audit marker cap to 24",
        "previous backlog\nand Issues 1 and 2 below are implemented",
        "covers 82 source files, 5,899 unsafe\n  mentions, and 462 operation IDs",
        "covers those 462 operation IDs with\n  990 automated evidence rows",
        "No source audit operation ID has more than 24 source markers",
        "`os-memory-config-boundary`, `typed-tld-accessors`, and\n  `arena-id-memid-helpers` at 24 markers each",
        "retired broad marker families have been split into focused source IDs",
        "typed\n  page/arena accessors",
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
        "`raw-pointer-parameter` in\n  `src/allocator/alloc.rs` with 47 active entries",
        "`raw-pointer-parameter` in\n  `src/allocator/page.rs` with 44",
        "`raw-pointer-parameter` in\n  `src/internal/mod.rs` with 41",
        "`bare-domain-usize` in\n  `src/internal/bitmap.rs` with 33",
        "`bare-domain-usize` in `src/raw/macros.rs`\n  with 32",
        "`raw-pointer-parameter` in `src/allocator/aligned.rs` with 30",
        "raw-pointer domains are `mi_theap_t` with 111 entries",
        "`mi_page_t` with 110",
        "`mi_heap_t` with 67",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 124 module files recursively and\n  enforces a 280-line cap",
        "No release-audit module exceeds that cap",
        "release-audit modules are:\n  `interposition_exports/linux_override.rs` at 279 lines",
        "`primitive_options.rs` at 279 lines",
        "`strict_policy.rs` at 257 lines",
        "`release_policy/callbacks.rs` at 255 lines",
        "`unsafe_performance/allocation_free/marker_counts.rs` at 248 lines",
        "`unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs` at 248 lines",
        "`interposition_exports/linux_override.rs` at 279 lines",
        "Issue 2 split release-audit assertion families into focused child modules",
        "thread/TLS platform primitives",
        "safe callback guards",
        "raw pointer nullability",
        "Windows virtual memory",
        "stats merge policy",
        "Validation for commit `8ade753` passed",
        "Completed in this pass",
        "missed 25-marker `typed-page-accessors` hotspot was split",
        "[x] Split `thread-heap-visit-boundary` into independent visitation",
        "[x] Split `stats-output-boundary` by stack-buffer formatting",
        "[x] Split `free-validation-boundary` by null/alignment validation",
        "Split the 25-27 marker IDs by operation family",
        "`aligned-realloc-heap-export-boundary`",
        "Completed in this pass. The release-audit cap is now 280 lines",
        "largest\nmodule is 279 lines",
        "[x] Split `release_policy/security/unsafe_audit_evidence/focused_paths.rs`",
        "moving platform primitive evidence checks into a\n      focused child module",
        "[x] Split `unsafe_performance/lifecycle_boundaries/thread_tls.rs` by thread\n      lifecycle",
        "[x] Split `release_policy/callbacks.rs` into callback ABI inventory",
        "[x] Split `release_policy/api_surface/raw_facade.rs` into raw facade shape",
        "cap to 280",
        "Split the strict-type allowlist inventory release-audit checks by concern",
        "Add measured guards for the top raw-pointer domains",
        "Document which hotspot families are permanent C ABI compatibility edges",
        "1. [x] Lower the source audit marker cap to 24.",
        "2. [x] Split release-audit modules over 280 lines.",
        "3. [ ] Tighten strict-type allowlist hotspot governance.",
        "[x] No source audit operation ID has more than 24 source markers",
        "[x] Release-audit modules are split enough to lower the recursive module cap\n      to 280 lines",
        "Strict-type allowlist hotspot governance is split, measured, and tied to",
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
            line_count <= 280,
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
