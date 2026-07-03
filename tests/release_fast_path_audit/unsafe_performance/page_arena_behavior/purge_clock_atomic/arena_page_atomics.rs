use super::PageArenaBehaviorSources;

pub(super) fn assert_arena_page_atomic_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources
            .atomic_types
            .contains("pub(crate) struct AtomicArenaPage(*const AtomicPtr<c_void>);")
            && sources.atomic_types.contains("impl AtomicArenaPage")
            && sources
                .atomic_types
                .contains("pub(crate) fn from_cell(cell: *const AtomicPtr<c_void>) -> Self")
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
                .contains("AtomicArenaPage::from_cell(ptr::addr_of!(")
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
            && sources.arena_rs.contains(
                "mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast())",
            )
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
                .matches("crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(")
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
                + sources
                    .arena_purge_rs
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
