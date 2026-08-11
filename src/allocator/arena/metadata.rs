//! Arena metadata layout, registration, initialization, and teardown.
//!
//! This module owns construction and destruction of the arena metadata graph:
//! slice bitmaps, per-heap page tables, registry publication, and optional
//! separated page metadata. Allocation policy and raw compatibility entrypoints
//! remain in the parent module.

use super::*;

pub(super) unsafe fn mi_arena_pages_size(
    slice_count: crate::types::SliceCount,
) -> (crate::types::ReservedBytes, crate::types::ReservedBytes) {
    let slice_count = slice_count.get() as usize;
    crate::internal::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_pages_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 1 + MI_ARENA_BIN_COUNT;
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bitmaps_size = bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) };
    let size = base_size + bitmaps_size;
    (
        crate::types::ReservedBytes::new(size),
        crate::types::ReservedBytes::new(base_size),
    )
}

unsafe fn mi_arena_info_slices_needed(
    slice_count: crate::types::SliceCount,
    bitmap_base: *mut usize,
) -> usize {
    let slice_count = slice_count.get() as usize;
    crate::internal::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 4 + MI_ARENA_BIN_COUNT;
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bitmaps_size = (bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) })
        + unsafe { mi_bbitmap_size(slice_count, ptr::null_mut()) };
    let pages_size = if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        slice_count * size_of::<mi_page_t>()
    } else {
        0
    };
    let size = base_size + bitmaps_size + pages_size;
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let info_size = _mi_align_up(size, unsafe {
        crate::platform::os::_mi_os_page_size_typed().get()
    }) + unsafe { crate::platform::os::_mi_os_secure_guard_page_size() };
    let info_slices = mi_slice_count_of_size(info_size);
    if !bitmap_base.is_null() {
        unsafe { *bitmap_base = base_size };
    }
    info_slices
}

unsafe fn mi_arena_bitmap_init(
    slice_count: crate::types::SliceCount,
    base: *mut *mut u8,
) -> *mut mi_bitmap_t {
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bitmap = unsafe { *base }.cast::<mi_bitmap_t>();
    let size = unsafe { mi_bitmap_init(bitmap, slice_count.get() as usize, true) };
    unsafe { *base = (*base).add(size) };
    bitmap
}

unsafe fn mi_arena_bbitmap_init(
    slice_count: crate::types::SliceCount,
    base: *mut *mut u8,
) -> *mut mi_bbitmap_t {
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bbitmap = unsafe { *base }.cast::<mi_bbitmap_t>();
    let size = unsafe { mi_bbitmap_init(bbitmap, slice_count.get() as usize, true) };
    unsafe { *base = (*base).add(size) };
    bbitmap
}

pub(super) unsafe fn mi_arena_pages_alloc(
    arena: ArenaPtr,
    range: crate::types::SliceRange,
    alloc_size: crate::types::ReservedBytes,
    bitmap_base: crate::types::ReservedBytes,
) -> *mut mi_arena_pages_t {
    crate::internal::mi_assert_internal!(range.start_usize() == 0);
    crate::internal::mi_assert_internal!(
        // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        range.end_usize() <= unsafe { arena.slice_count() }.get() as usize
    );
    let slice_count = range.count();
    let arena_pages =
        unsafe { mi_heap_zalloc_aligned(mi_heap_main(), alloc_size.get(), MI_BCHUNK_SIZE) }
            .cast::<mi_arena_pages_t>();
    if arena_pages.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let mut base = unsafe { arena_pages.cast::<u8>().add(bitmap_base.get()) };
    crate::internal::mi_assert_internal!((base as usize).is_multiple_of(MI_BCHUNK_SIZE));
    unsafe { (*arena_pages).pages = mi_arena_bitmap_init(slice_count, &mut base) };
    for i in 0..MI_ARENA_BIN_COUNT {
        unsafe { (*arena_pages).pages_abandoned[i] = mi_arena_bitmap_init(slice_count, &mut base) };
    }
    arena_pages
}

pub(super) unsafe fn mi_arenas_unsafe_destroy(subproc: SubprocPtr) {
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    // SAFETY: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    for i in 0..arena_count {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
        let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
        if !arena.is_null() {
            unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .store(ptr::null_mut(), crate::types::MemoryOrder::Release);
            }
            // SAFETY: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
            if mi_memid_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid }) {
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
    // SAFETY: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
    let _ = unsafe {
        crate::types::AtomicUsizeField::from_raw(
            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
        )
        .compare_exchange_acq_rel(&mut expected, 0)
    };
}

