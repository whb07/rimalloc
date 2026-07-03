use super::*;

#[path = "thread_stats/lifecycle_helpers.rs"]
mod lifecycle_helpers;
#[path = "thread_stats/marker_inventory.rs"]
mod marker_inventory;
#[path = "thread_stats/stats_merge.rs"]
mod stats_merge;

pub(super) fn assert_thread_heap_and_stats_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let internal = sources.internal.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let atomic_rs = sources.atomic_rs.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let ptr_rs = sources.ptr_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();
    let init_rs = sources.init_rs.as_str();

    marker_inventory::assert_stats_marker_inventory(unsafe_allowlist, options_rs, stats_rs);
    stats_merge::assert_stats_merge_boundaries(stats_rs);
    lifecycle_helpers::assert_lifecycle_helper_boundaries(init_rs, options_rs, theap_rs);
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
            "crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!((*theap).heap))\n            .exchange_acq_rel(ptr::null_mut())"
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
