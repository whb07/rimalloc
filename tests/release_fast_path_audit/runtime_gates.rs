use super::*;

#[test]
fn debug_assert_macros_are_release_gated_like_c_mimalloc() {
    let internal_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/internal/mod.rs");
    let source = fs::read_to_string(internal_rs).expect("read src/internal/mod.rs");

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
fn huge_allocation_stats_are_release_gated_like_c_mimalloc() {
    let page_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/page.rs");
    let source = fs::read_to_string(page_rs).expect("read src/allocator/page.rs");
    let start = source
        .find("unsafe fn mi_huge_page_alloc")
        .expect("find mi_huge_page_alloc");
    let end = source[start..]
        .find("\n}\n\n#[inline(always)]")
        .map(|offset| start + offset)
        .expect("find end of mi_huge_page_alloc");
    let body = &source[start..end];

    assert!(
        body.contains(
            "if MI_STAT > 0 {\n                __mi_stat_increase(\n                    ptr::addr_of_mut!((*theap).stats.malloc_huge),"
        ) && body.contains(
            "__mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_huge_count), 1);"
        ),
        "release huge allocation stats must compile out when MI_STAT == 0, matching C mi_theap_stat_* macros"
    );
}

#[test]
fn preloading_state_matches_c_mimalloc_lifecycle() {
    let init_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/init.rs");
    let source = fs::read_to_string(init_rs).expect("read src/allocator/init.rs");
    let preload_fn = source
        .find("pub(crate) extern \"C\" fn _mi_preloading() -> bool")
        .expect("find _mi_preloading");
    let thread_init = source
        .find("pub unsafe extern \"C\" fn mi_thread_init()")
        .expect("find mi_thread_init");
    let auto_init = source
        .find("pub(crate) unsafe extern \"C\" fn _mi_auto_process_init()")
        .expect("find _mi_auto_process_init");
    let process_init_inner = source
        .find("unsafe fn mi_process_init_inner()")
        .expect("find mi_process_init_inner");
    let process_init = source
        .find("pub unsafe extern \"C\" fn mi_process_init()")
        .expect("find mi_process_init");
    let process_done = source
        .find("pub unsafe extern \"C\" fn mi_process_done()")
        .expect("find mi_process_done");

    assert!(
        source.contains("static OS_PRELOADING: AtomicBool = AtomicBool::new(true);"),
        "_mi_preloading should default to true until process initialization, matching C"
    );
    assert!(
        source[..preload_fn].ends_with("#[inline(never)]\n")
            && source[preload_fn..].contains("OS_PRELOADING.load(Ordering::Relaxed)")
            && !source[preload_fn..preload_fn + 120].contains("false"),
        "_mi_preloading must read process state instead of returning a constant"
    );
    assert!(
        source[auto_init..].contains("OS_PRELOADING.store(false, Ordering::Relaxed);"),
        "_mi_auto_process_init should clear preloading before allocator initialization"
    );
    assert!(
        source[thread_init..auto_init].contains("mi_process_init()")
            && !source[thread_init..auto_init].contains("_mi_auto_process_init()"),
        "allocation-triggered thread init should call mi_process_init, not the constructor-only auto init path"
    );
    assert!(
        source[process_init..process_done].contains("_mi_auto_process_init()")
            && !source[process_init..process_done].contains("mi_process_init_inner()"),
        "mi_process_init should enter the same post-preload activation path as process attach"
    );
    assert!(
        source[process_init_inner..process_init].contains("_mi_process_is_initialized = true;"),
        "mi_process_init_inner should set process initialization state like C's mi_process_init_once"
    );
    assert!(
        source[process_init_inner..process_init].contains("mi_detect_cpu_features();")
            && source[process_init_inner..process_init].find("mi_detect_cpu_features();")
                < source[process_init_inner..process_init].find("mi_subproc_main_init();"),
        "process initialization should detect CPU copy/popcnt features before allocator subsystem initialization"
    );
    assert!(
        source[process_done..].contains("OS_PRELOADING.store(true, Ordering::Relaxed);"),
        "mi_process_done should restore preloading after allocator teardown"
    );
}
