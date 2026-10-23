use super::*;

#[repr(C)]
pub(super) struct MiHeapPrefix {
    pub(super) subproc: *mut mi_subproc_t,
    pub(super) heap_seq: usize,
    pub(super) next: *mut mi_heap_t,
    pub(super) prev: *mut mi_heap_t,
    pub(super) theap: mi_thread_local_t,
    pub(super) exclusive_arena: *mut mi_arena_t,
    pub(super) numa_node: i32,
    pub(super) _padding: u32,
    pub(super) theaps: *mut mi_theap_t,
    pub(super) theaps_lock: crate::atomic::mi_lock_t,
    pub(super) abandoned_count: [AtomicUsize; MI_BIN_COUNT],
    pub(super) os_abandoned_pages: *mut mi_page_t,
    pub(super) os_abandoned_pages_lock: crate::atomic::mi_lock_t,
    pub(super) arena_pages: [AtomicPtr<c_void>; MI_MAX_ARENAS],
    pub(super) arena_pages_lock: crate::atomic::mi_lock_t,
}

pub(super) fn mi_heap_abandoned_count(
    counts: &[AtomicUsize; MI_BIN_COUNT],
    bin: crate::core_types::Bin,
) -> crate::core_types::AtomicAbandonedCount<'_> {
    crate::core_types::AtomicAbandonedCount::from_cell(&counts[bin.as_usize()])
}

pub(super) unsafe fn mi_arena_index(arena: *mut mi_arena_t) -> crate::core_types::ArenaIndex {
    let arena_idx = unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx };
    crate::internal::mi_assert!(arena_idx < MI_MAX_ARENAS);
    unsafe { crate::core_types::ArenaIndex::new_unchecked(arena_idx as u16) }
}

pub(super) fn mi_heap_arena_page_cell(
    pages: &[AtomicPtr<c_void>; MI_MAX_ARENAS],
    arena_idx: crate::core_types::ArenaIndex,
) -> crate::core_types::AtomicArenaPage<'_> {
    crate::core_types::AtomicArenaPage::from_cell(&pages[arena_idx.get() as usize])
}

#[repr(C)]
pub(super) struct MiSubprocPrefix {
    pub(super) subproc_seq: usize,
    pub(super) next: *mut mi_subproc_t,
    pub(super) prev: *mut mi_subproc_t,
    pub(super) arena_count: AtomicUsize,
    pub(super) arenas: [*mut mi_arena_t; MI_MAX_ARENAS],
    pub(super) arena_reserve_lock: crate::atomic::mi_lock_t,
    pub(super) purge_expire: AtomicI64,
    pub(super) heap_main: *mut mi_heap_t,
    pub(super) heaps: *mut mi_heap_t,
    pub(super) heaps_lock: crate::atomic::mi_lock_t,
    pub(super) thread_count: AtomicUsize,
    pub(super) thread_total_count: AtomicUsize,
    pub(super) heap_count: AtomicUsize,
}

pub(super) unsafe fn mi_subproc_arena_field(
    subproc: *mut mi_subproc_t,
    arena_idx: ArenaIndex,
) -> *const AtomicPtr<mi_arena_t> {
    unsafe {
        ptr::addr_of!((*subproc.cast::<MiSubprocPrefix>()).arenas[arena_idx.get() as usize]).cast()
    }
}

/// Load one published arena registry slot with acquire ordering.
///
/// # Safety
/// `subproc` must remain live and initialized with stable arena table storage,
/// and `arena_idx` must be below its published arena count. Concurrent registry
/// growth may use its publication protocol, but teardown or relinking existing
/// entries must not race this load.
pub(super) unsafe fn mi_arena_from_index_typed(
    subproc: SubprocPtr,
    arena_idx: ArenaIndex,
) -> *mut mi_arena_t {
    // SAFETY: the caller keeps this bounded registry cell live; acquire matches
    // the publication protocol before exposing the arena pointer.
    unsafe {
        crate::core_types::AtomicPtrField::from_cell(&*mi_subproc_arena_field(
            subproc.as_ptr(),
            arena_idx,
        ))
        .load(crate::core_types::MemoryOrder::Acquire)
    }
}

#[repr(C)]
pub(super) struct MiArenaPrefix {
    pub(super) memid: crate::internal::memid::mi_memid_t,
    pub(super) subproc: *mut mi_subproc_t,
    pub(super) arena_idx: usize,
    pub(super) slice_count: usize,
    pub(super) info_slices: usize,
    pub(super) numa_node: i32,
    pub(super) is_exclusive: bool,
    pub(super) _padding: [u8; 7],
    pub(super) purge_expire: AtomicI64,
    pub(super) commit_fun: Option<mi_commit_fun_t>,
    pub(super) commit_fun_arg: *mut c_void,
    pub(super) total_size: usize,
    pub(super) parent: *mut mi_arena_t,
    pub(super) slices_free: *mut crate::bitmap::mi_bbitmap_t,
    pub(super) slices_committed: *mut crate::bitmap::mi_bitmap_t,
    pub(super) slices_dirty: *mut crate::bitmap::mi_bitmap_t,
    pub(super) slices_purge: *mut crate::bitmap::mi_bitmap_t,
    pub(super) pages_meta: *mut mi_page_t,
    pub(super) pages_main: MiArenaPagesPrefix,
}

#[repr(C)]
pub(super) struct MiArenaPagesPrefix {
    pub(super) pages: *mut mi_bitmap_t,
    pub(super) pages_abandoned: [*mut mi_bitmap_t; MI_ARENA_BIN_COUNT],
}

#[allow(non_camel_case_types)]
pub(super) type mi_arena_pages_t = MiArenaPagesPrefix;
