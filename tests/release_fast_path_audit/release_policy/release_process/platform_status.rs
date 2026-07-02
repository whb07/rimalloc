use std::fs;
use std::path::Path;

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
