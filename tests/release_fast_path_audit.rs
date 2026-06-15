use std::fs;
use std::mem::offset_of;
use std::path::Path;

use mechanical_mimalloc::types::mi_page_t;
#[cfg(not(feature = "guarded"))]
use mechanical_mimalloc::types::mi_theap_t;

#[test]
fn debug_assert_macros_are_release_gated_like_c_mimalloc() {
    let internal_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/internal.rs");
    let source = fs::read_to_string(internal_rs).expect("read src/internal.rs");

    assert!(
        source.contains("if $crate::internal::MI_DEBUG > 0 && !$expr"),
        "mi_debug_assert! must compile out its expression when MI_DEBUG == 0"
    );
    assert!(
        source.contains("if $crate::internal::MI_DEBUG > 0 {\n            let left = &$left;"),
        "mi_debug_assert_eq! must compile out operand evaluation when MI_DEBUG == 0"
    );
}

#[test]
fn default_linux_malloc_override_uses_c_tls_fast_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tls_shim = fs::read_to_string(root.join("src/tls_shim.c")).expect("read src/tls_shim.c");
    let build_rs = fs::read_to_string(root.join("build.rs")).expect("read build.rs");
    let alloc_rs = fs::read_to_string(root.join("src/alloc.rs")).expect("read src/alloc.rs");
    let override_rs =
        fs::read_to_string(root.join("src/alloc_override.rs")).expect("read src/alloc_override.rs");

    assert!(
        tls_shim.contains("mi_tls_theap_default")
            && tls_shim.contains("tls_model(\"initial-exec\")")
            && tls_shim.contains("visibility(\"hidden\")"),
        "default Linux malloc assembly should read the hidden initial-exec TLS heap"
    );
    assert!(
        build_rs.contains("target_arch == \"x86_64\"")
            && build_rs.contains("cargo:rustc-cfg=mi_c_malloc_fast"),
        "assembly malloc fast override must be limited to the supported target"
    );
    assert!(
        override_rs.contains("#[unsafe(naked)]")
            && override_rs.contains("pub unsafe extern \"C\" fn malloc")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_malloc")
            && override_rs.contains("core::arch::naked_asm!")
            && override_rs.contains("mi_tls_theap_default@GOTTPOFF")
            && override_rs.contains("[rdi + rax*8 + 0x110]")
            && override_rs.contains("jmp _mi_malloc_generic"),
        "default Linux malloc override should use the direct naked-assembly TLS fast path"
    );
    assert!(
        alloc_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_malloc"),
        "Rust mi_malloc export gate must stay paired with the assembly fast override"
    );
    assert!(
        override_rs.contains("cfg(all(not(windows), not(mi_c_malloc_fast)))")
            && override_rs.contains("export_name = \"malloc\""),
        "Rust malloc override gate must stay paired with the assembly fast override"
    );
    assert_eq!(offset_of!(mi_page_t, free), 0x08);
    assert_eq!(offset_of!(mi_page_t, used), 0x10);
    #[cfg(not(feature = "guarded"))]
    assert_eq!(offset_of!(mi_theap_t, pages_free_direct), 0x110);
}
