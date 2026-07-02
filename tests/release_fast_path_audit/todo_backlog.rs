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
        "covers 82 source files, 5,893 unsafe\n  mentions, and 163 operation IDs",
        "covers those 163 operation IDs with\n  383 automated evidence rows",
        "still has 26 wildcard entries left",
        "4\n  raw-pointer-parameter entries, 13 bare-domain `usize` entries, 4\n  pointer/usize round-trip entries",
        "No remaining path has more than one strict-type wildcard entry",
        "`src/allocator/heap.rs`,\n  `src/platform/mod.rs`, `src/raw/macros.rs`, `src/types/layout.rs`",
        "`allocation-abi-entry-boundary` with 104 markers",
        "`typed-pointer-test-fixtures` with 87 markers",
        "`macos-zone-override` with 83 markers",
        "`heap-lifecycle-boundary` with 80 markers",
        "`options-callback-boundary` and `internal-translation-boundary` with 76\n  markers each",
        "`stats-merge-boundary` with 75 markers",
        "`thread-heap-lifecycle` and `internal-bitmap-boundary` with 71 markers each",
        "`raw-posix-compat` with 70 markers",
        "`free-path-boundary` with 69 markers",
        "`aligned-heap-allocation-boundary` with 68 markers",
        "`aligned-realloc-boundary` with 67 markers",
        "`page-map-register-boundary` with 66 markers",
        "`raw-macro-libc-forwarding-boundary` and `internal-atomic-field-boundary`\n  with 65 markers each",
        "previous `init-thread-tls-boundary` aggregate is split into\n  `init-thread-heap-tls-boundary` with 56 markers",
        "`init-thread-lifecycle-boundary` with 48 markers",
        "`init-platform-tls-boundary` with 17 markers",
        "`init-tld-lifecycle-boundary` with 7 markers",
        "previous\n  `init-subproc-boundary` aggregate is split into\n  `init-subproc-heap-association-boundary` with 36 markers",
        "`init-subproc-registry-boundary` with 32 markers",
        "`init-subproc-identity-boundary` with 16 markers",
        "`init-subproc-stats-count-boundary` with 12 markers",
        "`init-main-subproc-lifecycle-boundary` with 6 markers",
        "previous `page-allocation-boundary` aggregate is split into\n  `page-queue-transition-boundary` with 49 markers",
        "`page-free-list-refill-boundary` with 30 markers",
        "`page-stats-public-boundary` with 19 markers",
        "`page-huge-allocation-boundary` with 4 markers",
        "previous `arena-allocation-boundary` aggregate is split into\n  `arena-slice-allocation-boundary` with 30 markers",
        "`arena-public-reserve-boundary` with 18 markers",
        "`arena-initialization-boundary` with 17 markers",
        "`arena-os-fallback-boundary` with 15 markers",
        "`arena-huge-reservation-boundary` with 14 markers",
        "`arena-reserve-growth-boundary` with 11 markers",
        "previous `arena-page-lifecycle-boundary` aggregate is split into\n  `arena-page-reabandon-unabandon-boundary` and `arena-page-free-boundary`\n  with 23 markers each",
        "`arena-page-fresh-boundary` with 22 markers",
        "`arena-page-abandon-boundary` with 17 markers",
        "`arena-page-regular-boundary` with 11 markers",
        "`arena-page-ownership-transition-boundary` with 6 markers",
        "`arena-heap-page-publication-boundary` with 3 markers",
        "`arena-page-singleton-boundary` with 2 markers",
        "previous `internal-atomic-facade` aggregate is split into\n  `internal-atomic-field-boundary` with 65 markers",
        "`internal-atomic-platform-lock-boundary` with 20 markers",
        "`internal-atomic-guard-once-boundary` with 12 markers",
        "`internal-atomic-pointer-boundary` with 6 markers",
        "Remote-free/thread-free\n  typed atomic ownership remains in the existing `typed-atomic-domain` boundary",
        "previous `windows-primitive-boundary` aggregate is split into\n  `windows-virtual-memory-boundary` with 32 markers",
        "`windows-tls-fls-boundary` with 27 markers",
        "`windows-large-page-boundary` with 22 markers",
        "`windows-process-info-boundary` with 11 markers",
        "`windows-numa-boundary` with 8 markers",
        "`windows-output-env-boundary` with 7 markers",
        "`windows-etw-boundary` with 4 markers",
        "`windows-random-boundary` with 2 markers",
        "`windows-dynamic-import-boundary` with 1 marker",
        "previous `unix-primitive-backend` aggregate is split into\n  `unix-mmap-syscall-boundary` with 26 markers",
        "`unix-huge-page-boundary` with 14 markers",
        "`unix-output-env-boundary` with 13 markers",
        "`unix-process-info-boundary` with 11 markers",
        "`unix-thread-tls-boundary` with 9 markers",
        "`unix-random-boundary` and `unix-numa-boundary` with 7 markers each",
        "`unix-ffi-import-boundary` with 3 markers",
        "release-audit root harness is 247 lines under a 300-line root cap",
        "module cap is now 850 lines",
        "`unsafe_performance/raw_facade/raw_override.rs` at 754 lines",
        "`unsafe_performance/page_arena_behavior.rs` at 724 lines",
        "`release_policy/docs_and_linking.rs` at 633 lines",
        "`release_policy/api_surface.rs` at 517 lines",
        "`datatype_acceptance/strict_type.rs` at 512 lines",
        "`release_policy/release_process.rs` at 504 lines",
        "`release_policy/security.rs` at 451 lines",
        "`unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 404 lines",
        "`unsafe_performance/os_memory.rs` at 403 lines",
        "`unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs`\n  at 400 lines",
        "previous raw macro, allocation/aligned/stats/page-map/OS, typed pointer,\n  and root release-audit aggregate splits are complete",
        "Split `init-thread-tls-boundary` into thread lifecycle, thread-heap TLS",
        "Split `init-subproc-boundary` into subproc identity/registry",
        "Split page allocation markers by free-list refill/search",
        "Split arena allocation markers by slice allocation",
        "Split arena page lifecycle markers by fresh/regular/singleton page setup",
        "Split `internal-atomic-facade` into scalar atomic field",
        "thread-free atomic ownership remains covered by the existing\n      `typed-atomic-domain` boundary",
        "Split `windows-primitive-boundary` into virtual memory, process info",
        "dynamic import operation IDs",
        "Split `unix-primitive-backend` where existing helper groups already",
        "huge-page, NUMA, and FFI import behavior",
        "Split `unsafe_performance/lifecycle_boundaries.rs` into process lifecycle",
        "Split `unsafe_performance/platform_primitive_boundaries.rs` into Windows",
        "Split `unsafe_performance/raw_facade.rs` into raw override",
        "Lower the release-audit module cap below 1,000",
        "1. [x] Split init thread/TLS and subproc audit clusters.",
        "2. [x] Split page and arena allocation lifecycle clusters.",
        "3. [x] Split platform and atomic facade audit clusters.",
        "4. [x] Split remaining large release-audit modules and lower the module cap.",
        "Remaining unsafe audit operation IDs above 100 markers are split",
        "Release-audit modules are split enough to lower the module cap to 850\n      lines",
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
            line_count <= 850,
            "release fast-path audit module {} should stay reviewable; found {line_count} lines",
            path.display()
        );
    }
}
