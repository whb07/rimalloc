use std::fs;
use std::path::Path;

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
            && unsafe_doc.contains("SAFETY[AUDIT:unsafe-callback-guard-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:unsafe-callback-test-fixtures]")
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
