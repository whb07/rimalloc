use std::fs;
use std::path::Path;

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
    let raw_macro_allocation_ids = [
        "raw-macro-bootstrap-boundary",
        "raw-macro-fast-override-boundary",
        "raw-macro-macos-interpose-boundary",
        "raw-macro-windows-crt-boundary",
        "raw-macro-windows-aligned-boundary",
        "raw-macro-unix-core-boundary",
        "raw-macro-unix-string-boundary",
        "raw-macro-unix-realloc-boundary",
        "raw-macro-unix-aligned-posix-boundary",
        "raw-macro-unix-product-realloc-boundary",
        "raw-macro-wasi-libc-boundary",
        "raw-macro-linux-libc-boundary",
        "raw-macro-allocation-query-boundary",
        "raw-macro-cxx-delete-boundary",
        "raw-macro-cxx-array-delete-boundary",
        "raw-macro-cxx-aligned-boundary",
        "raw-macro-cxx-nothrow-boundary",
        "raw-macro-cxx-new-boundary",
        "raw-macro-header-forwarding-boundary",
    ];

    for audit_id in raw_macro_allocation_ids {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            raw_macros_rs.contains(&marker) && unsafe_doc.contains(&marker),
            "raw macro allocation audit tag {audit_id} should be mirrored in source and docs"
        );
    }

    assert!(
        abi_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && headers_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-allocation-boundary]")
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
