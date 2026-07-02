use super::*;

#[path = "release_policy/security.rs"]
mod security;

#[test]
fn release_candidates_require_full_benchmark_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let check_bench_full = fs::read_to_string(root.join("scripts/check-bench-full.sh"))
        .expect("read full bench script");

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n   `scripts/env-info.sh`",
        "target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv",
        "target/bench-full/interpose/criterion/benchmark-thresholds-full.csv",
        "artifact modes, Rust/C ratios, and noisy-benchmark notes",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require full benchmark evidence: {required}"
        );
    }

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n  `scripts/env-info.sh`",
        "target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv",
        "target/bench-full/interpose/criterion/benchmark-thresholds-full.csv",
        "Rust-vs-C ratios come from the native scenario\nbenchmarks in the default Rust library mode",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should define release candidate benchmark evidence: {required}"
        );
    }

    for required in [
        "CARGO_TARGET_DIR=\"$default_target_dir\"",
        "CARGO_TARGET_DIR=\"$interpose_target_dir\"",
        "--csv-out \"$default_target_dir/criterion/benchmark-thresholds-full.csv\"",
        "--csv-out \"$interpose_target_dir/criterion/benchmark-thresholds-full.csv\"",
        "cargo bench --features \"bench interpose\" --bench hotpath",
        "--allow-missing",
    ] {
        assert!(
            check_bench_full.contains(required),
            "full benchmark script should keep release artifact modes separate: {required}"
        );
    }

    for required in [
        "## Release Candidate Artifact-Mode Evidence",
        "default Rust library",
        "interpose hot path",
        "Do not reuse Criterion baselines across artifact modes.",
        "Rust-vs-C ratios emitted by `scripts/summarize-criterion.py`",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report template should identify artifact mode evidence: {required}"
        );
    }
}

#[test]
fn strict_wrapper_hot_path_changes_require_assembly_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let strict_types_md =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read docs/strict-types.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let capture_release_assembly =
        fs::read_to_string(root.join("scripts/capture-release-assembly.sh"))
            .expect("read capture-release-assembly.sh");

    for required in [
        "New strict internal wrappers on allocator hot paths require release assembly",
        "Run `scripts/capture-release-assembly.sh`",
        "free-list pop, local free, remote free, small\n  page lookup, and any changed hot path",
        "no added hot-path branch, call, load, store,\n  atomic memory operation, lock prefix, stack spill, formatting, allocation,\n  panic path, bounds-check slow path, or wrapper-constructor traffic",
        "rejects\n  dynamic dispatch in allocator, internal, platform, and strict-type modules",
    ] {
        assert!(
            strict_types_md.contains(required),
            "strict-type policy should gate new hot-path wrappers with assembly evidence: {required}"
        );
    }

    for required in [
        "A new strict internal wrapper changes release assembly for free-list pop",
        "local free, remote free, small page lookup",
        "adding a branch, call, memory operation, lock prefix, stack spill, formatting",
        "panic path, bounds-check slow path, or wrapper-constructor traffic",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should treat unproven strict-wrapper assembly changes as regressions: {required}"
        );
    }

    for required in [
        "For new strict internal wrappers on allocator hot paths",
        "`scripts/capture-release-assembly.sh` output covering free-list pop, local\n   free, remote free, small page lookup, and any changed hot path",
        "fail signoff\n   unless the evidence shows no added branch, call, memory traffic, stack spill",
        "panic path, bounds-check slow path, or wrapper\n   constructor traffic without measured justification",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should fail unproven strict-wrapper hot-path changes: {required}"
        );
    }

    for required in [
        "## Strict-Type Assembly Comparison Checklist",
        "Use `scripts/capture-release-assembly.sh` before and after each strict-type\nhot-path migration.",
        "`mi_malloc`",
        "`mi_malloc_small`",
        "`mi_free`",
        "`_mi_theap_malloc_zero_ex`",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report should preserve strict-wrapper assembly evidence guidance: {required}"
        );
    }

    for symbol in [
        "capture_symbol mi_malloc",
        "capture_symbol mi_malloc_small",
        "capture_symbol mi_free",
        "capture_symbol _mi_theap_malloc_zero_ex",
    ] {
        assert!(
            capture_release_assembly.contains(symbol),
            "assembly capture script should include strict-wrapper hot-path symbol: {symbol}"
        );
    }
}

#[test]
fn release_notes_publish_per_release_performance_summaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let changelog_md = fs::read_to_string(root.join("CHANGELOG.md")).expect("read CHANGELOG.md");

    for required in [
        "## Per-Release Performance Summary Template",
        "native scenario geomean Rust/C median ratio",
        "hot-path deltas against previous release evidence",
        "release-blocking regressions, if any",
        "noisy watch items separated from release-blocking regressions",
        "links to raw Criterion summaries and threshold CSV artifacts",
        "Release notes must link to the matching `BENCHMARK_RESULTS.md` section",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report should define per-release performance summary fields: {required}"
        );
    }

    for required in [
        "Publish a per-release performance summary in `BENCHMARK_RESULTS.md`",
        "Include geomean Rust/C ratios, hot-path deltas against the\n  previous release evidence",
        "release-blocking regressions, noisy watch items",
        "links to raw Criterion summaries or threshold CSV artifacts",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should require per-release summaries: {required}"
        );
    }

    for required in [
        "Publish the per-release performance summary in `BENCHMARK_RESULTS.md`",
        "summarizing geomean Rust/C ratios, hot-path deltas against\n   previous release evidence",
        "noisy watch items, release-blocking regressions",
        "links to raw Criterion summaries or threshold CSV artifacts",
        "link the\n   release notes to `BENCHMARK_RESULTS.md` or the attached performance report",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require linked performance summaries: {required}"
        );
    }

    for required in [
        "Release notes must link to `BENCHMARK_RESULTS.md`",
        "geomean Rust/C ratios, hot-path deltas, noisy watch\n  items",
        "release-blocking regressions, and raw Criterion or threshold CSV\n  artifacts",
    ] {
        assert!(
            changelog_md.contains(required),
            "release notes should carry performance-summary link policy: {required}"
        );
    }
}

#[test]
fn rust_safety_model_page_covers_safe_and_raw_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let safety_model =
        fs::read_to_string(root.join("docs/rust-safety-model.md")).expect("read safety model");

    assert!(
        readme.contains("docs/rust-safety-model.md") && readme.contains("Rust safety model"),
        "README should link to the dedicated Rust safety model page"
    );

    for required in [
        "# Rust Safety Model",
        "`Allocation` and `HeapAllocation<'heap>` own byte allocations",
        "heap allocations borrow the heap witness",
        "Uninitialized memory is never exposed as `&[u8]` or `&mut [u8]`.",
        "`unsafe_callbacks::ErrorCallbackGuard`",
        "`unsafe_callbacks::DeferredFreeCallbackGuard` are Tier U unsafe Rust APIs",
        "Callers must\nserialize registration, restoration, and callback entry",
        "The raw Rust facade mirrors the C API.",
        "C ABI exports and allocator hot paths parse raw inputs at narrow boundaries",
        "`SAFETY[AUDIT:*]` tags",
        "Allocator hot paths are guarded by no-allocation evidence.",
        "Strict internal types live under `src/types/` and are crate-private.",
        "Hot wrappers\nuse transparent representation, inline constructors/accessors, and layout/niche\ntests",
    ] {
        assert!(
            safety_model.contains(required),
            "Rust safety model should document required boundary: {required}"
        );
    }
}

#[test]
fn architecture_map_covers_public_layers_and_policy_gates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let architecture =
        fs::read_to_string(root.join("ARCHITECTURE.md")).expect("read ARCHITECTURE.md");

    for required in [
        "# Architecture",
        "Tier S, safe Rust facade: `src/safe/`",
        "Tier U, unsafe Rust conveniences: `src/ffi/callbacks.rs`",
        "Tier R, raw Rust facade: `src/raw/`",
        "Tier C, exported C ABI: exported symbols implemented through `src/ffi/`",
        "Tier I, internal implementation: `src/allocator/`, `src/types/`,",
        "`src/internal/`, and `src/platform/`",
        "Build glue: `build.rs` owns C ABI and interposition build setup",
        "Policy gates: `scripts/` and source-shape tests enforce API inventory",
        "`scripts/check-layer-boundaries.py`",
    ] {
        assert!(
            architecture.contains(required),
            "ARCHITECTURE.md should cover {required}"
        );
    }
}

