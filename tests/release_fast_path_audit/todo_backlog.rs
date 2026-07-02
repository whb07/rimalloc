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
        "# TODO: Next Audit Cluster, Strict-Type, and Harness Backlog",
        "## Issue 1: Split OS memory and page-map initialization audit clusters",
        "## Issue 2: Split thread lifecycle and thread-local audit clusters",
        "## Issue 3: Split arena visitor and page queue audit clusters",
        "## Issue 4: Split atomic, FFI libc, and raw macro facade audit clusters",
        "## Issue 5: Tighten strict-type allowlist hotspots",
        "## Issue 6: Split release-audit modules over 500 lines and lower the cap",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`9e797a8`",
        "previous backlog is implemented, validated, committed, and\npushed",
        "covers 82 source files, 5,893 unsafe\n  mentions, and 300 operation IDs",
        "covers those 300 operation IDs with\n  654 automated evidence rows",
        "No source audit operation ID is over 60 source markers",
        "largest remaining\n  source ID is `arena-heap-page-visitors` with 56 markers",
        "`typed-atomic-domain` with 51 markers",
        "`ffi-libc-helpers` with 50 markers",
        "`page-queue-transition-boundary` with 49 markers",
        "`raw-macro-cxx-boundary`\n  with 46 markers",
        "`options-message-output-boundary` with 39 markers",
        "`arena-purge-boundary` with 38 markers",
        "`internal-page-metadata-boundary`, `arena-registry-boundary`, and\n  `allocation-realloc-entry-boundary` with 37 markers each",
        "has 634 active entries: 335\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
        "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 2 heap-allocating-type entries",
        "largest strict-type allowlist file hotspots are `src/internal/mod.rs` with\n  57 active entries",
        "`src/allocator/page.rs` with 54",
        "`src/allocator/alloc.rs` with 53",
        "`src/allocator/aligned.rs` with 50",
        "`src/internal/bitmap.rs` with 45",
        "`src/raw/macros.rs` with 32",
        "largest\n  raw-pointer domains are `mi_theap_t` with 114 entries",
        "`mi_page_t` with 110",
        "`mi_heap_t` with 67",
        "has 1 retained wildcard entry, the\n  `heap-allocating-type` Loom proof-model boundary",
        "Strict-type wildcard guard: total=1; heap-allocating-type=1.",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "release-audit module cap is 600 lines",
        "only modules over 500 lines are\n  `datatype_acceptance/strict_type.rs` at 592 lines",
        "`release_policy/security.rs` at 597 lines",
        "`release_policy/api_surface.rs` at 523 lines",
        "`release_policy/release_process.rs` at 504 lines",
        "Full validation for Issue 2 passed",
        "Issue 1 split `os-memory-allocation-boundary` into primitive allocation",
        "It split `page-map-init-boundary` into 32-bit commit\n  bitmap",
        "Issue 2 split `init-thread-heap-tls-boundary` into empty sentinel",
        "It split `init-thread-lifecycle-boundary` into thread\n  identity",
        "It split\n  `threadlocal-key-manager` into empty sentinel",
        "stale broad source IDs\n> `os-memory-allocation-boundary` and `page-map-init-boundary` are gone",
        "stale broad source IDs\n> `init-thread-heap-tls-boundary`, `init-thread-lifecycle-boundary`, and\n> `threadlocal-key-manager` are gone",
        "Split `os-memory-allocation-boundary` by operation",
        "Split `init-thread-heap-tls-boundary` into focused IDs",
        "Split `arena-heap-page-visitors` into focused IDs",
        "Split `typed-atomic-domain` by wrapper family",
        "Add a release-audit guard that computes strict-type allowlist counts by\n      kind, file, kind/file pair, and raw-pointer domain",
        "lower the release-audit module cap from 600 to 500",
        "1. [x] Split OS memory and page-map initialization audit clusters.",
        "2. [x] Split thread lifecycle and thread-local audit clusters.",
        "3. [ ] Split arena visitor and page queue audit clusters.",
        "4. [ ] Split atomic, FFI libc, and raw macro facade audit clusters.",
        "5. [ ] Tighten strict-type allowlist hotspots.",
        "6. [ ] Split release-audit modules over 500 lines and lower the cap.",
        "No source audit operation ID has more than 40 source markers",
        "Strict-type allowlist hotspots are measured in release-audit coverage",
        "Release-audit modules are split enough to lower the module cap to 500\n      lines",
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
            line_count <= 600,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
