use super::*;

#[test]
fn platform_primitive_backends_stay_off_migration_allowlist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    for rel in [
        "src/platform/emscripten.rs",
        "src/platform/macos.rs",
        "src/platform/wasi.rs",
        "src/platform/windows.rs",
    ] {
        assert!(
            !allowlist
                .lines()
                .any(|line| line.starts_with(&format!("{rel}\t"))),
            "{rel} should stay off the unsafe migration allowlist"
        );
    }
}
