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
        "## Completed: Process initialization boundary moved off the file-level unsafe allowance",
        "## Completed: Internal bitmap boundary moved off the file-level unsafe allowance",
        "## Completed: Hot allocation entry boundary moved off the file-level unsafe allowance",
        "## Completed: Page allocation boundary moved off the file-level unsafe allowance",
        "## Issue: Arena region boundary still uses a file-level unsafe allowance",
        "## Issue: Internal translation boundary still uses a file-level unsafe allowance",
        "## Issue: Windows primitive translation boundary still uses a file-level unsafe allowance",
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
        "`da44c1a`",
        "`src/allocator/free.rs`,\n`src/allocator/options.rs`, `src/allocator/aligned.rs`,",
        "`src/allocator/init.rs`,\n`src/internal/bitmap.rs`, `src/allocator/alloc.rs`, and",
        "`src/allocator/page.rs` off the unsafe migration allowlist",
        "docs/unsafe-migration-allowlist.tsv",
        "3 entries: 1 production and\n  2 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 3",
        "`src/allocator/free.rs` to 69",
        "`SAFETY[AUDIT:options-callback-boundary]` markers",
        "`src/allocator/aligned.rs` to 158",
        "`src/allocator/stats.rs` to 161",
        "`src/raw/macros.rs` to 178",
        "`SAFETY[AUDIT:raw-macro-allocation-boundary]` markers",
        "`src/allocator/init.rs` to 277",
        "`SAFETY[AUDIT:init-lifecycle-boundary]` markers",
        "`src/internal/bitmap.rs` to 71",
        "`SAFETY[AUDIT:internal-bitmap-boundary]` markers",
        "`src/allocator/alloc.rs` to 166",
        "`SAFETY[AUDIT:allocation-entry-boundary]` markers",
        "`src/allocator/page.rs` to 102",
        "`SAFETY[AUDIT:page-allocation-boundary]` markers",
        "The remaining production allowlist entry with no local markers is",
        "`src/allocator/arena.rs` at 3,915 lines with 607 `unsafe` hits",
        "Remaining translation allowlist entries with no local markers are",
        "`src/internal/mod.rs` at 1,533 lines with 163 `unsafe` hits",
        "`src/platform/windows.rs` at 2,191 lines with 214 `unsafe` hits",
        "`strict-type-allowlist.txt` still has 280 entries",
        "150 raw-pointer-parameter\n  entries, 105 bare-domain `usize` entries, 13 pointer/usize round-trip",
        "`REMOVE_ENTRY_CAP` remains 0",
        "`src/allocator/aligned.rs`,\n  `src/internal/mod.rs`, `src/platform/unix.rs`, `src/platform/windows.rs`,",
        "`src/allocator/alloc.rs` and\n  `src/allocator/init.rs` each still have 3 path-wide entries; both are no",
        "longer on the unsafe migration allowlist",
        "SAFETY[AUDIT:init-lifecycle-boundary]` has 277 markers",
        "SAFETY[AUDIT:raw-macro-allocation-boundary]` has 178 markers",
        "SAFETY[AUDIT:allocation-entry-boundary]` has 166 markers",
        "SAFETY[AUDIT:stats-reporting-boundary]` has 161 markers",
        "SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers",
        "SAFETY[AUDIT:aligned-allocation-boundary]` has 158 markers",
        "SAFETY[AUDIT:page-map-boundary]` has 153 markers",
        "SAFETY[AUDIT:os-memory-boundary]` has 136 markers",
        "SAFETY[AUDIT:internal-atomic-facade]` has 103 markers",
        "SAFETY[AUDIT:page-allocation-boundary]` has 102 markers",
        "SAFETY[AUDIT:unix-primitive-backend]` has 90 markers",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers",
        "SAFETY[AUDIT:macos-zone-override]` has 83 markers",
        "SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers",
        "SAFETY[AUDIT:options-callback-boundary]` has 76 markers",
        "SAFETY[AUDIT:thread-heap-lifecycle]` has 71 markers",
        "SAFETY[AUDIT:internal-bitmap-boundary]` has 71 markers",
        "SAFETY[AUDIT:raw-posix-compat]` has 70 markers",
        "SAFETY[AUDIT:free-path-boundary]` has 69 markers",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,504 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,150 lines",
        "`tests/release_fast_path_audit.rs` is 1,935 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,131 lines",
        "`tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`\n  is 831 lines",
        "SAFETY[AUDIT:init-lifecycle-boundary]",
        "SAFETY[AUDIT:internal-bitmap-boundary]",
        "SAFETY[AUDIT:arena-region-boundary]",
        "SAFETY[AUDIT:internal-translation-boundary]",
        "SAFETY[AUDIT:windows-primitive-boundary]",
        "1. [x] Migrate `src/allocator/init.rs` off the unsafe migration allowlist",
        "2. [x] Migrate `src/internal/bitmap.rs` off the unsafe migration allowlist",
        "3. [x] Migrate `src/allocator/alloc.rs` off the unsafe migration allowlist",
        "4. [x] Migrate `src/allocator/page.rs` off the unsafe migration allowlist",
        "5. [ ] Migrate `src/allocator/arena.rs` off the unsafe migration allowlist",
        "6. [ ] Migrate `src/internal/mod.rs` off the unsafe migration allowlist",
        "7. [ ] Migrate `src/platform/windows.rs` off the unsafe migration allowlist",
        "reaches zero when the final entry is removed",
        "The completed init, bitmap, allocation, and page migrations plus all",
        "3 remaining allowlisted files are covered by operation-level audit markers",
        "rescan the codebase with fresh eyes for the next\n      class of mechanical-translation debt",
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
