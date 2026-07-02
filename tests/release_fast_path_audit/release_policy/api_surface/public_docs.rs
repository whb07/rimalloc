use std::fs;
use std::path::Path;

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
