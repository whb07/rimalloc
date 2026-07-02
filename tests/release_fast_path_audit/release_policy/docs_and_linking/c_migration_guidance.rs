use std::fs;
use std::path::Path;

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
