use super::super::PageArenaBehaviorSources;

#[path = "memid_view/thread_heap_refcount.rs"]
mod thread_heap_refcount;

pub(super) fn assert_memid_view_dispatch(sources: &PageArenaBehaviorSources<'_>) {
    thread_heap_refcount::assert_thread_heap_refcount_dispatch(sources);
    let arena_region = format!("{}\n{}", sources.arena_rs, sources.arena_os_memory_rs);

    assert!(
        sources.arena_purge_rs.contains(
            "unsafe fn mi_arena_schedule_purge(arena: ArenaPtr, range: crate::core_types::SliceRange)"
        ) && sources.arena_purge_rs.contains(
            "unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && sources.arena_purge_rs.contains(
            "unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && !sources.arena_purge_rs.contains(
            "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) -> bool"
        ) && !sources.arena_purge_rs.contains(
            "unsafe fn mi_arena_try_purge_range(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> bool"
        ) && sources.arena_ids_rs.contains("match crate::core_types::MemId::view(memid)")
            && sources
                .arena_ids_rs
                .contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && sources.arena_ids_rs.contains("let range = arena_memid.range;")
            && sources
                .arena_ids_rs
                .contains("fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && sources.arena_ids_rs.contains("crate::core_types::MemIdView::Os(_)")
            && sources.memid_rs.contains("pub(crate) struct ExternalMemId")
            && sources.memid_rs.contains("External(ExternalMemId)")
            && sources.memid_rs.contains("impl MemId<ExternalMem>")
            && sources.memid_rs.contains("pub(crate) fn external(")
            && sources.arena_os_memory_rs.contains(
                "memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();"
            )
            && sources
                .arena_os_memory_rs
                .contains("crate::core_types::MemId::<crate::core_types::ExternalMem>::external")
            && sources
                .arena_ids_rs
                .contains("crate::core_types::MemIdView::External(external)")
            && sources
                .os_rs
                .contains("crate::core_types::MemIdView::External(external)")
            && sources.arena_ids_rs.contains("let os_size = external.size.get();")
            && sources
                .arena_debug_rs
                .contains("let page_slice_count = match crate::core_types::MemId::view(unsafe { (*raw).memid })")
            && sources
                .arena_debug_rs
                .contains("crate::core_types::MemIdView::Arena(a) => a.range.count_usize() as isize")
            && sources.os_rs.contains("external.size.get()")
            && sources
                .os_rs
                .contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && sources.memid_rs.contains("pub(crate) fn os_huge(")
            && sources.memid_rs.contains("fn os_with_kind(")
            && sources.internal_memid.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os(")
            && sources
                .os_rs
                .contains("crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(")
            && !sources.os_rs.contains("mi_memkind_is_os(memid.memkind)")
            && !sources.os_rs.contains("(*memid).memkind = crate::internal::mi_memkind_t::MI_MEM_OS_HUGE")
            && !arena_region.contains("memid.mem.os.")
            && !arena_region.contains("memid.mem.arena.")
            && !arena_region.contains("memid.memkind = mi_memkind_t::MI_MEM_EXTERNAL")
            && !arena_region.contains("memid.memkind = mi_memkind_t::MI_MEM_NONE")
            && !sources.os_rs.contains("memid.mem.os.")
            && !sources.internal.contains("memid.mem.os.base = base")
            && !sources.internal.contains("memid.mem.os.size = size")
            && !sources.internal_memid.contains("memid.mem.os.base = base")
            && !sources.internal_memid.contains("memid.mem.os.size = size")
            && sources.page_map.contains("fn mi_memid_os_base_size(memid: mi_memid_t)")
            && sources
                .page_map
                .contains("crate::core_types::MemIdView::Os(os)")
            && sources
                .page_map
                .contains("crate::core_types::MemIdView::External(external)")
            && !sources.page_map.contains("memid.mem.os.")
            && !sources.page_map.contains("mi_page_map_memid.mem.os.")
            && sources
                .init_rs
                .contains("fn mi_memid_is_static(memid: mi_memid_t) -> bool")
            && sources
                .init_rs
                .contains("crate::core_types::MemIdView::Static(_)")
            && sources.memid_rs.contains("impl MemId<StaticMem>")
            && sources.memid_rs.contains("pub(crate) const fn static_mem() -> Self")
            && sources.init_rs.contains(
                "crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw()"
            )
            && sources
                .init_rs
                .contains("!mi_memid_is_static((*main_prefix).memid)")
            && sources
                .init_rs
                .contains("!mi_memid_is_static(theap_main.memid)")
            && !sources.init_rs.contains(
                "crate::internal::_mi_memid_create(mi_memkind_t::MI_MEM_STATIC)"
            )
            && !sources
                .init_rs
                .contains("(*main_prefix).memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && !sources
                .init_rs
                .contains("theap_main.memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && sources.internal.contains("pub(crate) mod memid;")
            && sources.internal.contains("pub use memid::*;")
            && !sources.internal.contains("pub enum mi_memkind_t")
            && sources
                .internal_memid
                .contains("//! Raw memory-origin identifiers from mimalloc's internal header.")
            && sources.internal_memid.contains("pub enum mi_memkind_t")
            && sources
                .internal_memid
                .contains("pub fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && sources
                .internal_memid
                .contains("pub fn mi_memid_needs_no_free(memid: mi_memid_t) -> bool")
            && sources
                .internal
                .contains("pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool")
            && sources
                .internal_memid
                .contains("crate::core_types::MemIdView::None(_)")
            && sources
                .internal_memid
                .contains("crate::core_types::MemIdView::External(_)")
            && sources
                .internal_memid
                .contains("crate::core_types::MemIdView::Static(_)")
            && sources
                .internal
                .contains("crate::core_types::MemIdView::Arena(_)")
            && !sources.internal.contains("pub unsafe fn _mi_memid_create(")
            && !sources
                .internal_memid
                .contains("pub unsafe fn _mi_memid_create(")
            && !sources.internal.contains("raw.memkind = memkind")
            && sources.internal.contains("mi_page_is_in_arena(page)")
            && sources.arena_meta_rs.contains("MemId::<MetaMem>::meta(")
            && sources
                .arena_meta_rs
                .contains("NonZeroU32::new_unchecked(block_count as u32)")
            && sources.memid_rs.contains("impl MemId<MetaMem>")
            && sources.memid_rs.contains("is_pinned: true")
            && sources.memid_rs.contains("initially_committed: true")
            && sources.memid_rs.contains("initially_zero: true")
            && !sources
                .internal
                .contains("crate::types::mi_memkind_is_os(memid.memkind)")
            && !sources
                .internal
                .contains("crate::types::mi_memkind_needs_no_free(memid.memkind)")
            && !sources
                .internal
                .contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !sources.internal.contains("memid.mem.meta.meta_page = mpage")
            && !sources.internal.contains("memid.mem.meta.block_index = block_idx as u32")
            && !sources
                .internal
                .contains("memid.mem.meta.block_count = block_count as u32")
            && !sources
                .internal_memid
                .contains("memid.mem.meta.meta_page = mpage")
            && !sources
                .internal_memid
                .contains("memid.mem.meta.block_index = block_idx as u32")
            && !sources
                .internal_memid
                .contains("memid.mem.meta.block_count = block_count as u32")
            && sources.arena_meta_rs.contains(
                "if let crate::core_types::MemIdView::Meta(meta) = crate::core_types::MemId::view(memid)"
            )
            && sources
                .arena_meta_rs
                .contains("let block_count = meta.block_count.get() as usize;")
            && sources
                .arena_meta_rs
                .contains("let block_idx = meta.block_index.0 as usize;")
            && !sources.arena_meta_rs.contains("memid.mem.meta.")
            && !sources
                .arena_meta_rs
                .contains("memid.memkind == crate::internal::mi_memkind_t::MI_MEM_META")
            && !arena_region.contains("const fn mi_memkind_is_os(memkind: mi_memkind_t)")
            && !arena_region.contains("mi_memkind_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.memkind })")
            && !sources.arena_purge_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            )
            && !sources.arena_purge_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange)"
            )
            && !sources.arena_purge_rs.contains(
                "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            ),
        "arena free/purge path should dispatch through MemIdView and carry SliceRange"
    );
}
