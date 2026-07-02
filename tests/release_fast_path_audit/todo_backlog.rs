use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "release_policy.rs",
    "release_policy/security.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/bitmap_boundaries.rs",
    "unsafe_performance/heap_page_visitors.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
    "unsafe_performance/local_boundaries.rs",
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
        "## Issue: Scalar test fixtures are still file-level unsafe debt",
        "## Issue: Pointer wrapper tests are the last allowlisted test fixture",
        "## Issue: Page wrapper production code still uses file-level unsafe allowance",
        "## Issue: Release audit unsafe_performance.rs still has a page lifecycle assertion wall",
        "## Issue: Page strict-type entries remain broad after page child splits",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`a6a560c`",
        "migrated atomic test fixtures and random context\nboundaries",
        "split arena heap\nvisit/delete/move/destroy helpers",
        "narrowed arena\nraw-pointer strict-type entries",
        "docs/unsafe-migration-allowlist.tsv",
        "41 entries: 38 production,\n  0 test, and 3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 41",
        "`src/types/scalar/tests.rs` now uses",
        "`SAFETY[AUDIT:typed-scalar-test-fixtures]` markers for 31 unchecked scalar",
        "is off the unsafe migration allowlist",
        "`src/types/ptr/tests.rs` now uses",
        "`SAFETY[AUDIT:typed-pointer-test-fixtures]` markers for 87 grouped",
        "stack-local metadata and pointer arithmetic fixture blocks",
        "No test files remain in `docs/unsafe-migration-allowlist.tsv`",
        "`src/types/page.rs`, which is 258 lines with 5 `unsafe` hits and 0 local",
        "`SAFETY[AUDIT:typed-page-test-fixtures]`",
        "`src/types/arena.rs` at 260 lines",
        "`src/types/page_queues.rs` at 304 lines",
        "`src/types/heap.rs` at 312 lines",
        "`src/allocator/arena_meta.rs` at 316 lines",
        "`src/raw/posix.rs` at 329 lines",
        "strict-type-allowlist.txt",
        "0 remaining `remove:` entries",
        "`REMOVE_ENTRY_CAP` is 0",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 59 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 56 markers",
        "SAFETY[AUDIT:arena-debug-render]` has 35 markers",
        "SAFETY[AUDIT:owned-allocation]` has 32 markers",
        "SAFETY[AUDIT:unsafe-callback]` has 28 markers",
        "SAFETY[AUDIT:arena-id-memid-helpers]` has 25 markers",
        "SAFETY[AUDIT:page-count-accessors]` has 24 markers",
        "SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 23 markers",
        "`tests/release_fast_path_audit.rs` is 2,893 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,561 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 662 lines",
        "page lifecycle and page-queue assertion block around `_mi_page_free`",
        "`_mi_page_abandon`, `_mi_page_free_collect`, `_mi_page_unfull`",
        "`src/allocator/page.rs` still has path-wide\n  raw-pointer entries",
        "`src/allocator/page/bins.rs`,\n  `src/allocator/page/counts.rs`, `src/allocator/page/flags.rs`, and",
        "SAFETY[AUDIT:typed-scalar-test-fixtures]",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]",
        "SAFETY[AUDIT:typed-page-domain]",
        "Regenerate unsafe inventory and lower `ALLOWLIST_ENTRY_CAP`",
        "Move the page lifecycle and page-queue assertion cluster into a new module",
        "Replace broad page raw-pointer entries with child-module or symbol-level",
        "No test files remain in `docs/unsafe-migration-allowlist.tsv`",
        "Keep `REMOVE_ENTRY_CAP` at 0",
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
