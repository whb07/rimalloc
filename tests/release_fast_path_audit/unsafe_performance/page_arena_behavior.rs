use super::*;

#[path = "page_arena_behavior/arena_helpers.rs"]
mod arena_helpers;
#[path = "page_arena_behavior/arena_page_allocation.rs"]
mod arena_page_allocation;
#[path = "page_arena_behavior/memid_arena_dispatch.rs"]
mod memid_arena_dispatch;
#[path = "page_arena_behavior/page_map_atomic_helpers.rs"]
mod page_map_atomic_helpers;
#[path = "page_arena_behavior/page_map_range_witnesses.rs"]
mod page_map_range_witnesses;
#[path = "page_arena_behavior/purge_clock_atomic.rs"]
mod purge_clock_atomic;

pub(super) struct PageArenaBehaviorSources<'a> {
    pub(super) page_map: &'a str,
    pub(super) internal: &'a str,
    pub(super) internal_memid: &'a str,
    pub(super) arena_rs: &'a str,
    pub(super) arena_debug_rs: &'a str,
    pub(super) arena_ids_rs: &'a str,
    pub(super) arena_os_memory_rs: &'a str,
    pub(super) arena_prefix_rs: &'a str,
    pub(super) arena_purge_rs: &'a str,
    pub(super) arena_meta_rs: &'a str,
    pub(super) free_rs: &'a str,
    pub(super) memid_rs: &'a str,
    pub(super) arena_types: &'a str,
    pub(super) page_types: &'a str,
    pub(super) scalar_types: &'a str,
    pub(super) atomic_types: &'a str,
    pub(super) ptr_rs: &'a str,
    pub(super) os_rs: &'a str,
    pub(super) stats_rs: &'a str,
    pub(super) theap_rs: &'a str,
    pub(super) init_rs: &'a str,
    pub(super) heap_rs: &'a str,
}

pub(super) fn assert_page_arena_behavior_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let alloc_rs = sources.alloc_rs.as_str();
    let alloc_aligned_rs = sources.alloc_aligned_rs.as_str();
    let arena_debug_rs = sources.arena_debug_rs.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let bitmap_rs = sources.bitmap_rs.as_str();
    let bitmap_boundary_rs = format!("{bitmap_rs}\n{}", sources.bitmap_chunkmap_rs);
    let callbacks_rs = sources.callbacks_rs.as_str();
    let freelist_types = sources.freelist_types.as_str();
    let page_rs = sources.page_rs.as_str();
    let page_counts_rs = sources.page_counts_rs.as_str();
    let page_flags_rs = sources.page_flags_rs.as_str();
    let page_queues_rs = sources.page_queues_rs.as_str();
    let page_states_rs = sources.page_states_rs.as_str();
    let page_stats_bin_rs = sources.page_stats_bin_rs.as_str();
    let page_validation_rs = sources.page_validation_rs.as_str();
    let threadlocal_rs = sources.threadlocal_rs.as_str();

    let sources = PageArenaBehaviorSources {
        page_map: sources.page_map.as_str(),
        internal: sources.internal.as_str(),
        internal_memid: sources.internal_memid.as_str(),
        arena_rs: sources.arena_rs.as_str(),
        arena_debug_rs,
        arena_ids_rs: sources.arena_ids_rs.as_str(),
        arena_os_memory_rs: sources.arena_os_memory_rs.as_str(),
        arena_prefix_rs: sources.arena_prefix_rs.as_str(),
        arena_purge_rs: sources.arena_purge_rs.as_str(),
        arena_meta_rs: sources.arena_meta_rs.as_str(),
        free_rs: sources.free_rs.as_str(),
        memid_rs: sources.memid_rs.as_str(),
        arena_types: sources.arena_types.as_str(),
        page_types: sources.page_types.as_str(),
        scalar_types: sources.scalar_types.as_str(),
        atomic_types: sources.atomic_types.as_str(),
        ptr_rs: sources.ptr_rs.as_str(),
        os_rs: sources.os_rs.as_str(),
        stats_rs: sources.stats_rs.as_str(),
        theap_rs: sources.theap_rs.as_str(),
        init_rs: sources.init_rs.as_str(),
        heap_rs: sources.heap_rs.as_str(),
    };

    arena_boundaries::assert_arena_region_boundaries(arena_boundaries::BoundarySources {
        arena_rs: sources.arena_rs,
        arena_os_memory_rs: sources.arena_os_memory_rs,
        arena_purge_rs: sources.arena_purge_rs,
        unsafe_allowlist,
    });

    page_lifecycle::assert_page_lifecycle_and_queue_boundaries(page_lifecycle::BoundarySources {
        alloc_rs,
        alloc_aligned_rs,
        arena_rs: sources.arena_rs,
        atomic_types: sources.atomic_types,
        free_rs: sources.free_rs,
        freelist_types,
        internal: sources.internal,
        page_counts_rs,
        page_flags_rs,
        page_queues_rs,
        page_rs,
        page_states_rs,
        page_stats_bin_rs,
        page_validation_rs,
        theap_rs: sources.theap_rs,
    });

    bitmap_boundaries::assert_bitmap_traversal_and_regular_boundaries(
        &bitmap_boundary_rs,
        unsafe_allowlist,
        sources.arena_rs,
        sources.arena_purge_rs,
        arena_heap_pages_rs,
        callbacks_rs,
        threadlocal_rs,
    );
    bitmap_boundaries::assert_bitmap_helper_boundaries(&bitmap_boundary_rs);
    arena_helpers::assert_arena_helper_boundaries(
        sources.arena_rs,
        arena_debug_rs,
        sources.arena_purge_rs,
        sources.arena_types,
    );
    memid_arena_dispatch::assert_memid_and_arena_dispatch_boundaries(&sources);
    page_map_range_witnesses::assert_page_map_range_witnesses(&sources);
    page_map_atomic_helpers::assert_page_map_atomic_helpers(&sources);
    arena_page_allocation::assert_arena_page_allocation_boundaries(&sources);
    purge_clock_atomic::assert_purge_clock_and_atomic_boundaries(&sources);
}
