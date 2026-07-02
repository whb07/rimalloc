use super::*;

const RELEASE_AUDIT_MODULES: &[&str] = &[
    "allocation_contracts.rs",
    "allowance_inventory.rs",
    "datatype_acceptance.rs",
    "datatype_acceptance/callbacks.rs",
    "datatype_acceptance/domain_wrappers.rs",
    "datatype_acceptance/heap_lifecycle.rs",
    "datatype_acceptance/padding_and_page_state.rs",
    "datatype_acceptance/page_queues.rs",
    "datatype_acceptance/spec_reconciliation.rs",
    "datatype_acceptance/strict_type.rs",
    "interposition_exports.rs",
    "primitive_options.rs",
    "public_facade.rs",
    "release_policy.rs",
    "release_policy/api_surface.rs",
    "release_policy/benchmarking.rs",
    "release_policy/callbacks.rs",
    "release_policy/docs_and_linking.rs",
    "release_policy/release_process.rs",
    "release_policy/security.rs",
    "runtime_gates.rs",
    "safe_api.rs",
    "strict_policy.rs",
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
        "# TODO: Next Audit Granularity and Harness Backlog",
        "## Issue 1: Split init thread/TLS and subproc audit clusters",
        "## Issue 2: Split page and arena allocation lifecycle clusters",
        "## Issue 3: Split platform and atomic facade audit clusters",
        "## Issue 4: Split remaining large release-audit modules and lower the module cap",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`985da34`",
        "previous backlog is implemented, validated, committed, and\npushed",
        "covers 82 source files, 5,893 unsafe\n  mentions, and 123 operation IDs",
        "covers those 123 operation IDs with\n  306 automated evidence rows",
        "still has 26 wildcard entries left",
        "4\n  raw-pointer-parameter entries, 13 bare-domain `usize` entries, 4\n  pointer/usize round-trip entries",
        "No remaining path has more than one strict-type wildcard entry",
        "`src/allocator/heap.rs`,\n  `src/platform/mod.rs`, `src/raw/macros.rs`, `src/types/layout.rs`",
        "`init-thread-tls-boundary` with 129 markers",
        "`windows-primitive-boundary` with 114 markers",
        "`arena-page-lifecycle-boundary` with 107 markers",
        "`arena-allocation-boundary` with 105 markers",
        "`allocation-abi-entry-boundary` with 104 markers",
        "`internal-atomic-facade` with 103 markers",
        "`page-allocation-boundary` with 102 markers",
        "`init-subproc-boundary` with 102 markers",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "module cap is still 1,000 lines",
        "`unsafe_performance/lifecycle_boundaries.rs` at 922 lines",
        "`unsafe_performance/platform_primitive_boundaries.rs` at 895 lines",
        "`unsafe_performance/raw_facade.rs` at 852 lines",
        "`unsafe_performance/page_arena_behavior.rs` at 724 lines",
        "`release_policy/docs_and_linking.rs` at 633 lines",
        "`release_policy/api_surface.rs` at 517 lines",
        "`datatype_acceptance/strict_type.rs` at 512 lines",
        "`release_policy/release_process.rs` at 504 lines",
        "previous raw macro, allocation/aligned/stats/page-map/OS, typed pointer,\n  and root release-audit aggregate splits are complete",
        "Split `init-thread-tls-boundary` into thread lifecycle, thread-heap TLS",
        "Split `init-subproc-boundary` into subproc identity/registry",
        "Split page allocation markers by free-list refill/search",
        "Split arena allocation markers by slice allocation",
        "Split arena page lifecycle markers by fresh/regular/singleton page setup",
        "Split `internal-atomic-facade` into lock/once, atomic field",
        "Split `windows-primitive-boundary` into virtual memory, process info",
        "Split `unix-primitive-backend` where existing helper groups already",
        "Split `unsafe_performance/lifecycle_boundaries.rs` into process lifecycle",
        "Split `unsafe_performance/platform_primitive_boundaries.rs` into Windows",
        "Split `unsafe_performance/raw_facade.rs` into raw override",
        "Lower the release-audit module cap below 1,000",
        "1. [ ] Split init thread/TLS and subproc audit clusters.",
        "2. [ ] Split page and arena allocation lifecycle clusters.",
        "3. [ ] Split platform and atomic facade audit clusters.",
        "4. [ ] Split remaining large release-audit modules and lower the module cap.",
        "Remaining unsafe audit operation IDs above 100 markers are split",
        "Release-audit modules are split enough to lower the 1,000-line module cap",
        "`TODO.md` and its release-audit guard reflect the current measured counts",
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
        root_lines <= 300,
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
