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
        "## Issue: macOS malloc-zone override still uses a file-level unsafe allowance",
        "## Issue: Platform TLS and primitive glue still use a file-level unsafe allowance",
        "## Issue: Internal atomic facade still uses a file-level unsafe allowance",
        "## Issue: Page-map registration and lookup still use a file-level unsafe allowance",
        "## Issue: Scalar domain wrappers still use a file-level unsafe allowance",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`2f705ab`",
        "migrated `src/raw/posix.rs`, `src/lib.rs`,\n`src/ffi/libc.rs`, `src/allocator/threadlocal.rs`, and\n`src/allocator/heap.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "18 entries: 15 production and\n  3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 18",
        "The completed pass moved `src/raw/posix.rs`, `src/lib.rs`,\n  `src/ffi/libc.rs`, `src/allocator/threadlocal.rs`, and\n  `src/allocator/heap.rs`",
        "The current pass moved `src/platform/macos.rs` to 83",
        "`SAFETY[AUDIT:macos-zone-override]` markers for malloc-zone callback",
        "batch pointer-array reads/writes, fork-hook exports",
        "The current pass also moved `src/platform/mod.rs` to 60",
        "`SAFETY[AUDIT:platform-tls-primitives]` markers for platform TLS",
        "allocator init/done compatibility shims",
        "The current pass also moved `src/internal/atomic.rs` to 103",
        "`SAFETY[AUDIT:internal-atomic-facade]` markers for pthread mutex",
        "fallback spin locks, and Linux lock init/done behavior",
        "The current pass also moved `src/allocator/page_map.rs` to 153",
        "`SAFETY[AUDIT:page-map-boundary]` markers for page-map globals",
        "safe arbitrary-pointer lookup, and heap-region forwarding",
        "`src/types/scalar.rs` at 1,097 lines with 33 `unsafe` hits",
        "`src/allocator/thread_heap.rs` at 1,267 lines with 120 `unsafe` hits",
        "`src/platform/unix.rs` at 1,371 lines with 143 `unsafe` hits",
        "`src/platform/os.rs` at 1,614 lines with 200 `unsafe` hits",
        "`src/types/ptr.rs` at 1,617 lines with 362 `unsafe` hits",
        "`src/allocator/free.rs` at 1,621 lines with 121 `unsafe` hits",
        "`src/allocator/options.rs` at 1,715 lines with 78 `unsafe` hits",
        "`src/allocator/aligned.rs` at 1,766 lines with 215 `unsafe` hits",
        "`src/allocator/stats.rs` at 2,021 lines with 196 `unsafe` hits",
        "`src/raw/macros.rs` at 2,223 lines with 381 `unsafe` hits",
        "`strict-type-allowlist.txt` has 280 entries",
        "including 75 path-wide\n  `symbol = *` entries",
        "150 raw-pointer-parameter\n  entries, 105 bare-domain `usize` entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "`src/allocator/aligned.rs`, `src/allocator/stats.rs`,",
        "`src/allocator/thread_heap.rs`, `src/internal/mod.rs`,",
        "`src/platform/unix.rs`, and `src/platform/windows.rs`, each with\n  4 path-wide entries",
        "SAFETY[AUDIT:page-map-boundary]` has 153 markers",
        "SAFETY[AUDIT:internal-atomic-facade]` has 103 markers",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers",
        "SAFETY[AUDIT:macos-zone-override]` has 83 markers",
        "SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers",
        "SAFETY[AUDIT:raw-posix-compat]` has 70 markers",
        "SAFETY[AUDIT:platform-tls-primitives]` has 60 markers",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers",
        "SAFETY[AUDIT:typed-atomic-domain]` has 51 markers",
        "SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers",
        "SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,416 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit.rs` is 1,895 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,108 lines",
        "SAFETY[AUDIT:macos-zone-override]",
        "SAFETY[AUDIT:platform-tls-primitives]",
        "SAFETY[AUDIT:internal-atomic-facade]",
        "SAFETY[AUDIT:page-map-boundary]",
        "SAFETY[AUDIT:scalar-domain-wrappers]",
        "Migrate `src/platform/macos.rs` off the unsafe migration allowlist",
        "Migrate `src/platform/mod.rs` off the unsafe migration allowlist",
        "Migrate `src/internal/atomic.rs` off the unsafe migration allowlist",
        "Migrate `src/allocator/page_map.rs` off the unsafe migration allowlist",
        "Migrate `src/types/scalar.rs` off the unsafe migration allowlist",
        "Platform TLS/zone behavior, atomic ordering, page-map lookup, scalar",
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
