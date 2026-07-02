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
        "# TODO: Next Audit Cluster, Strict-Type, and Harness Backlog",
        "## Issue 1: Split options output and diagnostic audit clusters",
        "## Issue 2: Split arena registry and purge audit clusters",
        "## Issue 3: Split allocation entry and raw override audit clusters",
        "## Issue 4: Split typed page-state and subproc association audit clusters",
        "## Issue 5: Tighten strict-type allowlist hotspots",
        "## Issue 6: Split release-audit modules over 400 lines and lower the cap",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`f34084c`",
        "previous backlog is implemented, validated, committed, and\npushed",
        "covers 82 source files, 5,893 unsafe\n  mentions, and 337 operation IDs",
        "covers those 337 operation IDs with\n  725 automated evidence rows",
        "No source audit operation ID is over 39 source markers",
        "largest remaining\n  source IDs are `options-message-output-boundary` with 39 markers",
        "`options-message-output-boundary` with 39 markers",
        "`arena-purge-boundary` with 38 markers",
        "`allocation-realloc-entry-boundary`\n  and `arena-registry-boundary` with 37 markers each",
        "`init-subproc-heap-association-boundary`,\n  `raw-macro-override-entry-boundary`, and `typed-page-state-domain` with 36\n  markers each",
        "`typed-heap-thread-tld-pointer-test-fixtures` with 35 markers",
        "`allocation-heap-dispatch-boundary` and `arena-debug-render` with 34\n  markers each",
        "has 631 active entries: 332\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 2 heap-allocating-type entries",
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
        "largest\n  raw-pointer domains are `mi_theap_t` with 111 entries",
        "`mi_page_t` with 110",
        "`mi_heap_t` with 67",
        "has 1 retained wildcard entry, the\n  `heap-allocating-type` Loom proof-model boundary",
        "Strict-type wildcard guard: total=1; heap-allocating-type=1.",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 91 module files recursively and\n  enforces a 500-line cap",
        "largest release-audit modules are:\n  `unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 448 lines",
        "`interposition_exports.rs` at 444 lines",
        "`release_policy/security/unsafe_audit_evidence.rs` at 421 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs` at 421 lines",
        "`unsafe_performance/allocation_free.rs` at 409 lines",
        "`unsafe_performance/os_memory.rs` at 409 lines",
        "Full validation for Issue 6 passed",
        "delayed output buffers,\nstderr/file output, output callback routing, recursion guards",
        "Split `options-message-output-boundary` into focused IDs",
        "Split `arena-registry-boundary` by operation",
        "Split `arena-purge-boundary` by operation",
        "Split `allocation-realloc-entry-boundary` into realloc request parsing",
        "Split `allocation-heap-dispatch-boundary` into default thread-heap\n      dispatch",
        "Split `allocation-cxx-new-boundary` and `raw-macro-override-entry-boundary`",
        "Split `typed-page-state-domain` into uninit initialization",
        "Split `init-subproc-heap-association-boundary` into subproc prefix/main\n      heap lookup",
        "Audit the top raw-pointer hotspot families",
        "Split or tighten bare-domain `usize` allowlist entries",
        "Replace the retained Loom wildcard with exact symbol coverage",
        "Lower the release-audit module cap from 500 to 400",
        "1. [ ] Split options output and diagnostic audit clusters.",
        "2. [ ] Split arena registry and purge audit clusters.",
        "3. [ ] Split allocation entry and raw override audit clusters.",
        "4. [ ] Split typed page-state and subproc association audit clusters.",
        "5. [ ] Tighten strict-type allowlist hotspots.",
        "6. [ ] Split release-audit modules over 400 lines and lower the cap.",
        "No source audit operation ID has more than 32 source markers",
        "Strict-type allowlist hotspots are measured in release-audit coverage",
        "Release-audit modules are split enough to lower the module cap to 400\n      lines",
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
            line_count <= 500,
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