unsafe fn mi_arenas_add(
    subproc: SubprocPtr,
    arena_ptr: ArenaPtr,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let subproc = subproc.as_ptr();
    let arena = arena_ptr.as_ptr();
    // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
    crate::internal::mi_assert_internal!(unsafe { arena_ptr.slice_count() }.get() > 0);
    if !arena_id.is_null() {
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
    let mut count = unsafe { mi_arenas_get_count_typed(SubprocPtr::new_unchecked(subproc)) };
    for i in 0..count {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
        if unsafe { mi_arena_from_index_typed(SubprocPtr::new_unchecked(subproc), arena_idx) }
            .is_null()
        {
            unsafe { arena_ptr.set_index(arena_idx) };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
            if unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .compare_exchange_release(&mut expected, arena)
            } {
                if !arena_id.is_null() {
                    // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
                    unsafe { *arena_id = mi_arena_id_from_arena_typed(arena_ptr) };
                }
                return true;
            }
        }
    }

    while count < MI_MAX_ARENAS {
        let next_count = count + 1;
        // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
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
            // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
            unsafe { arena_ptr.set_index(arena_idx) };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            if unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .compare_exchange_release(&mut expected, arena)
            } {
                // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
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
                    // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
                    unsafe { *arena_id = mi_arena_id_from_arena_typed(arena_ptr) };
                }
                return true;
            }
        }
    }

    // SAFETY: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).arena_idx = 0;
        (*arena.cast::<MiArenaPrefix>()).subproc = ptr::null_mut();
    }
    false
}

#[allow(clippy::too_many_arguments)]
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

    let mut bitmap_base = 0usize;
    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    let info_slices = unsafe { mi_arena_info_slices_needed(slice_count, &mut bitmap_base) };
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
            // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
            commit_size -= unsafe { crate::platform::os::_mi_os_secure_guard_page_size() };
        }
        let ok = if let Some(commit_fun) = commit_fun {
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
            // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
            unsafe { _mi_os_commit(arena.cast::<c_void>(), commit_size, ptr::null_mut()) }
        };
        if !ok {
            crate::allocator::options::_mi_warning_message_fmt(format_args!(
                "unable to commit meta-data for OS memory"
            ));
            return ptr::null_mut();
        }
    } else if !memid.is_pinned {
        // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
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
        // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
        unsafe {
            crate::internal::_mi_memzero(
                arena.cast::<c_void>(),
                mi_size_of_slices(info_slices)
                    - crate::platform::os::_mi_os_secure_guard_page_size(),
            )
        };
    }

    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
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

    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    let mut base = unsafe {
        mi_arena_start(ArenaPtr::new_unchecked(arena))
            .as_ptr()
            .add(bitmap_base)
    };
    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_free = mi_arena_bbitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_committed =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_dirty = mi_arena_bitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_purge = mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).pages_main.pages =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    for i in 0..MI_ARENA_BIN_COUNT {
        // SAFETY: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_main.pages_abandoned[i] =
                mi_arena_bitmap_init(slice_count, &mut base)
        };
    }
    if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        // SAFETY: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_meta = base.cast::<mi_page_t>();
            base = base.add(slice_count_usize * size_of::<mi_page_t>());
        }
    } else {
        // SAFETY: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
        unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta = ptr::null_mut() };
    }
    crate::internal::mi_assert_internal!(
        mi_size_of_slices(info_slices)
            >= unsafe {
                base.offset_from(
                    mi_arena_start(ArenaPtr::new_unchecked(arena))
                        .as_ptr()
                        .cast_const(),
                ) as usize
            }
    );

    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        mi_bbitmap_unsafe_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            info_slices,
            (*arena.cast::<MiArenaPrefix>()).slice_count - info_slices,
        )
    };
    if memid.initially_committed {
        // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }
    if !memid.initially_zero {
        // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }

    // SAFETY: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    if !unsafe { mi_arenas_add(subproc, ArenaPtr::new_unchecked(arena), arena_id) } {
        return ptr::null_mut();
    }
    arena
}