#[test]
fn core_types_remains_crate_private_not_public_raw_api() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    assert!(
        lib_rs.contains("pub(crate) use types as core_types;")
            && !lib_rs.contains("pub mod core_types;")
            && !lib_rs.contains("pub use core_types")
            && !lib_rs.contains("pub use crate::core_types"),
        "core_types should remain crate-private at the crate root"
    );

    assert!(
        !raw_rs.contains("mod core_types")
            && !raw_rs.contains("pub mod core_types")
            && !raw_rs.contains("pub use core_types")
            && !raw_rs.contains("pub use crate::core_types")
            && !raw_rs.contains("crate::core_types"),
        "default raw modules should not expose core_types or strict wrapper routes"
    );
}

#[test]
fn abi_module_owns_c_header_declarations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let mimalloc_rs =
        fs::read_to_string(root.join("src/ffi/mimalloc.rs")).expect("read src/ffi/mimalloc.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    assert!(
        abi_rs.contains("Mechanical translation of `include/mimalloc.h`")
            && abi_rs.contains("unsafe extern \"C\"")
            && abi_rs.contains("pub fn mi_malloc(size: usize) -> *mut c_void;")
            && abi_rs.contains("pub fn mi_heap_new() -> *mut mi_heap_t;"),
        "src/ffi/abi.rs should own the raw mimalloc C header declarations"
    );
    assert!(
        mimalloc_rs.contains("pub(crate) use crate::abi::*;")
            && !mimalloc_rs.contains("unsafe extern \"C\""),
        "src/ffi/mimalloc.rs should stay an internal compatibility alias, not the public header mirror"
    );
    assert!(
        raw_rs.contains("pub mod abi") && raw_rs.contains("pub use crate::abi::*;"),
        "raw facade should expose ABI declarations through the ABI owner"
    );
}

#[test]
fn abi_c_declarations_remain_raw_not_strict_typed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let declarations = source_item("src/ffi/abi.rs", &abi_rs, "unsafe extern \"C\"");

    for forbidden in [
        "crate::core_types",
        "core_types::",
        "NonNull",
        "HeapPtr",
        "ThreadHeapLocal",
        "BorrowedHeap",
        "LayoutRequest",
        "RequestedSize",
        "PaddedSize",
        "UsableSize",
        "BlockSize",
        "ArenaPtr",
        "SliceCount",
        "SliceIndex",
        "ThreadSeq",
        "OsMemId",
        "OsMemPtr",
        "CommittedBytes",
        "ReservedBytes",
        "ValidatedFree",
        "LocalPage",
        "ForeignPage",
        "FullPage",
        "NormalPageQueue",
        "FullPageQueue",
        "MemIdView",
        "MemoryOrder",
    ] {
        assert!(
            !declarations.contains(forbidden),
            "src/ffi/abi.rs C declarations should not expose strict type `{forbidden}`"
        );
    }

    assert!(
        declarations.contains("pub fn mi_malloc(size: usize) -> *mut c_void;")
            && declarations.contains("pub fn mi_free(p: *mut c_void);")
            && declarations.contains("pub fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void;")
            && declarations.contains("pub fn mi_heap_new() -> *mut mi_heap_t;")
            && declarations.contains("pub fn mi_heap_visit_blocks(\n        heap: *mut mi_heap_t,\n        visit_blocks: bool,\n        visitor: Option<mi_block_visit_fun>,\n        arg: *mut c_void,\n    ) -> bool;")
            && declarations.contains("pub fn mi_process_info(\n        elapsed_msecs: *mut usize,"),
        "src/ffi/abi.rs declarations should keep the C header raw pointer shape"
    );
}

#[test]
fn translated_raw_layouts_stay_in_types_substrate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    for layout in [
        "pub struct mi_page_queue_t",
        "pub struct mi_block_s",
        "pub struct mi_heap_area_t",
        "pub struct mi_page_s",
        "pub struct mi_heap_s",
        "pub struct mi_theap_s",
        "pub struct mi_arena_s",
        "pub struct mi_subproc_s",
        "pub struct mi_arena_pages_s",
        "pub struct mi_tld_s",
    ] {
        assert!(
            types_rs.contains("#[repr(C)]") && types_rs.contains(layout),
            "src/types/constants.rs should remain the private raw repr(C) layout substrate for {layout}"
        );
        assert!(
            !abi_rs.contains(layout) && !raw_rs.contains(layout),
            "{layout} should not be redefined in public ABI/facade modules"
        );
    }

    assert!(
        abi_rs.contains("pub use crate::types::{")
            && abi_rs.contains("mi_heap_t")
            && abi_rs.contains("mi_theap_t")
            && raw_rs.contains("pub mod abi {\n    pub use crate::abi::*;\n}")
            && !types_rs.contains("use crate::core_types")
            && !abi_rs.contains("core_types")
            && !raw_rs.contains("core_types"),
        "raw layout exposure should flow from src/types/constants.rs through raw::abi without leaking strict wrappers"
    );
}

#[test]
fn public_c_layout_exposure_uses_raw_abi_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let api_doc =
        fs::read_to_string(root.join("docs/api-inventory.md")).expect("read api inventory");

    assert!(
        abi_rs.contains("pub use crate::types::{")
            && abi_rs.contains("mi_heap_area_t")
            && abi_rs.contains("mi_heap_t")
            && abi_rs.contains("mi_subproc_id_t")
            && raw_rs.contains("pub mod abi {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub mod mimalloc {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub use mimalloc::*;"),
        "public C layouts and aliases should be exposed through raw::abi, raw::mimalloc, and root raw C-header re-exports"
    );
    assert!(
        !raw_rs.contains("pub mod types")
            && api_doc.contains("By default, `raw` exposes the root C-header mirror plus `raw::abi`,\n`raw::mimalloc`, and `raw::headers`.")
            && !lib_rs.contains("pub mod types;")
            && !lib_rs.contains("pub mod core_types;")
            && !raw_rs.contains("pub use crate::core_types")
            && !abi_rs.contains("core_types"),
        "raw::types should remain unavailable and core_types must not be a public C layout route"
    );
}

#[test]
fn raw_facade_remains_c_shaped_without_strict_type_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    assert!(
        raw_rs.contains("Explicit C-shaped compatibility facade")
            && raw_rs.contains("pub mod abi {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub mod mimalloc {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub mod headers {\n    pub use crate::headers::*;\n}")
            && raw_rs.contains("pub use mimalloc::*;"),
        "src/raw/mod.rs should keep the default facade as the C-header mirror"
    );

    for module in [
        "alloc",
        "alloc_aligned",
        "alloc_override",
        "alloc_posix",
        "arena",
        "arena_meta",
        "atomic",
        "bitmap",
        "bits",
        "free",
        "heap",
        "init",
        "internal",
        "libc",
        "memory",
        "options",
        "os",
        "page",
        "page_map",
        "prim",
        "random",
        "static_c",
        "stats",
        "theap",
        "threadlocal",
        "track",
        "types",
    ] {
        assert!(
            !raw_rs.contains(&format!("pub mod {module}"))
                && !raw_rs.contains(&format!("pub use crate::{module}::*")),
            "raw::{module} should not expose implementation modules"
        );
    }

    for forbidden in [
        "pub mod core_types",
        "pub use crate::core_types",
        "crate::core_types",
        "core_types::",
        "HeapPtr",
        "ThreadHeapLocal",
        "BorrowedHeap",
        "LayoutRequest",
        "RequestedSize",
        "PaddedSize",
        "UsableSize",
        "BlockSize",
        "ArenaPtr",
        "SliceCount",
        "SliceIndex",
        "ThreadSeq",
        "OsMemId",
        "OsMemPtr",
        "CommittedBytes",
        "ReservedBytes",
        "ValidatedFree",
        "LocalPage",
        "ForeignPage",
        "FullPage",
        "NormalPageQueue",
        "FullPageQueue",
        "MemIdView",
        "MemoryOrder",
    ] {
        assert!(
            !raw_rs.contains(forbidden),
            "src/raw/mod.rs should not expose strict type `{forbidden}`"
        );
    }
}

#[test]
fn raw_macro_unsafe_boundary_policy_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let headers_rs = fs::read_to_string(root.join("src/ffi/headers.rs")).expect("read headers.rs");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let unsafe_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit");

    assert!(
        abi_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && headers_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && abi_doc.contains("## Raw Macro Unsafe Boundary"),
        "raw macro unsafe-boundary audit tag should be mirrored in source and docs"
    );
    assert!(
        abi_doc.contains("Typed allocation macros in `src/ffi/abi.rs`")
            && abi_doc.contains("Header override macros in `src/ffi/headers.rs`")
            && abi_doc.contains("There are no intentionally safe public allocation macros today")
            && lib_rs.contains("Header override macros follow the same raw unsafe-boundary policy")
            && lib_rs.contains("mechanical_mimalloc::malloc!(64)"),
        "raw macro docs should distinguish typed/header macros and state that all current public allocation macros require caller unsafe"
    );
    assert!(
        !abi_rs.contains("unsafe { $crate::raw::")
            && !headers_rs.contains("unsafe { $crate::raw::"),
        "public raw macros should not hide raw calls inside internal unsafe blocks"
    );
}

