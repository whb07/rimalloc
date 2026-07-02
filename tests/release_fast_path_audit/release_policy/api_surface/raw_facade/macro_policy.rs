use std::fs;
use std::path::Path;

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
