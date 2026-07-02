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
        "# TODO: Fresh-Eyes Audit Density, Release Module, Strict-Type, and Page Module Backlog",
        "## Issue 1: Lower the source audit marker cap to 17",
        "## Issue 2: Split release-audit modules over 200 lines",
        "## Issue 3: Tighten strict-type hotspot caps again",
        "## Issue 4: Split the next largest production module under 2,800 lines",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`eff9713`",
        "implemented source audit marker cap is 17",
        "release-audit module\ncap is 200",
        "strict-type file hotspots are capped at 45 active entries",
        "kind/file hotspots at 30 entries",
        "raw-pointer file/domain hotspots at 25\nentries",
        "`src/allocator/page.rs`\nis split below the 2,800-line production module cap",
        "next mechanical-translation debt is concentrated in audit-density\nceilings and the largest remaining translated production modules",
        "covers 90 source files, 5,898 unsafe\n  mentions, and 529 operation IDs",
        "covers those 529 operation IDs with\n  1,129 automated evidence rows",
        "No source audit operation ID has more than 17 source markers",
        "Eleven IDs sit\n  at the 17-marker cap",
        "`aligned-global-allocation-export-boundary`",
        "`arena-initialization-boundary`",
        "`arena-page-abandon-boundary`",
        "`init-thread-default-heap-association-boundary`",
        "`typed-arena-subproc-pointer-test-fixtures`",
        "The next useful source marker\n  cap is 16 entries",
        "has 632 active entries: 330\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
        "It has zero wildcard entries and\n  zero removable entries",
        "It has no retained wildcard\n  entries",
        "Strict-type wildcard guard: total=0; none.",
        "The active file hotspot cap is now 45 active entries",
        "The active kind/file hotspot cap is now 30 active entries",
        "No strict-type permanent reason group exceeds 40 entries",
        "C ABI parity or\n  translated-C source shape with 33 entries",
        "internal page metadata helper boundary with 33 entries",
        "C/platform ABI forwarding boundary\n  with 32 entries",
        "aligned allocation compatibility boundary with 30 entries",
        "thread-heap compatibility boundary with 24 entries",
        "scalar domain\n  constructor boundary with 24 entries",
        "largest strict-type raw-pointer file/domain hotspots are\n  `src/allocator/alloc.rs`/`mi_theap_t` with 21 entries",
        "`src/internal/mod.rs`/`mi_page_t` with 21",
        "`src/allocator/thread_heap.rs`/`mi_theap_t` with 20",
        "`src/allocator/free.rs`/`mi_page_t` with 19",
        "`src/allocator/page.rs`/`mi_theap_t` with 19",
        "The active raw-pointer\n  file/domain cap is now 25 entries",
        "remaining non-raw hotspots are\n  `src/internal/bitmap.rs`/`usize` with 25",
        "`src/types/scalar.rs`/`usize` with 24",
        "`src/raw/macros.rs`/`usize` with 23",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module-size guard discovers 174 module files recursively and\n  enforces a 200-line cap",
        "`unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 200 lines",
        "`unsafe_performance/page_lifecycle.rs` at 200 lines",
        "`unsafe_performance/page_arena_behavior/arena_page_allocation.rs` at 198 lines",
        "`unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view/memory_kinds.rs` at 196 lines",
        "`unsafe_performance/os_memory/os_wrappers.rs` at 195 lines",
        "`unsafe_performance/allocation_free/typed_helpers.rs` at 193 lines",
        "`unsafe_performance/raw_facade/raw_override/runtime_heap.rs` at 193 lines",
        "`datatype_acceptance/spec_reconciliation.rs` at 192 lines",
        "`todo_backlog.rs` at 191 lines",
        "No release-audit module currently exceeds\n  200 lines, but nine modules still exceed the next useful 190-line cap",
        "Production source module size remains dominated by translated-C-shaped files",
        "`src/allocator/arena.rs` at 3,159 lines",
        "`src/allocator/init.rs` at 2,612",
        "`src/allocator/page.rs` at 2,530",
        "`src/internal/bitmap.rs` at 2,422",
        "`src/raw/macros.rs` at 2,260",
        "`src/allocator/alloc.rs` at 1,952",
        "`src/allocator/page.rs` is now below the 2,800-line cap",
        "Split the old arena public reserve/manage boundary into\n      `arena-public-reserve-entry-boundary`",
        "`arena-managed-os-memory-boundary`; huge-page registration remains under\n      the existing `arena-huge-reservation-boundary`",
        "Split the old aligned OS allocation boundary into\n      `os-memory-aligned-request-boundary`",
        "`os-memory-aligned-fallback-boundary`, and\n      `os-memory-offset-allocation-boundary`",
        "Split the old callback invocation test fixture boundary into basic,\n      heap/block visit, and thread-local claim callback families",
        "Lower the source marker release-audit cap from 18 to 17",
        "Split allocation-free marker-count and typed-helper assertions",
        "Lower the recursive release-audit module cap from 220 to 200",
        "Reduce strict-type allowlist file hotspots so no file exceeds 45 active\n      entries",
        "Reduce strict-type kind/file hotspots so no kind/file pair exceeds 30\n      active entries",
        "Reduce raw-pointer file/domain hotspots so no pair exceeds 25 entries",
        "Reduce non-raw file/domain hotspots so no usize pair exceeds 25 entries",
        "Extract page lifecycle, queue/bin, or validation helper families",
        "Keep hot-path helpers monomorphic and allocation-free",
        "1. [x] Lower the source audit marker cap to 17.",
        "2. [x] Split release-audit modules over 200 lines.",
        "3. [x] Tighten strict-type hotspot caps again.",
        "4. [x] Split the next largest production module under 2,800 lines.",
        "No source audit operation ID has more than 17 source markers",
        "Release-audit modules are split enough to lower the recursive module cap\n      to 200 lines",
        "Strict-type file hotspots are capped at 45 active entries",
        "usize file/domain hotspots are capped at 25\n      entries",
        "`src/allocator/page.rs` is split below 2,800 lines",
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
            line_count <= 200,
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
