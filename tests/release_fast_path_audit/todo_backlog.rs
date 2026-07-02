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
        "## Issue: Arena typed domain wrappers still use a file-level unsafe allowance",
        "## Issue: Page queue wrappers still use a file-level unsafe allowance",
        "## Issue: Heap and thread-heap wrappers still use a file-level unsafe allowance",
        "## Issue: Free-list wrappers still mix unsafe migration debt with broad strict-type entries",
        "## Issue: Release audit root still has a datatype acceptance assertion wall",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`e8083ce`",
        "retired the scalar, pointer, and page typed-wrapper",
        "split the page lifecycle/page-queue release-audit",
        "narrowed the page strict-type allowlist",
        "docs/unsafe-migration-allowlist.tsv",
        "37 entries: 34 production and\n  3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 37",
        "`src/types/arena.rs` now uses 7",
        "`SAFETY[AUDIT:typed-arena-domain]` markers for slice-range count",
        "construction and arena role witnesses",
        "is off the unsafe migration\n  allowlist",
        "`src/types/page_queues.rs` now uses 10",
        "`SAFETY[AUDIT:typed-page-queue-domain]` markers for queue classification",
        "queued-page membership, full/unfull transitions, queue removal",
        "`src/types/heap.rs` now uses 15",
        "`SAFETY[AUDIT:typed-heap-domain]` markers for heap classification",
        "thread-heap state/locality checks, heap/tld projections",
        "`src/allocator/arena_meta.rs` at 316 lines with 22 `unsafe` hits",
        "`src/raw/posix.rs` at 329 lines with 81 `unsafe` hits",
        "`src/allocator/bench_diagnostics.rs` at 350 lines with",
        "`src/platform/wasi.rs` at 373 lines with 34 `unsafe` hits",
        "`src/lib.rs` at 398 lines with 21 `unsafe` hits and 8 local markers",
        "`src/types/freelist_wrappers.rs` is 409 lines with 46 `unsafe` hits",
        "`raw-pointer-parameter|src/types/freelist_wrappers.rs|*|mi_block_t`",
        "`pointer-usize-roundtrip|src/types/freelist_wrappers.rs|*|encoded-pointer`",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 89 markers",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 58 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 56 markers",
        "SAFETY[AUDIT:arena-debug-render]` has 35 markers",
        "SAFETY[AUDIT:typed-scalar-test-fixtures]` has 33 markers",
        "SAFETY[AUDIT:owned-allocation]` has 32 markers",
        "SAFETY[AUDIT:unsafe-callback]` has 28 markers",
        "SAFETY[AUDIT:arena-id-memid-helpers]` has 25 markers",
        "SAFETY[AUDIT:page-count-accessors]` has 24 markers",
        "SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 23 markers",
        "`tests/release_fast_path_audit.rs` is 2,902 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,374 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 662 lines",
        "`datatype_acceptance_criteria_have_cross_cutting_guards` test spans roughly",
        "lines 1,327 through 2,420",
        "`strict-type-allowlist.txt` still has 30 path-wide raw-pointer entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "SAFETY[AUDIT:typed-arena-domain]",
        "SAFETY[AUDIT:typed-page-queue-domain]",
        "SAFETY[AUDIT:typed-heap-domain]",
        "SAFETY[AUDIT:typed-freelist-wrappers]",
        "Replace broad strict-type entries for\n      `src/types/freelist_wrappers.rs`",
        "Move the datatype acceptance assertion cluster into a new module",
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
