use super::PageArenaBehaviorSources;

pub(super) fn assert_memid_and_arena_dispatch_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources.arena_rs.contains(
            "unsafe fn mi_arena_schedule_purge(arena: ArenaPtr, range: crate::core_types::SliceRange)"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && sources.arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && !sources.arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) -> bool"
        ) && !sources.arena_rs.contains(
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
            && sources.arena_rs.contains(
                "memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();"
            )
            && sources
                .arena_rs
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
            && !sources.arena_rs.contains("memid.mem.os.")
            && !sources.arena_rs.contains("memid.mem.arena.")
            && !sources.arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_EXTERNAL")
            && !sources.arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_NONE")
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
                .theap_rs
                .contains("let allocated_size = match crate::core_types::MemId::view(memid)")
            && sources
                .theap_rs
                .contains("crate::core_types::MemIdView::Arena(a)")
            && sources
                .theap_rs
                .contains("a.range.count_usize() * crate::arena::MI_ARENA_SLICE_SIZE")
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
            && sources.theap_rs.contains("crate::core_types::MemIdView::Meta(_)")
            && sources
                .theap_rs
                .contains("mi_memid_is_refcounted((*theap.as_ptr()).memid)")
            && sources.atomic_types.contains(
                "pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);"
            )
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn incref_acq_rel(self) -> usize")
            && sources
                .atomic_types
                .contains("pub(crate) unsafe fn decref_acq_rel(self) -> usize")
            && sources
                .ptr_rs
                .contains("pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount")
            && sources.theap_rs.matches(".refcount_cell()").count() == 3
            && sources.theap_rs.contains(".incref_acq_rel();")
            && sources.theap_rs.contains(".decref_acq_rel()")
            && !sources
                .theap_rs
                .contains("AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))")
            && !sources
                .theap_rs
                .contains("mi_atomic_increment_acq_rel(ptr::addr_of_mut!((*theap).refcount))")
            && !sources.theap_rs.contains("mi_atomic_fetch_sub_explicit(\n            ptr::addr_of_mut!((*theap).refcount),")
            && !sources.theap_rs.contains("memid.mem.os.")
            && !sources.theap_rs.contains("memid.memkind")
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
            && !sources
                .arena_rs
                .contains("const fn mi_memkind_is_os(memkind: mi_memkind_t)")
            && !sources.arena_rs.contains("mi_memkind_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.memkind })")
            && !sources.arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            )
            && !sources.arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange)"
            )
            && !sources.arena_rs.contains(
                "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            ),
        "arena free/purge path should dispatch through MemIdView and carry SliceRange"
    );
    assert!(
        sources.arena_ids_rs.contains(
            "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> mi_memid_t"
        ) && sources
            .arena_rs
            .contains("memid: unsafe { mi_memid_create_arena(arena, range) },")
            && sources
                .arena_ids_rs
                .contains("let Some((arena, range)) = (unsafe { mi_arena_range_from_memid(memid) }) else")
            && sources
                .arena_ids_rs
                .contains("unsafe { *slice_index = range.start_usize() };")
            && sources
                .arena_ids_rs
                .contains("unsafe { *slice_count = range.count_usize() };")
            && sources
                .scalar_types
                .contains("pub(crate) fn as_u32(self) -> Option<u32>")
            && sources
                .scalar_types
                .contains("pub(crate) const fn from_usize_const(count: usize) -> Option<Self>")
            && sources.arena_types.contains("pub(crate) fn start_u32(self) -> u32")
            && sources
                .arena_types
                .contains("pub(crate) const fn count_u32(self) -> u32")
            && sources.memid_rs.contains("slice_index: range.start_u32()")
            && sources.memid_rs.contains("slice_count: range.count_u32()")
            && sources
                .arena_rs
                .contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && !sources
                .arena_rs
                .contains("mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) as u32")
            && !sources
                .arena_rs
                .contains("mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) as u32")
            && !sources
                .arena_rs
                .contains("mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) as u32")
            && !sources.memid_rs.contains("range.start().0 as u32")
            && !sources.arena_ids_rs.contains(
                "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    slice_index: usize,\n    slice_count: usize,"
            )
            && !sources.arena_ids_rs.contains(
                "SliceRange::from_raw(slice_index as u32, slice_count as u32)"
            ),
        "arena memid construction should receive an already-typed SliceRange"
    );

    let reabandon_arena = crate::source_item(
        "src/allocator/arena.rs",
        sources.arena_rs,
        "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena",
    );
    assert!(
        sources.arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped"
        )
            && sources.arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(\n    page: crate::core_types::OwnedAbandonedPage,\n    _arena_memid: crate::core_types::ArenaMemId,"
            )
            && sources
                .arena_ids_rs
                .contains("unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool")
            && sources
                .arena_ids_rs
                .contains("crate::core_types::MemId::view(unsafe { (*page).memid })")
            && sources
                .arena_rs
                .contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !sources.arena_rs.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !sources.arena_rs.contains("(*page).memid.memkind != mi_memkind_t::MI_MEM_ARENA")
            && sources.arena_rs.contains("_mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page))")
            && sources.arena_rs.contains("_mi_arenas_page_abandon(page, theap)")
            && !reabandon_arena.contains("mi_page_memid_is_arena(page)")
            && !reabandon_arena.contains("MemId::view")
            && sources.free_rs.contains("crate::core_types::MemId::view((*page).memid)")
            && sources
                .free_rs
                .contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !sources.free_rs.contains(
                "(*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA"
            )
            && sources
                .free_rs
                .contains("_mi_arenas_page_try_reabandon_to_mapped_arena(")
            && !sources
                .free_rs
                .contains("_mi_arenas_page_try_reabandon_to_mapped(page)"),
        "free path should validate arena memids and carry the witness into the crate-local reabandon helper"
    );
    assert!(
        sources
            .arena_ids_rs
            .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_id_none")
            && sources
                .arena_ids_rs
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_from_id")
            && sources
                .arena_ids_rs
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_memid_is_suitable")
            && sources
                .arena_ids_rs
                .contains("static MI_ARENA_ID_NONE_KEEP")
            && sources
                .arena_ids_rs
                .contains("static MI_ARENA_FROM_ID_KEEP")
            && sources
                .arena_ids_rs
                .contains("unsafe { _mi_arena_id_none() }")
            && sources
                .arena_rs
                .contains("let arena = unsafe { _mi_arena_from_id(arena_id) };")
            && sources
                .heap_rs
                .contains("(*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);")
            && sources.free_rs.contains("_mi_arena_memid_is_suitable("),
        "arena id conversion and memid suitability helpers should keep C's local call boundaries"
    );
}
