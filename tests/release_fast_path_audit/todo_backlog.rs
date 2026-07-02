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
        "# TODO: Fresh-Eyes Audit Density, Module Cap, Strict-Type Hotspot, and Production Module Backlog",
        "## Issue 1: Lower the source audit marker cap to 18",
        "## Issue 2: Split release-audit modules over 220 lines",
        "## Issue 3: Tighten strict-type hotspot caps",
        "## Issue 4: Split the largest production module under 3,500 lines",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`8f8cb27`",
        "That backlog is implemented and pushed: source audit marker cap 20",
        "release-audit module cap 240",
        "strict-type permanent reason groups capped at\n40 entries",
        "The next mechanical-translation debt is concentrated in audit IDs\nthat still sit at the cap",
        "strict-type hotspots that still hide too many raw/usize boundaries",
        "and the largest translated production module",
        "covers 82 source files, 5,899 unsafe\n  mentions, and 504 operation IDs",
        "covers those 504 operation IDs with\n  1,076 automated evidence rows",
        "No source audit operation ID has more than 20 source markers",
        "Six IDs sit at\n  the 20-marker cap",
        "`stats-json-boundary`",
        "`raw-macro-unix-aligned-posix-boundary`",
        "`raw-macro-linux-libc-boundary`",
        "`internal-atomic-platform-lock-boundary`",
        "`emscripten-memory-boundary`",
        "`allocation-string-path-boundary`",
        "Nine more source audit operation IDs sit at 19 markers",
        "`windows-dll-lifecycle-boundary`",
        "`unsafe-callback-test-fixtures`",
        "`typed-page-state-test-fixtures`",
        "`stats-print-export-boundary`",
        "`raw-posix-realloc-boundary`",
        "`page-stats-public-boundary`",
        "`page-map-lookup-boundary`",
        "`os-memory-commit-boundary`",
        "`internal-bitmap-range-chunk-boundary`",
        "The next useful source marker cap is\n  18 entries",
        "has 634 active entries: 332\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
        "It has zero wildcard entries and\n  zero removable entries",
        "It has no retained wildcard\n  entries",
        "Strict-type wildcard guard: total=0; none.",
        "No strict-type permanent reason group exceeds 40 entries",
        "internal page metadata helper boundary with 33 entries",
        "C ABI parity or\n  translated-C source shape with 33 entries",
        "C/platform ABI forwarding boundary\n  with 32 entries",
        "aligned allocation compatibility boundary with 30 entries",
        "thread-heap compatibility boundary with 24 entries",
        "scalar domain\n  constructor boundary with 24 entries",
        "largest strict-type raw-pointer file/domain hotspots are\n  `src/internal/mod.rs`/`mi_page_t` with 33 entries",
        "`src/allocator/alloc.rs`/`mi_theap_t` with 30",
        "`src/allocator/page.rs`/`mi_page_t` with 25",
        "`src/allocator/thread_heap.rs`/`mi_theap_t` with 20",
        "`src/allocator/free.rs`/`mi_page_t` with 19",
        "`src/allocator/page.rs`/`mi_theap_t` with 19",
        "The next useful raw-pointer\n  file/domain cap is 30 entries",
        "remaining non-raw hotspots are\n  `src/internal/bitmap.rs`/`usize` with 33",
        "`src/raw/macros.rs`/`usize`\n  with 32",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 145 module files recursively and\n  enforces a 240-line cap",
        "`unsafe_performance/os_memory/platform_helpers.rs` at 240 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/emscripten.rs` at 237 lines",
        "`unsafe_performance/platform_primitive_boundaries/windows.rs` at 233 lines",
        "`unsafe_performance/page_arena_behavior/purge_clock_atomic.rs` at 232 lines",
        "`datatype_acceptance/strict_type/atomic_provenance_guards.rs` at 231 lines",
        "`unsafe_performance/lifecycle_boundaries/process_lifecycle/subproc_fast_paths.rs` at 223 lines",
        "No release-audit module currently exceeds 240 lines, but six modules still\n  exceed the next useful 220-line cap",
        "Production source module size remains dominated by translated-C-shaped files",
        "`src/allocator/arena.rs` at 4,255 lines",
        "`src/allocator/page.rs` at 2,914",
        "`src/allocator/alloc.rs` at 2,729",
        "`src/allocator/init.rs` at 2,612",
        "`src/internal/bitmap.rs` at 2,546",
        "`src/raw/macros.rs` at 2,401",
        "The next\n  useful production module cap is 3,500 lines",
        "Split the six 20-marker operation IDs into smaller source-audit families",
        "Split the nine 19-marker operation IDs by platform",
        "Lower the source marker release-audit cap from 20 to 18",
        "Split `unsafe_performance/os_memory/platform_helpers.rs` by allocation",
        "Lower the recursive release-audit module cap from 240 to 220",
        "Reduce strict-type allowlist file hotspots so no file exceeds 50 active\n      entries",
        "Reduce strict-type kind/file hotspots so no kind/file pair exceeds 40\n      active entries",
        "Reduce raw-pointer file/domain hotspots so no pair exceeds 30 entries",
        "Reduce non-raw file/domain hotspots so no usize pair exceeds 30 entries",
        "Extract reserve/manage-OS-memory entrypoints and huge-page reservation",
        "Keep hot-path helpers monomorphic and allocation-free",
        "1. [ ] Lower the source audit marker cap to 18.",
        "2. [ ] Split release-audit modules over 220 lines.",
        "3. [ ] Tighten strict-type hotspot caps.",
        "4. [ ] Split the largest production module under 3,500 lines.",
        "No source audit operation ID has more than 18 source markers",
        "Release-audit modules are split enough to lower the recursive module cap\n      to 220 lines",
        "Strict-type file hotspots are capped at 50 active entries",
        "usize file/domain hotspots are capped\n      at 30 active entries",
        "`src/allocator/arena.rs` is split below 3,500 lines",
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
