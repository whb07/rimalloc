pub(super) fn assert_heap_runtime_fast_paths(heap_rs: &str, free_rs: &str, page_rs: &str) {
    assert!(
        heap_rs.contains("pub unsafe extern \"C\" fn mi_heap_new_in_arena(")
            && heap_rs.contains("unsafe { crate::mimalloc::mi_thread_init() };")
            && heap_rs.contains("let heap_main = unsafe { mi_heap_main() };")
            && heap_rs.contains(
                "unsafe { mi_heap_zalloc(heap_main, core::mem::size_of::<mi_heap_t>()) }"
            )
            && heap_rs.contains("let theap_slot = _mi_thread_local_create();")
            && heap_rs.contains("let subproc = SubprocPtr::new_unchecked((*heap).subproc);")
            && heap_rs
                .contains("subproc_heaps::push_front(subproc, HeapPtr::new_unchecked(heap));"),
        "heap creation should keep initialization, TLS slot creation, and helper-owned subproc registration"
    );
    assert!(
        heap_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && heap_rs.contains("arena_pages.swap(ptr::null_mut(), Ordering::Relaxed)")
            && heap_rs.contains("_mi_stats_merge_into(")
            && heap_rs.contains("let subproc = SubprocPtr::new_unchecked((*heap).subproc);")
            && heap_rs.contains("subproc_heaps::remove(subproc, HeapPtr::new_unchecked(heap));")
            && heap_rs.contains("_mi_thread_local_free((*heap).theap);")
            && heap_rs.contains("mi_lock_done(ptr::addr_of_mut!((*heap).theaps_lock));")
            && heap_rs.contains("mi_free(heap.cast::<c_void>());"),
        "owned heap cleanup should keep arena-page free, stats merge, helper-owned unlink, TLS free, and lock teardown"
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
