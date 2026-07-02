pub(super) fn assert_subproc_fast_paths(
    init_rs: &str,
    heap_rs: &str,
    free_rs: &str,
    page_rs: &str,
) {
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
