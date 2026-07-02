use std::fs;
use std::path::Path;

#[path = "raw_facade/nullability.rs"]
mod nullability;

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
        "raw-macro-unix-page-aligned-boundary",
        "raw-macro-unix-aligned-posix-boundary",
        "raw-macro-unix-cfree-boundary",
        "raw-macro-unix-product-realloc-boundary",
        "raw-macro-wasi-libc-boundary",
        "raw-macro-linux-libc-boundary",
        "raw-macro-linux-libc-aligned-boundary",
        "raw-macro-allocation-query-boundary",
        "raw-macro-cxx-delete-boundary",
        "raw-macro-cxx-array-delete-boundary",
        "raw-macro-cxx-aligned-delete-boundary",
        "raw-macro-cxx-aligned-new64-boundary",
        "raw-macro-cxx-aligned-new32-boundary",
        "raw-macro-cxx-aligned-operator-boundary",
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
            && !raw_macros_rs.contains("SAFETY[AUDIT:raw-macro-cxx-aligned-boundary]")
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
