use super::*;

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