#[test]
fn repository_tests_do_not_import_private_raw_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = root.join("tests");
    let internal_modules = [
        "alloc",
        "alloc_aligned",
        "alloc_override",
        "alloc_posix",
        "arena",
        "arena_meta",
        "atomic",
        "bitmap",
        "bits",
        "free",
        "heap",
        "init",
        "internal",
        "libc",
        "memory",
        "options",
        "os",
        "page",
        "page_map",
        "prim",
        "random",
        "static_c",
        "stats",
        "theap",
        "threadlocal",
        "track",
        "types",
    ];

    for entry in fs::read_dir(tests_dir).expect("read tests directory") {
        let path = entry.expect("read tests entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read test source");
        let uses_raw_internal = internal_modules.iter().any(|module| {
            let direct = format!("use mechanical_mimalloc::raw::{module}");
            let group_inline = format!("use mechanical_mimalloc::raw::{{{module}");
            let group_multiline = format!("use mechanical_mimalloc::raw::{{\n    {module}");
            source.contains(&direct)
                || source.contains(&group_inline)
                || source.contains(&group_multiline)
        });

        assert!(
            !uses_raw_internal,
            "{} imports removed private raw implementation modules",
            path.display()
        );
    }
}

#[test]
fn raw_pointer_nullability_policy_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let ptr_rs = fs::read_to_string(root.join("src/types/ptr.rs")).expect("read src/types/ptr.rs");
    let safe_rs = fs::read_to_string(root.join("src/safe/mod.rs")).expect("read src/safe/mod.rs");
    let safe_allocations_rs = fs::read_to_string(root.join("src/safe/allocation.rs"))
        .expect("read src/safe/allocation.rs");
    let safe_errors_rs =
        fs::read_to_string(root.join("src/safe/error.rs")).expect("read src/safe/error.rs");
    let aligned_exports =
        fs::read_to_string(root.join("tests/aligned_exports.rs")).expect("read aligned_exports");
    let compat_exports =
        fs::read_to_string(root.join("tests/compat_exports.rs")).expect("read compat_exports");
    let fuzz_alloc_sequences = fs::read_to_string(root.join("tests/fuzz_alloc_sequences.rs"))
        .expect("read fuzz_alloc_sequences");
    let parity = fs::read_to_string(root.join("scripts/check-behavioral-parity.py"))
        .expect("read check-behavioral-parity.py");

    assert!(
        abi_doc.contains("## Raw Pointer Nullability")
            && abi_doc.contains("The raw C boundary uses raw pointers in signatures.")
            && abi_doc.contains("It does not use `NonNull`\nin raw C signatures")
            && abi_doc.contains("`mi_free` and matching deallocation APIs accept null")
            && abi_doc.contains("`mi_realloc` and matching realloc-family APIs treat a null input pointer as\n  an allocation request")
            && abi_doc.contains("out-pointers require valid writable storage")
            && abi_doc.contains("metadata pointers must be valid and\n  non-null")
            && abi_doc.contains("Safe Rust APIs must not expose C null/error conventions directly"),
        "docs/abi.md should define the raw pointer nullability policy across raw, internal, and safe APIs"
    );

    assert!(
        raw_rs.contains("Public raw signatures intentionally use `*mut` and `*const` rather than\n//! `NonNull`")
            && raw_rs.contains("Allocation functions\n//! return null on failure")
            && raw_rs.contains("`mi_free` accepts null as a no-op")
            && raw_rs.contains("`mi_realloc` with a null input pointer behaves like allocation")
            && raw_rs.contains("Metadata\n//! pointers, out-pointers, and callback context pointers are valid, non-null"),
        "src/raw/mod.rs rustdoc should state raw C nullability examples and non-null caller obligations"
    );
    assert!(
        abi_rs.contains("Public declarations in this module keep the C header pointer shape.")
            && abi_rs.contains("This module does not use\n//! `NonNull` in raw C signatures.")
            && !abi_rs.contains("NonNull<"),
        "src/ffi/abi.rs should keep public C-header declarations raw-pointer based"
    );

    assert!(
        ptr_rs.contains("pub(crate) struct $name(NonNull<$raw_type>);")
            && ptr_rs.contains("NonNull::new(ptr).map(Self)")
            && ptr_rs.contains("`ptr` must be non-null and point to valid metadata or data")
            && ptr_rs.contains("pub(crate) struct PageConstPtr(NonNull<mi_page_t>)")
            && ptr_rs.contains("NonNull::new(ptr as *mut mi_page_t).map(Self)"),
        "crate-private NonNull pointer wrappers should require checked or explicitly unsafe construction"
    );

    assert!(
        safe_errors_rs.contains("Cause: mimalloc returned null for a valid Rust [`Layout`].")
            && safe_allocations_rs
                .contains("let ptr = NonNull::new(ptr).ok_or(AllocError { layout })?;")
            && safe_allocations_rs.contains("pub const fn as_non_null(&self) -> NonNull<u8>")
            && safe_allocations_rs.contains("pub const fn as_ptr(&self) -> *const u8")
            && safe_allocations_rs.contains("pub const fn as_mut_ptr(&mut self) -> *mut u8")
            && !safe_rs.contains("Option<*mut")
            && !safe_rs.contains("Option<*const")
            && !safe_rs.contains("Result<*mut")
            && !safe_rs.contains("Result<*const")
            && !safe_allocations_rs.contains("Option<*mut")
            && !safe_allocations_rs.contains("Option<*const")
            && !safe_allocations_rs.contains("Result<*mut")
            && !safe_allocations_rs.contains("Result<*const"),
        "safe facade should translate allocator null returns before exposing non-null allocation handles"
    );

    assert!(
        aligned_exports.contains("mi_realloc_aligned(ptr::null_mut(), 96, 64)")
            && aligned_exports.contains("mi_heap_realloc_aligned(heap, ptr::null_mut(), 96, 64)")
            && compat_exports.contains("mi_aligned_recalloc(ptr::null_mut(), 4, 8, 64)")
            && fuzz_alloc_sequences.contains("mi_realloc(ptr::null_mut(), program.size(2048))")
            && parity.contains("mi_free(NULL);")
            && parity.contains("result_bool(\"free_plain_null_ok\", 1);")
            && parity.contains("result_bool(\"realloc_null_allocates\""),
        "tests and parity script should cover null-accepted free and realloc-family behavior"
    );
}

#[test]
fn public_callback_abi_inventory_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let deferred_rs = fs::read_to_string(root.join("src/allocator/page/deferred.rs"))
        .expect("read src/allocator/page/deferred.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");

    for callback in [
        "mi_output_fun",
        "mi_error_fun",
        "mi_deferred_free_fun",
        "mi_heap_visit_fun",
        "mi_block_visit_fun",
        "mi_commit_fun_t",
    ] {
        assert!(
            abi_doc.contains(callback) && types_rs.contains(&format!("pub type {callback}")),
            "{callback} should be listed in docs/abi.md and declared in src/types/constants.rs"
        );
    }

    for contract_word in [
        "Nullability",
        "Lifetime",
        "Threading",
        "Reentrancy",
        "Unwind",
    ] {
        assert!(
            types_rs.matches(contract_word).count() >= 6,
            "each public callback type should document {contract_word}"
        );
    }
    assert!(
        types_rs.matches("panic").count() >= 6 && types_rs.matches("unwind").count() >= 6,
        "each public callback type should document panic/unwind behavior"
    );

    for family in [
        "Output callback",
        "Error callback",
        "Deferred free callback",
        "Heap visitor callback",
        "Block visitor callback",
        "Managed-memory commit callback",
    ] {
        assert!(
            abi_doc.contains(family),
            "docs/abi.md should list the {family} callback family"
        );
    }

    assert!(
        abi_rs.contains(
            "pub fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void);"
        ) && abi_rs
            .contains("pub fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void);")
            && abi_rs.contains(
                "pub fn mi_register_deferred_free(deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void);"
            )
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && abi_rs.contains("commit_fun: Option<mi_commit_fun_t>"),
        "src/ffi/abi.rs should expose callback-accepting C declarations with nullable Option callbacks"
    );

    for forbidden in [
        "OutputCallback",
        "ErrorCallback",
        "DeferredFreeCallback",
        "HeapVisitCallback",
        "BlockVisitCallback",
        "ThreadLocalClaimCallback",
        "CallbackArg",
        "dyn Fn",
        "impl Fn",
    ] {
        assert!(
            !types_rs.contains(forbidden) && !abi_rs.contains(forbidden),
            "raw callback typedefs and ABI declarations must not use {forbidden}"
        );
    }

    assert!(
        options_rs.contains("pub extern \"C\" fn mi_register_output")
            && options_rs.contains("pub extern \"C\" fn mi_register_error")
            && deferred_rs.contains("pub unsafe extern \"C\" fn mi_register_deferred_free")
            && arena_rs.contains("pub unsafe extern \"C\" fn mi_manage_memory")
            && abi_doc.contains("mi_claim_fun_t")
            && abi_doc.contains("mi_forall_set_fun_t")
            && abi_doc.contains("crate-private implementation modules"),
        "callback registrations and internal callback families should be inventoried at their public boundary"
    );
}

