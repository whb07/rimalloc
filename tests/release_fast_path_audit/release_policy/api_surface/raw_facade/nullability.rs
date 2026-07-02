use std::fs;
use std::path::Path;

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
