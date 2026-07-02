use std::fs;
use std::path::Path;

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