#[test]
fn callback_trampolines_remain_no_alloc_no_unwind_raw_abi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let deferred_rs = fs::read_to_string(root.join("src/allocator/page/deferred.rs"))
        .expect("read src/allocator/page/deferred.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");
    let theap_rs = fs::read_to_string(root.join("src/allocator/thread_heap.rs"))
        .expect("read src/allocator/thread_heap.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");

    for (source_name, source, marker) in [
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        force: bool,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, message: *const c_char,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, err: i32,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, heap: HeapPtr,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        heap: HeapPtr,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        slice_index: usize,",
        ),
        (
            "src/allocator/options.rs",
            options_rs.as_str(),
            "pub extern \"C\" fn mi_register_output",
        ),
        (
            "src/allocator/options.rs",
            options_rs.as_str(),
            "pub extern \"C\" fn mi_register_error",
        ),
        (
            "src/allocator/page/deferred.rs",
            deferred_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn _mi_deferred_free",
        ),
        (
            "src/allocator/page/deferred.rs",
            deferred_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_register_deferred_free",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_heap_visit_blocks",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_heap_visit_abandoned_blocks",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_theap_visit_blocks",
        ),
        (
            "src/allocator/init.rs",
            init_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_subproc_visit_heaps",
        ),
    ] {
        let item = source_item(source_name, source, marker);
        assert_no_callback_alloc_or_unwind(source_name, marker, item);
    }

    assert!(
        types_rs.contains("pub type mi_output_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_error_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_deferred_free_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_heap_visit_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_block_visit_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_commit_fun_t = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_claim_fun_t =\n    unsafe extern \"C\" fn"),
        "src/types/constants.rs should keep public and unstable callback typedefs raw C ABI function pointers"
    );
    assert!(
        abi_rs.contains("out: Option<mi_output_fun>")
            && abi_rs.contains("fun: Option<mi_error_fun>")
            && abi_rs.contains("deferred_free: Option<mi_deferred_free_fun>")
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && abi_rs.contains("commit_fun: Option<mi_commit_fun_t>")
            && !types_rs.contains("CallbackArg")
            && !abi_rs.contains("CallbackArg"),
        "src/ffi/abi.rs should keep nullable raw callback typedefs at the C ABI boundary"
    );
}

#[test]
fn safe_callback_guards_document_lifetime_restore_and_unwind_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let safe_doc =
        fs::read_to_string(root.join("docs/safe-api.md")).expect("read docs/safe-api.md");
    let unsafe_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read docs/unsafe-audit.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let unsafe_callbacks_rs =
        fs::read_to_string(root.join("src/ffi/callbacks.rs")).expect("read unsafe callbacks");

    assert!(
        abi_doc.contains("Raw callback registration remains unsafe.")
            && abi_doc.contains("`unsafe_callbacks::ErrorCallbackGuard` and")
            && abi_doc.contains("restore the previous registration on drop")
            && abi_doc.contains(
                "catch panic or unwind before returning across the\ncallback C ABI boundary"
            )
            && abi_doc.contains("Nested borrowed guards must be dropped in LIFO order")
            && abi_doc.contains("Output callbacks remain raw"),
        "docs/abi.md should document the safe callback guard proof and remaining raw output callback gap"
    );
    assert!(
        safe_doc.contains("`unsafe_callbacks`")
            && safe_doc.contains("`FnMut` callback guards")
            && safe_doc.contains("Tier U unsafe Rust convenience APIs")
            && safe_doc.contains("catch Rust panics")
            && safe_doc.contains("Nested guards must be dropped in LIFO order")
            && unsafe_doc.contains("SAFETY[AUDIT:unsafe-callback]")
            && unsafe_doc.contains("restores that previous registration on drop")
            && unsafe_doc.contains("restores the process-global snapshot")
            && unsafe_doc.contains("catches Rust\n  panics before callback ABI return")
            && unsafe_doc.contains("`error_callback_trampoline::<F>`")
            && unsafe_doc.contains("`deferred_free_callback_trampoline::<F>`")
            && unsafe_doc.contains(
                "no concurrent or reentrant entry may access the same\n  borrowed callback"
            ),
        "safe API and unsafe audit docs should prove safe callback guard lifetime, restore, and unwind behavior"
    );

    assert!(
        lib_rs.contains("pub use ffi::callbacks as unsafe_callbacks;")
            && unsafe_callbacks_rs.contains("std::panic::catch_unwind")
            && unsafe_callbacks_rs.contains("mi_register_error")
            && unsafe_callbacks_rs.contains("mi_register_deferred_free")
            && !unsafe_callbacks_rs.contains("mi_register_output"),
        "unsafe callback namespace should expose only callback guards that catch panics and restore previous registrations"
    );
}

#[test]
fn public_docs_match_current_build_mode_and_layout() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let features_doc =
        fs::read_to_string(root.join("docs/features.md")).expect("read docs/features.md");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let api_inventory_doc =
        fs::read_to_string(root.join("docs/api-inventory.md")).expect("read docs/api-inventory.md");
    let compatibility_doc =
        fs::read_to_string(root.join("docs/compatibility.md")).expect("read docs/compatibility.md");
    let limitations_doc =
        fs::read_to_string(root.join("docs/limitations.md")).expect("read docs/limitations.md");
    let strict_types_doc =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read docs/strict-types.md");
    let memory_ordering_doc = fs::read_to_string(root.join("docs/memory-ordering.md"))
        .expect("read docs/memory-ordering.md");
    let unsafe_audit_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read docs/unsafe-audit.md");
    let assembly_doc = fs::read_to_string(root.join("docs/assembly-intrinsics.md"))
        .expect("read docs/assembly-intrinsics.md");
    let final_api_doc = fs::read_to_string(root.join("docs/final-api-surface-review.md"))
        .expect("read docs/final-api-surface-review.md");
    let raw_migration_doc = fs::read_to_string(root.join("docs/raw-facade-migration.md"))
        .expect("read docs/raw-facade-migration.md");
    let mimalloc_bench_rs =
        fs::read_to_string(root.join("benches/mimalloc_bench.rs")).expect("read mimalloc_bench.rs");

    for source in [&readme, &lib_rs, &abi_doc, &api_inventory_doc] {
        assert!(
            source.contains(
                "cargo rustc --features interpose --lib --release -- --crate-type cdylib"
            ),
            "public build docs should show the explicit interpose cdylib command"
        );
    }
    assert!(
        readme.contains("The default library build produces")
            && readme.contains("an `rlib` for Rust integration")
            && lib_rs.contains("The default library build produces")
            && lib_rs.contains("an `rlib` with a deliberately small Rust-facing surface")
            && abi_doc.contains("Default Cargo library builds produce an `rlib`"),
        "public docs should describe the rlib-first default build mode"
    );
    for required in [
        "## Choose a Path",
        "### 1. Rust Global Allocator",
        "### 2. Safe Rust Owned Bytes, Heaps, Stats, and Options",
        "### 3. C ABI Artifact and Interposition",
        "### Unsafe Raw Facade",
        "Use `MiMalloc` when you want Rust standard-library allocations",
        "Use the safe facade when you want Rust-native ownership and typed errors",
        "Use the opt-in C ABI artifact when you need mimalloc-compatible dynamic symbols",
        "Use `mechanical_mimalloc::raw` only when you can uphold mimalloc's pointer",
        "The raw Rust path surface is not stabilized as a Rust SemVer API.",
    ] {
        assert!(
            readme.contains(required),
            "README should be organized around the intended user stories: {required}"
        );
    }
    let global_allocator = readme.find("### 1. Rust Global Allocator").unwrap();
    let safe_facade = readme
        .find("### 2. Safe Rust Owned Bytes, Heaps, Stats, and Options")
        .unwrap();
    let c_abi = readme
        .find("### 3. C ABI Artifact and Interposition")
        .unwrap();
    let raw_facade = readme.find("### Unsafe Raw Facade").unwrap();
    assert!(
        global_allocator < safe_facade && safe_facade < c_abi && c_abi < raw_facade,
        "README should introduce safe Rust and C ABI user stories before the unsafe raw facade"
    );
    assert!(
        mimalloc_bench_rs
            .contains("cargo rustc --features interpose --lib --release -- --crate-type cdylib")
            && !mimalloc_bench_rs.contains("cargo build --release for Rust mimalloc cdylib")
            && !mimalloc_bench_rs.contains("run `cargo build --release`"),
        "native benchmark helper should build the opt-in interpose cdylib, not the default rlib artifact"
    );

    for feature in [
        "`bench-diagnostics`",
        "`c-abi`",
        "`cxx-overrides`",
        "`tls-shim`",
        "`interpose`",
    ] {
        assert!(
            readme.contains(feature)
                && lib_rs.contains(feature)
                && features_doc.contains(feature)
                && compatibility_doc.contains(feature),
            "{feature} should be documented wherever public feature policy is summarized"
        );
    }

    for stale in [
        "This repository currently builds both",
        "This crate is currently a dual artifact",
        "The crate builds a `cdylib`",
        "The Tier C `cdylib` exports",
        "keeps the ABI declaration owner flat",
        "instead of creating a\nnew `src/raw/` or `src/ffi/` tree",
        "`raw.rs` must remain",
        "`core_types`:",
        "`src/core_types`",
        "`src/types.rs` and `src/types/`",
        "`src/types.rs`",
        "`src/abi.rs`",
        "`src/alloc_override.rs`",
        "`src/init.rs`, `src/prim.rs`",
        "`src/atomic.rs`",
        "`src/atomic.rs` remains",
    ] {
        assert!(
            !readme.contains(stale)
                && !lib_rs.contains(stale)
                && !features_doc.contains(stale)
                && !abi_doc.contains(stale)
                && !api_inventory_doc.contains(stale)
                && !compatibility_doc.contains(stale)
                && !limitations_doc.contains(stale)
                && !strict_types_doc.contains(stale)
                && !memory_ordering_doc.contains(stale)
                && !unsafe_audit_doc.contains(stale)
                && !assembly_doc.contains(stale)
                && !final_api_doc.contains(stale)
                && !raw_migration_doc.contains(stale),
            "public docs should not retain stale build-mode or pre-layout wording: {stale}"
        );
    }
}

