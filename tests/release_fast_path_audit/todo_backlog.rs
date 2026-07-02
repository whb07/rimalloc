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
    "unsafe_performance/platform_primitive_boundaries.rs",
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
        "## Issue: Free path ownership boundary still uses a file-level unsafe allowance",
        "## Issue: Options, output, and callback state still use a file-level unsafe allowance",
        "## Issue: Aligned allocation boundary still uses a file-level unsafe allowance",
        "## Issue: Stats reporting boundary still uses a file-level unsafe allowance",
        "## Issue: Raw macro allocation facade still uses a file-level unsafe allowance",
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
        "`33a8f2d`",
        "`src/types/constants.rs`,\n`src/allocator/thread_heap.rs`, `src/platform/unix.rs`, `src/platform/os.rs`,\nand `src/types/ptr.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "8 entries: 6 production and\n  2 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 8",
        "The completed pass moved `src/types/constants.rs` to 8",
        "`SAFETY[AUDIT:raw-layout-abi-typedefs]` markers",
        "`SAFETY[AUDIT:raw-auto-trait]` markers",
        "`src/allocator/thread_heap.rs` to 71",
        "`SAFETY[AUDIT:thread-heap-lifecycle]` markers",
        "`src/platform/unix.rs` to 90",
        "`SAFETY[AUDIT:unix-primitive-backend]` markers",
        "`src/platform/os.rs` to 136",
        "`SAFETY[AUDIT:os-memory-boundary]`",
        "`src/types/ptr.rs` to 161",
        "`SAFETY[AUDIT:typed-pointer-accessors]` markers",
        "This pass moved `src/allocator/free.rs` to 69",
        "`SAFETY[AUDIT:free-path-boundary]` markers for free fast-path assembly",
        "abandoned collect/reclaim/reabandon decisions",
        "This pass also moved `src/allocator/options.rs` to 76",
        "`SAFETY[AUDIT:options-callback-boundary]` markers for option descriptors",
        "This pass also moved `src/allocator/aligned.rs` to 158",
        "`SAFETY[AUDIT:aligned-allocation-boundary]` markers for alignment-sensitive",
        "This pass also moved `src/allocator/stats.rs` to 161",
        "`SAFETY[AUDIT:stats-reporting-boundary]` markers for counter updates",
        "`src/raw/macros.rs` at 2,223 lines with 383 `unsafe` hits",
        "`src/allocator/init.rs` at 2,339 lines with 333 `unsafe` hits",
        "`src/internal/bitmap.rs` at 2,358 lines with 141 `unsafe` hits",
        "`src/allocator/alloc.rs` at 2,559 lines with 333 `unsafe` hits",
        "`src/allocator/page.rs` at 2,801 lines with 196 `unsafe` hits",
        "`src/allocator/arena.rs` at 3,915 lines with 607 `unsafe` hits",
        "`src/internal/mod.rs` at\n  1,533 lines with 163 `unsafe` hits",
        "`src/platform/windows.rs` at\n  2,191 lines with 214 `unsafe` hits",
        "`strict-type-allowlist.txt` has 280 entries",
        "150 raw-pointer-parameter\n  entries, 105 bare-domain `usize` entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "`src/allocator/aligned.rs`,\n  `src/allocator/stats.rs`, `src/allocator/thread_heap.rs`,",
        "`src/platform/windows.rs`, each with 4 path-wide entries",
        "SAFETY[AUDIT:stats-reporting-boundary]` has 161 markers",
        "SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers",
        "SAFETY[AUDIT:aligned-allocation-boundary]` has 158 markers",
        "SAFETY[AUDIT:page-map-boundary]` has 153 markers",
        "SAFETY[AUDIT:os-memory-boundary]` has 136 markers",
        "SAFETY[AUDIT:internal-atomic-facade]` has 103 markers",
        "SAFETY[AUDIT:unix-primitive-backend]` has 90 markers",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers",
        "SAFETY[AUDIT:macos-zone-override]` has 83 markers",
        "SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers",
        "SAFETY[AUDIT:options-callback-boundary]` has 76 markers",
        "SAFETY[AUDIT:thread-heap-lifecycle]` has 71 markers",
        "SAFETY[AUDIT:raw-posix-compat]` has 70 markers",
        "SAFETY[AUDIT:free-path-boundary]` has 69 markers",
        "SAFETY[AUDIT:platform-tls-primitives]` has 60 markers",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers",
        "SAFETY[AUDIT:typed-atomic-domain]` has 51 markers",
        "SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers",
        "SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,462 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,146 lines",
        "`tests/release_fast_path_audit.rs` is 1,919 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,131 lines",
        "SAFETY[AUDIT:free-path-boundary]",
        "SAFETY[AUDIT:options-callback-boundary]",
        "SAFETY[AUDIT:aligned-allocation-boundary]",
        "SAFETY[AUDIT:stats-reporting-boundary]",
        "SAFETY[AUDIT:raw-macro-allocation-boundary]",
        "1. [x] Migrate `src/allocator/free.rs` off the unsafe migration allowlist",
        "2. [x] Migrate `src/allocator/options.rs` off the unsafe migration allowlist",
        "3. [x] Migrate `src/allocator/aligned.rs` off the unsafe migration allowlist",
        "4. [x] Migrate `src/allocator/stats.rs` off the unsafe migration allowlist",
        "5. [ ] Migrate `src/raw/macros.rs` off the unsafe migration allowlist",
        "Free routing, option/callback behavior, aligned allocation layout, stats",
        "rescan the codebase with fresh\n      eyes",
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
