use std::fs;
use std::path::Path;

#[test]
fn release_signing_policy_and_dry_run_are_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let signing =
        fs::read_to_string(root.join("docs/release-signing.md")).expect("read release-signing");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read build-release-artifacts");
    let integrity = fs::read_to_string(root.join("scripts/check-release-integrity.py"))
        .expect("read check-release-integrity");

    for required in [
        "Release signing uses SSH Ed25519 signatures",
        "signed annotated Git tags",
        "git tag -s vX.Y.Z",
        "SHA256SUMS.sig",
        "ssh-ed25519-detached-sha256sums",
        "MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY",
        "mechanical_mimalloc_release",
        "ephemeral test-only",
        "Do not treat an allowed-signers file from an\nuntrusted asset bundle as the trust root.",
        "## Rotation",
    ] {
        assert!(
            signing.contains(required),
            "release signing policy should document {required}"
        );
    }

    for script_marker in [
        "ssh-keygen",
        "-Y",
        "sign",
        "verify_checksum_signature",
        "SHA256SUMS.sig",
        "release-signing.pub",
        "release-allowed-signers",
        "MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY",
        "ephemeral-dry-run-key",
        "ssh-ed25519-detached-sha256sums",
    ] {
        assert!(
            build_release.contains(script_marker),
            "release artifact builder should wire signing marker {script_marker}"
        );
    }

    assert!(
        integrity.contains("builder.validate_integrity(OUT_DIR, manifest)"),
        "release integrity dry run should verify signatures through the builder validation path"
    );
    assert!(
        release.contains("docs/release-signing.md")
            && release.contains("SHA256SUMS.sig")
            && release.contains("configured release SSH Ed25519 key")
            && release.contains("public key fingerprint from `release-artifacts.json`")
            && release.contains("signed annotated source tag with the release SSH\n   signing key"),
        "release checklist should include checksum signature and source tag verification"
    );
}

#[test]
fn release_dependency_report_is_attached_and_tool_aware() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dependencies =
        fs::read_to_string(root.join("docs/dependencies.md")).expect("read dependencies");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read build-release-artifacts");
    let integrity = fs::read_to_string(root.join("scripts/check-release-integrity.py"))
        .expect("read check-release-integrity");

    for required in [
        "target/release-artifacts/integrity/dependencies.md",
        "cargo metadata --locked --format-version 1 --all-features",
        "direct runtime, build, and development dependencies",
        "resolved third-party package graph",
        "C compiler (`CC` or `cc`)",
        "C++ compiler (`CXX` or `c++`)",
        "archiver (`AR` or `ar`)",
    ] {
        assert!(
            dependencies.contains(required),
            "dependency policy should document release dependency report item: {required}"
        );
    }

    for marker in [
        "write_dependency_report",
        "dependency-report",
        "cargo_metadata_all_features",
        "cargo metadata",
        "--all-features",
        "## Direct Runtime Dependencies",
        "## Direct Development Dependencies",
        "## Build Tool Requirements",
        "C compiler",
        "C++ compiler",
        "Archiver",
        "## Resolved Third-Party Packages",
    ] {
        assert!(
            build_release.contains(marker),
            "release artifact builder should generate dependency report marker {marker}"
        );
    }

    assert!(
        integrity.contains("builder.validate_integrity(OUT_DIR, manifest)"),
        "release integrity dry run should validate dependency report through builder validation"
    );
    assert!(
        release.contains("target/release-artifacts/integrity/dependencies.md")
            && release.contains("dependency/build-tool report")
            && release.contains("attach or link that\n   dependency/build-tool report"),
        "release checklist should require the generated dependency report in release notes"
    );
}