#[test]
fn feature_no_std_validation_covers_std_examples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let check_features =
        fs::read_to_string(root.join("scripts/check-features.sh")).expect("read check-features");
    let check_docs =
        fs::read_to_string(root.join("scripts/check-docs.sh")).expect("read check-docs");
    let features_doc =
        fs::read_to_string(root.join("docs/features.md")).expect("read docs/features.md");
    let safe_api_doc =
        fs::read_to_string(root.join("docs/safe-api.md")).expect("read docs/safe-api.md");
    let release_doc =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");

    let example_names = fs::read_dir(root.join("examples"))
        .expect("read examples")
        .map(|entry| entry.expect("read example entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            path.file_stem()
                .expect("example has file stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();

    for name in &example_names {
        let expected_stanza = format!(
            "[[example]]\nname = \"{name}\"\npath = \"examples/{name}.rs\"\nrequired-features = [\"std\"]"
        );
        assert!(
            manifest.contains(&expected_stanza),
            "Rust example {name} should be explicitly gated as std-only"
        );
    }
    assert!(
        !example_names.is_empty(),
        "feature audit should find Rust examples to classify"
    );

    for required in [
        "cargo check --lib --no-default-features",
        "cargo check --examples --features std",
    ] {
        assert!(
            check_features.contains(required) && features_doc.contains(required),
            "feature validation should include {required}"
        );
    }
    assert!(
        check_docs.contains("cargo doc --no-deps --no-default-features")
            && features_doc.contains("cargo doc --no-deps --no-default-features"),
        "docs validation should include no-default rustdoc generation"
    );
    assert!(
        features_doc.contains("Repository examples are `std`-only executable targets")
            && features_doc.contains("required-features = [\"std\"]")
            && safe_api_doc.contains("Executable repository examples")
            && safe_api_doc.contains("gated with `required-features = [\"std\"]`")
            && safe_api_doc.contains("process-global allocator state")
            && release_doc.contains("Confirm feature/no-std coverage includes")
            && release_doc.contains("cargo check --lib\n   --no-default-features")
            && release_doc.contains("cargo doc --no-deps --no-default-features")
            && release_doc.contains("cargo check --examples --features std"),
        "feature docs, safe API policy, and release checklist should describe no-std and std-example coverage"
    );
}

#[test]
fn ffi_linking_guide_matches_release_artifact_modes_and_consumers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let guide =
        fs::read_to_string(root.join("docs/ffi-linking.md")).expect("read docs/ffi-linking.md");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read scripts/build-release-artifacts.py");
    let artifact_consumer =
        fs::read_to_string(root.join("scripts/check-release-artifact-consumer.py"))
            .expect("read scripts/check-release-artifact-consumer.py");
    let link_metadata = fs::read_to_string(root.join("scripts/check-release-link-metadata.py"))
        .expect("read scripts/check-release-link-metadata.py");
    let dlopen = fs::read_to_string(root.join("scripts/check-release-dlopen.py"))
        .expect("read scripts/check-release-dlopen.py");
    let ld_preload = fs::read_to_string(root.join("scripts/check-release-ld-preload.py"))
        .expect("read scripts/check-release-ld-preload.py");
    let cxx_artifacts = fs::read_to_string(root.join("scripts/check-release-cxx-artifacts.py"))
        .expect("read scripts/check-release-cxx-artifacts.py");

    for heading in [
        "## Release Artifact Layout",
        "## Dynamic C ABI Linking",
        "## Static C ABI Linking",
        "## LD_PRELOAD Interposition",
        "## C++ Operator Override Modes",
        "## Headers and Link Metadata",
        "## Supported Targets and Caveats",
    ] {
        assert!(
            guide.contains(heading),
            "FFI linking guide should include {heading}"
        );
    }

    for artifact_name in [
        "rust-rlib",
        "c-abi-cdylib",
        "interpose-cdylib",
        "c-abi-static-preview",
        "interpose-static-preview",
    ] {
        assert!(
            guide.contains(artifact_name) && build_release.contains(artifact_name),
            "FFI linking guide should document staged artifact mode {artifact_name}"
        );
    }

    for command_or_flag in [
        "cargo rustc --release --features c-abi --lib -- --crate-type cdylib",
        "cargo rustc --features interpose --lib --release -- --crate-type cdylib",
        "RUSTFLAGS=\"-C lto=off -C linker-plugin-lto=no -C embed-bitcode=no\"",
        "--crate-type staticlib",
        "LD_PRELOAD=/path/to/libmechanical_mimalloc-interpose-cdylib.so ./program",
    ] {
        assert!(
            guide.contains(command_or_flag),
            "FFI linking guide should show the current artifact command or flag: {command_or_flag}"
        );
    }

    for header in [
        "mimalloc.h",
        "mimalloc-stats.h",
        "mimalloc-override.h",
        "mimalloc-new-delete.h",
    ] {
        assert!(
            guide.contains(header)
                && build_release.contains(header)
                && artifact_consumer.contains(header),
            "FFI linking guide should document staged header {header}"
        );
    }

    for metadata_target in [
        "mechanical_mimalloc.pc",
        "mechanical_mimalloc-config.cmake",
        "mechanical_mimalloc::c_abi",
        "mechanical_mimalloc::interpose",
        "mechanical_mimalloc::c_abi_static_preview",
        "mechanical_mimalloc::interpose_static_preview",
    ] {
        assert!(
            guide.contains(metadata_target)
                && (build_release.contains(metadata_target)
                    || link_metadata.contains(metadata_target)),
            "FFI linking guide should document generated link metadata target {metadata_target}"
        );
    }

    for validation_script in [
        "scripts/check-release-artifact-consumer.py",
        "scripts/check-release-link-metadata.py",
        "scripts/check-release-dlopen.py",
        "scripts/check-release-ld-preload.py",
        "scripts/check-release-cxx-artifacts.py",
    ] {
        assert!(
            guide.contains(validation_script),
            "FFI linking guide should name consumer validation script {validation_script}"
        );
    }
    assert!(
        artifact_consumer.contains("\"c-abi-cdylib\"")
            && artifact_consumer.contains("\"c-abi-static-preview\"")
            && artifact_consumer.contains("\"interpose-cdylib\"")
            && artifact_consumer.contains("\"interpose-static-preview\"")
            && dlopen.contains("\"c-abi-cdylib\"")
            && ld_preload.contains("\"interpose-cdylib\"")
            && cxx_artifacts.contains("\"interpose-cdylib\"")
            && cxx_artifacts.contains("\"interpose-static-preview\""),
        "consumer scripts should exercise every artifact mode claimed by the FFI linking guide"
    );
    assert!(
        guide.contains("Linux x86_64 GNU is the maintained production target")
            && guide.contains("Unsupported platforms must not infer a stable C\nABI")
            && guide.contains("Static archives are preview-only")
            && guide.contains("not a stable static ABI"),
        "FFI linking guide should state supported target and preview/static caveats"
    );
}

#[test]
fn symbol_versioning_policy_matches_symbol_scripts_and_manifests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy = fs::read_to_string(root.join("docs/symbol-versioning.md"))
        .expect("read docs/symbol-versioning.md");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let release_doc =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let symbol_script = fs::read_to_string(root.join("scripts/check-public-symbols.py"))
        .expect("read scripts/check-public-symbols.py");
    let abi_diff_script =
        fs::read_to_string(root.join("scripts/check-abi-diff.py")).expect("read abi diff script");
    let classification_tsv =
        fs::read_to_string(root.join("symbols/public-symbol-classification.tsv"))
            .expect("read public symbol classification");

    for heading in [
        "## Public C Header Symbols",
        "## Malloc-Family Interposition Symbols",
        "## C++ Operator Symbols",
        "## Extension ABI Naming Rules",
        "## Static Archive Internal-Symbol Policy",
        "## Release-to-Release ABI Diff Expectations",
        "## Required Checks",
    ] {
        assert!(
            policy.contains(heading),
            "symbol versioning policy should include {heading}"
        );
    }

    for header in [
        "mimalloc-3.3.2/include/mimalloc.h",
        "mimalloc-3.3.2/include/mimalloc-stats.h",
    ] {
        assert!(
            policy.contains(header) && symbol_script.contains(header),
            "symbol policy should document public header source {header}"
        );
    }

    for script_constant in [
        "MALLOC_OVERRIDE_SYMBOLS",
        "LIBC_OVERRIDE_SYMBOLS",
        "MALLOC_FAMILY_SYMBOLS",
        "CXX_OPERATOR_OVERRIDE_SYMBOLS",
    ] {
        assert!(
            policy.contains(script_constant) && symbol_script.contains(script_constant),
            "symbol policy should document script-controlled symbol group {script_constant}"
        );
    }

    for representative_symbol in [
        "malloc",
        "calloc",
        "realloc",
        "free",
        "posix_memalign",
        "__libc_malloc",
        "__posix_memalign",
        "_Znwm",
        "_Znam",
        "_ZdlPv",
        "_ZdaPv",
    ] {
        assert!(
            policy.contains(representative_symbol) && symbol_script.contains(representative_symbol),
            "symbol policy should name representative checked symbol {representative_symbol}"
        );
    }

    for classification in [
        "internal-export-required",
        "rust-extension",
        "platform-helper-extension",
    ] {
        assert!(
            policy.contains(classification) && classification_tsv.contains(classification),
            "symbol policy should define TSV classification {classification}"
        );
    }

    for manifest in [
        "c-abi-dynamic.tsv",
        "c-abi-secure-dynamic.tsv",
        "c-abi-guarded-dynamic.tsv",
        "c-abi-track-valgrind-dynamic.tsv",
        "c-abi-track-asan-dynamic.tsv",
        "interpose-dynamic.tsv",
    ] {
        assert!(
            policy.contains(manifest) && root.join("docs/abi-manifests").join(manifest).exists(),
            "symbol policy should list checked ABI manifest {manifest}"
        );
    }

    for command_or_marker in [
        "scripts/check-public-symbols.py --dynamic-mi",
        "scripts/check-public-symbols.py --dynamic-groups",
        "scripts/check-public-symbols.py --dynamic-manifests",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-abi-symbols.sh",
        "scripts/check-abi-diff.py --previous-ref <previous-tag>",
        "scripts/check-abi-diff.py --previous-dir <previous-release>/abi-manifests",
        "ABI-CHANGE: <manifest> <symbol>",
        "ABI-CHANGE: <manifest> *",
    ] {
        assert!(
            policy.contains(command_or_marker),
            "symbol policy should document release command or marker {command_or_marker}"
        );
    }
    assert!(
        abi_diff_script.contains("visibility-changed")
            && abi_diff_script.contains("binding-changed")
            && abi_diff_script.contains("version-changed")
            && abi_diff_script.contains("type-changed")
            && policy.contains("visibility-changed")
            && policy.contains("binding-changed")
            && policy.contains("version-changed")
            && policy.contains("type-changed"),
        "symbol policy should match ABI diff classifications"
    );
    assert!(
        policy.contains("mechanical_mimalloc_*")
            && policy.contains("There is no supported extension ABI in the current release policy")
            && policy.contains("Static archives are preview-only")
            && abi_doc.contains("docs/symbol-versioning.md")
            && release_doc.contains("docs/symbol-versioning.md")
            && readme.contains("docs/symbol-versioning.md"),
        "symbol policy should be linked and should define extension/static policy"
    );
}

