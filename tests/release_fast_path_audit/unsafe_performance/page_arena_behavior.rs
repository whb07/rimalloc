use super::*;

#[path = "page_arena_behavior/arena_helpers.rs"]
mod arena_helpers;

pub(super) fn assert_page_arena_behavior_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let page_map = sources.page_map.as_str();
    let internal = sources.internal.as_str();
    let internal_memid = sources.internal_memid.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let arena_rs = sources.arena_rs.as_str();
    let arena_debug_rs = sources.arena_debug_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let arena_ids_rs = sources.arena_ids_rs.as_str();
    let arena_prefix_rs = sources.arena_prefix_rs.as_str();
    let arena_meta_rs = sources.arena_meta_rs.as_str();
    let bitmap_rs = sources.bitmap_rs.as_str();
    let free_rs = sources.free_rs.as_str();
    let memid_rs = sources.memid_rs.as_str();
    let callbacks_rs = sources.callbacks_rs.as_str();
    let freelist_types = sources.freelist_types.as_str();
    let arena_types = sources.arena_types.as_str();
    let page_types = sources.page_types.as_str();
    let scalar_types = sources.scalar_types.as_str();
    let atomic_types = sources.atomic_types.as_str();
    let ptr_rs = sources.ptr_rs.as_str();
    let page_states_rs = sources.page_states_rs.as_str();
    let page_queues_rs = sources.page_queues_rs.as_str();
    let os_rs = sources.os_rs.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_counts_rs = sources.page_counts_rs.as_str();
    let page_flags_rs = sources.page_flags_rs.as_str();
    let page_validation_rs = sources.page_validation_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let theap_rs = sources.theap_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let threadlocal_rs = sources.threadlocal_rs.as_str();
    let alloc_aligned_rs = sources.alloc_aligned_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();

    arena_boundaries::assert_arena_region_boundaries(arena_boundaries::BoundarySources {
        arena_rs,
        unsafe_allowlist,
    });

    page_lifecycle::assert_page_lifecycle_and_queue_boundaries(page_lifecycle::BoundarySources {
        alloc_rs,
        alloc_aligned_rs,
        arena_rs,
        atomic_types,
        free_rs,
        freelist_types,
        internal,
        page_counts_rs,
        page_flags_rs,
        page_queues_rs,
        page_rs,
        page_states_rs,
        page_validation_rs,
        theap_rs,
    });

    bitmap_boundaries::assert_bitmap_traversal_and_regular_boundaries(
        bitmap_rs,
        unsafe_allowlist,
        arena_rs,
        arena_heap_pages_rs,
        callbacks_rs,
        threadlocal_rs,
    );
    bitmap_boundaries::assert_bitmap_helper_boundaries(bitmap_rs);
    arena_helpers::assert_arena_helper_boundaries(arena_rs, arena_debug_rs, arena_types);
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_schedule_purge(arena: ArenaPtr, range: crate::core_types::SliceRange)"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && !arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) -> bool"
        ) && !arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> bool"
        ) && arena_ids_rs.contains("match crate::core_types::MemId::view(memid)")
            && arena_ids_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && arena_ids_rs.contains("let range = arena_memid.range;")
            && arena_ids_rs.contains("fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && arena_ids_rs.contains("crate::core_types::MemIdView::Os(_)")
            && memid_rs.contains("pub(crate) struct ExternalMemId")
            && memid_rs.contains("External(ExternalMemId)")
            && memid_rs.contains("impl MemId<ExternalMem>")
            && memid_rs.contains("pub(crate) fn external(")
            && arena_rs.contains(
                "memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();"
            )
            && arena_rs
                .contains("crate::core_types::MemId::<crate::core_types::ExternalMem>::external")
            && arena_ids_rs.contains("crate::core_types::MemIdView::External(external)")
            && os_rs.contains("crate::core_types::MemIdView::External(external)")
            && arena_ids_rs.contains("let os_size = external.size.get();")
            && arena_debug_rs
                .contains("let page_slice_count = match crate::core_types::MemId::view(unsafe { (*raw).memid })")
            && arena_debug_rs.contains("crate::core_types::MemIdView::Arena(a) => a.range.count_usize() as isize")
            && os_rs.contains("external.size.get()")
            && os_rs.contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && memid_rs.contains("pub(crate) fn os_huge(")
            && memid_rs.contains("fn os_with_kind(")
            && internal_memid.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os(")
            && os_rs.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(")
            && !os_rs.contains("mi_memkind_is_os(memid.memkind)")
            && !os_rs.contains("(*memid).memkind = crate::internal::mi_memkind_t::MI_MEM_OS_HUGE")
            && !arena_rs.contains("memid.mem.os.")
            && !arena_rs.contains("memid.mem.arena.")
            && !arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_EXTERNAL")
            && !arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_NONE")
            && !os_rs.contains("memid.mem.os.")
            && !internal.contains("memid.mem.os.base = base")
            && !internal.contains("memid.mem.os.size = size")
            && !internal_memid.contains("memid.mem.os.base = base")
            && !internal_memid.contains("memid.mem.os.size = size")
            && page_map.contains("fn mi_memid_os_base_size(memid: mi_memid_t)")
            && page_map.contains("crate::core_types::MemIdView::Os(os)")
            && page_map.contains("crate::core_types::MemIdView::External(external)")
            && !page_map.contains("memid.mem.os.")
            && !page_map.contains("mi_page_map_memid.mem.os.")
            && theap_rs.contains("let allocated_size = match crate::core_types::MemId::view(memid)")
            && theap_rs.contains("crate::core_types::MemIdView::Arena(a)")
            && theap_rs.contains("a.range.count_usize() * crate::arena::MI_ARENA_SLICE_SIZE")
            && theap_rs.contains("fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool")
            && theap_rs.contains("crate::core_types::MemIdView::Unknown(code)")
            && theap_rs.contains("code.0 > crate::core_types::MemKindCode::STATIC.0")
            && theap_rs.contains("match crate::core_types::MemId::view(memid)")
            && theap_rs.contains("crate::core_types::MemIdView::HeapMain(_)")
            && theap_rs.contains("crate::core_types::MemIdView::Meta(_)")
            && theap_rs.contains("mi_memid_is_refcounted((*theap.as_ptr()).memid)")
            && atomic_types.contains(
                "pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);"
            )
            && atomic_types.contains("pub(crate) unsafe fn incref_acq_rel(self) -> usize")
            && atomic_types.contains("pub(crate) unsafe fn decref_acq_rel(self) -> usize")
            && ptr_rs.contains("pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount")
            && theap_rs.matches(".refcount_cell()").count() == 3
            && theap_rs.contains(".incref_acq_rel();")
            && theap_rs.contains(".decref_acq_rel()")
            && !theap_rs.contains("AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))")
            && !theap_rs.contains("mi_atomic_increment_acq_rel(ptr::addr_of_mut!((*theap).refcount))")
            && !theap_rs.contains("mi_atomic_fetch_sub_explicit(\n            ptr::addr_of_mut!((*theap).refcount),")
            && !theap_rs.contains("memid.mem.os.")
            && !theap_rs.contains("memid.memkind")
            && init_rs.contains("fn mi_memid_is_static(memid: mi_memid_t) -> bool")
            && init_rs.contains("crate::core_types::MemIdView::Static(_)")
            && memid_rs.contains("impl MemId<StaticMem>")
            && memid_rs.contains("pub(crate) const fn static_mem() -> Self")
            && init_rs.contains(
                "crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw()"
            )
            && init_rs.contains("!mi_memid_is_static((*main_prefix).memid)")
            && init_rs.contains("!mi_memid_is_static(theap_main.memid)")
            && !init_rs.contains(
                "crate::internal::_mi_memid_create(mi_memkind_t::MI_MEM_STATIC)"
            )
            && !init_rs.contains("(*main_prefix).memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && !init_rs.contains("theap_main.memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && internal.contains("pub(crate) mod memid;")
            && internal.contains("pub use memid::*;")
            && !internal.contains("pub enum mi_memkind_t")
            && internal_memid.contains("//! Raw memory-origin identifiers from mimalloc's internal header.")
            && internal_memid.contains("pub enum mi_memkind_t")
            && internal_memid.contains("pub fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && internal_memid.contains("pub fn mi_memid_needs_no_free(memid: mi_memid_t) -> bool")
            && internal.contains("pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool")
            && internal_memid.contains("crate::core_types::MemIdView::None(_)")
            && internal_memid.contains("crate::core_types::MemIdView::External(_)")
            && internal_memid.contains("crate::core_types::MemIdView::Static(_)")
            && internal.contains("crate::core_types::MemIdView::Arena(_)")
            && !internal.contains("pub unsafe fn _mi_memid_create(")
            && !internal_memid.contains("pub unsafe fn _mi_memid_create(")
            && !internal.contains("raw.memkind = memkind")
            && internal.contains("mi_page_is_in_arena(page)")
            && arena_meta_rs.contains("MemId::<MetaMem>::meta(")
            && arena_meta_rs.contains("NonZeroU32::new_unchecked(block_count as u32)")
            && memid_rs.contains("impl MemId<MetaMem>")
            && memid_rs.contains("is_pinned: true")
            && memid_rs.contains("initially_committed: true")
            && memid_rs.contains("initially_zero: true")
            && !internal.contains("crate::types::mi_memkind_is_os(memid.memkind)")
            && !internal.contains("crate::types::mi_memkind_needs_no_free(memid.memkind)")
            && !internal.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !internal.contains("memid.mem.meta.meta_page = mpage")
            && !internal.contains("memid.mem.meta.block_index = block_idx as u32")
            && !internal.contains("memid.mem.meta.block_count = block_count as u32")
            && !internal_memid.contains("memid.mem.meta.meta_page = mpage")
            && !internal_memid.contains("memid.mem.meta.block_index = block_idx as u32")
            && !internal_memid.contains("memid.mem.meta.block_count = block_count as u32")
            && arena_meta_rs.contains(
                "if let crate::core_types::MemIdView::Meta(meta) = crate::core_types::MemId::view(memid)"
            )
            && arena_meta_rs.contains("let block_count = meta.block_count.get() as usize;")
            && arena_meta_rs.contains("let block_idx = meta.block_index.0 as usize;")
            && !arena_meta_rs.contains("memid.mem.meta.")
            && !arena_meta_rs.contains("memid.memkind == crate::internal::mi_memkind_t::MI_MEM_META")
            && !arena_rs.contains("const fn mi_memkind_is_os(memkind: mi_memkind_t)")
            && !arena_rs.contains("mi_memkind_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.memkind })")
            && !arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            )
            && !arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange)"
            )
            && !arena_rs.contains(
                "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            ),
        "arena free/purge path should dispatch through MemIdView and carry SliceRange"
    );
    assert!(
        page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    submap: *mut mi_submap_t,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_set_range_prim(\n    page: PagePtr,\n    mut idx: crate::core_types::PageMapIndex,\n    mut sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_clear_range_prim("
        ) && page_map.contains(
            "unsafe fn mi_page_map_set_range(\n    page: PagePtr,\n    idx: crate::core_types::PageMapIndex,\n    sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_clear_range("
        ) && page_map.contains("unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool")
            && page_map.contains("unsafe fn mi_page_map_unregister_typed(page: PagePtr)")
            && page_map.contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && page_map.contains("unsafe { mi_page_map_register_typed(page) }")
            && page_map.contains("unsafe { mi_page_map_unregister_typed(page) };")
            && !page_map.contains("unsafe fn mi_page_map_ensure_committed(idx: usize, slice_count: usize)")
            && !page_map.contains("unsafe fn mi_page_map_ensure_committed(idx: usize, submap:")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    mut slice_count: usize,")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    slice_count: crate::core_types::SliceCount,")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: crate::core_types::PageMapIndex,")
            && !page_map.contains("unsafe fn mi_page_map_set_range(\n    page: *mut mi_page_t,\n    idx: usize,\n    sub_idx: usize,\n    slice_count: usize,")
            && !page_map.contains("fn mi_page_map_get_idx(\n    page: *mut mi_page_t,"),
        "page-map range helpers should validate raw indexes/counts once and carry typed indexes plus SliceCount internally"
    );
    assert!(
        arena_rs.contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && memid_rs.contains("slice_index: range.start_u32()")
            && memid_rs.contains("slice_count: range.count_u32()")
            && page_map.contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && !arena_rs.contains(" as u32")
            && !page_map.contains(" as u32")
            && !memid_rs.contains(" as u32")
            && !arena_rs.contains("SliceRange::from_raw(slice_index as")
            && !arena_rs.contains("SliceRange::from_raw(slice_index, slice_count as")
            && !page_map.contains("SliceCount::from_usize(slice_count as"),
        "arena and page-map slice fields should use checked typed conversions instead of lossy casts"
    );
    assert!(
        page_types.contains("pub(crate) struct RegisteredPageRange {\n    page: PagePtr,\n    index: PageMapIndex,\n    sub_index: PageMapSubIndex,\n    slice_count: SliceCount,\n}")
            && page_types.contains("pub(crate) unsafe fn new_unchecked(\n        page: PagePtr,\n        index: PageMapIndex,\n        sub_index: PageMapSubIndex,\n        slice_count: SliceCount,\n    ) -> Self")
            && page_types.contains("pub(crate) fn page(self) -> PagePtr")
            && page_types.contains("pub(crate) fn index(self) -> PageMapIndex")
            && page_types.contains("pub(crate) fn sub_index(self) -> PageMapSubIndex")
            && page_types.contains("pub(crate) fn slice_count(self) -> SliceCount")
            && page_map.contains("crate::core_types::RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)\n    };")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.index() == idx);")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.sub_index() == sub_idx);")
            && page_map.contains("crate::internal::mi_debug_assert!(registered.slice_count() == slice_count);")
            && page_map.contains("Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })"),
        "RegisteredPageRange should be an opaque page-map registration witness over PagePtr, PageMapIndex, PageMapSubIndex, and SliceCount"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && atomic_types
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_types.contains("expected: &mut *mut T,")
            && page_map.contains(
                "crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });"
            )
            && page_map.contains("atomic_submap.load(crate::core_types::MemoryOrder::Acquire)")
            && page_map.contains("atomic_submap.compare_exchange_acq_rel(&mut expect, new_sub)")
            && !page_map.contains("mi_atomic_load_ptr_acquire(_mi_page_map.add(idx))")
            && !page_map.contains(
                "mi_atomic_cas_strong_acq_rel(\n                        _mi_page_map.add(idx).cast::<usize>()"
            ),
        "page-map submap slot atomics should use AtomicPtrField"
    );
    assert!(
        internal.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of!((*theap).heap).cast_mut())\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && internal.contains(
            "crate::core_types::AtomicPtrField(_mi_page_map_base().add(idx))\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && !internal.contains("crate::atomic::mi_atomic_load_ptr_acquire(&(*theap).heap)")
            && !internal.contains("crate::atomic::mi_atomic_load_ptr_acquire(_mi_page_map_base().add(idx))"),
        "internal thread-heap and page-map pointer loads should use AtomicPtrField"
    );
    assert!(
        arena_ids_rs.contains(
            "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> mi_memid_t"
        ) && arena_rs.contains("memid: unsafe { mi_memid_create_arena(arena, range) },")
            && arena_ids_rs.contains("let Some((arena, range)) = (unsafe { mi_arena_range_from_memid(memid) }) else")
            && arena_ids_rs.contains("unsafe { *slice_index = range.start_usize() };")
            && arena_ids_rs.contains("unsafe { *slice_count = range.count_usize() };")
            && scalar_types.contains("pub(crate) fn as_u32(self) -> Option<u32>")
            && scalar_types.contains("pub(crate) const fn from_usize_const(count: usize) -> Option<Self>")
            && arena_types.contains("pub(crate) fn start_u32(self) -> u32")
            && arena_types.contains("pub(crate) const fn count_u32(self) -> u32")
            && memid_rs.contains("slice_index: range.start_u32()")
            && memid_rs.contains("slice_count: range.count_u32()")
            && arena_rs.contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && !arena_rs.contains("mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) as u32")
            && !arena_rs.contains("mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) as u32")
            && !arena_rs.contains("mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) as u32")
            && !memid_rs.contains("range.start().0 as u32")
            && !arena_ids_rs.contains(
                "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    slice_index: usize,\n    slice_count: usize,"
            )
            && !arena_ids_rs.contains(
                "SliceRange::from_raw(slice_index as u32, slice_count as u32)"
            ),
        "arena memid construction should receive an already-typed SliceRange"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_try_alloc_at(\n    arena: ArenaPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(") -> Option<(*mut c_void, crate::core_types::ArenaAllocation)>")
            && arena_types.contains(
                "pub(crate) struct ArenaAllocation {\n    pub(crate) arena: ArenaPtr,\n    pub(crate) range: SliceRange,\n    pub(crate) memid: crate::internal::mi_memid_t,\n}"
            )
            && arena_rs.contains("let mut allocation = crate::core_types::ArenaAllocation {")
            && arena_rs.contains("unsafe { *memid = allocation.assume_init().memid };")
            && arena_rs.contains("mi_arena_try_alloc_at(arena, slice_count, commit, tseq)")
            && !arena_rs.contains(
                "unsafe fn mi_arena_try_alloc_at(\n    arena: *mut mi_arena_t,\n    slice_count: crate::core_types::SliceCount,"
            )
            && !arena_rs.contains("mi_arena_try_alloc_at(arena, slice_count.get() as usize"),
        "arena allocation helper should accept typed SliceCount and return ArenaAllocation"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arenas_try_alloc(\n    heap: BorrowedHeap,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("    alignment: crate::core_types::Alignment,")
            && arena_rs.contains("    req_arena: Option<ArenaPtr>,")
            && arena_rs.contains("    numa_node: crate::core_types::NumaNode,")
            && arena_rs.contains("let Some(subproc) = (unsafe { heap.ptr().subproc() }) else")
            && arena_rs.contains("heap: HeapPtr::new(heap),")
            && arena_rs.contains("crate::core_types::NumaNode::new(numa_node),")
            && arena_rs.contains("req_arena: ArenaPtr::new(req_arena),")
            && arena_rs.contains("let Some(heap_ptr) = HeapPtr::new(heap) else")
            && arena_rs.contains("BorrowedHeap::new_unchecked(heap_ptr),")
            && !arena_rs.contains(
                "unsafe fn mi_arenas_try_alloc(\n    heap: *mut mi_heap_t,"
            ),
        "arena try-alloc should receive typed heap, requested arena, slice count, and NUMA node below raw entry points"
    );
    assert!(
        arena_types.contains("#[repr(transparent)]\npub(crate) struct SharedArena(pub(crate) ArenaPtr);")
            && arena_types.contains("#[repr(transparent)]\npub(crate) struct ExclusiveArena(pub(crate) ArenaPtr);")
            && arena_types.contains("#[repr(transparent)]\npub(crate) struct ParentArena(pub(crate) ArenaPtr);")
            && arena_types.matches("SAFETY[AUDIT:typed-arena-domain]").count() >= 7
            && arena_types.contains("impl SharedArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self>")
            && arena_types.contains(
                "before packaging the shared-arena witness.\n        if unsafe { arena.is_exclusive() } {\n            None"
            )
            && arena_types.contains("impl ExclusiveArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self>")
            && arena_types.contains(
                "before packaging the exclusive-arena witness.\n        if unsafe { arena.is_exclusive() } {\n            Some(Self(arena))"
            )
            && arena_types.contains("impl ParentArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self>")
            && arena_types.contains(
                "packaging a root parent-arena witness.\n        if unsafe { arena.parent().is_none() } {\n            Some(Self(arena))"
            ),
        "arena role wrappers should have checked constructors based on raw arena state"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_pages_size(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_info_slices_needed(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_bitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_bbitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_initialize(\n    subproc: SubprocPtr,\n    start: OsMemPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("let slice_count_usize = slice_count.get() as usize;")
            && arena_rs.contains("let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && arena_rs.contains("mi_arena_initialize(\n                SubprocPtr::new_unchecked(subproc),\n                OsMemPtr::new_unchecked(start),\n                typed_slice_count,")
            && !arena_rs.contains("unsafe fn mi_arena_pages_size(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_info_slices_needed(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_bitmap_init(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_bbitmap_init(slice_count: usize"),
        "arena metadata sizing and initialization should receive typed SliceCount"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && arena_rs.contains("let block_size_typed = unsafe { crate::core_types::BlockSize::new_unchecked(block_size) };")
            && arena_rs.contains(
                "let block_alignment_typed =\n        unsafe { crate::core_types::MaybeAlignment::new_unchecked(block_alignment) };",
            )
            && arena_rs.contains("const MI_SMALL_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("const MI_MEDIUM_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("const MI_LARGE_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_SMALL_PAGE_SLICE_COUNT, block_size_typed)")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_MEDIUM_PAGE_SLICE_COUNT, block_size_typed)")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_LARGE_PAGE_SLICE_COUNT, block_size_typed)")
            && !arena_rs.contains("unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: usize,"),
        "arena page allocation helpers should carry typed SliceCount after raw entry-point validation"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_now()")
            && stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_start()")
            && stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_end(")
            && stats_rs.contains("let t0 = _mi_clock_now();")
            && stats_rs.contains("let diff = _mi_clock_now() - t0;")
            && stats_rs.contains("_mi_clock_now() - start - CLOCK_DIFF.load")
            && os_rs.contains("let start_t = _mi_clock_start();")
            && os_rs.contains("let mut elapsed = _mi_clock_end(start_t);")
            && arena_rs.contains("let expire = crate::stats::_mi_clock_now() + delay as i64;")
            && arena_rs.contains("let now = crate::stats::_mi_clock_now();"),
        "clock helpers should keep C's timing helper boundaries and main callers"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool")
            && arena_rs.matches("crate::core_types::AtomicI64Field(").count() == 7
            && arena_rs
                .matches(".compare_exchange_acq_rel(&mut expire0, expire)")
                .count()
                == 2
            && arena_rs.contains(".load(crate::core_types::MemoryOrder::Relaxed)")
            && arena_rs.contains(".load(crate::core_types::MemoryOrder::Acquire)")
            && arena_rs
                .matches(".store(0, crate::core_types::MemoryOrder::Release)")
                .count()
                == 2
            && arena_rs.contains(
                "now + ((delay as mi_msecs_t) / 10),\n                    crate::core_types::MemoryOrder::Release,"
            )
            && !arena_rs.contains("mi_atomic_casi64_strong_acq_rel")
            && !arena_rs.contains("mi_atomic_loadi64_explicit")
            && !arena_rs.contains("mi_atomic_loadi64_acquire")
            && !arena_rs.contains("mi_atomic_storei64_release")
            && !arena_rs.contains("mi_memory_order_e::mi_memory_order_relaxed"),
        "arena purge expiry should use typed atomic i64 field wrappers"
    );
    assert!(
            atomic_types.contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && atomic_types.contains("desired: usize,")
            && atomic_types.contains(
                "atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)"
            )
            && arena_rs
                .matches("ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)")
                .count()
                == 2
            && arena_rs.contains(".compare_exchange_acq_rel(&mut expected, 0)")
            && arena_rs.contains("let next_count = count + 1;")
            && arena_rs.contains(".compare_exchange_acq_rel(&mut count, next_count)")
            && !arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            )
            && !arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            ),
        "arena_count compare-exchange should use AtomicUsizeField"
    );
    assert!(
        atomic_types
            .contains("pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);")
            && atomic_types.contains("impl AtomicAbandonedCount")
            && atomic_types.contains("pub(crate) unsafe fn load_relaxed(self) -> usize")
            && atomic_types
                .contains("pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize")
            && atomic_types
                .contains("pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize")
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_heap_abandoned_count(\n    heap: *mut mi_heap_t,\n    bin: crate::core_types::Bin,"
            )
            && arena_prefix_rs.contains("abandoned_count[bin.as_usize()]")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1)")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1)")
            && !arena_rs.contains(".abandoned_count[bin].load(Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[bin].fetch_add(1, Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[bin].fetch_sub(1, Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[i].load(Ordering::Relaxed)"),
        "arena abandoned-count atomics should go through AtomicAbandonedCount with typed Bin indexes"
    );
    assert!(
        atomic_types
            .contains("pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);")
            && atomic_types.contains("impl AtomicArenaPage")
            && atomic_types.contains(
                "pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t"
            )
            && atomic_types.contains(
                "pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t)"
            )
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_arena_index(arena: *mut mi_arena_t) -> crate::core_types::ArenaIndex"
            )
            && !arena_rs.contains(
                "unsafe fn mi_heap_arena_page_cell(\n    heap: *mut mi_heap_t,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_heap_arena_page_cell(\n    heap: BorrowedHeap,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && arena_rs.contains(
                "unsafe fn mi_heap_ensure_arena_pages(\n    heap: BorrowedHeap,\n    arena: ArenaPtr,\n    arena_idx: ArenaIndex,"
            )
            && arena_prefix_rs.contains("arena_pages[arena_idx.get() as usize]")
            && arena_rs.matches("mi_heap_arena_page_cell(heap, arena_idx).load_acquire()").count()
                == 1
            && arena_rs
                .matches("mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire()")
                .count()
                == 1
            && arena_rs
                .contains("mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast())")
            && !arena_rs.contains(".arena_pages[arena_idx].load(Ordering::Acquire)")
            && !arena_rs.contains(".arena_pages[arena_idx]\n                    .store")
            && !arena_rs.contains(".arena_pages[i].load(Ordering::Acquire)"),
        "heap arena page-table atomics should use AtomicArenaPage with typed ArenaIndex values"
    );
    assert!(
        atomic_types
            .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_release(")
            && arena_prefix_rs
                .matches("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(")
                .count()
                == 1
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_subproc_arena_cell(\n    subproc: *mut mi_subproc_t,\n    arena_idx: ArenaIndex,"
            )
            && arena_prefix_rs.contains(
                "pub(super) unsafe fn mi_arena_from_index_typed(\n    subproc: SubprocPtr,\n    arena_idx: ArenaIndex,"
            )
            && arena_prefix_rs
                .matches("(*subproc.cast::<MiSubprocPrefix>()).arenas[")
                .count()
                == 1
            && arena_rs.contains("let Some(arena_idx) = ArenaIndex::from_usize(idx)")
            && arena_rs
                .matches("mi_arena_from_index_typed(")
                .count()
                >= 5
            && arena_rs
                .matches(".compare_exchange_release(&mut expected, arena)")
                .count()
                == 2
            && arena_rs
                .contains(".store(ptr::null_mut(), crate::core_types::MemoryOrder::Release);")
            && !arena_rs.contains("mi_arena_from_index(subproc, i)")
            && !arena_rs.contains("mi_atomic_cas_ptr_strong_release")
            && !arena_rs.contains("mi_atomic_store_ptr_release")
            && !arena_rs.contains("mi_atomic_load_ptr_acquire"),
        "arena table pointer atomics should use AtomicPtrField with typed ArenaIndex values"
    );

    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc_aligned"
        ) && arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc")
            && arena_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_contain")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all"
            )
            && arena_rs.contains("mi_arenas_try_alloc(")
            && arena_rs.contains("mi_arena_os_alloc_aligned(")
            && arena_rs.contains("_mi_arenas_alloc_aligned(")
            && arena_rs.contains("mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), None)")
            && arena_rs.contains("static MI_ARENAS_CONTAIN_KEEP")
            && arena_rs.contains("mi_arenas_unsafe_destroy(subproc)")
            && theap_rs.contains("let theap = _mi_arenas_alloc(")
            && init_rs.contains("_mi_arenas_unsafe_destroy_all(subproc)"),
        "arena allocation, containment, and unsafe destroy helpers should keep C's local call boundaries"
    );
    assert!(
        arena_rs.contains("struct ArenaAllocTypedArgs {\n    heap: Option<HeapPtr>,\n    layout: crate::core_types::LayoutRequest,\n    align_offset: crate::core_types::AlignmentOffset,")
            && arena_rs.contains("unsafe fn mi_arenas_alloc_aligned_typed(args: ArenaAllocTypedArgs)")
            && arena_rs.contains("let size = layout.requested();")
            && arena_rs.contains("let alignment = layout.alignment();")
            && arena_rs.contains("arena_slice_count: Option<crate::core_types::SliceCount>,")
            && arena_rs.contains("fn mi_arena_slice_count_for_request(\n    size: crate::core_types::RequestedSize,")
            && arena_rs.contains("crate::core_types::SliceCount::from_usize(mi_slice_count_of_size(size.get()))")
            && arena_rs.contains("arena_slice_count: mi_arena_slice_count_for_request(layout.requested())")
            && arena_rs.contains("crate::core_types::LayoutRequest::aligned_at(")
            && arena_rs.contains("crate::core_types::AlignmentOffset::new(align_offset)")
            && arena_rs.contains("struct ArenaAllocArgs {\n    heap: Option<HeapPtr>,\n    size: crate::core_types::RequestedSize,")
            && arena_rs.contains("unsafe fn mi_arenas_alloc_typed(args: ArenaAllocArgs)")
            && arena_rs.contains("crate::core_types::LayoutRequest::aligned(size.get(), alignment)")
            && arena_rs.contains("let size = crate::core_types::RequestedSize::new(size);")
            && arena_rs.contains("crate::core_types::AlignmentOffset::new(0)")
            && arena_rs.contains("req_arena: ArenaPtr::new(req_arena)")
            && arena_rs.contains("tseq: crate::core_types::ThreadSeq::new(tseq)")
            && arena_rs.contains("numa_node: crate::core_types::NumaNode::new(numa_node)"),
        "arena allocation ABI wrappers should parse raw size/alignment into typed layout domains before using arena internals"
    );
    let reabandon_arena = source_item(
        "src/allocator/arena.rs",
        arena_rs,
        "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena",
    );
    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped"
        )
            && arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(\n    page: crate::core_types::OwnedAbandonedPage,\n    _arena_memid: crate::core_types::ArenaMemId,"
            )
            && arena_ids_rs
                .contains("unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool")
            && arena_ids_rs.contains("crate::core_types::MemId::view(unsafe { (*page).memid })")
            && arena_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !arena_rs.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !arena_rs.contains("(*page).memid.memkind != mi_memkind_t::MI_MEM_ARENA")
            && arena_rs.contains("_mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page))")
            && arena_rs.contains("_mi_arenas_page_abandon(page, theap)")
            && !reabandon_arena.contains("mi_page_memid_is_arena(page)")
            && !reabandon_arena.contains("MemId::view")
            && free_rs.contains("crate::core_types::MemId::view((*page).memid)")
            && free_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !free_rs.contains(
                "(*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA"
            )
            && free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped_arena(")
            && !free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped(page)"),
        "free path should validate arena memids and carry the witness into the crate-local reabandon helper"
    );
    assert!(
        arena_ids_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arena_id_none")
            && arena_ids_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_arena_from_id")
            && arena_ids_rs
                .contains("pub(crate) unsafe extern \"C\" fn _mi_arena_memid_is_suitable")
            && arena_ids_rs.contains("static MI_ARENA_ID_NONE_KEEP")
            && arena_ids_rs.contains("static MI_ARENA_FROM_ID_KEEP")
            && arena_ids_rs.contains("unsafe { _mi_arena_id_none() }")
            && arena_rs.contains("let arena = unsafe { _mi_arena_from_id(arena_id) };")
            && heap_rs.contains("(*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);")
            && free_rs.contains("_mi_arena_memid_is_suitable("),
        "arena id conversion and memid suitability helpers should keep C's local call boundaries"
    );
}
