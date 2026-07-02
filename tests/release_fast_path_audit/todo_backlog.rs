use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "release_policy.rs",
    "release_policy/security.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/bitmap_boundaries.rs",
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
        "## Issue: Atomic test fixtures still hide unsafe migration debt",
        "## Issue: Random remains a small production unsafe allowlisted module",
        "## Issue: Arena heap page visitors remain embedded in arena.rs",
        "## Issue: Release audit unsafe_performance.rs still carries broad source assertions",
        "## Issue: Arena strict-type entries are still broad after child splits",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`eee7448`",
        "migrated memid test fixtures and callback wrappers",
        "moved\narena id/memid helpers into `src/allocator/arena/ids.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "43 entries: 38 production, 2 test,\n  and 3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 43",
        "`src/types/scalar/tests.rs` is 412 lines",
        "`src/types/ptr/tests.rs` is 618 lines",
        "`src/types/atomic/tests.rs` now uses\n  `SAFETY[AUDIT:typed-atomic-test-fixtures]`",
        "It is 356 lines with 16 local audit markers",
        "`src/internal/random.rs` now uses\n  `SAFETY[AUDIT:random-context-boundaries]`",
        "It is 282 lines with 12\n  local audit markers",
        "`src/allocator/arena/heap_pages.rs` now owns heap visit/delete/move/destroy\n  helpers",
        "`SAFETY[AUDIT:arena-heap-page-visitors]`",
        "It is 543\n  lines with 56 local audit markers",
        "`src/types/page.rs` is 258 lines",
        "`src/types/arena.rs` is 260 lines",
        "`src/types/page_queues.rs` is 304 lines",
        "`src/types/heap.rs` is 312 lines",
        "`src/allocator/arena_meta.rs` is 316 lines",
        "strict-type-allowlist.txt",
        "0 remaining `remove:` entries",
        "`REMOVE_ENTRY_CAP` is 0",
        "`src/allocator/arena.rs` still has broad\n  permanent raw-pointer and pointer/usize compatibility entries",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/arena/ids.rs`, which is 244 lines",
        "`src/allocator/arena/heap_pages.rs`, which is 543 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers",
        "SAFETY[AUDIT:arena-debug-render]` has 34 markers",
        "SAFETY[AUDIT:owned-allocation]` has 31 markers",
        "SAFETY[AUDIT:unsafe-callback]` has 26 markers",
        "SAFETY[AUDIT:arena-id-memid-helpers]` has 24 markers",
        "SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 22 markers",
        "SAFETY[AUDIT:page-count-accessors]` has 22 markers",
        "`tests/release_fast_path_audit.rs` is 2,893 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,583 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 662 lines",
        "`tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is\n  207 lines",
        "`tests/release_fast_path_audit/release_policy/security.rs` is 181 lines",
        "`tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is\n  162 lines",
        "`tests/release_fast_path_audit/unsafe_performance/size_class_boundaries.rs`\n  is 132 lines",
        "heap visit/delete/move/destroy cluster now lives in\n  `src/allocator/arena/heap_pages.rs`",
        "`mi_heap_visit_info_t`, `mi_heap_delete_visit_info_t`",
        "SAFETY[AUDIT:typed-atomic-test-fixtures]",
        "SAFETY[AUDIT:random-context-boundaries]",
        "`src/internal/random.rs` off the unsafe migration allowlist",
        "`src/allocator/arena.rs` is smaller again without API, ABI, or hot-path\n      performance drift",
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
