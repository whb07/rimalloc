use super::*;

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
    let raw_macros_rs =
        fs::read_to_string(root.join("src/raw/macros.rs")).expect("read src/raw/macros.rs");
    let unsafe_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit");

    assert!(
        abi_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && headers_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-allocation-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:raw-macro-allocation-boundary]")
            && abi_doc.contains("## Raw Macro Unsafe Boundary"),
        "raw macro unsafe-boundary audit tags should be mirrored in source and docs"
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
