//! Arena metadata layout, registration, initialization, and teardown.
//!
//! This module owns construction and destruction of the arena metadata graph:
//! slice bitmaps, per-heap page tables, registry publication, and optional
//! separated page metadata. Allocation policy and raw compatibility entrypoints
//! remain in the parent module.

use super::*;

pub(super) fn mi_arena_pages_size(
    slice_count: crate::types::SliceCount,
) -> (crate::types::ReservedBytes, crate::types::ReservedBytes) {
    let slice_count = slice_count.get() as usize;
    crate::internal::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_pages_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 1 + MI_ARENA_BIN_COUNT;
    // SAFETY: the null chunk-count output means this size query performs no raw
    // pointer access, which is the unsafe callee's only caller obligation.
    let bitmaps_size = bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) };
    let size = base_size + bitmaps_size;
    (
        crate::types::ReservedBytes::new(size),
        crate::types::ReservedBytes::new(base_size),
    )
}

fn mi_arena_info_slices_needed(
    slice_count: crate::types::SliceCount,
    os_page_size: crate::types::OsPageSize,
    secure_guard_size: usize,
) -> (usize, usize) {
    let slice_count = slice_count.get() as usize;
    crate::internal::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 4 + MI_ARENA_BIN_COUNT;
    // SAFETY: the null chunk-count output means this size query performs no raw
    // pointer access, which is the unsafe callee's only caller obligation.
    let bitmaps_size = (bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) })
        // SAFETY: the null chunk-count output likewise means this size query
        // performs no raw pointer access.
        + unsafe { mi_bbitmap_size(slice_count, ptr::null_mut()) };
    let pages_size = if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        slice_count * size_of::<mi_page_t>()
    } else {
        0
    };
    let size = base_size + bitmaps_size + pages_size;
    let info_size = _mi_align_up(size, os_page_size.get()) + secure_guard_size;
    let info_slices = mi_slice_count_of_size(info_size);
    (info_slices, base_size)
}

/// # Safety
/// `*base` must point into a live zeroed metadata allocation with enough aligned
/// writable storage for a bitmap of `slice_count` bits. The advanced cursor
/// must remain within that same allocation and be used under exclusive access.
unsafe fn mi_arena_bitmap_init(
    slice_count: crate::types::SliceCount,
    base: &mut *mut u8,
) -> *mut mi_bitmap_t {
    let bitmap = (*base).cast::<mi_bitmap_t>();
    // SAFETY: the caller provides aligned zeroed writable storage sized for
    // this typed slice count and retains exclusive initialization access.
    let size = unsafe { mi_bitmap_init(bitmap, slice_count.get() as usize, true) };
    // SAFETY: the caller guarantees the initialized bitmap size remains within
    // the same metadata allocation; advancing preserves the cursor epoch.
    *base = unsafe { (*base).add(size) };
    bitmap
}

/// # Safety
/// `*base` must point into a live zeroed metadata allocation with enough aligned
/// writable storage for a bbitmap of `slice_count` bits. The advanced cursor
/// must remain within that same allocation and be used under exclusive access.
unsafe fn mi_arena_bbitmap_init(
    slice_count: crate::types::SliceCount,
    base: &mut *mut u8,
) -> *mut mi_bbitmap_t {
    let bbitmap = (*base).cast::<mi_bbitmap_t>();
    // SAFETY: the caller provides aligned zeroed writable storage sized for
    // this typed slice count and retains exclusive initialization access.
    let size = unsafe { mi_bbitmap_init(bbitmap, slice_count.get() as usize, true) };
    // SAFETY: the caller guarantees the initialized bbitmap size stays within
    // the same metadata allocation; advancing preserves the cursor epoch.
    *base = unsafe { (*base).add(size) };
    bbitmap
}

