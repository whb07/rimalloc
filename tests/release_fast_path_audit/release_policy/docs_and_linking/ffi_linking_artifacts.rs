use std::fs;
use std::path::Path;

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
