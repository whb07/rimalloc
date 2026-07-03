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
        "# TODO: Fresh-Eyes Source-Marker, Release Module, Strict-Type, and Arena Module Backlog",
        "## Issue 1: Lower the source audit marker cap to 16",
        "## Issue 2: Split release-audit modules over 190 lines",
        "## Issue 3: Tighten strict-type and permanent-reason hotspot caps",
        "## Issue 4: Split the next largest production module under 3,000 lines",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`83d42e1`",
        "implemented source audit marker cap is 16",
        "release-audit module\ncap is 190",
        "strict-type file hotspots are capped at 40 active entries",
        "kind/file hotspots at 25 entries",
        "raw-pointer file/domain hotspots at 20\nentries",
        "usize file/domain hotspots at 24 entries",
        "permanent reason groups at\n32 entries",
        "`src/allocator/page.rs` is split below the 2,800-line production\nmodule cap",
        "`src/allocator/arena.rs` is split below the 3,000-line\nproduction module cap",
        "covers 103 source files, 5,899 unsafe\n  mentions, and 540 operation IDs",
        "covers those 540 operation IDs with\n  1,155 automated evidence rows",
        "No source audit operation ID has more than 16 source markers",
        "Fourteen IDs\n  sit at the 16-marker cap",
        "`typed-layout-domain`",
        "`stats-public-merge-boundary`",
        "`macos-zone-lookup-boundary`",
        "`init-thread-done-cleanup-boundary`",
        "`arena-meta-allocator`",
        "source marker cap would be 15 entries",
        "has 632 active entries: 330\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
        "It has zero wildcard entries and\n  zero removable entries",
        "It has no retained wildcard\n  entries",
        "Strict-type wildcard guard: total=0; none.",
        "The active file hotspot cap is 40 active entries",
        "next useful file\n  hotspot cap is 35 active entries",
        "The active kind/file\n  hotspot cap is 25 active entries",
        "next useful kind/file cap is 23 active\n  entries",
        "No strict-type permanent reason group exceeds 32 entries",
        "C/platform ABI forwarding boundary with 32 entries",
        "C ABI parity or\n  translated-C source shape with 32 entries",
        "aligned allocation compatibility\n  boundary with 30 entries",
        "internal page metadata helper boundary with 26\n  entries",
        "next useful permanent reason cap is\n  30 entries",
        "largest strict-type raw-pointer file/domain hotspots are\n  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20 entries",
        "`src/allocator/thread_heap.rs`/`mi_theap_t` with 20",
        "`src/allocator/free.rs`/`mi_page_t` with 19",
        "`src/allocator/alloc.rs`/`mi_theap_t` with 18",
        "`src/allocator/page.rs`/`mi_theap_t` with 15",
        "`src/internal/page_metadata.rs`/`mi_page_t` with 15",
        "next useful raw-pointer file/domain cap is\n  19 entries",
        "remaining non-raw hotspots are\n  `src/types/scalar.rs`/`usize` with 24",
        "`src/internal/bitmap.rs`/`usize` with 23",
        "`src/raw/macros.rs`/`usize` with 23",
        "next useful usize file/domain cap is 23 entries",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 184 module files recursively and\n  enforces a 190-line cap",
        "`unsafe_performance/raw_facade/raw_override/os_random.rs` at 190 lines",
        "`todo_backlog.rs` at 190 lines",
        "`unsafe_performance/os_memory/platform_helpers.rs` at 189 lines",
        "`unsafe_performance/page_lifecycle.rs` at 186 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/wasi.rs` at 185 lines",
        "`datatype_acceptance/strict_type_allowlist_inventory.rs` at 183 lines",
        "`strict_policy.rs` at 182 lines",
        "`unsafe_performance/raw_facade/bootstrap_query.rs` at 181 lines",
        "No release-audit module currently exceeds\n  190 lines; the next useful recursive module cap would be 180 lines",
        "Production source module size remains dominated by translated-C-shaped files",
        "`src/allocator/arena.rs` at 2,711 lines",
        "`src/allocator/init.rs` at 2,612",
        "`src/allocator/page.rs` at 2,003",
        "`src/internal/bitmap.rs` at 2,372",
        "`src/platform/windows.rs` at 2,305",
        "`src/raw/macros.rs` at 2,260",
        "`src/allocator/stats.rs` at 2,182",
        "No production source module currently exceeds the 3,000-line cap",
        "next\n  useful production module cap would be 2,800 lines",
        "Lower the source marker release-audit cap from 17 to 16",
        "Lower the recursive release-audit module cap from 200 to 190",
        "Reduce strict-type allowlist file hotspots so no file exceeds 40 active\n      entries",
        "Reduce strict-type kind/file hotspots so no kind/file pair exceeds 25\n      active entries",
        "Reduce permanent reason groups so no group exceeds 32 entries",
        "Extract arena slice geometry, page table lookup, registry iteration, or\n      page allocation/free helper families",
        "Keep hot-path helpers monomorphic and allocation-free",
        "1. [x] Lower the source audit marker cap to 16.",
        "2. [x] Split release-audit modules over 190 lines.",
        "3. [x] Tighten strict-type and permanent-reason hotspot caps.",
        "4. [x] Split the next largest production module under 3,000 lines.",
        "No source audit operation ID has more than 16 source markers",
        "Release-audit modules are split enough to lower the recursive module cap\n      to 190 lines",
        "Strict-type file hotspots are capped at 40 active entries",
        "permanent reason groups are capped at 32 entries",
        "`src/allocator/arena.rs` is split below 3,000 lines",
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
            line_count <= 190,
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
