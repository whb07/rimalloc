use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "datatype_acceptance.rs",
    "release_policy.rs",
    "release_policy/security.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/bitmap_boundaries.rs",
    "unsafe_performance/heap_page_visitors.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
    "unsafe_performance/local_boundaries.rs",
    "unsafe_performance/page_lifecycle.rs",
    "unsafe_performance/size_class_boundaries.rs",
    "unsafe_performance/tail_guards.rs",
    "unsafe_performance/threadlocal_boundaries.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: MemId typed views still use a file-level unsafe allowance",
        "## Issue: Layout request wrappers still use a file-level unsafe allowance",
        "## Issue: Page-state witnesses still use a file-level unsafe allowance",
        "## Issue: Benchmark diagnostics still carry a file-level unsafe allowance",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`c500ffb`",
        "migrated arena, page-queue, heap, and free-list typed",
        "split the datatype acceptance release-audit wall",
        "docs/unsafe-migration-allowlist.tsv",
        "33 entries: 30 production and\n  3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 33",
        "`src/types/memid.rs` now uses 4",
        "`SAFETY[AUDIT:typed-memid-view]` markers for discriminant-gated union reads",
        "`SAFETY[AUDIT:typed-memid-test-fixtures]` marker for its lint-touch",
        "is off the unsafe migration allowlist",
        "`src/types/layout.rs` now uses 16",
        "`SAFETY[AUDIT:typed-layout-domain]` markers for unchecked layout/product",
        "`SAFETY[AUDIT:typed-layout-test-fixtures]` markers for lint-touch fixtures",
        "`src/types/page_states.rs` now uses 36",
        "`SAFETY[AUDIT:typed-page-state-domain]` markers for page/thread-heap",
        "`SAFETY[AUDIT:typed-page-state-test-fixtures]` markers for lint-touch",
        "`src/types/atomic.rs` at 755 lines with 101 `unsafe` hits",
        "`src/allocator/arena_meta.rs` at 316 lines with 24 `unsafe` hits",
        "`src/raw/posix.rs` at 329 lines with 81 `unsafe` hits",
        "`src/allocator/bench_diagnostics.rs` at 350 lines with 8 `unsafe` hits",
        "`src/platform/wasi.rs` at 373 lines with 34 `unsafe` hits",
        "`src/lib.rs` at 398 lines with 23 `unsafe` hits and 8 local markers",
        "`strict-type-allowlist.txt` has 75 path-wide entries",
        "including 29 path-wide raw-pointer entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 88 markers",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 58 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 56 markers",
        "SAFETY[AUDIT:typed-page-state-domain]` has 36 markers",
        "SAFETY[AUDIT:arena-debug-render]` has 35 markers",
        "SAFETY[AUDIT:owned-allocation]` has 32 markers",
        "SAFETY[AUDIT:typed-scalar-test-fixtures]` has 32 markers",
        "SAFETY[AUDIT:unsafe-callback]` has 28 markers",
        "SAFETY[AUDIT:typed-freelist-wrappers]` has 25 markers",
        "SAFETY[AUDIT:arena-id-memid-helpers]` has 25 markers",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,384 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit.rs` is 1,832 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,103 lines",
        "SAFETY[AUDIT:typed-memid-view]",
        "SAFETY[AUDIT:typed-layout-domain]",
        "SAFETY[AUDIT:typed-page-state-domain]",
        "SAFETY[AUDIT:bench-diagnostics-ffi]",
        "Migrate `src/types/memid.rs` off the unsafe migration allowlist",
        "Migrate `src/types/layout.rs` off the unsafe migration allowlist",
        "Migrate `src/types/page_states.rs` off the unsafe migration allowlist",
        "Migrate `src/allocator/bench_diagnostics.rs` off the unsafe migration",
        "Existing typed-domain behavior, C ABI behavior, and allocation-free hot",
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
            line_count <= 3_000,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
