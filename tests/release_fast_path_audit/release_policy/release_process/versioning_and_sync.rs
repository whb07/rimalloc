use std::fs;
use std::path::Path;

#[test]
fn supported_version_policy_is_concrete_and_release_aligned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Until the first published release",
        "current `master` branch only",
        "Latest published release line",
        "Receives security patch releases",
        "Older release lines",
        "Unsupported unless release notes explicitly announce extended support",
        "Release candidate or other pre-release branches",
        "Receive fixes only until the final release for that version is published",
        "There is no implicit long-term support line",
        "affected\nversions, the first fixed version",
        "required upgrade path",
        "Pre-release branches stop receiving security fixes",
    ] {
        assert!(
            security.contains(required),
            "security policy should define supported-version marker {required}"
        );
    }

    for required in [
        "Supported Version Policy",
        "`SECURITY.md` is the authoritative supported-version policy",
        "latest published release line receives security patch releases",
        "older release\nlines are unsupported unless release notes explicitly announce extended support",
        "release candidate or other pre-release branches stop receiving fixes",
        "affected versions, the\nfirst fixed version",
        "whether older lines receive fixes",
        "when pre-release branches stop\n   receiving fixes",
    ] {
        assert!(
            release.contains(required),
            "release policy should align supported-version marker {required}"
        );
    }
}

#[test]
fn upstream_sync_policy_includes_cadence_and_required_evidence_updates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let upstream = fs::read_to_string(root.join("docs/upstream.md")).expect("read upstream docs");

    for required in [
        "Review Cadence",
        "at least quarterly and before every\nrelease candidate",
        "upstream publishes a security\nrelease, CVE",
        "Do not update vendored sources without updating parity,\nABI, release, and benchmark evidence",
        "When upstream public headers or C ABI declarations change, re-run",
        "scripts/check-public-abi.py",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-abi-symbols.sh",
        "scripts/check-release-artifact-consumer.py",
        "When upstream implementation behavior changes, re-run",
        "scripts/check-behavioral-parity.py",
        "scripts/check-unsafe-audit-evidence.py",
        "scripts/check-fuzz-smoke.sh",
        "For allocator hot paths, page/arena behavior, platform primitives",
        "Evidence Updates",
        "`../UPSTREAM_DIFF.md`: baseline version",
        "`PARITY.md`: supported parity matrix",
        "ABI manifests under `docs/abi-manifests/`",
        "`BENCHMARK_RESULTS.md`: benchmark command lines",
        "geomean Rust/C ratios",
    ] {
        assert!(
            upstream.contains(required),
            "upstream sync policy should include marker {required}"
        );
    }
}

#[test]
fn generated_artifact_hygiene_is_centralized() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs =
        fs::read_to_string(root.join("docs/generated-artifacts.md")).expect("read artifact docs");
    let script = fs::read_to_string(root.join("scripts/check-generated-artifacts.py"))
        .expect("read artifact checker");
    let gitignore = fs::read_to_string(root.join(".gitignore")).expect("read .gitignore");
    let check_local =
        fs::read_to_string(root.join("scripts/check-local.sh")).expect("read check-local");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release docs");

    for required in [
        "Generated Artifact Hygiene",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-bench-full.sh",
        "scripts/build-release-artifacts.py",
        "scripts/check-generated-artifacts.py",
        "target/release-artifacts/",
        "target/criterion",
        "RUST_DEPENDENCY_EDGES.csv",
        "parse_bench*.py",
    ] {
        assert!(
            docs.contains(required),
            "generated artifact docs should include marker {required}"
        );
    }

    for required in [
        "REQUIRED_IGNORED",
        "GENERATED_UNTRACKED_PATTERNS",
        "DOC_MARKERS",
        "ls-files",
        "check-ignore",
        "docs/generated-artifacts.md",
    ] {
        assert!(
            script.contains(required),
            "generated artifact checker should include marker {required}"
        );
    }

    for required in [
        "/RUST_DEPENDENCY_EDGES.csv",
        "/parse_bench*.py",
        "*.profraw",
        "*.profdata",
        "*.gcda",
        "*.gcno",
    ] {
        assert!(
            gitignore.contains(required),
            ".gitignore should keep local generated artifact marker {required}"
        );
    }

    assert!(
        check_local.contains("scripts/check-generated-artifacts.py")
            && release.contains("scripts/check-generated-artifacts.py")
            && release.contains("docs/generated-artifacts.md"),
        "local and release checks should run the generated artifact hygiene gate"
    );
}