/// Allocate and initialize one per-heap arena-page bitmap table.
///
/// # Safety
/// `arena` must remain live with initialized stable slice count. `range` must
/// start at zero and fit that arena. `alloc_size` and `bitmap_base` must be the
/// matching values produced by `mi_arena_pages_size(range.count())`, large
/// enough for the main and every abandoned bitmap. The allocator's main heap
/// must be initialized; the caller owns publication or cleanup of the result.
pub(super) unsafe fn mi_arena_pages_alloc(
    arena: ArenaPtr,
    range: crate::types::SliceRange,
    alloc_size: crate::types::ReservedBytes,
    bitmap_base: crate::types::ReservedBytes,
) -> *mut mi_arena_pages_t {
    crate::internal::mi_assert_internal!(range.start_usize() == 0);
    crate::internal::mi_assert_internal!(
        // SAFETY: the caller keeps the live arena's initialized slice count
        // stable for validation against the typed range.
        range.end_usize() <= unsafe { arena.slice_count() }.get() as usize
    );
    let slice_count = range.count();
    // SAFETY: allocator initialization keeps the main heap live; the matching
    // layout size requests zeroed BCHUNK-aligned writable table storage.
    let arena_pages =
        unsafe { mi_heap_zalloc_aligned(mi_heap_main(), alloc_size.get(), MI_BCHUNK_SIZE) }
            .cast::<mi_arena_pages_t>();
    if arena_pages.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller-proven layout places bitmap_base within the returned
    // allocation and preserves BCHUNK alignment.
    let mut base = unsafe { arena_pages.cast::<u8>().add(bitmap_base.get()) };
    crate::internal::mi_assert_internal!((base as usize).is_multiple_of(MI_BCHUNK_SIZE));
    // SAFETY: the matching layout reserves zeroed aligned space for the main
    // bitmap, and this function exclusively advances the allocation cursor.
    unsafe { (*arena_pages).pages = mi_arena_bitmap_init(slice_count, &mut base) };
    for i in 0..MI_ARENA_BIN_COUNT {
        // SAFETY: each checked bin has one subsequent zeroed aligned bitmap in
        // the caller-proven allocation; cursor and table writes are exclusive.
        unsafe { (*arena_pages).pages_abandoned[i] = mi_arena_bitmap_init(slice_count, &mut base) };
    }
    arena_pages
}

/// # Safety
/// `subproc` must remain live and exclusively owned for final teardown. No
/// thread may concurrently inspect or mutate its arena registry, arenas, pages,
/// or accounting. Every published arena prefix and OS MemId must remain valid;
/// OS-owned arena backing must be releasable exactly once by this operation.
pub(super) unsafe fn mi_arenas_unsafe_destroy(subproc: SubprocPtr) {
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    // SAFETY: exclusive teardown keeps the live subproc's initialized arena
    // count stable while the registry bound is copied.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    for i in 0..arena_count {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY: exclusive teardown keeps the table live; the typed index is
        // within the stable count and registry capacity.
        let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
        if !arena.is_null() {
            // SAFETY: exclusive teardown owns the live registry cell selected
            // by the checked index and publishes its removal with release order.
            unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .store(ptr::null_mut(), crate::types::MemoryOrder::Release);
            }
            // SAFETY: the removed arena remains live with initialized stable
            // MemId while teardown decides whether it owns OS backing.
            if mi_memid_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid }) {
                // SAFETY: this OS-owned arena was unlinked exactly once; its
                // live prefix provides matching base, size, MemId, and subproc
                // for final committed backing release.
                unsafe {
                    crate::internal::_mi_os_free_ex(
                        mi_arena_start(ArenaPtr::new_unchecked(arena))
                            .as_ptr()
                            .cast::<c_void>(),
                        mi_arena_size(ArenaPtr::new_unchecked(arena)).get(),
                        true,
                        (*arena.cast::<MiArenaPrefix>()).memid,
                        subproc,
                    );
                }
            }
        }
    }
    let mut expected = arena_count;
    // SAFETY: exclusive teardown owns the live aligned atomic arena-count
    // field; all registry cells above are already cleared before publishing zero.
    let _ = unsafe {
        crate::types::AtomicUsizeField::from_raw(
            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
        )
        .compare_exchange_acq_rel(&mut expected, 0)
    };
}

