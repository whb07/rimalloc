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

// SAFETY: callers pass live heap metadata whose
// prefix layout is checked against the translated C layout.
pub(super) unsafe fn mi_heap_abandoned_count(
    heap: *mut mi_heap_t,
    bin: crate::core_types::Bin,
) -> crate::core_types::AtomicAbandonedCount {
    // SAFETY: the heap prefix owns the abandoned
    // count array and `bin` is a checked bin index.
    unsafe {
        crate::core_types::AtomicAbandonedCount::from_cell(ptr::addr_of!(
            (*heap.cast::<MiHeapPrefix>()).abandoned_count[bin.as_usize()]
        ))
    }
}

// SAFETY: callers pass live arena metadata whose
// prefix carries the registered arena table index.
pub(super) unsafe fn mi_arena_index(arena: *mut mi_arena_t) -> crate::core_types::ArenaIndex {
    // SAFETY: the arena prefix is allocator-owned
    // and initialized before arena registration exposes it to callers.
    let arena_idx = unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx };
    crate::internal::mi_assert!(arena_idx < MI_MAX_ARENAS);
    // SAFETY: the bounds check above proves the
    // value fits the internal arena index domain.
    unsafe { crate::core_types::ArenaIndex::new_unchecked(arena_idx as u16) }
}

// SAFETY: callers pass a borrowed heap witness
// and a checked arena index for the heap arena-page cell.
pub(super) unsafe fn mi_heap_arena_page_cell(
    heap: BorrowedHeap,
    arena_idx: crate::core_types::ArenaIndex,
) -> crate::core_types::AtomicArenaPage {
    let heap = heap.ptr().as_ptr();
    // SAFETY: the heap prefix owns the arena page
    // table and `arena_idx` has already been checked for the table domain.
    unsafe {
        crate::core_types::AtomicArenaPage::from_cell(ptr::addr_of!(
            (*heap.cast::<MiHeapPrefix>()).arena_pages[arena_idx.get() as usize]
        ))
    }
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

// SAFETY: callers pass live subproc metadata and
// a checked arena index for the arena pointer table cell.
pub(super) unsafe fn mi_subproc_arena_cell(
    subproc: *mut mi_subproc_t,
    arena_idx: ArenaIndex,
) -> crate::core_types::AtomicPtrField<mi_arena_t> {
    // SAFETY: the subproc prefix owns the arena
    // table and `arena_idx` has already been checked for the table domain.
    unsafe {
        crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(
            (*subproc.cast::<MiSubprocPrefix>()).arenas[arena_idx.get() as usize]
        ))
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
        mi_subproc_arena_cell(subproc.as_ptr(), arena_idx)
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
