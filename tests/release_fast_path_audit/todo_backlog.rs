use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "datatype_acceptance.rs",
    "datatype_acceptance/callbacks.rs",
    "datatype_acceptance/domain_wrappers.rs",
    "datatype_acceptance/heap_lifecycle.rs",
    "datatype_acceptance/padding_and_page_state.rs",
    "datatype_acceptance/page_queues.rs",
    "datatype_acceptance/spec_reconciliation.rs",
    "datatype_acceptance/strict_type.rs",
    "release_policy.rs",
    "release_policy/api_surface.rs",
    "release_policy/benchmarking.rs",
    "release_policy/callbacks.rs",
    "release_policy/docs_and_linking.rs",
    "release_policy/release_process.rs",
    "release_policy/security.rs",
    "runtime_gates.rs",
    "todo_backlog.rs",
    "unsafe_performance.rs",
    "unsafe_performance/allocation_free.rs",
    "unsafe_performance/arena_boundaries.rs",
    "unsafe_performance/bitmap_boundaries.rs",
    "unsafe_performance/heap_page_visitors.rs",
    "unsafe_performance/lifecycle_boundaries.rs",
    "unsafe_performance/local_boundaries.rs",
    "unsafe_performance/os_memory.rs",
    "unsafe_performance/page_arena_behavior.rs",
    "unsafe_performance/page_lifecycle.rs",
    "unsafe_performance/platform_primitive_boundaries.rs",
    "unsafe_performance/raw_facade.rs",
    "unsafe_performance/shared.rs",
    "unsafe_performance/size_class_boundaries.rs",
    "unsafe_performance/tail_guards.rs",
    "unsafe_performance/thread_stats.rs",
    "unsafe_performance/threadlocal_boundaries.rs",
];

#[test]
fn rust_idiom_backlog_tracks_current_work() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let required_headings = [
        "# TODO: Next Strict-Type and Audit Granularity Backlog",
        "## Issue 1: Refresh audit evidence paths after the release-audit split",
        "## Issue 2: Narrow the remaining allocator/init strict-type wildcards",
        "## Issue 3: Split the raw macro allocation audit cluster",
        "## Issue 4: Split allocation, aligned allocation, stats, page-map, and OS audit clusters",
        "## Issue 5: Split typed pointer accessor audit coverage",
        "## Issue 6: Split the release fast-path root harness and lower the root cap",
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
        "Fresh-eyes review after splitting typed pointer accessor audit coverage on\n`master`",
        "covers 82 source files, 5,893 unsafe\n  mentions, and 123 operation IDs",
        "covers those 123 operation IDs with\n  306 automated evidence rows",
        "26 wildcard entries left",
        "4\n  raw-pointer-parameter entries, 13 bare-domain `usize` entries, 4\n  pointer/usize round-trip entries",
        "No remaining path has more than one wildcard entry",
        "`src/allocator/heap.rs`,\n  `src/platform/mod.rs`, `src/raw/macros.rs`, `src/types/layout.rs`",
        "`init-thread-tls-boundary` with 129 markers",
        "`windows-primitive-boundary` with 114 markers",
        "`arena-page-lifecycle-boundary` with 107 markers",
        "`arena-allocation-boundary` with 105 markers",
        "`allocation-abi-entry-boundary` with 104 markers",
        "`internal-atomic-facade` with 103 markers",
        "`page-allocation-boundary` with 102 markers",
        "`init-subproc-boundary` with 102 markers",
        "previous raw macro aggregate is split into\n  `raw-macro-libc-forwarding-boundary` with 65 markers",
        "`raw-macro-cxx-boundary` with 46 markers",
        "`raw-macro-override-entry-boundary` with 36 markers",
        "`raw-macro-bootstrap-boundary` with 15 markers",
        "`raw-macro-allocation-query-boundary` with 10 markers",
        "`raw-macro-header-forwarding-boundary` with 6 markers",
        "previous allocation, aligned allocation, stats, page-map, and OS\n  aggregates are split into focused review units",
        "`stats-merge-boundary` with 75 markers",
        "`aligned-heap-allocation-boundary` with 68 markers",
        "`aligned-realloc-boundary` with 67 markers",
        "`page-map-register-boundary` with 66 markers",
        "`os-memory-allocation-boundary` with 59 markers",
        "previous `typed-pointer-accessors` aggregate with 161 markers is split\n  into pointer-family review units",
        "`typed-subproc-accessors` with 31 markers",
        "`typed-arena-accessors` with 27 markers",
        "`typed-page-accessors` with 25 markers",
        "`typed-tld-accessors` with 24 markers",
        "`typed-heap-accessors` with 16 markers",
        "`typed-thread-heap-accessors` with 14 markers",
        "`typed-page-queue-accessors` with 10 markers",
        "`typed-block-accessors` with 8 markers",
        "`typed-pointer-constructor-boundary` with 6 markers",
        "Audit evidence rows for allocation, aligned allocation, split raw macro IDs,\n  OS memory, page-map, stats, and typed pointer checks point at focused",
        "release-audit modules instead of the old aggregate harness paths",
        "root harness is still\n  1,935 lines",
        "`unsafe_performance/lifecycle_boundaries.rs` at 922 lines",
        "`unsafe_performance/platform_primitive_boundaries.rs` at 895 lines",
        "`unsafe_performance/raw_facade.rs` at 826 lines",
        "`unsafe_performance/page_arena_behavior.rs` at 724 lines",
        "1. [x] Refresh audit evidence paths after the release-audit split.",
        "2. [x] Narrow the remaining allocator/init strict-type wildcards.",
        "3. [x] Split the raw macro allocation audit cluster.",
        "4. [x] Split allocation, aligned allocation, stats, page-map, and OS audit",
        "5. [x] Split typed pointer accessor audit coverage.",
        "6. [ ] Split the release fast-path root harness and lower the root cap.",
        "[x] Unsafe audit evidence rows point at the focused tests",
        "[x] Remaining strict-type wildcard entries are either narrowed",
        "The largest unsafe audit operation IDs are split into narrower review\n      units",
        "The release fast-path root harness is split enough to lower its cap",
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
    let root_lines = fs::read_to_string(&audit_root)
        .expect("read release fast-path audit root")
        .lines()
        .count();

    assert!(
        root_lines <= 2_000,
        "release fast-path audit root should stay a small harness; found {root_lines} lines"
    );

    for module in RELEASE_AUDIT_MODULES {
        let path = module_dir.join(module);
        let line_count = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
            .lines()
            .count();

        assert!(
            line_count <= 1_000,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
