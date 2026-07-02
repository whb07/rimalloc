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
        "# TODO: Fresh-Eyes Audit Density, Module Cap, and Strict-Type Classification Backlog",
        "## Issue 1: Lower the source audit marker cap to 20",
        "## Issue 2: Split release-audit modules over 240 lines",
        "## Issue 3: Split strict-type permanent reason groups over 40",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`5c47626`",
        "That backlog is implemented and pushed: source audit marker cap 22",
        "release-audit module cap 260",
        "strict-type permanent reason/file-domain\nhotspot governance",
        "Issue 2 has split the release-audit modules enough to\nenforce a 240-line recursive cap",
        "broad strict-type permanent reason buckets",
        "covers 82 source files, 5,899 unsafe\n  mentions, and 504 operation IDs",
        "covers those 504 operation IDs with\n  1,076 automated evidence rows",
        "No source audit operation ID has more than 20 source markers",
        "Six IDs now sit\n  at the 20-marker cap",
        "`stats-json-boundary`",
        "`raw-macro-unix-aligned-posix-boundary`",
        "`raw-macro-linux-libc-boundary`",
        "`internal-atomic-platform-lock-boundary`",
        "`emscripten-memory-boundary`",
        "`allocation-string-path-boundary`",
        "has 634 active entries: 332\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
        "It has zero wildcard entries and\n  zero removable entries",
        "largest strict-type permanent reason groups are allocation compatibility\n  boundary with 47 entries",
        "typed-domain constructor boundary with 42 entries",
        "internal translated C helper boundary with 41 entries",
        "C ABI parity or\n  translated-C source shape with 33 entries",
        "C/platform ABI forwarding boundary\n  with 32 entries",
        "aligned allocation compatibility boundary with 30\n  entries",
        "The next useful permanent reason cap is 40 entries",
        "largest strict-type raw-pointer file/domain hotspots are\n  `src/internal/mod.rs`/`mi_page_t` with 33 entries",
        "`src/allocator/alloc.rs`/`mi_theap_t` with 30",
        "`src/allocator/page.rs`/`mi_page_t` with 25",
        "`src/allocator/thread_heap.rs`/`mi_theap_t`",
        "`src/allocator/free.rs`/`mi_page_t` with 19",
        "`src/allocator/page.rs`/`mi_theap_t` with 19",
        "The current raw-pointer\n  file/domain cap is 33 entries",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 145 module files recursively and\n  enforces a 240-line cap",
        "`unsafe_performance/os_memory/platform_helpers.rs` at 240 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/emscripten.rs` at 237 lines",
        "`unsafe_performance/platform_primitive_boundaries/windows.rs` at 233 lines",
        "`unsafe_performance/page_arena_behavior/purge_clock_atomic.rs` at 232 lines",
        "`datatype_acceptance/strict_type/atomic_provenance_guards.rs` at 231 lines",
        "`unsafe_performance/lifecycle_boundaries/process_lifecycle/subproc_fast_paths.rs` at 223 lines",
        "No release-audit module currently exceeds 240 lines",
        "Split the former 22-marker IDs by responsibility",
        "Split the former 21-marker IDs by operation family",
        "no source audit operation ID exceeds 20 source\n      markers",
        "Done. The recursive release-audit module cap is now 240 lines.",
        "Split `strict_policy.rs` so the strict-type hot-path",
        "Lower the recursive release-audit module cap from 260 to 240",
        "Split `allocation compatibility boundary` into more specific permanent\n      disposition families",
        "no permanent reason group exceeds 40 entries",
        "1. [x] Lower the source audit marker cap to 20.",
        "2. [x] Split release-audit modules over 240 lines.",
        "3. [ ] Split strict-type permanent reason groups over 40.",
        "Release-audit modules are split enough to lower the recursive module cap\n      to 240 lines",
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
            line_count <= 240,
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
