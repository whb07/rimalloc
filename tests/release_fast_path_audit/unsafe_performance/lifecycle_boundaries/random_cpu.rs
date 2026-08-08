use super::*;

pub(super) fn assert_random_and_cpu_boundaries(sources: &UnsafePerformanceSources) {
    let init_rs = sources.init_rs.as_str();
    let internal = sources.internal.as_str();
    let prim_rs = sources.prim_rs.as_str();

    assert!(
        init_rs.contains("core::arch::global_asm!(\".hidden _mi_cpu_stosb_max\")")
            && init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static mut _mi_cpu_stosb_max")
            && !init_rs.contains("_mi_cpu_movsb_max")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static mut _mi_cpu_stosb_max"),
        "the remaining CPU fill threshold should keep a stable hidden C name without becoming a public dynamic ABI symbol"
    );
    assert!(
        init_rs.contains("fn mi_detect_cpu_features()")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 0, 0)")
            && init_rs.contains("amd = cpu_info[2] == 0x444d_4163;")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 7, 0)")
            && init_rs.contains("fsrm = (cpu_info[3] & (1 << 4)) != 0;")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 7, 1)")
            && init_rs.contains("fsrs = (cpu_info[1] & (1 << 11)) != 0;")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 1, 0)")
            && init_rs.contains("_mi_cpu_has_popcnt = (cpu_info[2] & (1 << 23)) != 0")
            && init_rs.contains(
                "if fsrs || (amd && fsrm) {\n            unsafe { _mi_cpu_stosb_max = 127 };"
            ),
        "CPU feature detection should initialize popcnt and the short fill threshold"
    );
    assert!(
    internal.contains("\"rep stosb\"")
        && internal.contains("pub unsafe fn mi_rep_stosb(dst: *mut c_void, val: u8, n: usize)")
        && internal.contains("if n <= mi_cpu_stosb_max() {\n                mi_rep_stosb(dst, val as u8, n);\n                return;\n            }"),
    "_mi_memset should keep C's CPU-thresholded rep stosb shape instead of always lowering to external memset"
);
    for internal_static in [
        "_mi_cpu_stosb_max",
        "_mi_theap_default_slot",
        "_mi_theap_default_expansion_slot",
        "_mi_theap_cached_slot",
        "_mi_theap_cached_expansion_slot",
        "mi_tls_raw_index_default",
        "mi_tls_raw_index_cached",
        "_mi_theap_default_key",
        "_mi_theap_cached_key",
        "__mi_theap_main",
        "_mi_process_is_initialized",
        "_mi_cpu_has_popcnt",
    ] {
        assert!(
            !init_rs.contains(&format!(
                "#[unsafe(no_mangle)]\npub static mut {internal_static}"
            )),
            "{internal_static} should stay crate-local instead of a dynamic storage export"
        );
    }
    assert!(
        !prim_rs.contains("pub static mut __mi_theap_default")
            && !prim_rs.contains("pub static mut __mi_theap_cached"),
        "unused fallback TLS globals should not be exported as dynamic storage symbols"
    );
    assert!(
        !internal.contains("pub static mut _mi_cpu_stosb_max")
            && !prim_rs.contains("pub static mut _mi_process_is_initialized")
            && !prim_rs.contains("pub static mut _mi_theap_default_slot")
            && !prim_rs.contains("pub static mut _mi_theap_cached_slot")
            && !prim_rs.contains("pub static mut _mi_theap_default_expansion_slot")
            && !prim_rs.contains("pub static mut _mi_theap_cached_expansion_slot")
            && !prim_rs.contains("pub static mut _mi_theap_default_key")
            && !prim_rs.contains("pub static mut _mi_theap_cached_key"),
        "internal storage should be routed through crate paths, not C-linkage extern statics"
    );
    for main_static in [
        "pub static _mi_theap_empty_wrong",
        "pub static mut theap_main",
        "pub static mut heap_main",
    ] {
        assert!(
            !init_rs.contains(main_static),
            "{main_static} should stay crate-local instead of a dynamic export"
        );
    }
}
