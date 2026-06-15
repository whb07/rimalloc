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
            && tls_shim.contains("visibility(\"hidden\")")
            && tls_shim.contains("= &_mi_theap_empty"),
        "default Linux malloc assembly should read the hidden initial-exec TLS heap initialized like C"
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
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_array_fast_64")
            && override_rs.contains("core::arch::naked_asm!")
            && override_rs.contains("mi_tls_theap_default@GOTTPOFF")
            && !override_rs.contains("\"test rdi, rdi\"")
            && override_rs.contains("[rdi + rax*8 + 0x110]")
            && override_rs.contains("slow = sym crate::page::_mi_malloc_generic")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_unix_free_fast")
            && override_rs.contains("page_map = sym crate::page_map::_mi_page_map")
            && override_rs.contains("slow = sym crate::free::mi_free_ex_slow")
            && override_rs.contains("retire = sym crate::page::_mi_page_retire")
            && override_rs.contains("jmp mi_new"),
        "default Linux malloc/new/free overrides should use direct naked-assembly fast paths"
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
    assert!(
        override_rs.contains("#[unsafe(export_name = \"free\")]")
            && override_rs.contains("mi_unix_free_fast")
            && override_rs.contains(
                "cfg(all(not(windows), not(mi_c_malloc_fast)))]\n#[unsafe(export_name = \"free\")]"
            ),
        "Unix free export must stay paired with the assembly fast override"
    );
    assert_eq!(offset_of!(mi_page_t, free), 0x08);
    assert_eq!(offset_of!(mi_page_t, used), 0x10);
    #[cfg(not(feature = "guarded"))]
    assert_eq!(offset_of!(mi_theap_t, pages_free_direct), 0x110);
}

#[test]
fn cxx_new_wrappers_export_itanium_abi_symbols() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let override_rs =
        fs::read_to_string(root.join("src/alloc_override.rs")).expect("read src/alloc_override.rs");

    assert!(
        override_rs.contains("export_name = \"_Znwm\"")
            && override_rs.contains("export_name = \"_Znam\"")
            && override_rs.contains("not(mi_c_malloc_fast)")
            && override_rs.contains("export_name = \"_ZnwmRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamRKSt9nothrow_t\""),
        "64-bit C++ operator new wrappers must export Itanium ABI symbols"
    );
    assert!(
        override_rs.contains("export_name = \"_ZnwmSt11align_val_t\"")
            && override_rs.contains("export_name = \"_ZnamSt11align_val_t\"")
            && override_rs.contains("export_name = \"_ZnwmSt11align_val_tRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamSt11align_val_tRKSt9nothrow_t\""),
        "aligned C++ operator new wrappers must export Itanium ABI symbols"
    );
}

#[test]
fn linux_cdylib_export_script_hides_internal_symbols() {
    let build_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
        .expect("read build.rs");

    assert!(
        build_rs.contains("cargo:rustc-link-arg=-Wl,--version-script="),
        "Linux cdylib builds should use a version script so internal symbols are not preemptible"
    );
    assert!(
        build_rs.contains("mi_*;") && build_rs.contains("local:\n    *;"),
        "version script should export public mi_* APIs while hiding unmatched internals"
    );
    assert!(
        !build_rs.contains("_Z*;")
            && build_rs.contains("_Zda*;")
            && build_rs.contains("_Zdl*;")
            && build_rs.contains("_Zna*;")
            && build_rs.contains("_Znw*;")
            && build_rs.contains("malloc*;")
            && build_rs.contains("free*;")
            && build_rs.contains("realloc*;"),
        "version script must keep allocator and C++ ABI override names exported"
    );
}

#[test]
fn free_path_internals_are_not_dynamic_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let page_map = fs::read_to_string(root.join("src/page_map.rs")).expect("read src/page_map.rs");
    let internal = fs::read_to_string(root.join("src/internal.rs")).expect("read src/internal.rs");
    let free_rs = fs::read_to_string(root.join("src/free.rs")).expect("read src/free.rs");
    let page_rs = fs::read_to_string(root.join("src/page.rs")).expect("read src/page.rs");

    for internal_name in [
        "pub static mut _mi_page_map",
        "pub unsafe extern \"C\" fn _mi_page_map_init",
        "pub unsafe extern \"C\" fn _mi_page_map_register",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister_range",
        "pub unsafe extern \"C\" fn _mi_page_map_unsafe_destroy",
        "pub unsafe extern \"C\" fn _mi_safe_ptr_page",
    ] {
        let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
        assert!(
            !page_map.contains(&no_mangle_export),
            "{internal_name} should stay crate-internal instead of a dynamic export"
        );
    }

    assert!(
        !internal.contains("pub static mut _mi_page_map:")
            && !internal.contains("pub fn _mi_page_map_init() -> bool;")
            && internal.contains("pub(crate) use crate::page_map::{"),
        "page-map internals should be imported through crate paths, not extern declarations"
    );
    assert!(
        !free_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_free_generic"),
        "_mi_free_generic should not be a dynamic export"
    );
    assert!(
        !page_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_page_retire"),
        "_mi_page_retire should not be a dynamic export"
    );
    assert!(
        !page_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_malloc_generic"),
        "_mi_malloc_generic should not be a dynamic export"
    );
}

#[test]
fn generic_malloc_refill_helper_is_force_inlined() {
    let page_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/page.rs"))
        .expect("read src/page.rs");

    assert!(
        page_rs.contains("#[inline(always)]\nunsafe fn mi_find_page("),
        "mi_find_page should stay force-inlined into _mi_malloc_generic like C's generic path"
    );
}

#[test]
fn release_page_stat_writes_follow_c_mimalloc_gates() {
    let page_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/page.rs"))
        .expect("read src/page.rs");

    for (stat, needle) in [
        (
            "pages_retire",
            "if MI_STAT > 0 {\n            __mi_stat_counter_increase(&mut (*theap).stats.pages_retire, 1);",
        ),
        (
            "pages_extended",
            "if MI_STAT > 0 {\n        __mi_stat_counter_increase(&mut (*theap).stats.pages_extended, 1);",
        ),
        (
            "page_committed",
            "if MI_STAT > 0 {\n        __mi_stat_increase(&mut (*theap).stats.page_committed, extend * bsize);",
        ),
    ] {
        assert!(
            page_rs.contains(needle),
            "{stat} updates should stay behind MI_STAT > 0 like C mimalloc"
        );
    }
}
