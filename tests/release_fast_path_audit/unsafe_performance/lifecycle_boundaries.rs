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

#[test]
fn lifecycle_and_local_boundary_helpers_stay_classified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");
    let arena_heap_pages_rs = fs::read_to_string(root.join("src/allocator/arena/heap_pages.rs"))
        .expect("read src/allocator/arena/heap_pages.rs");
    let atomic_types =
        fs::read_to_string(root.join("src/types/atomic.rs")).expect("read core_types/atomic.rs");
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let heap_rs =
        fs::read_to_string(root.join("src/allocator/heap.rs")).expect("read src/allocator/heap.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let internal =
        fs::read_to_string(root.join("src/internal/mod.rs")).expect("read src/internal/mod.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let page_flags_rs = fs::read_to_string(root.join("src/allocator/page/flags.rs"))
        .expect("read src/allocator/page/flags.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let stats_rs = fs::read_to_string(root.join("src/allocator/stats.rs"))
        .expect("read src/allocator/stats.rs");
    let theap_rs = fs::read_to_string(root.join("src/allocator/thread_heap.rs"))
        .expect("read src/allocator/thread_heap.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/mod.rs\t"))
            && prim_rs
                .matches("SAFETY[AUDIT:platform-tls-primitives]")
                .count()
                >= 54,
        "platform TLS primitive boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert_init_lifecycle_markers(&init_rs, &prim_rs, &unsafe_allowlist);
    assert!(
        prim_rs.contains("pub const MI_TLS_MODEL_THREAD_LOCAL")
            && prim_rs.contains("pub const MI_TLS_MODEL_FIXED_SLOT")
            && prim_rs.contains("pub const MI_TLS_MODEL_DYNAMIC_WIN32")
            && prim_rs.contains("pub const MI_TLS_MODEL_DYNAMIC_PTHREADS")
            && prim_rs.contains("pub const MI_THEAP_INITASNULL")
            && prim_rs.contains("pub const MI_TLS_MODEL_FIXED_SLOT_DEFAULT: usize = 108;")
            && prim_rs.contains("pub const MI_TLS_MODEL_FIXED_SLOT_CACHED: usize = 109;")
            && prim_rs.contains("pub const MI_TLS_EXPANSION_SLOT: usize")
            && prim_rs.contains("fn mi_prim_apple_tls_slot(slot: usize) -> *mut c_void")
            && prim_rs.contains("fn _mi_tls_theap_default_get() -> *mut mi_theap_t;")
            && prim_rs.contains("fn _mi_tls_theap_cached_get() -> *mut mi_theap_t;")
            && prim_rs.contains("__mi_theap_default@GOTTPOFF")
            && prim_rs.contains("__mi_theap_cached@GOTTPOFF")
            && prim_rs.contains("\"lea {empty}, [rip + _mi_theap_empty]\""),
        "platform TLS primitive layer should preserve model constants, Apple fixed-slot path, Linux C shim declarations, and x86_64 TLS assembly"
    );
    for marker in [
        "pub unsafe fn _mi_theap_default_raw(",
        "pub unsafe fn _mi_theap_default(",
        "pub unsafe fn _mi_theap_cached(",
        "pub(crate) fn mi_prim_thread_local_default_set(",
        "pub(crate) fn mi_prim_thread_local_cached_set(",
        "pub(crate) unsafe fn _mi_page_associated_theap_peek(",
        "pub(crate) unsafe extern \"C\" fn _mi_allocator_init(",
        "pub(crate) unsafe extern \"C\" fn _mi_allocator_done(",
    ] {
        let item = source_item("src/platform/mod.rs", &prim_rs, marker);
        assert_no_rust_heap_constructs("src/platform/mod.rs", marker, item);
    }
    let default_raw = source_item(
        "src/platform/mod.rs",
        &prim_rs,
        "pub unsafe fn _mi_theap_default_raw(",
    );
    assert!(
        default_raw.contains("unsafe { mi_linux_x64_theap_default_get() }")
            && default_raw.contains("unsafe { _mi_tls_theap_default_get() }")
            && default_raw.contains("mi_prim_tls_slot(MI_TLS_MODEL_FIXED_SLOT_DEFAULT)")
            && default_raw
                .contains("_mi_thread_local_get(_mi_theap_default_key) as *mut mi_theap_t"),
        "raw default thread-heap lookup should preserve Linux fast path, C shim fallback, fixed-slot path, and dynamic key path"
    );
    let cached = source_item(
        "src/platform/mod.rs",
        &prim_rs,
        "pub unsafe fn _mi_theap_cached(",
    );
    assert!(
        cached.contains("mi_linux_x64_theap_cached_get()")
            && cached.contains("unsafe { _mi_tls_theap_cached_get() }")
            && cached.matches("unsafe { __mi_theap_empty() }").count() >= 4
            && cached.contains("mi_prim_tls_slot(MI_TLS_MODEL_FIXED_SLOT_CACHED)")
            && cached.contains("_mi_thread_local_get(_mi_theap_cached_key) as *mut mi_theap_t"),
        "cached thread-heap lookup should preserve Linux fast path, empty fallback, fixed-slot path, and dynamic key path"
    );
    let page_assoc = source_item(
        "src/platform/mod.rs",
        &prim_rs,
        "pub(crate) unsafe fn _mi_page_associated_theap_peek(",
    );
    assert!(
        page_assoc.contains("let heap = unsafe { (*page).heap };")
            && page_assoc.contains("crate::init::mi_theap_main_tls_peek()")
            && page_assoc.contains("unsafe { _mi_heap_theap_peek(heap) }")
            && page_assoc.contains("ThreadHeapPtr::new(theap)"),
        "page-associated thread-heap lookup should keep non-initializing heap peeks and typed pointer wrapping"
    );

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
    super::threadlocal_boundaries::assert_dynamic_threadlocal_boundaries(&threadlocal_rs);
    assert!(
        init_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_cached_set(")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_default_safe("
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_main_safe("
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_empty_get("
            )
            && init_rs.contains("static MI_THEAP_CACHED_SET_KEEP")
            && init_rs.contains("static MI_THEAP_DEFAULT_SAFE_KEEP")
            && init_rs.contains("static MI_THEAP_MAIN_SAFE_KEEP")
            && init_rs.contains("static MI_THEAP_EMPTY_GET_KEEP"),
        "thread-heap accessors should keep C's local helper boundaries"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_thread_id(")
            && init_rs.contains("static MI_THREAD_ID_KEEP"),
        "_mi_thread_id should keep C's local helper boundary"
    );
    assert!(
        alloc_rs.contains(
            "#[inline(always)]\npub(crate) unsafe extern \"C\" fn _mi_theap_malloc_zero_ex"
        ) && alloc_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_malloc_zero")
            && alloc_rs.contains("static MI_THEAP_MALLOC_ZERO_EX_KEEP")
            && alloc_rs.contains("static MI_THEAP_MALLOC_ZERO_KEEP"),
        "_mi_theap_malloc_zero_ex should keep C's force-inline body and both allocation helpers should retain local definitions"
    );
    assert!(
        alloc_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn mi_theap_try_new")
            && alloc_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_try_new")
            && alloc_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_try_new")
            && alloc_rs.contains("mi_heap_try_new_typed(heap, size, false)")
            && alloc_rs.contains("mi_try_new(size, true)")
            && alloc_rs.contains("static MI_THEAP_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_HEAP_TRY_NEW_KEEP"),
        "C++ new retry paths should keep C's separate theap/default-heap/explicit-heap helper boundaries"
    );
    assert!(
        page_rs.contains(
            "#[inline(never)]\npub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
        )
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_page_reclaim_raw")
            && page_rs.contains("static MI_THEAP_PAGE_RECLAIM_KEEP")
            && page_rs.contains("mi_theap_page_reclaim_raw;")
            && page_rs.contains("_mi_theap_page_reclaim(wrapper, theap_wrapper);")
            && page_rs
                .contains("let wrapper = crate::core_types::OwnedAbandonedPage::new_unchecked(page_ptr);")
            && !page_rs.contains("pub unsafe extern \"C\" fn _mi_theap_page_reclaim")
            && atomic_types.contains("pub(crate) unsafe fn load(self) -> PageThreadId")
            && atomic_types.contains("pub(crate) unsafe fn store_owner(self, owner: PageOwnerId)")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_owner(\n        self,\n        expected: &mut PageThreadId,")
            && internal.contains("let atomic_tid = crate::core_types::AtomicPageThreadId(\n            crate::core_types::AtomicUsizeField(")
            && internal.contains("atomic_tid.load().raw()")
            && internal.contains("let mut old = atomic_tid.load();")
            && internal.contains("atomic_tid.compare_exchange_owner(&mut old, new)")
            && internal.contains("let new = old.with_owner(crate::core_types::PageOwnerId::AbandonedMapped);")
            && internal.contains("let new = old.with_owner(crate::core_types::PageOwnerId::Abandoned);")
            && page_flags_rs
                .contains("let Some(flag) = crate::core_types::PageFlags::new(newflag) else")
            && page_flags_rs.contains("let old = atomic_tid.set_flags(flag, set);")
            && page_flags_rs.contains("let tid = atomic_tid.load();")
            && page_flags_rs.contains("let mut xtid_old = atomic_tid.load();")
            && page_flags_rs.contains("let xtid = xtid_old.with_owner(owner);")
            && page_flags_rs.contains("atomic_tid.compare_exchange_owner(&mut xtid_old, xtid)")
            && arena_rs.contains("unsafe { atomic_tid.store_owner(crate::core_types::PageOwnerId::AbandonedMapped) };")
            && !internal.contains("atomic_tid.load_relaxed().raw()")
            && !internal.contains("atomic_tid.compare_exchange_release(&mut old, new)")
            && !page_flags_rs.contains("atomic_tid.compare_exchange_release(&mut xtid_old, xtid)")
            && !arena_rs.contains("atomic_tid.set_owner_release(crate::core_types::PageOwnerId::AbandonedMapped)")
            && !internal.contains("mi_atomic_or_relaxed(&mut (*page).xthread_id")
            && !internal.contains("mi_atomic_and_relaxed(&mut (*page).xthread_id")
            && !page_flags_rs.contains("mi_atomic_cas_weak_release(&mut (*page).xthread_id")
            && !page_flags_rs.contains("mi_atomic_or_relaxed(&mut (*page).xthread_id")
            && !page_flags_rs.contains("mi_atomic_and_relaxed(&mut (*page).xthread_id")
            && !arena_rs.contains("MI_THREADID_ABANDONED_MAPPED | flags")
            && page_rs.contains(
                "let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ));"
            )
            && page_rs.contains("let remote = atomic_tf.collect_acq_rel();")
            && page_rs.contains("remote.collect_into_local(page_wrapper, local);")
            && !page_rs.contains("atomic_tf.take_head_preserving_owner()")
            && atomic_types.contains("pub(crate) fn push_remote_acq_rel(")
            && atomic_types.contains(
                "pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList"
            )
            && atomic_types.contains("pub(crate) unsafe fn claim_acq_rel(self) -> bool")
            && atomic_types.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool"
            )
            && atomic_types.contains("fn field(self) -> AtomicUsizeField")
            && atomic_types.contains("self.field().compare_exchange_weak(")
            && atomic_types.contains("self.field().fetch_or(1, Ordering::AcqRel)")
            && atomic_types.contains("AtomicUsize::from_ptr(self.0)")
            && !atomic_types.contains("self.0.cast::<core::sync::atomic::AtomicUsize>()")
            && !atomic_types.contains("as_atomic(")
            && !atomic_types.contains("atomic_ref(")
            && !atomic_types.contains("-> &'static Atomic")
            && atomic_types.contains("Atomic access to a raw `usize` field.")
            && atomic_types.contains("Atomic access to a raw `i64` field.")
            && atomic_types.contains("Atomic access to a raw pointer field.")
            && internal.contains("crate::core_types::ThreadFreeWord(tf).head_raw()")
            && internal.contains("crate::core_types::ThreadFreeWord(tf).is_owned()")
            && internal.contains("crate::core_types::ThreadFreeWord::from_parts(")
            && internal.contains("pub unsafe fn mi_page_thread_free(page: *const mi_page_t) -> *mut mi_block_t")
            && internal.contains("atomic_tf.load_relaxed().head_raw()")
            && internal.contains("pub unsafe fn mi_page_is_owned(page: *const mi_page_t) -> bool")
            && internal.contains("atomic_tf.load_relaxed().is_owned()")
            && internal.contains("let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),")
            && internal.contains("atomic_tf.claim_acq_rel()")
            && !internal.contains("atomic_tf.claim_ownership()")
            && arena_rs.contains("(*page).xthread_free = crate::core_types::ThreadFreeWord::from_parts(\n            None,\n            crate::core_types::ThreadFreeOwner::Unowned,")
            && !arena_rs.contains("(*page).xthread_free = 0")
            && free_rs.contains(
                "let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ));"
            )
            && free_rs.contains("match atomic_tf.push_remote_acq_rel(&mut tf_old, bp)")
            && free_rs.contains("while !atomic_tf.release_acq_rel(&mut tf_expect)")
            && free_rs.contains("tf_expect = atomic_tf.load_relaxed();")
            && !free_rs.contains("atomic_tf.compare_exchange_push(")
            && !free_rs.contains("atomic_tf.compare_exchange_release_ownership(")
            && !free_rs.contains("mi_tf_create(expected_thread_free")
            && arena_rs.contains(
                "let atomic_tf =\n        crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(unsafe {\n            ptr::addr_of_mut!((*page).xthread_free)\n        }));"
            )
            && arena_rs.contains("let mut tf_old = atomic_tf.load_relaxed();")
            && arena_rs.contains("while tf_old.head().is_some()")
            && arena_rs.contains("if unsafe { atomic_tf.release_acq_rel(&mut tf_old) }")
            && !arena_rs.contains("atomic_tf.compare_exchange_release_ownership(")
            && !arena_rs.contains("mi_tf_create(ptr::null_mut(), false)")
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "mi_page_queue_push_at_end_typed(\n            crate::core_types::ThreadHeapLocal::new_unchecked"
            )
            && page_rs.contains("crate::core_types::NormalPageQueue")
            && page_rs.contains("crate::core_types::ActivePage"),
        "_mi_theap_page_reclaim should keep C's local abandoned-page reclaim helper boundary while collecting remote frees through typed wrappers"
    );
    assert!(
        page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_page_free_list_extend")
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_page_extend_free")
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_page_fresh_alloc")
            && page_rs.contains("static MI_PAGE_FREE_LIST_EXTEND_KEEP")
            && page_rs.contains("static MI_PAGE_EXTEND_FREE_KEEP")
            && page_rs.contains("static MI_PAGE_FRESH_ALLOC_KEEP")
            && page_rs.contains("mi_page_free_list_extend(page, bsize, extend);")
            && page_rs.contains("if !mi_page_extend_free(theap, page)")
            && page_rs.contains("let block_size = mi_page_queue_block_size_raw(pq);")
            && page_rs.contains("let page = mi_page_fresh_alloc(theap, pq, block_size, 0);")
            && page_rs.contains(
                "crate::internal::mi_assert_internal!(block_size == mi_page_block_size(page));"
            )
            && page_rs
                .contains("let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);"),
        "fresh-page allocation and page free-list extension should keep C's local helper boundaries"
    );
    assert!(
        free_rs.contains(
            "unsafe fn mi_abandoned_page_try_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,"
        )
            && free_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn mi_page_usable_aligned_size_of"
            )
            && free_rs.contains("static MI_ABANDONED_PAGE_TRY_RECLAIM_KEEP")
            && free_rs.contains("static MI_PAGE_USABLE_ALIGNED_SIZE_OF_KEEP")
            && free_rs.contains("&& mi_abandoned_page_try_reclaim(page, reclaim_on_free)")
            && free_rs.contains("_mi_theap_page_reclaim(\n            page,")
            && free_rs.contains("crate::core_types::OwnedAbandonedPage::new_unchecked")
            && free_rs.contains("unsafe fn mi_free_try_collect_mt_typed")
            && !free_rs.contains("_mi_theap_page_reclaim(theap, page)")
            && free_rs.contains("mi_page_usable_aligned_size_of_typed(page, p)"),
        "abandoned-page reclaim should route internal abandoned-page collection through typed page/heap witnesses"
    );
    assert!(
        theap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_area_visit_blocks"
        ) && theap_rs.contains("static MI_THEAP_AREA_VISIT_BLOCKS_KEEP")
            && theap_rs.contains("visitor: BlockVisitCallback")
            && theap_rs.contains("arg: BlockVisitCallbackArg")
            && theap_rs.contains("let heap = HeapPtr::new_unchecked(mi_page_heap(page));")
            && callbacks_rs
                .contains("pub(crate) unsafe fn invoke(self, heap: HeapPtr, arg: HeapVisitCallbackArg) -> bool")
            && callbacks_rs.contains("heap: HeapPtr,\n        area: *const mi_heap_area_t,")
            && callbacks_rs.contains("fun(heap.as_ptr(), area, block, block_size, arg.as_ptr())")
            && init_rs.contains("ok = visitor.invoke(HeapPtr::new_unchecked(heap), arg);")
            && theap_rs.contains("mi_get_fast_divisor(bsize, &mut magic, &mut shift);")
            && arena_heap_pages_rs.contains(
                "_mi_theap_area_visit_blocks(ptr::addr_of!(area), page, (*vinfo).visitor, (*vinfo).arg)"
            )
            && !arena_heap_pages_rs.contains("unsafe fn mi_theap_area_visit_blocks("),
        "heap and arena block visitation should share C's theap area block visitor helper"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_area_visitor")
            && theap_rs.contains(
                "static MI_THEAP_AREA_VISITOR_KEEP: mi_theap_area_visit_fun = mi_theap_area_visitor;"
            )
            && theap_rs.contains("Some(mi_theap_area_visitor)"),
        "mi_theap_area_visitor should keep C's local theap area visitor callback boundary"
    );
    assert!(
        arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_alloc")
            && arena_rs.contains("static MI_ARENAS_PAGE_ALLOC_KEEP")
            && page_rs.contains(
                "let page = _mi_arenas_page_alloc(theap, block_size, page_alignment.get());",
            ),
        "arena page allocation should keep C's _mi_arenas_page_alloc local helper boundary"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_print_visitor")
            && stats_rs
                .contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_aggregate_visitor")
            && stats_rs.contains("static MI_HEAP_PRINT_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_print_visitor;")
            && stats_rs.contains(
                "static MI_HEAP_AGGREGATE_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_aggregate_visitor;"
            )
            && stats_rs.contains("Some(mi_heap_print_visitor as mi_heap_visit_fun)")
            && stats_rs.contains("Some(mi_heap_aggregate_visitor as mi_heap_visit_fun)"),
        "heap stats visitors should keep C's local callback helper boundaries"
    );
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
        theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_incref")
            && theap_rs.contains("unsafe fn _mi_theap_incref_typed(theap: ThreadHeapPtr)")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_decref")
            && theap_rs.contains("#[inline(always)]\nunsafe fn mi_theap_decref_inline(theap: ThreadHeapPtr)")
            && theap_rs.contains("mi_theap_decref_inline(ThreadHeapPtr::new_unchecked(theap));\n        true")
            && theap_rs.contains("unsafe fn _mi_theap_delete_typed(theap: ThreadHeapPtr, acquire_tld_theaps_lock: bool)")
            && theap_rs.contains("match theap.state()")
            && theap_rs.contains("ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap)")
            && theap_rs.contains("ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return")
            && !theap_rs.contains("    _mi_theap_decref(theap);\n    true"),
        "_mi_theap_free should inline C's decref tail while incref/decref/delete use typed thread-heap wrappers below their raw shims"
    );
    assert!(
        theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_random_next")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_delete")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_create")
            && !theap_rs.contains("static MI_THEAP_RANDOM_NEXT_KEEP")
            && theap_rs.contains("static MI_THEAP_CREATE_KEEP")
            && theap_rs.contains("static MI_THEAP_DELETE_KEEP"),
        "theap random/create/delete helpers should keep C's local helper boundaries without a random-next function pointer anchor"
    );
    assert!(
        !unsafe_allowlist.contains("src/allocator/heap.rs")
            && heap_rs
                .matches("SAFETY[AUDIT:heap-lifecycle-boundary]")
                .count()
                >= 70,
        "heap lifecycle unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/thread_heap.rs\t"))
            && theap_rs
                .matches("SAFETY[AUDIT:thread-heap-lifecycle]")
                .count()
                >= 70,
        "thread-heap lifecycle unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
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
    assert!(
        init_rs.contains(
            "core::arch::global_asm!(\".hidden _mi_cpu_movsb_max\", \".hidden _mi_cpu_stosb_max\")"
        ) && init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static mut _mi_cpu_movsb_max")
            && init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static mut _mi_cpu_stosb_max")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static mut _mi_cpu_movsb_max")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static mut _mi_cpu_stosb_max"),
        "CPU copy thresholds should use stable hidden C names like C without becoming public dynamic ABI symbols"
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
            && init_rs.contains("if fsrm {\n            unsafe { _mi_cpu_movsb_max = 127 };")
            && init_rs.contains(
                "if fsrs || (amd && fsrm) {\n            unsafe { _mi_cpu_stosb_max = 127 };"
            ),
        "CPU feature detection should initialize C's popcnt and short rep movsb/stosb thresholds"
    );
    assert!(
        internal.contains("\"rep stosb\"")
            && internal.contains("pub unsafe fn mi_rep_stosb(dst: *mut c_void, val: u8, n: usize)")
            && internal.contains("if n <= mi_cpu_stosb_max() {\n                mi_rep_stosb(dst, val as u8, n);\n                return;\n            }"),
        "_mi_memset should keep C's CPU-thresholded rep stosb shape instead of always lowering to external memset"
    );
    for internal_static in [
        "_mi_cpu_movsb_max",
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
        !internal.contains("pub static mut _mi_cpu_movsb_max")
            && !internal.contains("pub static mut _mi_cpu_stosb_max")
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
