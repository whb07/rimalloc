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
        "## Issue: Raw POSIX compatibility facade still uses a file-level unsafe allowance",
        "## Issue: Crate root still uses a file-level unsafe allowance",
        "## Issue: libc string and formatting helpers still use a file-level unsafe allowance",
        "## Issue: Dynamic thread-local key manager still uses a file-level unsafe allowance",
        "## Issue: Heap lifecycle helpers still use a file-level unsafe allowance",
        "## Definition of done for this backlog",
    ];

    for heading in required_headings {
        assert!(
            todo.contains(heading),
            "TODO.md should keep backlog heading `{heading}`"
        );
    }

    let required_evidence = [
        "`7d64cc0`",
        "migrated `src/ffi/abi.rs`,\n`src/allocator/arena_meta.rs`, `src/platform/wasi.rs`,\n`src/platform/emscripten.rs`, and `src/types/atomic.rs`",
        "docs/unsafe-migration-allowlist.tsv",
        "22 entries: 19 production and\n  3 translation files",
        "`ALLOWLIST_ENTRY_CAP` is 22",
        "The completed pass moved `src/ffi/abi.rs`, `src/allocator/arena_meta.rs`,\n  `src/platform/wasi.rs`, `src/platform/emscripten.rs`, and\n  `src/types/atomic.rs`",
        "`src/raw/posix.rs` now uses 70 `SAFETY[AUDIT:raw-posix-compat]` markers",
        "POSIX out-pointer writes",
        "`reallocarr` pointer-slot updates",
        "`src/lib.rs` now uses 13 crate-root `SAFETY[AUDIT:global-alloc]` and",
        "the personality export, and is off the unsafe migration allowlist",
        "`src/ffi/libc.rs` now uses 50 `SAFETY[AUDIT:ffi-libc-helpers]` markers",
        "number-output reversal, environment forwarding, and",
        "`src/allocator/threadlocal.rs` now uses 50",
        "versioned key claim/free, and helper",
        "`src/allocator/heap.rs` now uses 80",
        "under-utilized page\n  inspection, and lock-protected teardown",
        "`src/platform/macos.rs` at 630 lines with 97 `unsafe` hits",
        "`src/platform/mod.rs` at 855 lines with 75 `unsafe` hits",
        "`src/internal/atomic.rs` at 914 lines with 135 `unsafe` hits",
        "`src/allocator/page_map.rs` at 1,052 lines with 184 `unsafe` hits",
        "`src/types/scalar.rs` at 1,097 lines with 33 `unsafe` hits",
        "`strict-type-allowlist.txt` still has 75 path-wide entries",
        "including 29 path-wide raw-pointer entries, 21 path-wide bare-domain `usize`",
        "13 path-wide pointer/usize round-trip entries",
        "`REMOVE_ENTRY_CAP` remains 0",
        "`src/allocator/page.rs` has 54 entries",
        "`src/internal/bitmap.rs` has 33 entries",
        "`src/allocator/arena.rs` has 24 entries",
        "`src/internal/mod.rs` has 19 entries",
        "`src/allocator/aligned.rs` has 19 entries",
        "SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers",
        "SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers",
        "SAFETY[AUDIT:raw-posix-compat]` has 70 markers",
        "SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers",
        "SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers",
        "SAFETY[AUDIT:typed-atomic-domain]` has 51 markers",
        "SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers",
        "SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers",
        "SAFETY[AUDIT:emscripten-primitive-boundary]` has 48 markers",
        "`src/allocator/arena.rs` is 3,915 lines",
        "`src/allocator/page.rs` is 2,801 lines",
        "`src/allocator/alloc.rs` is 2,559 lines",
        "`src/internal/bitmap.rs` is 2,358 lines",
        "`src/allocator/init.rs` is 2,339 lines",
        "`src/raw/macros.rs` is 2,223 lines",
        "`src/platform/windows.rs` is 2,191 lines",
        "`tests/release_fast_path_audit/unsafe_performance.rs` is 2,386 lines",
        "`tests/release_fast_path_audit/release_policy.rs` is 2,126 lines",
        "`tests/release_fast_path_audit.rs` is 1,869 lines",
        "`tests/release_fast_path_audit/datatype_acceptance.rs` is 1,108 lines",
        "SAFETY[AUDIT:raw-posix-compat]",
        "SAFETY[AUDIT:global-alloc]",
        "SAFETY[AUDIT:panic-boundary]",
        "SAFETY[AUDIT:ffi-libc-helpers]",
        "SAFETY[AUDIT:threadlocal-key-manager]",
        "SAFETY[AUDIT:heap-lifecycle-boundary]",
        "Migrate `src/raw/posix.rs` off the unsafe migration allowlist",
        "Migrate `src/lib.rs` off the unsafe migration allowlist",
        "Migrate `src/ffi/libc.rs` off the unsafe migration allowlist",
        "Migrate `src/allocator/threadlocal.rs` off the unsafe migration",
        "Migrate `src/allocator/heap.rs` off the unsafe migration allowlist",
        "POSIX/libc compatibility behavior, crate-root public API shape,",
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
