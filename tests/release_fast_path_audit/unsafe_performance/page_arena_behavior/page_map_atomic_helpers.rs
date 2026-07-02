use super::PageArenaBehaviorSources;

pub(super) fn assert_page_map_atomic_helpers(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && sources.atomic_types.contains("expected: &mut *mut T,")
            && sources.page_map.contains(
                "crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });"
            )
            && sources
                .page_map
                .contains("atomic_submap.load(crate::core_types::MemoryOrder::Acquire)")
            && sources
                .page_map
                .contains("atomic_submap.compare_exchange_acq_rel(&mut expect, new_sub)")
            && !sources
                .page_map
                .contains("mi_atomic_load_ptr_acquire(_mi_page_map.add(idx))")
            && !sources.page_map.contains(
                "mi_atomic_cas_strong_acq_rel(\n                        _mi_page_map.add(idx).cast::<usize>()"
            ),
        "page-map submap slot atomics should use AtomicPtrField"
    );
    assert!(
        sources.internal.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of!((*theap).heap).cast_mut())\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && sources.internal.contains(
            "crate::core_types::AtomicPtrField(_mi_page_map_base().add(idx))\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && !sources
            .internal
            .contains("crate::atomic::mi_atomic_load_ptr_acquire(&(*theap).heap)")
            && !sources
                .internal
                .contains("crate::atomic::mi_atomic_load_ptr_acquire(_mi_page_map_base().add(idx))"),
        "internal thread-heap and page-map pointer loads should use AtomicPtrField"
    );
}
