pub(super) struct BoundarySources<'a> {
    pub(super) page_map: &'a str,
    pub(super) unsafe_allowlist: &'a str,
    pub(super) alloc_rs: &'a str,
    pub(super) arena_rs: &'a str,
    pub(super) arena_debug_rs: &'a str,
    pub(super) arena_heap_pages_rs: &'a str,
    pub(super) arena_meta_rs: &'a str,
    pub(super) deferred_rs: &'a str,
    pub(super) free_rs: &'a str,
    pub(super) heap_rs: &'a str,
    pub(super) init_rs: &'a str,
    pub(super) internal: &'a str,
    pub(super) page_rs: &'a str,
    pub(super) theap_rs: &'a str,
}

pub(super) fn assert_page_map_and_deferred_boundaries(sources: BoundarySources<'_>) {
    let BoundarySources {
        page_map,
        unsafe_allowlist,
        alloc_rs,
        arena_rs,
        arena_debug_rs,
        arena_heap_pages_rs,
        arena_meta_rs,
        deferred_rs,
        free_rs,
        heap_rs,
        init_rs,
        internal,
        page_rs,
        theap_rs,
    } = sources;
    for internal_name in [
        "pub static mut _mi_page_map",
        "pub unsafe extern \"C\" fn _mi_page_map_init",
        "pub unsafe extern \"C\" fn _mi_page_map_register",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister_range",
        "pub unsafe extern \"C\" fn _mi_page_map_unsafe_destroy",
        "pub unsafe extern \"C\" fn _mi_safe_ptr_page",
    ] {
        let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
        assert!(
            !page_map.contains(&no_mangle_export),
            "{internal_name} should stay crate-internal instead of a dynamic export"
        );
    }
    for (audit_id, expected_count) in [
        ("page-map-global-boundary", 12),
        ("page-map-32bit-commit-bitmap-boundary", 8),
        ("page-map-32bit-bootstrap-boundary", 9),
        ("page-map-top-level-lazy-commit-boundary", 4),
        ("page-map-global-bootstrap-boundary", 12),
        ("page-map-initial-submap-boundary", 4),
        ("page-map-null-sentinel-boundary", 8),
        ("page-map-teardown-boundary", 13),
        ("page-map-commit-boundary", 5),
        ("page-map-page-register-boundary", 14),
        ("page-map-range-register-boundary", 6),
        ("page-map-page-unregister-boundary", 15),
        ("page-map-range-unregister-boundary", 10),
        ("page-map-submap-publication-boundary", 9),
        ("page-map-range-primitive-boundary", 5),
        ("page-map-lookup-boundary", 19),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            page_map.matches(&marker).count(),
            expected_count,
            "page-map unsafe boundary {audit_id} should retain exactly {expected_count} local operation markers"
        );
    }
    assert!(
        !unsafe_allowlist.contains("src/allocator/page_map.rs")
            && !page_map.contains("SAFETY[AUDIT:page-map-boundary]")
            && !page_map.contains("SAFETY[AUDIT:page-map-register-boundary]")
            && !page_map.contains("SAFETY[AUDIT:page-map-init-boundary]")
            && page_map.contains("pub(crate) static mut _mi_page_map")
            && page_map.contains("static mut mi_page_map_max_address")
            && page_map.contains("static mut mi_page_map_memid")
            && page_map.contains("static mut mi_page_map_lock")
            && page_map.contains(
                "crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });"
            )
            && page_map
                .contains("RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)")
            && page_map.contains("pub unsafe extern \"C\" fn mi_is_in_heap_region"),
        "page-map unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
    assert!(
        page_map
            .matches("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_init(")
            .count()
            == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_register(",
                )
                .count()
                == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_unregister(",
                )
                .count()
                == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_unregister_range(",
                )
                .count()
                == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_unsafe_destroy(",
                )
                .count()
                == 2
            && page_map.contains("static MI_PAGE_MAP_UNREGISTER_RANGE_KEEP")
            && arena_rs.contains("_mi_page_map_register(page)")
            && arena_rs.contains("_mi_page_map_unregister(page)")
            && init_rs.contains("_mi_page_map_unsafe_destroy(subproc)"),
        "page-map init, register, unregister, range-unregister, and unsafe-destroy helpers should keep C's local call boundaries"
    );
    assert!(
        page_map
            .matches("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_safe_ptr_page(")
            .count()
            == 2
            && page_map
                .matches("pub(crate) unsafe fn _mi_safe_ptr_page_registered(")
                .count()
                == 2
            && page_map.contains("unsafe { _mi_safe_ptr_page_registered(p) }\n        .map(|registered| registered.page().as_ptr())")
            && alloc_rs.contains("crate::free::mi_validate_pointer(p.cast_mut(), msg)")
            && free_rs.contains("let registered = _mi_safe_ptr_page_registered(p);")
            && free_rs.contains("Some(registered.page())")
            && heap_rs.contains("let page: *mut mi_page_t = unsafe { _mi_safe_ptr_page(p) };")
            && heap_rs.contains("let page = _mi_safe_ptr_page(p);")
            && arena_debug_rs.contains("let page = unsafe { _mi_safe_ptr_page(start) };"),
        "safe pointer page lookup should keep C's _mi_safe_ptr_page local call boundary while typed validation requires a registered-page witness"
    );
    assert!(
        theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_area_init(")
            && theap_rs.contains("let bsize = full_bsize.get();")
            && theap_rs.contains("let reserved = crate::page::mi_page_reserved_count(page);")
            && theap_rs.contains("(*area).reserved = reserved.as_usize() * bsize;")
            && theap_rs.contains("_mi_heap_area_init(ptr::addr_of_mut!(xarea.area), page);")
            && arena_rs.contains("use crate::theap::{")
            && arena_rs.contains("_mi_heap_area_init")
            && arena_heap_pages_rs.contains("_mi_heap_area_init(ptr::addr_of_mut!(area), page)")
            && !arena_heap_pages_rs.contains("unsafe fn mi_heap_area_init(")
            && !internal.contains("pub fn _mi_heap_area_init("),
        "heap area visitation should keep C's shared _mi_heap_area_init local helper boundary"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_heap_main(")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_theap_main(")
            && init_rs.contains("static MI_IS_THEAP_MAIN_KEEP")
            && heap_rs.contains("if _mi_is_heap_main(heap) {")
            && heap_rs.contains("if let Some(heap) = MainHeap::new(heap)")
            && heap_rs.contains("} else if let Some(heap) = OwnedHeap::new(heap)")
            && page_rs.contains("(*page).heap = if _mi_is_heap_main(heap) {")
            && arena_heap_pages_rs.contains("if unsafe { _mi_is_heap_main(heap_from) }")
            && theap_rs.contains(
                "crate::internal::mi_assert_internal!(_mi_is_heap_main(\n                    _mi_theap_heap(theap).cast_const()\n                ));"
            ),
        "main heap/theap predicates should keep C's local helper boundaries and major callers"
    );
    assert!(
        arena_meta_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_meta_is_meta_page(")
            && arena_meta_rs.contains("SAFETY[AUDIT:arena-meta-allocator]")
            && arena_meta_rs.contains("static MI_META_IS_META_PAGE_KEEP")
            && arena_meta_rs.contains("mi_meta_pages.load(Ordering::Acquire)")
            && arena_meta_rs.contains("mpage = unsafe { mi_meta_page_next(mpage) };")
            && arena_debug_rs.contains("} else if unsafe { _mi_meta_is_meta_page(start) }"),
        "meta page debug classification should keep C's _mi_meta_is_meta_page helper boundary"
    );
    assert!(
        deferred_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_deferred_free(")
            && deferred_rs.contains("let heartbeat = mi_theap_increment_heartbeat(theap);")
            && deferred_rs.contains("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_free))\n            .load(crate::core_types::MemoryOrder::Acquire);")
            && deferred_rs.contains(
                "let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);"
            )
            && deferred_rs.contains("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_arg))\n                .load(crate::core_types::MemoryOrder::Acquire);")
            && deferred_rs
                .contains("let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);")
            && deferred_rs.contains(".store(arg.as_ptr(), crate::core_types::MemoryOrder::Release);")
            && deferred_rs.contains("let f = callback.storage_ptr();")
            && deferred_rs.contains("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_free))\n            .store(f, crate::core_types::MemoryOrder::Release);")
            && !deferred_rs.contains("mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_free))")
            && !deferred_rs.contains("mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_arg))")
            && !deferred_rs.contains("mi_atomic_store_ptr_release(ptr::addr_of_mut!(deferred_arg)")
            && !deferred_rs.contains("mi_atomic_store_ptr_release(ptr::addr_of_mut!(deferred_free)")
            && !deferred_rs.contains("fun(force, heartbeat.get(), arg);")
            && deferred_rs.contains("callback.invoke(force, heartbeat, arg);")
            && page_rs.contains("_mi_deferred_free(theap, false);")
            && theap_rs.contains("_mi_deferred_free(theap, force);"),
        "deferred free callback dispatch should keep C's _mi_deferred_free helper boundary"
    );
}
