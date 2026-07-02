use super::*;

#[test]
fn linux_interpose_mode_keeps_static_and_libc_compatibility() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let override_rs =
        fs::read_to_string(root.join("src/raw/macros.rs")).expect("read src/raw/macros.rs");

    assert!(
        cargo_toml.contains("crate-type = [\"rlib\"]")
            && cargo_toml.contains("c-abi = []")
            && cargo_toml.contains("cxx-overrides = [\"c-abi\"]")
            && cargo_toml.contains("tls-shim = [\"c-abi\"]")
            && cargo_toml.contains("interpose = [\"c-abi\", \"cxx-overrides\", \"tls-shim\"]")
            && cargo_toml.contains("lto = \"fat\"")
            && cargo_toml.contains("codegen-units = 1"),
        "ordinary builds should stay rlib-first while the explicit interpose feature restores the optimized cdylib artifact mode"
    );
    assert!(
        lib_rs.contains("not(feature = \"std\")")
            && lib_rs.contains("no_std")
            && lib_rs.contains("#[panic_handler]")
            && lib_rs.contains("#[link_name = \"abort\"]")
            && lib_rs.contains("extern \"C\" fn rust_eh_personality()"),
        "Linux release cdylibs should avoid linking Rust std panic/backtrace machinery unless the explicit std feature is enabled for tooling"
    );
    for export_name in [
        "reallocf",
        "strdup",
        "strndup",
        "malloc_good_size",
        "vfree",
        "_aligned_malloc",
        "__libc_realloc",
        "__libc_cfree",
        "__libc_valloc",
        "__libc_pvalloc",
        "__libc_memalign",
        "__posix_memalign",
    ] {
        assert!(
            override_rs.contains(&format!("export_name = \"{export_name}\"")),
            "Rust Unix/Linux override surface should export {export_name} like C mimalloc"
        );
    }
    assert!(
        override_rs.contains(".weak reallocarr")
            && override_rs.contains(".set reallocarr, mi_reallocarr")
            && override_rs.contains("export_name = \"__libc_malloc\"")
            && override_rs.contains("export_name = \"__libc_calloc\"")
            && override_rs.contains("export_name = \"__libc_free\""),
        "Rust override surface should keep C's libc aliases and weak reallocarr compatibility"
    );
}