/// # Safety
/// `subproc` must remain live with an initialized atomic arena registry.
/// `arena_ptr` must be a live fully initialized but unpublished arena belonging
/// to that subproc; its index and subproc fields are exclusively writable until
/// successful publication. If non-null, `arena_id` must be aligned and writable
/// for one output value. Concurrent registry growth must use this same atomic
/// protocol, and arena destruction is excluded for the operation.
unsafe fn mi_arenas_add(
    subproc: SubprocPtr,
    arena_ptr: ArenaPtr,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let subproc = subproc.as_ptr();
    let arena = arena_ptr.as_ptr();
    // SAFETY: the caller keeps the unpublished arena live with an initialized
    // non-zero stable slice count.
    crate::internal::mi_assert_internal!(unsafe { arena_ptr.slice_count() }.get() > 0);
    if !arena_id.is_null() {
        // SAFETY: the caller supplies aligned writable output storage until the
        // registration attempt completes.
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    // SAFETY: the live subproc registry and atomic count remain initialized;
    // concurrent growth follows the same atomic protocol.
    let mut count = unsafe { mi_arenas_get_count_typed(SubprocPtr::new_unchecked(subproc)) };
    for i in 0..count {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY: the checked index is within the acquired count snapshot and
        // the caller excludes registry destruction.
        if unsafe { mi_arena_from_index_typed(SubprocPtr::new_unchecked(subproc), arena_idx) }
            .is_null()
        {
            // SAFETY: before publication the caller exclusively owns this
            // arena's index field and supplies the checked candidate index.
            unsafe { arena_ptr.set_index(arena_idx) };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            // SAFETY: this initialized registry cell belongs to the live
            // subproc; release CAS publishes the fully initialized arena once.
            if unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .compare_exchange_release(&mut expected, arena)
            } {
                if !arena_id.is_null() {
                    // SAFETY: successful publication fixed the arena index;
                    // caller-provided output storage remains writable.
                    unsafe { *arena_id = mi_arena_id_from_arena_typed(arena_ptr) };
                }
                return true;
            }
        }
    }

    while count < MI_MAX_ARENAS {
        let next_count = count + 1;
        // SAFETY: the live aligned atomic count belongs to this subproc;
        // acq-rel CAS reserves one new table slot under the registry protocol.
        if unsafe {
            crate::types::AtomicUsizeField::from_raw(
                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
            )
            .compare_exchange_acq_rel(&mut count, next_count)
        } {
            let Some(arena_idx) = ArenaIndex::from_usize(count) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY: the reserved count value produced a checked index, and
            // the unpublished arena index field remains exclusively writable.
            unsafe { arena_ptr.set_index(arena_idx) };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            // SAFETY: the reserved initialized registry cell belongs to this
            // live subproc; release CAS publishes the initialized arena once.
            if unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .compare_exchange_release(&mut expected, arena)
            } {
                // SAFETY: the published arena retains a live initialized
                // subproc pointer whose atomic-compatible counter field accepts
                // this registration increment.
                unsafe {
                    __mi_stat_counter_increase(
                        ptr::addr_of_mut!(
                            (*(*arena.cast::<MiArenaPrefix>()).subproc)
                                .stats
                                .arena_count
                        ),
                        1,
                    )
                };
                if !arena_id.is_null() {
                    // SAFETY: publication fixed the arena index and the
                    // caller-provided output storage remains writable.
                    unsafe { *arena_id = mi_arena_id_from_arena_typed(arena_ptr) };
                }
                return true;
            }
        }
    }

    // SAFETY: publication failed, so the arena prefix remains exclusively
    // caller-owned and its index/subproc fields may be reset before rollback.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).arena_idx = 0;
        (*arena.cast::<MiArenaPrefix>()).subproc = ptr::null_mut();
    }
    false
}