#[test]
fn c_mimalloc_migration_guide_maps_headers_apis_callbacks_and_platforms() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let guide = fs::read_to_string(root.join("docs/c-mimalloc-migration.md"))
        .expect("read docs/c-mimalloc-migration.md");
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let release_doc =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let compatibility_doc =
        fs::read_to_string(root.join("docs/compatibility.md")).expect("read compatibility docs");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let ffi_linking =
        fs::read_to_string(root.join("docs/ffi-linking.md")).expect("read ffi linking docs");
    let safe_doc = fs::read_to_string(root.join("docs/safe-api.md")).expect("read safe API docs");
    let features_doc =
        fs::read_to_string(root.join("docs/features.md")).expect("read feature docs");

    for heading in [
        "## Header to Artifact Map",
        "## Allocation and Free APIs",
        "## Callback Migration",
        "## Options and Statistics",
        "## Unsupported Platform and Feature Differences",
    ] {
        assert!(
            guide.contains(heading),
            "C mimalloc migration guide should include {heading}"
        );
    }

    for linked_doc in [
        "docs/compatibility.md",
        "docs/abi.md",
        "docs/ffi-linking.md",
        "docs/symbol-versioning.md",
        "docs/platforms.md",
        "docs/limitations.md",
    ] {
        assert!(
            guide.contains(linked_doc),
            "C mimalloc migration guide should link {linked_doc}"
        );
    }
    assert!(
        readme.contains("docs/c-mimalloc-migration.md")
            && release_doc.contains("docs/c-mimalloc-migration.md"),
        "migration guide should be discoverable from README and release policy docs"
    );

    for header in [
        "mimalloc.h",
        "mimalloc-stats.h",
        "mimalloc-override.h",
        "mimalloc-new-delete.h",
    ] {
        assert!(
            guide.contains(header) && ffi_linking.contains(header),
            "migration guide should map C header {header} to release artifacts"
        );
    }
    for artifact in [
        "c-abi-cdylib",
        "interpose-cdylib",
        "interpose-static-preview",
    ] {
        assert!(
            guide.contains(artifact) && ffi_linking.contains(artifact),
            "migration guide should map to artifact {artifact}"
        );
    }

    for c_api in [
        "mi_malloc",
        "mi_zalloc",
        "mi_calloc",
        "mi_realloc",
        "mi_free",
        "mi_heap_*",
    ] {
        assert!(
            guide.contains(c_api),
            "migration guide should discuss C allocation API {c_api}"
        );
    }
    for rust_choice in [
        "MiMalloc",
        "Allocation",
        "InitializedAllocation",
        "Heap",
        "HeapAllocation",
        "mechanical_mimalloc::raw",
        "raw::headers",
        "c-header-macros",
    ] {
        assert!(
            guide.contains(rust_choice),
            "migration guide should map to Rust choice {rust_choice}"
        );
    }

    for callback_or_obligation in [
        "unsafe_callbacks::ErrorCallbackGuard",
        "unsafe_callbacks::DeferredFreeCallbackGuard",
        "mi_register_error",
        "mi_register_deferred_free",
        "Tier U",
        "Nested guards must be dropped in LIFO order",
        "Output callbacks",
    ] {
        assert!(
            guide.contains(callback_or_obligation)
                && (abi_doc.contains(callback_or_obligation)
                    || safe_doc.contains(callback_or_obligation)
                    || compatibility_doc.contains(callback_or_obligation)),
            "migration guide should align callback migration term {callback_or_obligation}"
        );
    }
    assert!(
        guide.contains("serialize registration")
            && guide.contains("restoration, and callback entry")
            && safe_doc.contains("concurrent registration")
            && safe_doc.contains("restoration, or callback entry remains the caller's obligation")
            && abi_doc.contains("restoration, or callback entry for the borrowed closure"),
        "migration guide should align callback serialization obligations with safe and ABI docs"
    );

    for option_or_stats in [
        "GlobalOptions",
        "OptionKey",
        "OptionError",
        "StatsSnapshot",
        "StatCount",
        "StatTotal",
        "ProcessInfo",
    ] {
        assert!(
            guide.contains(option_or_stats) && safe_doc.contains(option_or_stats),
            "migration guide should map safe option/stat wrapper {option_or_stats}"
        );
    }

    for feature_or_platform in [
        "Linux x86_64 GNU",
        "`std`",
        "`secure`",
        "`guarded`",
        "`track-valgrind`",
        "`track-asan`",
        "`c-abi`",
        "`cxx-overrides`",
        "`tls-shim`",
        "`interpose`",
    ] {
        assert!(
            guide.contains(feature_or_platform)
                && (features_doc.contains(feature_or_platform)
                    || ffi_linking.contains(feature_or_platform)
                    || compatibility_doc.contains(feature_or_platform)),
            "migration guide should align feature/platform caveat {feature_or_platform}"
        );
    }
    assert!(
        guide.contains("default features are empty")
            && features_doc.contains("Default features are intentionally empty")
            && guide.contains("Static archives are\npreview-only")
            && ffi_linking.contains("Static archives are preview-only"),
        "migration guide should align default-feature and static-preview caveats"
    );
}

