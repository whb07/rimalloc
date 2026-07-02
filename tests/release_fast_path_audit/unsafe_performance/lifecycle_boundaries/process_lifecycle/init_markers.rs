pub(super) fn assert_init_lifecycle_markers(init_rs: &str, prim_rs: &str, unsafe_allowlist: &str) {
    let marker_total = [
        "SAFETY[AUDIT:init-main-subproc-lifecycle-boundary]",
        "SAFETY[AUDIT:init-platform-tls-boundary]",
        "SAFETY[AUDIT:init-process-boundary]",
        "SAFETY[AUDIT:init-subproc-prefix-boundary]",
        "SAFETY[AUDIT:init-subproc-main-heap-load-boundary]",
        "SAFETY[AUDIT:init-subproc-main-heap-export-boundary]",
        "SAFETY[AUDIT:init-subproc-current-main-heap-boundary]",
        "SAFETY[AUDIT:init-subproc-main-heap-predicate-boundary]",
        "SAFETY[AUDIT:init-subproc-heap-visit-boundary]",
        "SAFETY[AUDIT:init-subproc-identity-boundary]",
        "SAFETY[AUDIT:init-subproc-create-boundary]",
        "SAFETY[AUDIT:init-subproc-metadata-destroy-boundary]",
        "SAFETY[AUDIT:init-subproc-list-unlink-boundary]",
        "SAFETY[AUDIT:init-subproc-destroy-loop-boundary]",
        "SAFETY[AUDIT:init-subproc-registry-teardown-boundary]",
        "SAFETY[AUDIT:init-subproc-public-destroy-boundary]",
        "SAFETY[AUDIT:init-subproc-stats-count-boundary]",
        "SAFETY[AUDIT:init-thread-identity-boundary]",
        "SAFETY[AUDIT:init-thread-done-entry-boundary]",
        "SAFETY[AUDIT:init-thread-done-dispatch-boundary]",
        "SAFETY[AUDIT:init-thread-init-default-heap-boundary]",
        "SAFETY[AUDIT:init-thread-done-cleanup-boundary]",
        "SAFETY[AUDIT:init-thread-done-tls-reset-boundary]",
        "SAFETY[AUDIT:init-thread-init-entry-boundary]",
        "SAFETY[AUDIT:init-thread-heap-empty-sentinel-boundary]",
        "SAFETY[AUDIT:init-thread-heap-main-tls-boundary]",
        "SAFETY[AUDIT:init-thread-heap-main-static-boundary]",
        "SAFETY[AUDIT:init-thread-default-heap-association-boundary]",
        "SAFETY[AUDIT:init-thread-cached-heap-association-boundary]",
        "SAFETY[AUDIT:init-thread-heap-safe-access-boundary]",
        "SAFETY[AUDIT:init-options-guarded-boundary]",
        "SAFETY[AUDIT:init-random-cpu-boundary]",
        "SAFETY[AUDIT:init-tld-lifecycle-boundary]",
        "SAFETY[AUDIT:init-platform-hooks-boundary]",
    ]
    .into_iter()
    .map(|marker| init_rs.matches(marker).count())
    .sum::<usize>();
    let split_marker_minimums = [
        ("init-main-subproc-lifecycle-boundary", 6),
        ("init-platform-tls-boundary", 17),
        ("init-subproc-prefix-boundary", 1),
        ("init-subproc-main-heap-load-boundary", 5),
        ("init-subproc-main-heap-export-boundary", 8),
        ("init-subproc-current-main-heap-boundary", 7),
        ("init-subproc-main-heap-predicate-boundary", 11),
        ("init-subproc-heap-visit-boundary", 4),
        ("init-subproc-identity-boundary", 16),
        ("init-subproc-create-boundary", 6),
        ("init-subproc-metadata-destroy-boundary", 3),
        ("init-subproc-list-unlink-boundary", 1),
        ("init-subproc-destroy-loop-boundary", 14),
        ("init-subproc-registry-teardown-boundary", 2),
        ("init-subproc-public-destroy-boundary", 6),
        ("init-subproc-stats-count-boundary", 12),
        ("init-thread-identity-boundary", 1),
        ("init-thread-done-entry-boundary", 6),
        ("init-thread-done-dispatch-boundary", 10),
        ("init-thread-init-default-heap-boundary", 4),
        ("init-thread-done-cleanup-boundary", 16),
        ("init-thread-done-tls-reset-boundary", 5),
        ("init-thread-init-entry-boundary", 6),
        ("init-thread-heap-empty-sentinel-boundary", 3),
        ("init-thread-heap-main-tls-boundary", 17),
        ("init-thread-heap-main-static-boundary", 1),
        ("init-thread-default-heap-association-boundary", 17),
        ("init-thread-cached-heap-association-boundary", 11),
        ("init-thread-heap-safe-access-boundary", 7),
        ("init-tld-lifecycle-boundary", 7),
    ];

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/init.rs\t"))
            && !init_rs.contains("SAFETY[AUDIT:init-lifecycle-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-heap-association-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-registry-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-tls-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-heap-tls-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-lifecycle-boundary]")
            && marker_total >= 277,
        "init lifecycle unsafe boundaries should stay split into focused operation markers without a migration allowlist entry"
    );
    for (audit_id, minimum) in split_marker_minimums {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = init_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "init unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
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
            >= 17
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
