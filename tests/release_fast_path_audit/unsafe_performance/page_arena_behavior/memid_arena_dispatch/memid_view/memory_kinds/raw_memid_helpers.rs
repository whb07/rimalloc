use super::super::super::super::PageArenaBehaviorSources;

pub(super) fn raw_memid_helpers_are_split(
    sources: &PageArenaBehaviorSources<'_>,
    arena_region: &str,
) -> bool {
    sources.internal.contains("pub(crate) mod memid;")
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
        && !sources
            .internal
            .contains("memid.mem.meta.block_index = block_idx as u32")
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
        && !arena_region.contains(
            "mi_memkind_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.memkind })",
        )
}
