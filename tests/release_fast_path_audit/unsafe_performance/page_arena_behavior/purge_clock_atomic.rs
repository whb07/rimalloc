use super::PageArenaBehaviorSources;

pub(super) fn assert_purge_clock_and_atomic_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources
            .stats_rs
            .contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_now()")
            && sources
                .stats_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_start()")
            && sources
                .stats_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_end(")
            && sources.stats_rs.contains("let t0 = _mi_clock_now();")
            && sources
                .stats_rs
                .contains("let diff = _mi_clock_now() - t0;")
            && sources
                .stats_rs
                .contains("_mi_clock_now() - start - CLOCK_DIFF.load")
            && sources.os_rs.contains("let start_t = _mi_clock_start();")
            && sources
                .os_rs
                .contains("let mut elapsed = _mi_clock_end(start_t);")
            && sources
                .arena_rs
                .contains("let expire = crate::stats::_mi_clock_now() + delay as i64;")
            && sources
                .arena_rs
                .contains("let now = crate::stats::_mi_clock_now();"),
        "clock helpers should keep C's timing helper boundaries and main callers"
    );
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && sources.atomic_types.contains(
                "pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool"
            )
            && sources
                .arena_rs
                .matches("crate::core_types::AtomicI64Field(")
                .count()
                == 7
            && sources
                .arena_rs
                .matches(".compare_exchange_acq_rel(&mut expire0, expire)")
                .count()
                == 2
            && sources
                .arena_rs
                .contains(".load(crate::core_types::MemoryOrder::Relaxed)")
            && sources
                .arena_rs
                .contains(".load(crate::core_types::MemoryOrder::Acquire)")
            && sources
                .arena_rs
                .matches(".store(0, crate::core_types::MemoryOrder::Release)")
                .count()
                == 2
            && sources.arena_rs.contains(
                "now + ((delay as mi_msecs_t) / 10),\n                    crate::core_types::MemoryOrder::Release,"
            )
            && !sources.arena_rs.contains("mi_atomic_casi64_strong_acq_rel")
            && !sources.arena_rs.contains("mi_atomic_loadi64_explicit")
            && !sources.arena_rs.contains("mi_atomic_loadi64_acquire")
            && !sources.arena_rs.contains("mi_atomic_storei64_release")
            && !sources
                .arena_rs
                .contains("mi_memory_order_e::mi_memory_order_relaxed"),
        "arena purge expiry should use typed atomic i64 field wrappers"
    );
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && sources.atomic_types.contains("desired: usize,")
            && sources.atomic_types.contains(
                "atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)"
            )
            && sources
                .arena_rs
                .matches("ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)")
                .count()
                == 2
            && sources
                .arena_rs
                .contains(".compare_exchange_acq_rel(&mut expected, 0)")
            && sources.arena_rs.contains("let next_count = count + 1;")
            && sources
                .arena_rs
                .contains(".compare_exchange_acq_rel(&mut count, next_count)")
            && !sources.arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            )
            && !sources.arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            ),
        "arena_count compare-exchange should use AtomicUsizeField"
    );
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);")
            && sources.atomic_types.contains("impl AtomicAbandonedCount")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn load_relaxed(self) -> usize")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize")
            && sources.arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_heap_abandoned_count(\n    heap: *mut mi_heap_t,\n    bin: crate::core_types::Bin,"
            )
            && sources.arena_prefix_rs.contains("abandoned_count[bin.as_usize()]")
            && sources
                .arena_rs
                .contains("mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1)")
            && sources
                .arena_rs
                .contains("mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1)")
            && !sources
                .arena_rs
                .contains(".abandoned_count[bin].load(Ordering::Relaxed)")
            && !sources
                .arena_rs
                .contains(".abandoned_count[bin].fetch_add(1, Ordering::Relaxed)")
            && !sources
                .arena_rs
                .contains(".abandoned_count[bin].fetch_sub(1, Ordering::Relaxed)")
            && !sources
                .arena_rs
                .contains(".abandoned_count[i].load(Ordering::Relaxed)"),
        "arena abandoned-count atomics should go through AtomicAbandonedCount with typed Bin indexes"
    );
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);")
            && sources.atomic_types.contains("impl AtomicArenaPage")
            && sources.atomic_types.contains(
                "pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t"
            )
            && sources.atomic_types.contains(
                "pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t)"
            )
            && sources.arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_arena_index(arena: *mut mi_arena_t) -> crate::core_types::ArenaIndex"
            )
            && !sources.arena_rs.contains(
                "unsafe fn mi_heap_arena_page_cell(\n    heap: *mut mi_heap_t,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && sources.arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_heap_arena_page_cell(\n    heap: BorrowedHeap,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && sources.arena_rs.contains(
                "unsafe fn mi_heap_ensure_arena_pages(\n    heap: BorrowedHeap,\n    arena: ArenaPtr,\n    arena_idx: ArenaIndex,"
            )
            && sources
                .arena_prefix_rs
                .contains("arena_pages[arena_idx.get() as usize]")
            && sources
                .arena_rs
                .matches("mi_heap_arena_page_cell(heap, arena_idx).load_acquire()")
                .count()
                == 1
            && sources
                .arena_rs
                .matches("mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire()")
                .count()
                == 1
            && sources
                .arena_rs
                .contains("mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast())")
            && !sources
                .arena_rs
                .contains(".arena_pages[arena_idx].load(Ordering::Acquire)")
            && !sources.arena_rs.contains(".arena_pages[arena_idx]\n                    .store")
            && !sources
                .arena_rs
                .contains(".arena_pages[i].load(Ordering::Acquire)"),
        "heap arena page-table atomics should use AtomicArenaPage with typed ArenaIndex values"
    );
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn compare_exchange_release(")
            && sources
                .arena_prefix_rs
                .matches("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(")
                .count()
                == 1
            && sources.arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_subproc_arena_cell(\n    subproc: *mut mi_subproc_t,\n    arena_idx: ArenaIndex,"
            )
            && sources.arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_arena_from_index_typed(\n    subproc: SubprocPtr,\n    arena_idx: ArenaIndex,"
            )
            && sources
                .arena_prefix_rs
                .matches("(*subproc.cast::<MiSubprocPrefix>()).arenas[")
                .count()
                == 1
            && sources
                .arena_rs
                .contains("let Some(arena_idx) = ArenaIndex::from_usize(idx)")
            && sources
                .arena_rs
                .matches("mi_arena_from_index_typed(")
                .count()
                >= 5
            && sources
                .arena_rs
                .matches(".compare_exchange_release(&mut expected, arena)")
                .count()
                == 2
            && sources
                .arena_rs
                .contains(".store(ptr::null_mut(), crate::core_types::MemoryOrder::Release);")
            && !sources.arena_rs.contains("mi_arena_from_index(subproc, i)")
            && !sources.arena_rs.contains("mi_atomic_cas_ptr_strong_release")
            && !sources.arena_rs.contains("mi_atomic_store_ptr_release")
            && !sources.arena_rs.contains("mi_atomic_load_ptr_acquire"),
        "arena table pointer atomics should use AtomicPtrField with typed ArenaIndex values"
    );
}
