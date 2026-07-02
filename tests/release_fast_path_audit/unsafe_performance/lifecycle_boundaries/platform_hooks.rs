use super::*;

pub(super) fn assert_platform_hook_boundaries(sources: &UnsafePerformanceSources) {
    let init_rs = sources.init_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();

    assert!(
    prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_process_attach()")
        && prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_process_detach()")
        && prim_rs.contains(
            "pub(crate) static _MI_PROCESS_ATTACH_CTOR: unsafe extern \"C\" fn() = mi_process_attach;"
        )
        && prim_rs.contains(
            "pub(crate) static _MI_PROCESS_DETACH_DTOR: unsafe extern \"C\" fn() = mi_process_detach;"
        ),
    "ELF process lifecycle hooks should keep C's constructor and destructor helper names"
);
    assert!(
        prim_rs.contains("pub(crate) use unix::{")
            && !prim_rs.contains("pub fn _mi_prim_mem_init(")
            && !prim_rs.contains("pub fn _mi_prim_alloc(")
            && !prim_rs.contains("pub fn _mi_prim_thread_yield(")
            && !prim_rs.contains("pub fn _mi_theap_empty_get(")
            && !prim_rs.contains("pub fn _mi_heap_theap_get_or_init(")
            && !prim_rs.contains("pub fn _mi_heap_theap_get_peek("),
        "Unix primitive helpers should be routed through crate paths, not extern declarations"
    );
    assert!(
        !init_rs.contains("#[unsafe(no_mangle)]\npub static _mi_page_empty"),
        "_mi_page_empty should stay crate-local so page-empty tables use relative relocations"
    );
    assert!(
        init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static _mi_theap_empty")
            && init_rs.contains("core::arch::global_asm!(\".hidden _mi_theap_empty\")")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static _mi_theap_empty"),
        "_mi_theap_empty needs a stable hidden local link name for TLS initialization without becoming a public dynamic ABI symbol"
    );
}
