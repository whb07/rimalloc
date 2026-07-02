#[path = "init_markers/inventory.rs"]
mod inventory;

pub(super) fn assert_init_lifecycle_markers(init_rs: &str, prim_rs: &str, unsafe_allowlist: &str) {
    inventory::assert_init_marker_inventory(init_rs, unsafe_allowlist);
    assert!(
        init_rs.contains("pub unsafe extern \"C\" fn mi_subproc_main")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_subproc_current")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_subproc_new")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_subproc_destroy")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_subproc_visit_heaps")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_heap_main")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_is_heap_main")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_is_theap_main"),
        "subproc identity, registry, stats, and main-heap lookup markers should stay reviewable independently"
    );
    assert!(
        init_rs
            .matches("SAFETY[AUDIT:init-process-boundary]")
            .count()
            >= 16
            && init_rs
                .matches("SAFETY[AUDIT:init-process-validation-boundary]")
                .count()
                >= 1
            && init_rs.contains("pub unsafe extern \"C\" fn mi_process_init")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_process_done")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_auto_process_init")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_auto_process_done")
            && init_rs.contains("OS_PRELOADING.store(false, Ordering::Relaxed);")
            && init_rs.contains("OS_PRELOADING.store(true, Ordering::Relaxed);")
            && init_rs.contains("_mi_allocator_init(ptr::addr_of_mut!(msg))")
            && init_rs.contains("_mi_allocator_done();"),
        "process init/teardown markers should preserve preloading gates and allocator lifecycle ordering"
    );
    assert!(
        init_rs.contains("pub unsafe extern \"C\" fn mi_thread_init")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_thread_done")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_thread_done")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_theap_default_set")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_theap_cached_set")
            && init_rs.contains("fn mi_tls_slots_init()")
            && init_rs.contains("fn mi_tls_slots_done()")
            && init_rs.contains("crate::prim::_mi_theap_default_raw()"),
        "thread lifecycle, TLD, TLS slot, and thread-heap TLS markers should preserve default/cached heap association and teardown"
    );
    assert!(
        init_rs
            .matches("SAFETY[AUDIT:init-options-guarded-boundary]")
            .count()
            >= 14
            && init_rs.contains("pub unsafe extern \"C\" fn mi_theap_guarded_set_sample_rate")
            && init_rs.contains("pub unsafe extern \"C\" fn mi_theap_guarded_set_size_bound")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_theap_options_init")
            && init_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_theap_guarded_init")
            && init_rs.contains("mi_option_get_clamp(")
            && init_rs.contains("core::sync::atomic::compiler_fence"),
        "guarded and option init markers should preserve bounded thread-heap configuration writes"
    );
    assert!(
        init_rs
            .matches("SAFETY[AUDIT:init-random-cpu-boundary]")
            .count()
            >= 13
            && init_rs.contains("fn mi_detect_cpu_features()")
            && init_rs.contains("fn mi_cpuid(")
            && init_rs.contains("_mi_cpu_has_popcnt = (cpu_info[2] & (1 << 23)) != 0")
            && init_rs.contains("if fsrm {\n            unsafe { _mi_cpu_movsb_max = 127 };")
            && init_rs.contains(
                "crate::random::_mi_random_reinit_if_weak(ptr::addr_of_mut!(theap_main.random))"
            ),
        "random reseeding and CPU feature markers should preserve process-local publication"
    );
    assert!(
        init_rs
            .matches("SAFETY[AUDIT:init-platform-hooks-boundary]")
            .count()
            >= 3
            && prim_rs
                .matches("SAFETY[AUDIT:init-platform-hooks-boundary]")
                .count()
                >= 6
            && prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_process_attach()")
            && prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_process_detach()")
            && prim_rs.contains(
                "pub(crate) static _MI_PROCESS_ATTACH_CTOR: unsafe extern \"C\" fn() = mi_process_attach;"
            )
            && prim_rs.contains(
                "pub(crate) static _MI_PROCESS_DETACH_DTOR: unsafe extern \"C\" fn() = mi_process_detach;"
            )
            && init_rs.contains("core::arch::global_asm!(\".hidden _mi_theap_empty\")")
            && init_rs.contains("core::arch::global_asm!(\".hidden subproc_main\")"),
        "platform hook markers should preserve constructor/destructor callbacks and hidden local init symbols"
    );
}
