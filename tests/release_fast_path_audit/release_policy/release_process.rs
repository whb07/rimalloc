use super::*;

#[test]
fn production_status_text_agrees_across_public_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let limitations =
        fs::read_to_string(root.join("docs/limitations.md")).expect("read limitations");
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for doc in [&readme, &limitations, &platforms, &release] {
        for required in [
            "Production status:",
            "Supported: Tier S safe Rust facade, Tier U unsafe callback guards, and Tier C",
            "dynamic C ABI release artifacts `c-abi-cdylib` and `interpose-cdylib`.",
            "Preview-only: staticlib release artifacts `c-abi-static-preview` and",
            "`interpose-static-preview`.",
            "Unstable: Tier R raw Rust facade path stability, translated implementation",
            "modules, internal strict types, extension ABI, and static archive internal",
            "symbols.",
            "Unsupported: source-present non-maintained platforms are not release artifact",
            "targets.",
            "Maintained target: Linux x86_64 GNU only.",
        ] {
            assert!(
                doc.contains(required),
                "production status docs should agree on: {required}"
            );
        }
    }
    assert!(
        readme.contains("The raw Rust facade is unstable as a Rust path surface")
            && limitations.contains("The `raw` facade exposes C-shaped unsafe APIs")
            && platforms.contains("CI may run smoke validation")
            && release.contains("Tier R is the unsafe raw Rust facade"),
        "production status text should keep raw instability and smoke-target caveats visible"
    );
}

#[test]
fn platform_promotion_policy_requires_release_evidence_checklist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");

    for required in [
        "## Platform Promotion Checklist",
        "Compile and test CI for the platform",
        "ABI/layout checks where the platform has a C ABI story",
        "C and C++ link tests where FFI artifacts are claimed",
        "Sanitizer or equivalent hardening evidence",
        "Platform primitive audit",
        "Benchmark evidence on the promoted platform",
        "Documented limitations",
        "Until every checklist item is complete, the platform remains source-present or\nsmoke-tested only, not maintained.",
    ] {
        assert!(
            platforms.contains(required),
            "docs/platforms.md should require promotion evidence: {required}"
        );
    }
    for check in [
        "scripts/check-layout.sh",
        "scripts/check-release-artifact-consumer.py",
        "scripts/check-release-link-metadata.py",
        "scripts/check-release-dlopen.py",
        "scripts/check-release-ld-preload.py",
        "scripts/check-release-cxx-artifacts.py",
        "ASan/TSan",
        "Miri",
        "Valgrind",
    ] {
        assert!(
            platforms.contains(check),
            "platform promotion checklist should name relevant validation evidence: {check}"
        );
    }
}

#[test]
fn source_present_platforms_stay_out_of_release_marketing() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let validation_workflow = fs::read_to_string(root.join(".github/workflows/validation.yml"))
        .expect("read validation workflow");

    for target in ["macOS", "Windows", "WASI", "Emscripten"] {
        assert!(
            readme.contains(target) && platforms.contains(target) && release.contains(target),
            "source-present or smoke target should be named without marketing overclaim: {target}"
        );
    }
    assert!(
        (readme.contains("Linux aarch64") || readme.contains("Linux\naarch64"))
            && platforms.contains("Linux aarch64")
            && release.contains("Linux aarch64"),
        "source-present or smoke target should be named without marketing overclaim: Linux aarch64"
    );
    for smoke_job in [
        "Linux aarch64 smoke validation",
        "macOS smoke validation",
        "Windows smoke validation",
    ] {
        assert!(
            validation_workflow.contains(smoke_job)
                && platforms.contains(smoke_job)
                && platforms.contains("do\nnot by themselves promote those platforms"),
            "CI smoke job should be documented as non-promoting: {smoke_job}"
        );
    }
    for required in [
        "Linux x86_64 GNU is the only maintained target.",
        "smoke-tested or\nsource-present only unless promoted in docs/platforms.md",
        "triaged as portability issues",
        "not release blockers unless\nthey reproduce on Linux x86_64 GNU",
        "cross-platform safety/security\nbug",
    ] {
        assert!(
            platforms.contains(required),
            "platform docs should include unsupported target release-note language: {required}"
        );
    }
    assert!(
        readme.contains("The maintained target for the current port is Linux x86_64 GNU only.")
            && readme.contains("they do not imply maintained target\nsupport")
            && readme.contains("source-present only unless `docs/platforms.md` promotes them")
            && release.contains("unsupported-target triage language from\n   `docs/platforms.md`")
            && release.contains("do not market Linux aarch64, macOS, Windows, WASI,\n   Emscripten, or other source-present targets as maintained"),
        "README and release policy should avoid source-present platform marketing claims"
    );
}

#[test]
fn private_vulnerability_reporting_process_is_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");

    for required in [
        "https://github.com/whb07/mechanical_mimalloc/security/advisories/new",
        "Do not file a public issue with vulnerability details",
        "If GitHub private vulnerability reporting is unavailable",
        "Acknowledge receipt within 3 business days",
        "initial triage within 7 calendar days",
        "severity/fix-plan update within 14 calendar days",
        "Coordinate public disclosure through a GitHub Security Advisory",
    ] {
        assert!(
            security.contains(required),
            "SECURITY.md should define concrete private reporting process: {required}"
        );
    }
    for report_detail in [
        "affected crate version, release tag, or commit SHA",
        "target triple",
        "`c-abi` cdylib",
        "`interpose` cdylib",
        "preview static archive",
        "enabled features",
        "minimized reproducer",
        "whether C mimalloc 3.3.2 reproduces",
        "sanitizer, Miri, Valgrind",
    ] {
        assert!(
            security.contains(report_detail),
            "SECURITY.md should tell reporters to include {report_detail}"
        );
    }
    assert!(
        security.contains("Linux x86_64 GNU is the currently maintained target")
            && security.contains("triaged as portability issues")
            && security.contains("reproduces on Linux x86_64 GNU")
            && security.contains("cross-platform safety or security bug")
            && security.contains("not release blockers for the maintained target line"),
        "SECURITY.md should keep unsupported-target security triage clear"
    );
}

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
