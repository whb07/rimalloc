use super::*;

pub(super) fn assert_thread_heap_and_stats_boundaries(sources: &UnsafePerformanceSources) {
    let internal = sources.internal.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let atomic_rs = sources.atomic_rs.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let ptr_rs = sources.ptr_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();
    let init_rs = sources.init_rs.as_str();

    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) fn _mi_raw_message(")
            && options_rs.contains("static MI_RAW_MESSAGE_KEEP"),
        "_mi_raw_message should keep C's local raw-message helper boundary"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_stats_init(")
            && stats_rs.contains("static MI_STATS_INIT_KEEP: extern \"C\" fn() = _mi_stats_init;"),
        "_mi_stats_init should keep C's local process statistics initialization boundary"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_auto_process_init()")
            && init_rs.contains(
                "static MI_AUTO_PROCESS_INIT_KEEP: unsafe extern \"C\" fn() = _mi_auto_process_init;"
            )
            && init_rs.contains("#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_auto_process_done()")
            && init_rs.contains(
                "static MI_AUTO_PROCESS_DONE_KEEP: unsafe extern \"C\" fn() = _mi_auto_process_done;"
            ),
        "_mi_auto_process_init and _mi_auto_process_done should keep C's local lifecycle helper boundaries"
    );
    assert!(
        stats_rs.contains(
            "#[inline(never)]\nunsafe extern \"C\" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t)"
        ) && stats_rs.contains(
            "if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src)"
        ) && stats_rs.contains(
            "unsafe { mi_stats_add_nonnull(stats, src) };"
        ) && stats_rs.contains(
            "#[inline(never)]\nunsafe fn mi_stats_add_nonnull(stats: *mut mi_stats_t, src: *const mi_stats_t)"
        ) && stats_rs.contains(
            "mi_stat_count_add_mt(\n                    ptr::addr_of_mut!((*stats).$field),\n                    ptr::addr_of!((*src).$field),\n                )"
        ) && stats_rs.contains(
            "mi_stat_counter_add_mt(\n                    ptr::addr_of_mut!((*stats).$field),\n                    ptr::addr_of!((*src).$field),\n                )"
        ) && stats_rs.contains(
            "if MI_STAT > 1 {\n        // SAFETY[AUDIT:stats-reporting-boundary]"
        )
            && stats_rs.contains("let stats_malloc_bins =")
            && stats_rs.contains("for i in 0..=MI_BIN_HUGE")
            && stats_rs.contains("let stats_page_bins =")
            && !stats_rs.contains("&mut stats.$field")
            && !stats_rs.contains("&src.$field")
            && !stats_rs.contains("iter_mut().zip(src.malloc_bins.iter())")
            && !stats_rs.contains("iter_mut().zip(src.page_bins.iter())")
            && !stats_rs.contains("._stat_reserved\n        .iter_mut()\n        .zip(src._stat_reserved.iter())")
            && !stats_rs.contains("._stat_counter_reserved\n        .iter_mut()\n        .zip(src._stat_counter_reserved.iter())")
            && !stats_rs.contains("stats.chunk_bins.iter_mut().zip(src.chunk_bins.iter())")
            && !stats_rs.contains("static MI_STATS_ADD_KEEP"),
        "mi_stats_add should keep C's raw-pointer aggregate helper boundary without a data-relocation keepalive, route checked callers to the non-null body, release bin-stat gates, and leave reserved extension arrays untouched"
    );
    let stats_add_nonnull_start = stats_rs
        .find("#[inline(never)]\nunsafe fn mi_stats_add_nonnull(")
        .expect("find mi_stats_add_nonnull");
    let stats_add_nonnull_end = stats_rs[stats_add_nonnull_start..]
        .find("\n}\n\nfn mi_stats_add_into")
        .map(|offset| stats_add_nonnull_start + offset)
        .expect("find end of mi_stats_add_nonnull");
    let stats_add_nonnull_body = &stats_rs[stats_add_nonnull_start..stats_add_nonnull_end];
    assert!(
        !stats_add_nonnull_body.contains("stats.is_null()")
            && !stats_add_nonnull_body.contains("src.is_null()")
            && !stats_add_nonnull_body.contains("ptr::eq(stats.cast_const(), src)"),
        "mi_stats_add_nonnull should keep the checked merge body free of duplicate entry guards"
    );
    let stats_add_into_start = stats_rs
        .find("fn mi_stats_add_into(to: &mut mi_stats_t, from: &mi_stats_t)")
        .expect("find mi_stats_add_into");
    let stats_add_into_end = stats_rs[stats_add_into_start..]
        .find("\n}\n\nfn mi_stats_copy")
        .map(|offset| stats_add_into_start + offset)
        .expect("find end of mi_stats_add_into");
    let stats_add_into_body = &stats_rs[stats_add_into_start..stats_add_into_end];
    assert!(
        stats_add_into_body
            .contains("unsafe { mi_stats_add(to as *mut mi_stats_t, from as *const mi_stats_t) };")
            && !stats_add_into_body.contains("mi_stats_add_nonnull("),
        "mi_stats_add_into should exercise C's checked raw mi_stats_add compatibility boundary"
    );
    let stats_merge_start = stats_rs
        .find("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into(")
        .expect("find _mi_stats_merge_into");
    let stats_merge_end = stats_rs[stats_merge_start..]
        .find("\n}\n\n#[repr(C)]")
        .map(|offset| stats_merge_start + offset)
        .expect("find end of _mi_stats_merge_into");
    let stats_merge_body = &stats_rs[stats_merge_start..stats_merge_end];
    assert!(
        stats_merge_body.contains("mi_stats_add_nonnull(to, from);")
            && stats_merge_body.contains("_mi_memzero(from.cast(), size_of::<mi_stats_t>());")
            && !stats_merge_body.contains("mi_stats_add_into("),
        "_mi_stats_merge_into should match C's direct checked stats-add body plus memzero sequence"
    );
    assert!(
        options_rs
            .contains("#[inline(never)]\npub(crate) fn _mi_message(args: fmt::Arguments<'_>)")
            && options_rs.contains(
                "mi_vfprintf_thread(None, ptr::null_mut(), c\"mimalloc: \".as_ptr(), args);"
            )
            && options_rs
                .contains("static MI_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_message;"),
        "_mi_message should keep C's local message helper boundary and mimalloc prefix path"
    );
    assert!(
        options_rs.contains("#[inline(never)]\nfn mi_recurse_enter_prim() -> bool")
            && options_rs.contains(
                "static MI_RECURSE_ENTER_PRIM_KEEP: fn() -> bool = mi_recurse_enter_prim;"
            )
            && options_rs.contains("#[inline(never)]\nfn mi_recurse_exit_prim()")
            && options_rs
                .contains("static MI_RECURSE_EXIT_PRIM_KEEP: fn() = mi_recurse_exit_prim;")
            && options_rs.contains("mi_recurse_enter_prim()")
            && options_rs.contains("mi_recurse_exit_prim();"),
        "option recursion guards should keep C's noinline primitive helper boundaries"
    );

    assert!(
        !init_rs.contains("fn _mi_theap_collect_abandon(")
            && !init_rs.contains("fn _mi_theap_create(")
            && !init_rs.contains("fn _mi_theap_free(")
            && !init_rs.contains("fn _mi_heap_force_destroy("),
        "thread initialization internals should call crate-local theap lifecycle helpers directly"
    );
    assert!(
        theap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_collect_abandon"
        ) && theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_free"),
        "thread teardown should keep C's heap collect/free call boundaries instead of inlining them into _mi_thread_done"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_init")
            && theap_rs.contains("let Some(theap) = ThreadHeapPtr::new(theap) else")
            && theap_rs.contains("let Some(heap) = HeapPtr::new(heap) else")
            && theap_rs.contains("let Some(tld) = TldPtr::new(tld) else")
            && theap_rs.contains(
                "unsafe fn _mi_theap_init_typed(theap: ThreadHeapPtr, heap: HeapPtr, tld: TldPtr)"
            )
            && theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_create")
            && theap_rs.contains("unsafe fn _mi_theap_create_typed(heap: HeapPtr, tld: TldPtr) -> Option<ThreadHeapPtr>")
            && theap_rs.contains("unsafe { _mi_theap_create_typed(heap, tld) }\n        .map(ThreadHeapPtr::as_ptr)")
            && theap_rs.contains("_mi_theap_init_typed(theap, heap, tld);\n        Some(theap)"),
        "_mi_theap_create and _mi_theap_init should parse raw metadata pointers once and use typed wrappers below the ABI shim"
    );
    assert!(
        theap_rs.contains("_mi_memcpy_aligned(\n            theap_raw.cast(),")
            && internal.contains("unsafe fn mi_cpu_movsb_max() -> usize")
            && internal.contains("\"mov {max}, qword ptr [rip + _mi_cpu_movsb_max]\"")
            && internal.contains("if n <= mi_cpu_movsb_max()")
            && internal.contains("\"rep movsb\"")
            && internal.contains("let movsb_max = mi_cpu_movsb_max();")
            && internal.contains("if n > movsb_max && n.is_multiple_of(size_of::<usize>())")
            && internal.contains("if n <= movsb_max")
            && internal.contains("\"rep movsq\"")
            && internal.contains("pub unsafe fn _mi_memcpy_aligned"),
        "_mi_theap_init should copy the empty template through C's _mi_memcpy_aligned path"
    );
    let theap_init_start = theap_rs
        .find("pub(crate) unsafe extern \"C\" fn _mi_theap_init")
        .expect("find _mi_theap_init");
    let theap_init_end = theap_rs[theap_init_start..]
        .find("pub(crate) unsafe extern \"C\" fn _mi_theap_random_next")
        .map(|offset| theap_init_start + offset)
        .expect("find _mi_theap_random_next after _mi_theap_init");
    let theap_init_body = &theap_rs[theap_init_start..theap_init_end];
    assert!(
        theap_init_body.contains(
            "(*theap_raw).cookie = _mi_random_next(ptr::addr_of_mut!((*theap_raw).random)) | 1;"
        ) && !theap_init_body.contains("_mi_theap_random_next(theap)"),
        "_mi_theap_init should match C's optimized artifact by calling _mi_random_next directly for the cookie"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize"
        ) && atomic_types.contains(
            "pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize"
        ) && atomic_types.contains("pub(crate) unsafe fn store_seq_cst(self, value: usize)")
            && theap_init_body.contains("theap.refcount_cell().store_seq_cst(1);")
            && theap_init_body.contains("theap.set_tld(Some(tld));")
            && theap_init_body.contains("theap.set_heap(Some(heap));")
            && !theap_init_body.contains(
                "mi_atomic_store_explicit(\n        ptr::addr_of_mut!((*theap).refcount),"
            )
            && !theap_init_body.contains("(*theap).refcount = 1;"),
        "_mi_theap_init should initialize C's thread-heap metadata through typed wrapper accessors"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicGuard<'a>")
            && atomic_types.contains(
                "pub(crate) unsafe fn try_acquire(field: AtomicUsizeField) -> Option<Self>"
            )
            && atomic_types.contains("impl Drop for AtomicGuard<'_>")
            && atomic_types.contains("self.field.store(0, Ordering::Release)")
            && atomic_rs.contains("_guard: crate::core_types::AtomicGuard<'static>")
            && atomic_rs.contains("crate::core_types::AtomicGuard::try_acquire(field)")
            && !atomic_rs.contains("guard: *mut mi_atomic_guard_t,\n}")
            && !atomic_rs.contains("mi_atomic_store_release(self.guard.cast::<usize>(), 0);"),
        "atomic guard scopes should acquire and release through typed AtomicGuard Drop"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>)"
        ) && atomic_types.contains("pub(crate) struct MiLockGuard<'a>")
            && atomic_types.contains("pub(crate) unsafe fn acquire(lock: MiLockPtr) -> Self")
            && atomic_types.contains("crate::atomic::mi_lock_acquire(lock.0.as_ptr())")
            && atomic_types.contains("impl Drop for MiLockGuard<'_>")
            && atomic_types.contains("crate::atomic::mi_lock_release(self.lock.0.as_ptr())")
            && ptr_rs.contains("pub(crate) unsafe fn arena_reserve_lock(self) -> MiLockPtr")
            && ptr_rs.contains("pub(crate) unsafe fn heaps_lock(self) -> MiLockPtr")
            && arena_rs.contains("MiLockGuard::acquire(subproc.arena_reserve_lock())")
            && init_rs.contains(
                "let lock = MiLockPtr(NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock)));"
            )
            && init_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && init_rs.contains(
                "let heaps_lock = unsafe { MiLockGuard::acquire(subproc.heaps_lock()) };"
            )
            && init_rs.contains("drop(heaps_lock);")
            && options_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && options_rs.contains(
                "Manual lock handling keeps this translated callback buffer path close to"
            )
            && options_rs
                .contains("Intentionally holds the output buffer lock while invoking `out`"),
        "subproc and options locks should acquire and release through typed MiLockGuard Drop"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T"
        ) && theap_rs.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of_mut!((*theap).heap))\n            .exchange_acq_rel(ptr::null_mut())"
        ) && !theap_rs.contains("mi_atomic_exchange_ptr_acq_rel"),
        "_mi_theap_free should clear the heap pointer through AtomicPtrField"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into"),
        "heap cleanup should keep C's stats merge call boundary instead of inlining it into _mi_theap_free"
    );
    assert!(
        stats_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt")
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt"
            ),
        "theap lifecycle should keep C's multithreaded stats update helper boundaries as crate-local calls"
    );
    assert!(
        stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt"
        ) && stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt"
        ) && stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt"
        ),
        "multithreaded stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_increase")
            && stats_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_decrease")
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease"
            )
            && stats_rs.contains("static MI_STAT_ADJUST_INCREASE_KEEP"),
        "thread-local stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        internal.contains("pub(crate) use crate::stats::{"),
        "internal stats helper re-exports should stay crate-local so hot call sites are direct local calls"
    );
}