#[test]
fn production_status_text_agrees_across_public_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let limitations =
        fs::read_to_string(root.join("docs/limitations.md")).expect("read limitations");
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for doc in [&readme, &limitations, &platforms, &release] {
        for required in [
            "Production status:",
            "Supported: Tier S safe Rust facade, Tier U unsafe callback guards, and Tier C",
            "dynamic C ABI release artifacts `c-abi-cdylib` and `interpose-cdylib`.",
            "Preview-only: staticlib release artifacts `c-abi-static-preview` and",
            "`interpose-static-preview`.",
            "Unstable: Tier R raw Rust facade path stability, translated implementation",
            "modules, internal strict types, extension ABI, and static archive internal",
            "symbols.",
            "Unsupported: source-present non-maintained platforms are not release artifact",
            "targets.",
            "Maintained target: Linux x86_64 GNU only.",
        ] {
            assert!(
                doc.contains(required),
                "production status docs should agree on: {required}"
            );
        }
    }
    assert!(
        readme.contains("The raw Rust facade is unstable as a Rust path surface")
            && limitations.contains("The `raw` facade exposes C-shaped unsafe APIs")
            && platforms.contains("CI may run smoke validation")
            && release.contains("Tier R is the unsafe raw Rust facade"),
        "production status text should keep raw instability and smoke-target caveats visible"
    );
}

#[test]
fn platform_promotion_policy_requires_release_evidence_checklist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");

    for required in [
        "## Platform Promotion Checklist",
        "Compile and test CI for the platform",
        "ABI/layout checks where the platform has a C ABI story",
        "C and C++ link tests where FFI artifacts are claimed",
        "Sanitizer or equivalent hardening evidence",
        "Platform primitive audit",
        "Benchmark evidence on the promoted platform",
        "Documented limitations",
        "Until every checklist item is complete, the platform remains source-present or\nsmoke-tested only, not maintained.",
    ] {
        assert!(
            platforms.contains(required),
            "docs/platforms.md should require promotion evidence: {required}"
        );
    }
    for check in [
        "scripts/check-layout.sh",
        "scripts/check-release-artifact-consumer.py",
        "scripts/check-release-link-metadata.py",
        "scripts/check-release-dlopen.py",
        "scripts/check-release-ld-preload.py",
        "scripts/check-release-cxx-artifacts.py",
        "ASan/TSan",
        "Miri",
        "Valgrind",
    ] {
        assert!(
            platforms.contains(check),
            "platform promotion checklist should name relevant validation evidence: {check}"
        );
    }
}

#[test]
fn source_present_platforms_stay_out_of_release_marketing() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let validation_workflow = fs::read_to_string(root.join(".github/workflows/validation.yml"))
        .expect("read validation workflow");

    for target in ["macOS", "Windows", "WASI", "Emscripten"] {
        assert!(
            readme.contains(target) && platforms.contains(target) && release.contains(target),
            "source-present or smoke target should be named without marketing overclaim: {target}"
        );
    }
    assert!(
        (readme.contains("Linux aarch64") || readme.contains("Linux\naarch64"))
            && platforms.contains("Linux aarch64")
            && release.contains("Linux aarch64"),
        "source-present or smoke target should be named without marketing overclaim: Linux aarch64"
    );
    for smoke_job in [
        "Linux aarch64 smoke validation",
        "macOS smoke validation",
        "Windows smoke validation",
    ] {
        assert!(
            validation_workflow.contains(smoke_job)
                && platforms.contains(smoke_job)
                && platforms.contains("do\nnot by themselves promote those platforms"),
            "CI smoke job should be documented as non-promoting: {smoke_job}"
        );
    }
    for required in [
        "Linux x86_64 GNU is the only maintained target.",
        "smoke-tested or\nsource-present only unless promoted in docs/platforms.md",
        "triaged as portability issues",
        "not release blockers unless\nthey reproduce on Linux x86_64 GNU",
        "cross-platform safety/security\nbug",
    ] {
        assert!(
            platforms.contains(required),
            "platform docs should include unsupported target release-note language: {required}"
        );
    }
    assert!(
        readme.contains("The maintained target for the current port is Linux x86_64 GNU only.")
            && readme.contains("they do not imply maintained target\nsupport")
            && readme.contains("source-present only unless `docs/platforms.md` promotes them")
            && release.contains("unsupported-target triage language from\n   `docs/platforms.md`")
            && release.contains("do not market Linux aarch64, macOS, Windows, WASI,\n   Emscripten, or other source-present targets as maintained"),
        "README and release policy should avoid source-present platform marketing claims"
    );
}

#[test]
fn private_vulnerability_reporting_process_is_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");

    for required in [
        "https://github.com/whb07/mechanical_mimalloc/security/advisories/new",
        "Do not file a public issue with vulnerability details",
        "If GitHub private vulnerability reporting is unavailable",
        "Acknowledge receipt within 3 business days",
        "initial triage within 7 calendar days",
        "severity/fix-plan update within 14 calendar days",
        "Coordinate public disclosure through a GitHub Security Advisory",
    ] {
        assert!(
            security.contains(required),
            "SECURITY.md should define concrete private reporting process: {required}"
        );
    }
    for report_detail in [
        "affected crate version, release tag, or commit SHA",
        "target triple",
        "`c-abi` cdylib",
        "`interpose` cdylib",
        "preview static archive",
        "enabled features",
        "minimized reproducer",
        "whether C mimalloc 3.3.2 reproduces",
        "sanitizer, Miri, Valgrind",
    ] {
        assert!(
            security.contains(report_detail),
            "SECURITY.md should tell reporters to include {report_detail}"
        );
    }
    assert!(
        security.contains("Linux x86_64 GNU is the currently maintained target")
            && security.contains("triaged as portability issues")
            && security.contains("reproduces on Linux x86_64 GNU")
            && security.contains("cross-platform safety or security bug")
            && security.contains("not release blockers for the maintained target line"),
        "SECURITY.md should keep unsupported-target security triage clear"
    );
}

#[test]
fn release_signing_policy_and_dry_run_are_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let signing =
        fs::read_to_string(root.join("docs/release-signing.md")).expect("read release-signing");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read build-release-artifacts");
    let integrity = fs::read_to_string(root.join("scripts/check-release-integrity.py"))
        .expect("read check-release-integrity");

    for required in [
        "Release signing uses SSH Ed25519 signatures",
        "signed annotated Git tags",
        "git tag -s vX.Y.Z",
        "SHA256SUMS.sig",
        "ssh-ed25519-detached-sha256sums",
        "MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY",
        "mechanical_mimalloc_release",
        "ephemeral test-only",
        "Do not treat an allowed-signers file from an\nuntrusted asset bundle as the trust root.",
        "## Rotation",
    ] {
        assert!(
            signing.contains(required),
            "release signing policy should document {required}"
        );
    }

    for script_marker in [
        "ssh-keygen",
        "-Y",
        "sign",
        "verify_checksum_signature",
        "SHA256SUMS.sig",
        "release-signing.pub",
        "release-allowed-signers",
        "MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY",
        "ephemeral-dry-run-key",
        "ssh-ed25519-detached-sha256sums",
    ] {
        assert!(
            build_release.contains(script_marker),
            "release artifact builder should wire signing marker {script_marker}"
        );
    }

    assert!(
        integrity.contains("builder.validate_integrity(OUT_DIR, manifest)"),
        "release integrity dry run should verify signatures through the builder validation path"
    );
    assert!(
        release.contains("docs/release-signing.md")
            && release.contains("SHA256SUMS.sig")
            && release.contains("configured release SSH Ed25519 key")
            && release.contains("public key fingerprint from `release-artifacts.json`")
            && release.contains("signed annotated source tag with the release SSH\n   signing key"),
        "release checklist should include checksum signature and source tag verification"
    );
}

