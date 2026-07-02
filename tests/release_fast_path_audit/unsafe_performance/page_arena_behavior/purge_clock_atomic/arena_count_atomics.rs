use super::PageArenaBehaviorSources;

pub(super) fn assert_arena_count_atomic_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && sources.atomic_types.contains(
                "pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool"
            )
            && sources
                .arena_purge_rs
                .matches("crate::core_types::AtomicI64Field(")
                .count()
                == 7
            && sources
                .arena_purge_rs
                .matches(".compare_exchange_acq_rel(&mut expire0, expire)")
                .count()
                == 2
            && sources
                .arena_purge_rs
                .contains(".load(crate::core_types::MemoryOrder::Relaxed)")
            && sources
                .arena_purge_rs
                .contains(".load(crate::core_types::MemoryOrder::Acquire)")
            && sources
                .arena_purge_rs
                .matches(".store(0, crate::core_types::MemoryOrder::Release)")
                .count()
                == 2
            && sources.arena_purge_rs.contains(
                "now + ((delay as mi_msecs_t) / 10),\n                    crate::core_types::MemoryOrder::Release,"
            )
            && !sources.arena_purge_rs.contains("mi_atomic_casi64_strong_acq_rel")
            && !sources.arena_purge_rs.contains("mi_atomic_loadi64_explicit")
            && !sources.arena_purge_rs.contains("mi_atomic_loadi64_acquire")
            && !sources.arena_purge_rs.contains("mi_atomic_storei64_release")
            && !sources
                .arena_purge_rs
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
}
