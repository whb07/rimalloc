use super::*;

#[test]
fn linux_cdylib_export_script_declares_allocator_symbols() {
    let build_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
        .expect("read build.rs");

    assert!(
        build_rs.contains("if c_abi")
            && build_rs.contains("cargo:rustc-link-arg=-Wl,--version-script="),
        "Linux C ABI artifact builds should pass the allocator export script through the cdylib link step"
    );
    assert!(
        build_rs.contains("PUBLIC_HEADERS")
            && build_rs.contains("CXX_EXPORT_PATTERNS")
            && build_rs.contains("MALLOC_EXPORT_PATTERNS")
            && build_rs.contains("public_mi_symbols_from_headers")
            && build_rs.contains("if cxx_shim_linked")
            && build_rs.contains("if interpose")
            && build_rs.contains("exports.push_str(&symbol)")
            && !build_rs.contains("mi_*;")
            && build_rs.contains("local:\n    *;"),
        "version script should export exact public-header mi_* APIs while hiding unmatched internals"
    );
    assert!(
        !build_rs.contains("_Z*;")
            && build_rs.contains("\"_Zda*\"")
            && build_rs.contains("\"_Zdl*\"")
            && build_rs.contains("\"_Zna*\"")
            && build_rs.contains("\"_Znw*\"")
            && build_rs.contains("\"malloc*\"")
            && build_rs.contains("\"free*\"")
            && build_rs.contains("\"realloc*\""),
        "version script must keep allocator and C++ ABI override names available only through mode groups"
    );
}
