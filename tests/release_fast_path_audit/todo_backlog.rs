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
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Fresh Rust Idiom Follow-up Backlog",
        "## Issue: Heap and metadata fixtures still hide unsafe migration debt",
        "## Issue: Tracking hooks still rely on file-level unsafe allowlisting",
        "## Issue: Broad strict-type usize allowances remain in alloc and aligned",
        "## Issue: Release audit modules still have broad assertion clusters",
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
        "`bf4b6df`",
        "migrated `src/types/page_queues/tests.rs` and\n`src/types/padding.rs`",
        "moved page validation helpers into `src/allocator/page/validation.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "49 entries: 41 production, 5 test,\n  and 3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 49",
        "`src/types/heap/tests.rs` is 212 lines",
        "`src/types/memid/tests.rs` is 243 lines",
        "`src/types/atomic/tests.rs` is 287 lines",
        "`src/types/scalar/tests.rs` is 412 lines",
        "`src/allocator/tracking.rs` is 185 lines",
        "`src/types/callbacks.rs` is 255 lines",
        "`src/internal/random.rs` is 257 lines",
        "`src/types/page.rs` is 258 lines",
        "`src/types/arena.rs` is 260 lines",
        "`src/types/page_queues.rs` is 304 lines",
        "`src/types/heap.rs` is 312 lines",
        "`src/allocator/arena_meta.rs` is 316 lines",
        "strict-type-allowlist.txt",
        "2 remaining `remove:` entries",
        "broad `bare-domain-usize` allowances for `alloc` and `aligned`",
        "`REMOVE_ENTRY_CAP` is 2",
        "`bare-domain-usize|src/allocator/alloc.rs|*|usize`",
        "`src/allocator/arena.rs` is 4,628 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,545 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "SAFETY[AUDIT:arena-debug-render]` has 35 markers",
        "SAFETY[AUDIT:owned-allocation]` has 32 markers",
        "SAFETY[AUDIT:unsafe-callback]` has 28 markers",
        "SAFETY[AUDIT:page-count-accessors]` has 24 markers",
        "SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 23 markers",
        "`tests/release_fast_path_audit.rs` is 2,864 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,651 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,115 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 703 lines",
        "`tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is\n  206 lines",
        "`tests/release_fast_path_audit/release_policy/security.rs` is 181 lines",
        "`tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is\n  160 lines",
        "SAFETY[AUDIT:typed-heap-test-fixtures]",
        "SAFETY[AUDIT:tracking-hooks]",
        "scripts/check-valgrind-tracking-parity.py",
        "scripts/check-asan-tracking-parity.py",
        "heap-visit/delete helpers or prefix accessor helpers",
        "`src/allocator/arena.rs` is smaller without API, ABI, or hot-path\n      performance drift",
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
