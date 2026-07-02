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
        "## Issue: Raw layout callback typedefs still use a file-level unsafe allowance",
        "## Issue: Thread-heap lifecycle still uses a file-level unsafe allowance",
        "## Issue: Unix primitive backend still uses a file-level unsafe allowance",
        "## Issue: OS memory boundary still uses a file-level unsafe allowance",
        "## Issue: Typed pointer accessors still use a file-level unsafe allowance",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`9d63307`",
        "migrated `src/platform/macos.rs`,\n`src/platform/mod.rs`, `src/internal/atomic.rs`,\n`src/allocator/page_map.rs`, and `src/types/scalar.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "12 entries: 10 production and\n  2 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 12",
        "The completed pass moved `src/platform/macos.rs`, `src/platform/mod.rs`,\n  `src/internal/atomic.rs`, `src/allocator/page_map.rs`, and\n  `src/types/scalar.rs`",
        "The current pass moved `src/types/constants.rs` to 8",
        "`SAFETY[AUDIT:raw-layout-abi-typedefs]` markers for raw callback typedef",
        "retaining 2 `SAFETY[AUDIT:raw-auto-trait]` markers",
        "The current pass moved `src/allocator/thread_heap.rs` to 71",
        "`SAFETY[AUDIT:thread-heap-lifecycle]` markers for visitor callback",
        "The current pass moved `src/platform/unix.rs` to 90",
        "`SAFETY[AUDIT:unix-primitive-backend]` markers for libc/syscall imports",
        "The current pass moved `src/platform/os.rs` to 136",
        "`SAFETY[AUDIT:os-memory-boundary]` markers for memory config",
        "The current pass moved `src/types/ptr.rs` to 161",
        "`SAFETY[AUDIT:typed-pointer-accessors]` markers for pointer wrapper",
        "`src/allocator/free.rs` at 1,621 lines with 121 `unsafe` hits",
        "`src/allocator/options.rs` at 1,715 lines with 79 `unsafe` hits",
        "`src/allocator/aligned.rs` at 1,766 lines with 215 `unsafe` hits",
        "`src/allocator/stats.rs` at 2,021 lines with 198 `unsafe` hits",
        "`src/raw/macros.rs` at 2,223 lines with 383 `unsafe` hits",
        "`src/allocator/init.rs` at 2,339 lines with 333 `unsafe` hits",
        "`src/internal/bitmap.rs` at 2,358 lines with 141 `unsafe` hits",
        "`src/allocator/alloc.rs` at 2,559 lines with 333 `unsafe` hits",
        "`src/allocator/page.rs` at 2,801 lines with 196 `unsafe` hits",
        "`src/allocator/arena.rs` at 3,915 lines with 607 `unsafe` hits",
        "`strict-type-allowlist.txt` has 280 entries",
        "including 75 path-wide\n  `symbol = *` entries",
        "150 raw-pointer-parameter\n  entries, 105 bare-domain `usize` entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "`src/allocator/aligned.rs`, `src/allocator/stats.rs`,",
        "`src/allocator/thread_heap.rs`, `src/internal/mod.rs`,",
        "`src/platform/unix.rs`, and `src/platform/windows.rs`, each with\n  4 path-wide entries",
        "SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers",
        "SAFETY[AUDIT:page-map-boundary]` has 153 markers",
        "SAFETY[AUDIT:os-memory-boundary]` has 136 markers",
        "SAFETY[AUDIT:internal-atomic-facade]` has 103 markers",
        "SAFETY[AUDIT:unix-primitive-backend]` has 90 markers",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers",
        "SAFETY[AUDIT:macos-zone-override]` has 83 markers",
        "SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers",
        "SAFETY[AUDIT:thread-heap-lifecycle]` has 71 markers",
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
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,433 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,146 lines",
        "`tests/release_fast_path_audit.rs` is 1,899 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,131 lines",
        "SAFETY[AUDIT:raw-layout-abi-typedefs]",
        "SAFETY[AUDIT:thread-heap-lifecycle]",
        "SAFETY[AUDIT:unix-primitive-backend]",
        "SAFETY[AUDIT:os-memory-boundary]",
        "SAFETY[AUDIT:typed-pointer-accessors]",
        "1. [x] Migrate `src/types/constants.rs` off the unsafe migration allowlist",
        "2. [x] Migrate `src/allocator/thread_heap.rs` off the unsafe migration",
        "3. [x] Migrate `src/platform/unix.rs` off the unsafe migration allowlist",
        "4. [x] Migrate `src/platform/os.rs` off the unsafe migration allowlist",
        "5. [x] Migrate `src/types/ptr.rs` off the unsafe migration allowlist",
        "Raw callback typedefs, thread-heap lifecycle behavior, Unix primitive",
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
