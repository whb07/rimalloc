use super::*;

pub(super) fn assert_thread_tls_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let callbacks_rs = sources.callbacks_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let internal = sources.internal.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_flags_rs = sources.page_flags_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();
    let threadlocal_rs = sources.threadlocal_rs.as_str();

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
        let item = source_item("src/platform/mod.rs", prim_rs, marker);
        assert_no_rust_heap_constructs("src/platform/mod.rs", marker, item);
    }
    let default_raw = source_item(
        "src/platform/mod.rs",
        prim_rs,
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
        prim_rs,
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
        prim_rs,
        "pub(crate) unsafe fn _mi_page_associated_theap_peek(",
    );
    assert!(
        page_assoc.contains("let heap = unsafe { (*page).heap };")
            && page_assoc.contains("crate::init::mi_theap_main_tls_peek()")
            && page_assoc.contains("unsafe { _mi_heap_theap_peek(heap) }")
            && page_assoc.contains("ThreadHeapPtr::new(theap)"),
        "page-associated thread-heap lookup should keep non-initializing heap peeks and typed pointer wrapping"
    );

    super::super::threadlocal_boundaries::assert_dynamic_threadlocal_boundaries(threadlocal_rs);
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
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/thread_heap.rs\t"))
            && theap_rs
                .matches("SAFETY[AUDIT:thread-heap-lifecycle]")
                .count()
                >= 70,
        "thread-heap lifecycle unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
}
