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
        "## Issue: FFI ABI declarations still use a file-level unsafe allowance",
        "## Issue: Arena metadata allocator still uses a file-level unsafe allowance",
        "## Issue: WASI primitive backend still uses a file-level unsafe allowance",
        "## Issue: Emscripten primitive backend still uses a file-level unsafe allowance",
        "## Issue: Atomic typed wrappers still use a file-level unsafe allowance",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`10995d2`",
        "migrated MemId views, layout requests, page-state\nwitnesses, and benchmark diagnostics",
        "docs/unsafe-migration-allowlist.tsv",
        "30 entries: 27 production and\n  3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 30",
        "The completed pass moved `src/types/memid.rs`, `src/types/layout.rs`,\n  `src/types/page_states.rs`, and `src/allocator/bench_diagnostics.rs`",
        "`src/allocator/arena_meta.rs` now uses 16",
        "`SAFETY[AUDIT:arena-meta-allocator]` markers for meta memid construction",
        "bitmap allocation/free, arena fallback, and meta-page debug classification",
        "`src/raw/posix.rs` at 329 lines with 81 `unsafe` hits",
        "`src/platform/wasi.rs` at 373 lines with 34 `unsafe` hits",
        "`src/lib.rs` at 398 lines with 23 `unsafe` hits and 8 local markers",
        "`src/platform/emscripten.rs` at 424 lines with 35 `unsafe` hits",
        "`src/ffi/libc.rs` at 518 lines with 46 `unsafe` hits",
        "`src/ffi/abi.rs` now uses 1",
        "`SAFETY[AUDIT:ffi-abi-declarations]` marker for its raw C declaration block",
        "preserves the existing C-header macro markers",
        "`src/types/atomic.rs` remains the next typed-wrapper domain at 755 lines",
        "101 `unsafe` hits and 0 local markers",
        "`strict-type-allowlist.txt` still has 75 path-wide entries",
        "including 29 path-wide raw-pointer entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "`src/allocator/page.rs` has 54 entries",
        "`src/internal/bitmap.rs` has 33 entries",
        "`src/allocator/arena.rs` has 24 entries",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers",
        "SAFETY[AUDIT:typed-page-state-domain]` has 36 markers",
        "SAFETY[AUDIT:arena-debug-render]` has 34 markers",
        "SAFETY[AUDIT:owned-allocation]` has 31 markers",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,384 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit.rs` is 1,832 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,103 lines",
        "SAFETY[AUDIT:arena-meta-allocator]",
        "SAFETY[AUDIT:wasi-primitive-boundary]",
        "SAFETY[AUDIT:emscripten-primitive-boundary]",
        "SAFETY[AUDIT:typed-atomic-domain]",
        "Migrate `src/ffi/abi.rs` off the unsafe migration allowlist",
        "Migrate `src/allocator/arena_meta.rs` off the unsafe migration allowlist",
        "Migrate `src/platform/wasi.rs` off the unsafe migration allowlist",
        "Migrate `src/platform/emscripten.rs` off the unsafe migration allowlist",
        "Migrate `src/types/atomic.rs` off the unsafe migration allowlist",
        "C ABI declaration shape, platform fallback behavior, atomic ordering, and",
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