#[allow(clippy::too_many_arguments)]
/// Initialize and publish an arena over caller-provided OS backing.
///
/// # Safety
/// `subproc` must remain live with an initialized registry. `start` must be the
/// exclusively owned, slice-aligned base of writable OS backing covering
/// `total_size` and every requested slice; `memid` must describe that exact
/// backing and its committed/zero/pinned state. `parent`, if non-null, must be a
/// live registered arena stable through publication. If supplied, `commit_fun`
/// and `commit_fun_arg` must remain valid for synchronous invocation, obey the
/// allocator commit contract, and not unwind. If non-null, `arena_id` must be
/// aligned writable output storage. Concurrent teardown is excluded; on success
/// ownership transfers to the subproc registry, while failure leaves backing
/// cleanup to the caller.
pub(super) unsafe fn mi_arena_initialize(
    subproc: SubprocPtr,
    start: OsMemPtr,
    slice_count: crate::types::SliceCount,
    parent: *mut mi_arena_t,
    total_size: crate::types::ReservedBytes,
    numa_node: crate::types::NumaNode,
    exclusive: bool,
    memid: mi_memid_t,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> *mut mi_arena_t {
    let subproc_raw = subproc.as_ptr();
    let start = start.as_ptr();
    let slice_count_usize = slice_count.get() as usize;
    crate::internal::mi_assert_internal!((start as usize).is_multiple_of(MI_ARENA_SLICE_ALIGN));
    crate::internal::mi_assert_internal!(mi_size_of_slices(slice_count_usize) >= MI_ARENA_MIN_SIZE);

    if slice_count_usize > MI_BITMAP_MAX_BIT_COUNT {
        crate::allocator::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is too large (size {} MiB, maximum is {} MiB)",
            mi_size_of_slices(slice_count_usize) / (1024 * 1024),
            mi_size_of_slices(MI_BITMAP_MAX_BIT_COUNT) / (1024 * 1024)
        ));
        return ptr::null_mut();
    }

    // SAFETY: arena initialization runs after OS configuration initialization;
    // these snapshots remain stable while the metadata layout is calculated.
    let os_page_size = unsafe { crate::platform::os::_mi_os_page_size_typed() };
    // SAFETY: the same initialized OS configuration supplies the secure guard
    // contribution for this one layout calculation.
    let secure_guard_size = unsafe { crate::platform::os::_mi_os_secure_guard_page_size() };
    let (info_slices, bitmap_base) =
        mi_arena_info_slices_needed(slice_count, os_page_size, secure_guard_size);
    if slice_count_usize < info_slices + 1 {
        crate::allocator::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is not large enough (size {} KiB, minimum required is {} KiB)",
            mi_size_of_slices(slice_count_usize) / 1024,
            mi_size_of_slices(info_slices + 1) / 1024
        ));
        return ptr::null_mut();
    } else if info_slices >= MI_ARENA_BCHUNK_BITS {
        crate::allocator::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is too large with respect to the maximum object size (size {} MiB, meta-info slices {}, maximum object slices are {})",
            mi_size_of_slices(slice_count_usize) / (1024 * 1024),
            info_slices,
            MI_ARENA_BCHUNK_BITS
        ));
        return ptr::null_mut();
    }

    let arena = start.cast::<mi_arena_t>();
    if !memid.initially_committed {
        let mut commit_size = mi_size_of_slices(info_slices);
        if !memid.is_pinned {
            commit_size -= secure_guard_size;
        }
        let ok = if let Some(commit_fun) = commit_fun {
            // SAFETY: the caller keeps the callback and opaque argument valid
            // and non-unwinding; the computed metadata prefix is within the
            // exact uncommitted backing described by MemId.
            unsafe {
                commit_fun(
                    true,
                    arena.cast::<c_void>(),
                    commit_size,
                    ptr::null_mut(),
                    commit_fun_arg,
                )
            }
        } else {
            // SAFETY: the caller-provided backing and matching MemId permit
            // committing this page-rounded metadata prefix under initialized OS state.
            unsafe { _mi_os_commit(arena.cast::<c_void>(), commit_size, ptr::null_mut()) }
        };
        if !ok {
            crate::allocator::options::_mi_warning_message_fmt(format_args!(
                "unable to commit meta-data for OS memory"
            ));
            return ptr::null_mut();
        }
    } else if !memid.is_pinned {
        // SAFETY: the committed unpinned backing covers all metadata slices;
        // their one-past address is valid for installing the configured guard.
        unsafe {
            crate::platform::os::_mi_os_secure_guard_page_set_before(
                arena
                    .cast::<u8>()
                    .add(mi_size_of_slices(info_slices))
                    .cast::<c_void>(),
                memid,
            )
        };
    }
    if !memid.initially_zero {
        // SAFETY: metadata commitment succeeded and the caller's writable
        // backing covers the non-guard prefix being initialized to zero.
        unsafe {
            crate::internal::_mi_memzero(
                arena.cast::<c_void>(),
                mi_size_of_slices(info_slices) - secure_guard_size,
            )
        };
    }

    // SAFETY: the metadata prefix is now committed, zeroed, live, and still
    // exclusively unpublished; initialize every arena-prefix field before any
    // registry release publication can expose it.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).subproc = subproc_raw;
        (*arena.cast::<MiArenaPrefix>()).memid = memid;
        (*arena.cast::<MiArenaPrefix>()).is_exclusive = exclusive;
        (*arena.cast::<MiArenaPrefix>()).slice_count = slice_count_usize;
        (*arena.cast::<MiArenaPrefix>()).info_slices = info_slices;
        let numa_node = if numa_node.is_any()
            && mi_option_is_enabled(mi_option_t::mi_option_arena_is_numa_local)
        {
            crate::platform::os::_mi_os_numa_node_typed()
        } else {
            numa_node
        };
        // This unpublished arena is exclusively initialized here; write the
        // complete typed NUMA value before registry publication.
        (*arena.cast::<MiArenaPrefix>()).numa_node = numa_node.get();
        (*arena.cast::<MiArenaPrefix>())
            .purge_expire
            .store(0, Ordering::Relaxed);
        (*arena.cast::<MiArenaPrefix>()).commit_fun = commit_fun;
        (*arena.cast::<MiArenaPrefix>()).commit_fun_arg = commit_fun_arg;
        (*arena.cast::<MiArenaPrefix>()).parent = parent;
        (*arena.cast::<MiArenaPrefix>()).total_size = total_size.get();
    }

    // SAFETY: the caller's slice-aligned live arena base and validated layout
    // place bitmap_base within the committed writable metadata prefix.
    let mut base = unsafe {
        mi_arena_start(ArenaPtr::new_unchecked(arena))
            .as_ptr()
            .add(bitmap_base)
    };
    // SAFETY: the layout calculation reserves the first aligned zeroed region
    // for the free-slice bbitmap; arena prefix and cursor are exclusively owned.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_free = mi_arena_bbitmap_init(slice_count, &mut base)
    };
    // SAFETY: the cursor now identifies the next aligned zeroed region reserved
    // for the committed-slice bitmap under exclusive initialization.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_committed =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY: the cursor now identifies the next aligned zeroed region reserved
    // for the dirty-slice bitmap under exclusive initialization.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_dirty = mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY: the cursor now identifies the next aligned zeroed region reserved
    // for the purge-slice bitmap under exclusive initialization.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_purge = mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY: the cursor now identifies the aligned zeroed main page bitmap in
    // the validated metadata layout; arena publication has not occurred.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).pages_main.pages =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    for i in 0..MI_ARENA_BIN_COUNT {
        // SAFETY: each checked bin consumes its distinct aligned zeroed bitmap
        // region from the validated layout under exclusive initialization.
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_main.pages_abandoned[i] =
                mi_arena_bitmap_init(slice_count, &mut base)
        };
    }
    if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        // SAFETY: after all bitmaps the validated layout reserves a writable
        // aligned mi_page_t array with one entry per slice; cursor advancement
        // remains within the metadata prefix.
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_meta = base.cast::<mi_page_t>();
            base = base.add(slice_count_usize * size_of::<mi_page_t>());
        }
    } else {
        // SAFETY: this unpublished prefix is exclusively writable; null is the
        // complete initialized representation for colocated page metadata.
        unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta = ptr::null_mut() };
    }
    crate::internal::mi_assert_internal!(
        mi_size_of_slices(info_slices)
            // SAFETY: base and the slice-aligned arena start share the same live
            // metadata allocation; base was advanced monotonically within it.
            >= unsafe {
                base.offset_from(
                    mi_arena_start(ArenaPtr::new_unchecked(arena))
                        .as_ptr()
                        .cast_const(),
                ) as usize
            }
    );

    // SAFETY: the initialized free bbitmap covers every slice; the validated
    // nonempty suffix after info_slices is in bounds and exclusively unpublished.
    unsafe {
        mi_bbitmap_unsafe_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            info_slices,
            (*arena.cast::<MiArenaPrefix>()).slice_count - info_slices,
        )
    };
    if memid.initially_committed {
        // SAFETY: the initialized committed bitmap covers the full slice count;
        // MemId proves every backing slice was initially committed.
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }
    if !memid.initially_zero {
        // SAFETY: the initialized dirty bitmap covers the full slice count;
        // MemId proves the caller-provided backing was not initially zero.
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }

    // SAFETY: every prefix field, bitmap, optional page table, and backing-state
    // bit is initialized; the caller keeps subproc/output storage live while
    // mi_arenas_add performs the one release publication or rollback.
    if !unsafe { mi_arenas_add(subproc, ArenaPtr::new_unchecked(arena), arena_id) } {
        return ptr::null_mut();
    }
    arena
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_metadata_size_helpers_preserve_aligned_layout_components() {
        let slice_count = crate::types::SliceCount::from_usize(MI_ARENA_BCHUNK_BITS)
            .expect("one bitmap chunk is a valid arena slice count");
        let (pages_size, bitmap_base) = mi_arena_pages_size(slice_count);
        assert!(bitmap_base.get().is_multiple_of(MI_BCHUNK_SIZE));
        assert!(pages_size.get() > bitmap_base.get());

        // SAFETY: the synthetic page size is explicitly nonzero and is used
        // only by the pure alignment calculation in this test.
        let page_size = unsafe { crate::types::OsPageSize::new_unchecked(4096) };
        let (info_slices, arena_bitmap_base) =
            mi_arena_info_slices_needed(slice_count, page_size, 4096);
        assert_eq!(
            arena_bitmap_base,
            _mi_align_up(size_of::<mi_arena_t>(), MI_BCHUNK_SIZE)
        );
        assert!(info_slices > 0);
        assert!(info_slices < slice_count.get() as usize);
    }
}