#[test]
fn release_dependency_report_is_attached_and_tool_aware() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dependencies =
        fs::read_to_string(root.join("docs/dependencies.md")).expect("read dependencies");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read build-release-artifacts");
    let integrity = fs::read_to_string(root.join("scripts/check-release-integrity.py"))
        .expect("read check-release-integrity");

    for required in [
        "target/release-artifacts/integrity/dependencies.md",
        "cargo metadata --locked --format-version 1 --all-features",
        "direct runtime, build, and development dependencies",
        "resolved third-party package graph",
        "C compiler (`CC` or `cc`)",
        "C++ compiler (`CXX` or `c++`)",
        "archiver (`AR` or `ar`)",
    ] {
        assert!(
            dependencies.contains(required),
            "dependency policy should document release dependency report item: {required}"
        );
    }

    for marker in [
        "write_dependency_report",
        "dependency-report",
        "cargo_metadata_all_features",
        "cargo metadata",
        "--all-features",
        "## Direct Runtime Dependencies",
        "## Direct Development Dependencies",
        "## Build Tool Requirements",
        "C compiler",
        "C++ compiler",
        "Archiver",
        "## Resolved Third-Party Packages",
    ] {
        assert!(
            build_release.contains(marker),
            "release artifact builder should generate dependency report marker {marker}"
        );
    }

    assert!(
        integrity.contains("builder.validate_integrity(OUT_DIR, manifest)"),
        "release integrity dry run should validate dependency report through builder validation"
    );
    assert!(
        release.contains("target/release-artifacts/integrity/dependencies.md")
            && release.contains("dependency/build-tool report")
            && release.contains("attach or link that\n   dependency/build-tool report"),
        "release checklist should require the generated dependency report in release notes"
    );
}

#[test]
fn reproducible_release_instructions_are_concrete_and_checked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let reproducibility =
        fs::read_to_string(root.join("docs/reproducibility.md")).expect("read reproducibility");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let script = fs::read_to_string(root.join("scripts/check-release-reproducibility.py"))
        .expect("read check-release-reproducibility");

    for required in [
        "Release Artifact Reproduction",
        "git status --short",
        "git submodule status --recursive",
        "rust-toolchain.toml",
        "Rust channel: `1.92.0`",
        "Required components: `rustfmt`, `clippy`",
        "cargo fetch --locked",
        "scripts/build-release-artifacts.py",
        "scripts/check-release-integrity.py",
        "target/release-artifacts/SHA256SUMS",
        "target/release-artifacts/release-artifacts.json",
        "target/release-artifacts/integrity/abi-symbols/",
        "scripts/env-info.sh > target/release-artifacts/integrity/environment.txt",
        "scripts/check-release-reproducibility.py",
    ] {
        assert!(
            reproducibility.contains(required),
            "reproducibility docs should include release reproduction marker {required}"
        );
    }

    for artifact_mode in [
        "c-abi-cdylib",
        "c-abi-static-preview",
        "interpose-cdylib",
        "interpose-static-preview",
    ] {
        assert!(
            reproducibility.contains(artifact_mode),
            "reproducibility docs should name artifact mode {artifact_mode}"
        );
    }

    for marker in [
        "tracked-file checkouts",
        "build_release",
        "artifact_inventory",
        "normalized_symbol_manifest",
        "SHA256SUMS",
        "integrity/abi-symbols",
        "release artifact names and ABI symbol manifests reproduce",
    ] {
        assert!(
            script.contains(marker),
            "reproducibility script should include marker {marker}"
        );
    }

    assert!(
        release.contains("scripts/check-release-reproducibility.py")
            && release.contains("reproduces artifact names and ABI symbol manifests"),
        "release checklist should require the reproduction gate"
    );
}

#[test]
fn supported_version_policy_is_concrete_and_release_aligned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Until the first published release",
        "current `master` branch only",
        "Latest published release line",
        "Receives security patch releases",
        "Older release lines",
        "Unsupported unless release notes explicitly announce extended support",
        "Release candidate or other pre-release branches",
        "Receive fixes only until the final release for that version is published",
        "There is no implicit long-term support line",
        "affected\nversions, the first fixed version",
        "required upgrade path",
        "Pre-release branches stop receiving security fixes",
    ] {
        assert!(
            security.contains(required),
            "security policy should define supported-version marker {required}"
        );
    }

    for required in [
        "Supported Version Policy",
        "`SECURITY.md` is the authoritative supported-version policy",
        "latest published release line receives security patch releases",
        "older release\nlines are unsupported unless release notes explicitly announce extended support",
        "release candidate or other pre-release branches stop receiving fixes",
        "affected versions, the\nfirst fixed version",
        "whether older lines receive fixes",
        "when pre-release branches stop\n   receiving fixes",
    ] {
        assert!(
            release.contains(required),
            "release policy should align supported-version marker {required}"
        );
    }
}

#[test]
fn upstream_sync_policy_includes_cadence_and_required_evidence_updates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let upstream = fs::read_to_string(root.join("docs/upstream.md")).expect("read upstream docs");

    for required in [
        "Review Cadence",
        "at least quarterly and before every\nrelease candidate",
        "upstream publishes a security\nrelease, CVE",
        "Do not update vendored sources without updating parity,\nABI, release, and benchmark evidence",
        "When upstream public headers or C ABI declarations change, re-run",
        "scripts/check-public-abi.py",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-abi-symbols.sh",
        "scripts/check-release-artifact-consumer.py",
        "When upstream implementation behavior changes, re-run",
        "scripts/check-behavioral-parity.py",
        "scripts/check-unsafe-audit-evidence.py",
        "scripts/check-fuzz-smoke.sh",
        "For allocator hot paths, page/arena behavior, platform primitives",
        "Evidence Updates",
        "`../UPSTREAM_DIFF.md`: baseline version",
        "`PARITY.md`: supported parity matrix",
        "ABI manifests under `docs/abi-manifests/`",
        "`BENCHMARK_RESULTS.md`: benchmark command lines",
        "geomean Rust/C ratios",
    ] {
        assert!(
            upstream.contains(required),
            "upstream sync policy should include marker {required}"
        );
    }
}

#[test]
fn generated_artifact_hygiene_is_centralized() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs =
        fs::read_to_string(root.join("docs/generated-artifacts.md")).expect("read artifact docs");
    let script = fs::read_to_string(root.join("scripts/check-generated-artifacts.py"))
        .expect("read artifact checker");
    let gitignore = fs::read_to_string(root.join(".gitignore")).expect("read .gitignore");
    let check_local =
        fs::read_to_string(root.join("scripts/check-local.sh")).expect("read check-local");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release docs");

    for required in [
        "Generated Artifact Hygiene",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-bench-full.sh",
        "scripts/build-release-artifacts.py",
        "scripts/check-generated-artifacts.py",
        "target/release-artifacts/",
        "target/criterion",
        "RUST_DEPENDENCY_EDGES.csv",
        "parse_bench*.py",
    ] {
        assert!(
            docs.contains(required),
            "generated artifact docs should include marker {required}"
        );
    }

    for required in [
        "REQUIRED_IGNORED",
        "GENERATED_UNTRACKED_PATTERNS",
        "DOC_MARKERS",
        "ls-files",
        "check-ignore",
        "docs/generated-artifacts.md",
    ] {
        assert!(
            script.contains(required),
            "generated artifact checker should include marker {required}"
        );
    }

    for required in [
        "/RUST_DEPENDENCY_EDGES.csv",
        "/parse_bench*.py",
        "*.profraw",
        "*.profdata",
        "*.gcda",
        "*.gcno",
    ] {
        assert!(
            gitignore.contains(required),
            ".gitignore should keep local generated artifact marker {required}"
        );
    }

    assert!(
        check_local.contains("scripts/check-generated-artifacts.py")
            && release.contains("scripts/check-generated-artifacts.py")
            && release.contains("docs/generated-artifacts.md"),
        "local and release checks should run the generated artifact hygiene gate"
    );
}
