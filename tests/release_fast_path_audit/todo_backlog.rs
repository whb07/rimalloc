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
    "unsafe_performance/lifecycle_boundaries/options_guarded.rs",
    "unsafe_performance/lifecycle_boundaries/platform_hooks.rs",
    "unsafe_performance/lifecycle_boundaries/process_lifecycle.rs",
    "unsafe_performance/lifecycle_boundaries/random_cpu.rs",
    "unsafe_performance/lifecycle_boundaries/thread_tls.rs",
    "unsafe_performance/local_boundaries.rs",
    "unsafe_performance/os_memory.rs",
    "unsafe_performance/page_arena_behavior.rs",
    "unsafe_performance/page_arena_behavior/arena_helpers.rs",
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
        "# TODO: Next Audit Granularity, Strict-Type, and Harness Backlog",
        "## Issue 1: Split allocation ABI entry audit cluster",
        "## Issue 2: Split broad runtime and internal marker clusters",
        "## Issue 3: Split raw/POSIX, bitmap, and release-audit no-export modules",
        "## Issue 4: Reduce strict-type wildcard debt",
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
        "Fresh-eyes review after completing the previous TODO backlog on `master` at\n`0bcbd7a`",
        "previous backlog is implemented, validated, committed, and\npushed",
        "covers 82 source files, 5,893 unsafe\n  mentions, and 216 operation IDs",
        "covers those 216 operation IDs with\n  487 automated evidence rows",
        "No unsafe audit operation ID is above 100 source markers",
        "`allocation-realloc-entry-boundary` with\n  37 markers",
        "`allocation-cxx-new-boundary` with 30 markers",
        "`allocation-product-entry-boundary` with 21 markers",
        "`allocation-string-path-boundary` with 20 markers",
        "`allocation-platform-shim-boundary` with 4 markers",
        "Issue 2 runtime/internal split removed the broad\n  `heap-lifecycle-boundary`, `options-callback-boundary`",
        "`options-message-output-boundary` with 39 markers",
        "`internal-page-metadata-boundary` with 37 markers",
        "`thread-heap-visit-boundary` with 28 markers",
        "`heap-ownership-query-boundary` with 23 markers",
        "Issue 3 raw/POSIX and bitmap split removed the broad\n  `internal-bitmap-boundary`, `raw-posix-compat`",
        "`raw-macro-unix-aligned-posix-boundary` and\n  `raw-macro-linux-libc-boundary` with 20 markers each",
        "`raw-posix-realloc-boundary` and `internal-bitmap-range-chunk-boundary` with\n  19 markers each",
        "`raw-posix-environment-boundary` and\n  `raw-posix-aligned-allocation-boundary` with 13 markers each",
        "`typed-pointer-test-fixtures` with 87 markers",
        "`macos-zone-override` with 83 markers",
        "`free-path-boundary` with 69 markers",
        "`aligned-heap-allocation-boundary` with 68 markers",
        "`aligned-realloc-boundary` with 67 markers",
        "`page-map-register-boundary` with 66 markers",
        "`internal-atomic-field-boundary` with 65 markers",
        "`os-memory-allocation-boundary` with 59 markers",
        "`init-thread-heap-tls-boundary` and `arena-heap-page-visitors` with 56\n  markers each",
        "has 515 active entries: 323\n  raw-pointer-parameter entries, 131 bare-domain `usize` entries, 35\n  metadata-reference entries",
        "still has 26 wildcard entries: 13\n  bare-domain `usize` entries, 4 raw-pointer-parameter entries, 4\n  pointer/usize round-trip entries",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "module cap is now 700 lines",
        "`unsafe_performance/page_arena_behavior.rs` at 656 lines",
        "`release_policy/docs_and_linking.rs` at 633 lines",
        "`release_policy/security.rs` at 526 lines",
        "`release_policy/api_surface.rs` at 523 lines",
        "`datatype_acceptance/strict_type.rs` at 512 lines",
        "`release_policy/release_process.rs` at 504 lines",
        "`unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 426 lines",
        "`interposition_exports.rs` at 413 lines",
        "`unsafe_performance/os_memory.rs` at 403 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs`\n  at 400 lines",
        "previous lifecycle, platform, raw facade, allocation ABI, page/arena,\n  atomic, and root release-audit aggregate splits are complete",
        "full workspace validation for the last item passed",
        "Split `allocation-abi-entry-boundary` into narrow operation IDs",
        "Keep the hot allocation path C-shaped",
        "Split `options-callback-boundary` into option storage/locking",
        "Split `internal-translation-boundary` into page metadata helpers",
        "Split `raw-posix-compat` by errno/platform shims",
        "Split `internal-bitmap-boundary` into bitmap traversal",
        "lower the\n      release-audit module cap from 850 to 700",
        "Replace wildcard entries with symbol-specific entries",
        "Add a strict-type guard that tracks the wildcard count by category",
        "1. [x] Split allocation ABI entry audit cluster.",
        "2. [x] Split broad runtime and internal marker clusters.",
        "3. [x] Split raw/POSIX, bitmap, and release-audit no-export modules.",
        "4. [ ] Reduce strict-type wildcard debt.",
        "[x] No unsafe audit operation ID has more than 100 source markers",
        "[x] Release-audit modules are split enough to lower the module cap to 700\n      lines",
        "Strict-type wildcard entries are reduced or guarded by category-specific\n      evidence",
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
            line_count <= 700,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
