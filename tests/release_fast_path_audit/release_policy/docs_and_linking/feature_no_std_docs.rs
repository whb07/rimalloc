use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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
