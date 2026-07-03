use super::super::super::PageArenaBehaviorSources;

pub(super) fn assert_thread_heap_refcount_dispatch(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources
            .theap_rs
            .contains("let allocated_size = match crate::core_types::MemId::view(memid)")
            && sources
                .theap_rs
                .contains("crate::core_types::MemIdView::Arena(a)")
            && sources
                .theap_rs
                .contains("a.range().count_usize() * crate::arena::MI_ARENA_SLICE_SIZE")
            && sources
                .theap_rs
                .contains("fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool")
            && sources
                .theap_rs
                .contains("crate::core_types::MemIdView::Unknown(code)")
            && sources
                .theap_rs
                .contains("code.0 > crate::core_types::MemKindCode::STATIC.0")
            && sources
                .theap_rs
                .contains("match crate::core_types::MemId::view(memid)")
            && sources
                .theap_rs
                .contains("crate::core_types::MemIdView::HeapMain(_)")
            && sources
                .theap_rs
                .contains("crate::core_types::MemIdView::Meta(_)")
            && sources
                .theap_rs
                .contains("mi_memid_is_refcounted((*theap.as_ptr()).memid)")
            && sources
                .atomic_types
                .contains("pub(crate) struct AtomicThreadHeapRefcount(AtomicUsizeField);")
            && sources
                .atomic_types
                .contains("pub(crate) fn from_field(field: AtomicUsizeField) -> Self")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn incref_acq_rel(self) -> usize")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn decref_acq_rel(self) -> usize")
            && sources
                .ptr_rs
                .contains("pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount")
            && sources
                .ptr_rs
                .contains("AtomicThreadHeapRefcount::from_field(AtomicUsizeField::from_raw(")
            && sources.theap_rs.matches(".refcount_cell()").count() == 3
            && sources.theap_rs.contains(".incref_acq_rel();")
            && sources.theap_rs.contains(".decref_acq_rel()")
            && !sources
                .theap_rs
                .contains("AtomicUsizeField::from_raw(ptr::addr_of_mut!((*theap).refcount))")
            && !sources
                .theap_rs
                .contains("mi_atomic_increment_acq_rel(ptr::addr_of_mut!((*theap).refcount))")
            && !sources.theap_rs.contains(
                "mi_atomic_fetch_sub_explicit(\n            ptr::addr_of_mut!((*theap).refcount),"
            )
            && !sources.theap_rs.contains("memid.mem.os.")
            && !sources.theap_rs.contains("memid.memkind"),
        "thread-heap memid/refcount paths should dispatch through typed MemIdView and AtomicThreadHeapRefcount"
    );
}
