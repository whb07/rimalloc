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
    "release_policy/docs_and_linking/c_migration_guidance.rs",
    "release_policy/docs_and_linking/feature_no_std_docs.rs",
    "release_policy/docs_and_linking/ffi_linking_artifacts.rs",
    "release_policy/docs_and_linking/public_docs_build_mode.rs",
    "release_policy/docs_and_linking/symbol_versioning.rs",
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
    "unsafe_performance/lifecycle_boundaries/options_guarded.rs",
    "unsafe_performance/lifecycle_boundaries/platform_hooks.rs",
    "unsafe_performance/lifecycle_boundaries/process_lifecycle.rs",
    "unsafe_performance/lifecycle_boundaries/random_cpu.rs",
    "unsafe_performance/lifecycle_boundaries/thread_tls.rs",
    "unsafe_performance/local_boundaries.rs",
    "unsafe_performance/os_memory.rs",
    "unsafe_performance/page_arena_behavior.rs",
    "unsafe_performance/page_arena_behavior/arena_helpers.rs",
    "unsafe_performance/page_arena_behavior/arena_page_allocation.rs",
    "unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs",
    "unsafe_performance/page_arena_behavior/page_map_atomic_helpers.rs",
    "unsafe_performance/page_arena_behavior/page_map_range_witnesses.rs",
    "unsafe_performance/page_arena_behavior/purge_clock_atomic.rs",
    "unsafe_performance/page_lifecycle.rs",
    "unsafe_performance/platform_primitive_boundaries.rs",
    "unsafe_performance/platform_primitive_boundaries/macos.rs",
    "unsafe_performance/platform_primitive_boundaries/shared_policy.rs",
    "unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs",
    "unsafe_performance/platform_primitive_boundaries/windows.rs",
    "unsafe_performance/raw_facade.rs",
    "unsafe_performance/raw_facade/bootstrap_query.rs",
    "unsafe_performance/raw_facade/cxx_compat.rs",
    "unsafe_performance/raw_facade/header_forwarding.rs",
    "unsafe_performance/raw_facade/libc_posix.rs",
    "unsafe_performance/raw_facade/raw_override.rs",
    "unsafe_performance/raw_facade/raw_override/allocator_page.rs",
    "unsafe_performance/raw_facade/raw_override/internal_imports.rs",
    "unsafe_performance/raw_facade/raw_override/libc_helpers.rs",
    "unsafe_performance/raw_facade/raw_override/options_stats.rs",
    "unsafe_performance/raw_facade/raw_override/os_random.rs",
    "unsafe_performance/raw_facade/raw_override/runtime_heap.rs",
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
        "# TODO: Next Audit Cluster and Harness Backlog",
        "## Issue 1: Split typed pointer and heap proof-fixture audit clusters",
        "## Issue 2: Split macOS zone and platform primitive marker clusters",
        "## Issue 3: Split free-path and aligned allocation marker clusters",
        "## Issue 4: Split page-map registration and internal atomic marker clusters",
        "## Issue 5: Split largest release-audit modules and lower the module cap",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`e44eabb`",
        "previous backlog is implemented, validated, committed, and\npushed",
        "covers 82 source files, 5,893 unsafe\n  mentions, and 270 operation IDs",
        "covers those 270 operation IDs with\n  589 automated evidence rows",
        "The largest source audit operation IDs are `os-memory-allocation-boundary`\n  with 59 markers",
        "`arena-heap-page-visitors` and\n  `init-thread-heap-tls-boundary` with 56 markers each",
        "`typed-atomic-domain` with 51 markers",
        "`threadlocal-key-manager` and\n  `ffi-libc-helpers` with 50 markers each",
        "`page-queue-transition-boundary` with 49 markers",
        "The completed typed fixture split left the largest new fixture IDs at\n  `typed-heap-thread-tld-pointer-test-fixtures` with 35 markers",
        "`typed-arena-subproc-pointer-test-fixtures` with 17 markers",
        "`typed-thread-heap-state-test-fixtures` with 15 markers",
        "The completed platform split left the largest new platform IDs at\n  `platform-thread-heap-association-boundary` with 32 markers",
        "`macos-zone-allocation-boundary` with 22 markers",
        "`emscripten-memory-boundary` with 20 markers",
        "`macos-zone-lookup-boundary` with 16 markers",
        "The completed free/aligned split left the largest new allocation IDs at\n  `free-validation-boundary` with 28 markers",
        "`aligned-realloc-heap-export-boundary` with 26 markers",
        "`aligned-global-allocation-export-boundary` with 17 markers",
        "`aligned-realloc-request-boundary` and\n  `aligned-heap-allocation-export-boundary` with 15 markers each",
        "The completed page-map/atomic split left the largest split IDs at\n  `internal-atomic-fetch-boundary` with 22 markers",
        "`page-map-lookup-boundary` with 19 markers",
        "`internal-atomic-cas-boundary` with 17 markers",
        "`page-map-page-unregister-boundary` with 15 markers",
        "`page-map-page-register-boundary` with 14 markers",
        "The largest remaining medium clusters are `init-thread-lifecycle-boundary`\n  with 48 markers",
        "`raw-macro-cxx-boundary` with 46 markers",
        "`page-map-init-boundary` with 45 markers",
        "`options-message-output-boundary` with 39 markers",
        "`arena-purge-boundary` with 38 markers",
        "has 634 active entries: 335\n  raw-pointer-parameter entries, 220 bare-domain `usize` entries, 40\n  metadata-reference entries",
        "has 1 retained wildcard entry, the\n  `heap-allocating-type` Loom proof-model boundary",
        "Strict-type wildcard guard: total=1; heap-allocating-type=1.",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "`datatype_acceptance/strict_type.rs` at 592 lines",
        "`release_policy/security.rs` at 584 lines",
        "`release_policy/api_surface.rs` at 523 lines",
        "`release_policy/release_process.rs` at 504 lines",
        "`unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 426 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs`\n  at 421 lines",
        "`interposition_exports.rs` at 413 lines",
        "`unsafe_performance/os_memory.rs` at 403 lines",
        "`allowance_inventory.rs` at 398 lines",
        "`unsafe_performance/allocation_free.rs` at 397 lines",
        "module cap is now 600 lines",
        "completed release-audit split moved\n  `unsafe_performance/page_arena_behavior.rs` behind focused child modules",
        "`unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs` at 367\n  lines",
        "`release_policy/docs_and_linking.rs` is now a 10-line module",
        "`release_policy/docs_and_linking/c_migration_guidance.rs` at 172 lines",
        "Full validation for the last completed item passed",
        "Split `typed-pointer-test-fixtures` by pointer family and proof purpose",
        "Split `typed-heap-test-fixtures` by ownership witness type",
        "Split `macos-zone-override` into operation IDs for zone declarations",
        "Preserve the raw platform ABI shape",
        "Split `free-path-boundary` by validation/double-free detection",
        "Split `aligned-heap-allocation-boundary` by aligned layout parsing",
        "Split `aligned-realloc-boundary` by old-allocation validation",
        "Split `page-map-register-boundary` into operation IDs",
        "Split `internal-atomic-field-boundary` into operation IDs",
        "Split `unsafe_performance/page_arena_behavior.rs` into focused modules",
        "Split `release_policy/docs_and_linking.rs` into focused modules",
        "lower the release\n      audit module cap from 700 to 600",
        "1. [x] Split typed pointer and heap proof-fixture audit clusters.",
        "2. [x] Split macOS zone and platform primitive marker clusters.",
        "3. [x] Split free-path and aligned allocation marker clusters.",
        "4. [x] Split page-map registration and internal atomic marker clusters.",
        "5. [x] Split largest release-audit modules and lower the module cap.",
        "[x] No source audit operation ID has more than 60 source markers",
        "[x] The largest remaining page-map and atomic clusters",
        "[x] Release-audit modules are split enough to lower the module cap to 600\n      lines",
        "[x] The strict-type wildcard guard remains at exactly one retained\n      proof-model wildcard",
        "[x] `TODO.md` and its release-audit guard reflect the current measured counts",
        "[x] For every completed item, run `cargo fmt --all`",
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
            line_count <= 600,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
