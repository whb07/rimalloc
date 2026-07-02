use std::fs;
use std::path::Path;

#[test]
fn reproducible_release_instructions_are_concrete_and_checked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let reproducibility =
        fs::read_to_string(root.join("docs/reproducibility.md")).expect("read reproducibility");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let script = fs::read_to_string(root.join("scripts/check-release-reproducibility.py"))
        .expect("read check-release-reproducibility");

    for required in [
        "Release Artifact Reproduction",
        "git status --short",
        "git submodule status --recursive",
        "rust-toolchain.toml",
        "Rust channel: `1.92.0`",
        "Required components: `rustfmt`, `clippy`",
        "cargo fetch --locked",
        "scripts/build-release-artifacts.py",
        "scripts/check-release-integrity.py",
        "target/release-artifacts/SHA256SUMS",
        "target/release-artifacts/release-artifacts.json",
        "target/release-artifacts/integrity/abi-symbols/",
        "scripts/env-info.sh > target/release-artifacts/integrity/environment.txt",
        "scripts/check-release-reproducibility.py",
    ] {
        assert!(
            reproducibility.contains(required),
            "reproducibility docs should include release reproduction marker {required}"
        );
    }

    for artifact_mode in [
        "c-abi-cdylib",
        "c-abi-static-preview",
        "interpose-cdylib",
        "interpose-static-preview",
    ] {
        assert!(
            reproducibility.contains(artifact_mode),
            "reproducibility docs should name artifact mode {artifact_mode}"
        );
    }

    for marker in [
        "tracked-file checkouts",
        "build_release",
        "artifact_inventory",
        "normalized_symbol_manifest",
        "SHA256SUMS",
        "integrity/abi-symbols",
        "release artifact names and ABI symbol manifests reproduce",
    ] {
        assert!(
            script.contains(marker),
            "reproducibility script should include marker {marker}"
        );
    }

    assert!(
        release.contains("scripts/check-release-reproducibility.py")
            && release.contains("reproduces artifact names and ABI symbol manifests"),
        "release checklist should require the reproduction gate"
    );
}
