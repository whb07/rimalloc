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
    "unsafe_performance/tail_guards.rs",
    "unsafe_performance/threadlocal_boundaries.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: Memid test fixtures still hide unsafe migration debt",
        "## Issue: Callback wrappers remain file-level unsafe allowlisted",
        "## Issue: Aligned still has the last broad strict-type usize allowance",
        "## Issue: Release audit unsafe_performance.rs remains oversized",
        "## Issue: Arena remains the largest translated production review unit",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`f17fc6b`",
        "migrated heap test fixtures and tracking hooks",
        "moved arena prefix\nlayouts/accessors into `src/allocator/arena/prefix.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "45 entries: 39 production, 3 test,\n  and 3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 45",
        "`src/types/atomic/tests.rs` is 287 lines",
        "`src/types/scalar/tests.rs` is 412 lines",
        "`src/types/ptr/tests.rs` is 618 lines",
        "`src/types/memid/tests.rs` now uses\n  `SAFETY[AUDIT:typed-memid-test-fixtures]`",
        "`src/types/callbacks.rs` now uses `SAFETY[AUDIT:callback-wrappers]`",
        "`src/internal/random.rs` is 257 lines",
        "`src/types/page.rs` is 258 lines",
        "`src/types/arena.rs` is 260 lines",
        "`src/types/page_queues.rs` is 304 lines",
        "`src/types/heap.rs` is 312 lines",
        "`src/allocator/arena_meta.rs` is 316 lines",
        "strict-type-allowlist.txt",
        "1 remaining `remove:` entry",
        "the broad `bare-domain-usize` allowance for `src/allocator/aligned.rs`",
        "`REMOVE_ENTRY_CAP` is 1",
        "`src/allocator/arena.rs` is 4,520 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers",
        "SAFETY[AUDIT:arena-debug-render]` has 34 markers",
        "SAFETY[AUDIT:owned-allocation]` has 31 markers",
        "SAFETY[AUDIT:unsafe-callback]` has 26 markers",
        "SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 22 markers",
        "SAFETY[AUDIT:page-count-accessors]` has 22 markers",
        "`tests/release_fast_path_audit.rs` is 2,875 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,655 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 660 lines",
        "`tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is\n  206 lines",
        "`tests/release_fast_path_audit/release_policy/security.rs` is 181 lines",
        "`tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is\n  160 lines",
        "SAFETY[AUDIT:typed-memid-test-fixtures]",
        "SAFETY[AUDIT:callback-wrappers]",
        "Lower `REMOVE_ENTRY_CAP` to 0",
        "heap visit/delete, arena id/memid, or purge helpers",
        "`src/allocator/arena.rs` is smaller again without API, ABI, or hot-path\n      performance drift",
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
