use super::*;

fn assert_init_lifecycle_markers(init_rs: &str, prim_rs: &str, unsafe_allowlist: &str) {
    let marker_total = [
        "SAFETY[AUDIT:init-main-subproc-lifecycle-boundary]",
        "SAFETY[AUDIT:init-platform-tls-boundary]",
        "SAFETY[AUDIT:init-process-boundary]",
        "SAFETY[AUDIT:init-subproc-heap-association-boundary]",
        "SAFETY[AUDIT:init-subproc-identity-boundary]",
        "SAFETY[AUDIT:init-subproc-registry-boundary]",
        "SAFETY[AUDIT:init-subproc-stats-count-boundary]",
        "SAFETY[AUDIT:init-thread-heap-tls-boundary]",
        "SAFETY[AUDIT:init-thread-lifecycle-boundary]",
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
        ("init-subproc-heap-association-boundary", 36),
        ("init-subproc-identity-boundary", 16),
        ("init-subproc-registry-boundary", 32),
        ("init-subproc-stats-count-boundary", 12),
        ("init-thread-heap-tls-boundary", 56),
        ("init-thread-lifecycle-boundary", 48),
        ("init-tld-lifecycle-boundary", 7),
    ];

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/init.rs\t"))
            && !init_rs.contains("SAFETY[AUDIT:init-lifecycle-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-subproc-boundary]")
            && !init_rs.contains("SAFETY[AUDIT:init-thread-tls-boundary]")
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

pub(super) fn assert_process_lifecycle_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let init_rs = sources.init_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let page_rs = sources.page_rs.as_str();

    assert_init_lifecycle_markers(init_rs, prim_rs, unsafe_allowlist);
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc()")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_to_id")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_main")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_from_id"
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_heap_main"
            )
            && init_rs.contains("static mut subproc_main: MiSubprocPrefix")
            && init_rs.contains("static mut subprocs: *mut mi_subproc_t =")
            && init_rs.contains("core::arch::global_asm!(\".hidden subproc_main\")")
            && init_rs.contains("#[cfg_attr(target_os = \"linux\", unsafe(no_mangle))]")
            && init_rs.contains("unsafe fn mi_subproc_main_addr()")
            && init_rs.contains("unsafe fn mi_subproc_main_addr_typed() -> MainSubproc")
            && init_rs.contains(
                "MainSubproc::new_unchecked(SubprocPtr::new_unchecked(mi_subproc_main_addr()))"
            )
            && init_rs.contains(
                "unsafe fn mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> Option<HeapPtr>"
            )
            && init_rs.contains("fn _mi_subproc_to_id_typed(subproc: Option<SubprocPtr>)")
            && init_rs.contains("unsafe fn _mi_subproc_typed() -> BorrowedSubproc")
            && init_rs.contains("unsafe fn _mi_subproc_from_id_typed(subproc_id: mi_subproc_id_t)")
            && init_rs.contains(
                "unsafe fn _mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> MainHeap"
            )
            && init_rs.contains("\"lea {main}, [rip + subproc_main]\"")
            && !init_rs.contains("mi_subproc_main_ptr")
            && init_rs.contains("crate::prim::_mi_theap_default_raw()")
            && init_rs.contains("static MI_SUBPROC_TO_ID_KEEP")
            && init_rs.contains("static MI_SUBPROC_MAIN_KEEP")
            && init_rs.contains("static MI_SUBPROC_FROM_ID_KEEP")
            && init_rs.contains("static MI_SUBPROC_HEAP_MAIN_KEEP")
            && prim_rs.contains("pub unsafe fn _mi_theap_default_raw()"),
        "subproc helpers should stay as C's small local helper boundaries and inspect the raw default TLS cell"
    );
    let subproc_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_main")
        .expect("find _mi_subproc_main");
    let subproc_main_end = init_rs[subproc_main_start..]
        .find("#[used]\nstatic MI_SUBPROC_MAIN_KEEP")
        .map(|offset| subproc_main_start + offset)
        .expect("find _mi_subproc_main keep marker");
    let subproc_main_body = &init_rs[subproc_main_start..subproc_main_end];
    assert!(
        subproc_main_body.contains("unsafe { mi_subproc_main_addr_typed().ptr().as_ptr() }")
            && !subproc_main_body.contains("mi_subproc_main_init()"),
        "_mi_subproc_main should use the typed main-subproc helper without process initialization"
    );
    let subproc_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc()")
        .expect("find _mi_subproc");
    let subproc_end = init_rs[subproc_start..]
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_from_id")
        .map(|offset| subproc_start + offset)
        .expect("find _mi_subproc_from_id marker");
    let subproc_body = &init_rs[subproc_start..subproc_end];
    assert!(
        subproc_body.contains("unsafe { _mi_subproc_typed().ptr().as_ptr() }")
            && subproc_body.contains("let subproc = if theap.is_null() {")
            && subproc_body.contains("unsafe { mi_subproc_main_addr_typed().ptr() }")
            && subproc_body.contains("let tld = unsafe { TldPtr::new_unchecked((*theap).tld) };")
            && subproc_body.contains("unsafe { tld.subproc() }")
            && subproc_body.contains("unsafe { BorrowedSubproc::new(subproc) }")
            && !subproc_body.contains("_mi_subproc_main()")
            && !subproc_body.contains("mi_subproc_main_init()"),
        "_mi_subproc should parse the raw default theap into typed subproc wrappers without process initialization"
    );
    let mi_heap_main_start = init_rs
        .find("pub unsafe extern \"C\" fn mi_heap_main")
        .expect("find mi_heap_main");
    let mi_heap_main_end = init_rs[mi_heap_main_start..]
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_heap_main")
        .map(|offset| mi_heap_main_start + offset)
        .expect("find _mi_is_heap_main marker");
    let mi_heap_main_body = &init_rs[mi_heap_main_start..mi_heap_main_end];
    assert!(
        mi_heap_main_body.contains("crate::prim::_mi_theap_default_raw()")
            && mi_heap_main_body.contains("unsafe { mi_subproc_main_addr() }")
            && mi_heap_main_body
                .contains("let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && mi_heap_main_body.contains("mi_heap_main_init();")
            && mi_heap_main_body.contains("crate::internal::mi_assert_internal!(!heap.is_null());")
            && !mi_heap_main_body.contains("_mi_subproc()")
            && !mi_heap_main_body.contains("_mi_subproc_heap_main"),
        "mi_heap_main should match C's direct raw-default-theap and subproc->heap_main fast path"
    );
    let is_heap_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_heap_main")
        .expect("find _mi_is_heap_main");
    let is_heap_main_end = init_rs[is_heap_main_start..]
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_theap_main")
        .map(|offset| is_heap_main_start + offset)
        .expect("find _mi_is_theap_main marker");
    let is_heap_main_body = &init_rs[is_heap_main_start..is_heap_main_end];
    assert!(
        is_heap_main_body.contains("let subproc = unsafe { mi_heap_subproc_raw(heap) };")
            && is_heap_main_body
                .contains("let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && is_heap_main_body.contains("mi_heap_main_init();")
            && is_heap_main_body
                .contains("main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && !is_heap_main_body.contains("_mi_subproc_heap_main")
            && !is_heap_main_body.contains("if heap.is_null()"),
        "_mi_is_heap_main should match C's valid-heap contract and inline subproc->heap_main load"
    );
    let is_theap_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_theap_main")
        .expect("find _mi_is_theap_main");
    let is_theap_main_end = init_rs[is_theap_main_start..]
        .find("#[used]\nstatic MI_IS_THEAP_MAIN_KEEP")
        .map(|offset| is_theap_main_start + offset)
        .expect("find MI_IS_THEAP_MAIN_KEEP marker");
    let is_theap_main_body = &init_rs[is_theap_main_start..is_theap_main_end];
    assert!(
        is_theap_main_body.contains("crate::internal::mi_theap_is_initialized(theap)")
            && is_theap_main_body
                .contains("let heap = unsafe { crate::internal::_mi_theap_heap(theap) };")
            && is_theap_main_body.contains("let subproc = unsafe { mi_heap_subproc_raw(heap) };")
            && is_theap_main_body
                .contains("let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && is_theap_main_body.contains("mi_heap_main_init();")
            && !is_theap_main_body.contains("_mi_is_heap_main")
            && !is_theap_main_body.contains("_mi_subproc_heap_main"),
        "_mi_is_theap_main should match C's nullable-theap guard and inline subproc->heap_main load"
    );
    let mi_thread_init_start = init_rs
        .find("pub unsafe extern \"C\" fn mi_thread_init")
        .expect("find mi_thread_init");
    let mi_thread_init_end = init_rs[mi_thread_init_start..]
        .find("#[unsafe(no_mangle)]\n/// # Safety\n/// The returned opaque identifier")
        .map(|offset| mi_thread_init_start + offset)
        .expect("find mi_subproc_new marker");
    let mi_thread_init_body = &init_rs[mi_thread_init_start..mi_thread_init_end];
    assert!(
        mi_thread_init_body.contains("let heap = unsafe { mi_heap_main() };")
            && mi_thread_init_body.contains("__mi_stat_increase_mt(")
            && !mi_thread_init_body.contains("if !heap.is_null()"),
        "mi_thread_init should trust mi_heap_main after thread-heap initialization like C"
    );
    let subproc_heap_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_heap_main")
        .expect("find _mi_subproc_heap_main");
    let subproc_heap_main_end = init_rs[subproc_heap_main_start..]
        .find("#[used]\nstatic MI_SUBPROC_HEAP_MAIN_KEEP")
        .map(|offset| subproc_heap_main_start + offset)
        .expect("find _mi_subproc_heap_main keep marker");
    let subproc_heap_main_body = &init_rs[subproc_heap_main_start..subproc_heap_main_end];
    assert!(
        subproc_heap_main_body.contains(
            "let subproc = unsafe { BorrowedSubproc::new(SubprocPtr::new_unchecked(subproc)) };"
        ) && subproc_heap_main_body
            .contains("unsafe { _mi_subproc_heap_main_typed(subproc).ptr().as_ptr() }")
            && subproc_heap_main_body
                .contains("let heap = unsafe { mi_subproc_heap_main_typed(subproc) };")
            && !subproc_heap_main_body.contains("subproc.is_null()")
            && !subproc_heap_main_body.contains("ptr::null_mut()"),
        "_mi_subproc_heap_main should enter the typed subproc wrapper path without a Rust-only null fast-path branch"
    );
    let thread_done_start = init_rs
        .find("pub(crate) unsafe extern \"C\" fn _mi_thread_done")
        .expect("find _mi_thread_done");
    let thread_done_end = init_rs[thread_done_start..]
        .find("#[inline(never)]\npub(crate) extern \"C\" fn _mi_preloading")
        .map(|offset| thread_done_start + offset)
        .expect("find _mi_preloading marker");
    let thread_done_body = &init_rs[thread_done_start..thread_done_end];
    assert!(
        thread_done_body.contains("let subproc = unsafe { (*tld).subproc };")
            && thread_done_body
                .contains("let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && thread_done_body.contains("mi_heap_main_init();")
            && thread_done_body.contains("heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && !thread_done_body.contains("_mi_subproc_heap_main((*tld).subproc)")
            && !thread_done_body.contains("let heap = unsafe { _mi_subproc_heap_main"),
        "_mi_thread_done should match C's inline subproc->heap_main load instead of calling _mi_subproc_heap_main"
    );
    assert!(
        heap_rs.contains("pub unsafe extern \"C\" fn mi_heap_new_in_arena(")
            && heap_rs.contains("unsafe { crate::mimalloc::mi_thread_init() };")
            && heap_rs.contains("let heap_main = unsafe { mi_heap_main() };")
            && heap_rs.contains(
                "unsafe { mi_heap_zalloc(heap_main, core::mem::size_of::<mi_heap_t>()) }"
            )
            && heap_rs.contains("let theap_slot = _mi_thread_local_create();")
            && heap_rs.contains("mi_lock_acquire(ptr::addr_of_mut!((*subproc).heaps_lock));")
            && heap_rs.contains("(*subproc).heaps = heap;")
            && heap_rs.contains("(*subproc).heap_count.fetch_add(1, Ordering::Relaxed);")
            && heap_rs.contains("__mi_stat_increase_mt(&mut (*subproc).stats.heaps, 1);"),
        "heap creation should keep initialization, TLS slot creation, and lock-protected subproc registration"
    );
    assert!(
        heap_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && heap_rs.contains("arena_pages.swap(ptr::null_mut(), Ordering::Relaxed)")
            && heap_rs.contains("_mi_stats_merge_into(")
            && heap_rs.contains("(*subproc).heap_count.fetch_sub(1, Ordering::Relaxed);")
            && heap_rs.contains("mi_lock_acquire(ptr::addr_of_mut!((*subproc).heaps_lock));")
            && heap_rs.contains("_mi_thread_local_free((*heap).theap);")
            && heap_rs.contains("mi_lock_done(ptr::addr_of_mut!((*heap).theaps_lock));")
            && heap_rs.contains("mi_free(heap.cast::<c_void>());"),
        "owned heap cleanup should keep arena-page free, stats merge, locked unlink, TLS free, and lock teardown"
    );
    assert!(
        heap_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_init_theap")
            && heap_rs.contains("static MI_HEAP_INIT_THEAP_KEEP")
            && heap_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_theap_get_or_init"
            )
            && heap_rs.contains("static MI_HEAP_THEAP_GET_OR_INIT_KEEP")
            && heap_rs.contains("theap = unsafe { mi_heap_init_theap(heap) };")
            && heap_rs.contains("_mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()")
            && heap_rs.contains(
                "_mi_thread_local_set((*heap).theap, MI_THEAP_TLS_BUSY.cast::<c_void>())"
            )
            && heap_rs.contains("theap = _mi_theap_create(heap, (*_mi_theap_default_safe()).tld);")
            && heap_rs.contains("_mi_thread_local_set((*heap).theap, theap.cast::<c_void>());")
            && heap_rs.contains("unsafe { _mi_theap_cached_set(theap) };"),
        "heap theap initialization should keep C's local helper boundaries"
    );
    assert!(
        heap_rs.contains("pub unsafe extern \"C\" fn mi_unsafe_heap_page_is_under_utilized(")
            && heap_rs.contains("BorrowedHeap::from_lookup_raw(heap)")
            && heap_rs.contains("let page = _mi_safe_ptr_page(p);")
            && heap_rs.contains("let used = crate::page::mi_page_used_count(page_raw);")
            && heap_rs.contains("let capacity = crate::page::mi_page_capacity_count(page_raw);")
            && heap_rs.contains("let reserved = crate::page::mi_page_reserved_count(page_raw);")
            && heap_rs.contains("let Some(page_heap) = HeapPtr::new(mi_page_heap(page_raw))")
            && heap_rs
                .contains("perc_threshold >= ((100usize * used.as_usize()) / capacity.as_usize())"),
        "under-utilized page inspection should keep typed page counters and optional heap ownership checks"
    );
    assert!(
        !free_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_free_generic"),
        "_mi_free_generic should not be a dynamic export"
    );
    assert!(
        free_rs.contains("#[inline(never)]")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_free_generic(")
            && free_rs.contains("if is_local {\n            mi_free_generic_local(page, p);")
            && free_rs.contains("} else {\n            mi_free_generic_mt(page, p);")
            && free_rs.contains("static MI_FREE_GENERIC_KEEP")
            && free_rs.contains("    _mi_free_generic;"),
        "_mi_free_generic should keep C's local runtime-integration dispatch boundary"
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
