use super::*;

pub(super) fn assert_options_and_guarded_boundaries(sources: &UnsafePerformanceSources) {
    let init_rs = sources.init_rs.as_str();

    assert!(
        init_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_options_init")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_guarded_init"
            )
            && init_rs.contains("core::sync::atomic::compiler_fence"),
        "_mi_theap_init should keep C's options and guarded-init helper call boundaries"
    );
    assert!(
        init_rs.contains("#[inline(always)]\nunsafe fn _mi_thread_init_theap_default()"),
        "thread default-heap initialization should stay inlined like C's static helper"
    );
    let thread_default_start = init_rs
        .find("#[inline(always)]\nunsafe fn _mi_thread_init_theap_default()")
        .expect("find _mi_thread_init_theap_default");
    let thread_default_end = init_rs[thread_default_start..]
        .find("\n}\n\n#[unsafe(no_mangle)]")
        .map(|offset| thread_default_start + offset)
        .expect("find end of _mi_thread_init_theap_default");
    let thread_default_body = &init_rs[thread_default_start..thread_default_end];
    assert!(
        thread_default_body.contains("if mi_is_main_thread_fast() {")
            && thread_default_body.contains("let tld = mi_tld_alloc_non_main(")
            && !thread_default_body.contains("_mi_is_main_thread()"),
        "thread default-heap initialization should inline C's main-thread predicate body and avoid a second main-thread check on the non-main branch"
    );
    assert!(
        init_rs.contains("crate::threadlocal::_mi_thread_locals_init();")
            && init_rs.find("crate::threadlocal::_mi_thread_locals_init();")
                < init_rs.find("let _ = crate::page_map::_mi_page_map_init();"),
        "process initialization should initialize dynamic TLS locks before page-map/TLS key creation"
    );
    let tld_alloc_start = init_rs
        .find("#[inline(always)]\nfn mi_tld_alloc()")
        .expect("find mi_tld_alloc");
    let tld_alloc_end = init_rs[tld_alloc_start..]
        .find("\n}\n\n#[inline(never)]")
        .map(|offset| tld_alloc_start + offset)
        .expect("find end of mi_tld_alloc");
    let tld_alloc_body = &init_rs[tld_alloc_start..tld_alloc_end];
    assert!(
        tld_alloc_body.contains("ptr::addr_of_mut!(subproc_main)")
            && !tld_alloc_body.contains("mi_subproc_main_init()")
            && !tld_alloc_body.contains("mi_subproc_main_ptr()"),
        "thread-local data allocation should use the initialized main subproc directly like C's mi_tld_alloc"
    );
}
