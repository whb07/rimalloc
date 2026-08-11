//! Arena purge scheduling, free dispatch, and collection helpers.
//!
//! These helpers keep delayed purge state and OS decommit handoff together
//! while the parent arena module retains page allocation and lifecycle logic.

use super::*;

#[allow(non_camel_case_types)]
#[repr(C)]
struct mi_purge_visit_info_t {
    now: mi_msecs_t,
    delay: mi_msecs_t,
    all_purged: bool,
    any_purged: bool,
}

unsafe fn mi_arena_schedule_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) {
    let arena_ptr = arena;
    let arena = arena.as_ptr();
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    let delay = mi_arena_purge_delay();
    // SAFETY: the typed arena remains live during scheduling, so its initialized
    // immutable allocation policy may be read before any purge bitmap mutation.
    let is_pinned = unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned };
    if is_pinned || delay < 0 || _mi_preloading() {
        return;
    }
    // SAFETY: purge scheduling records delayed purge windows and purge bitmaps under typed arena/subproc state.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });
    if delay == 0 {
        // SAFETY: purge scheduling records delayed purge windows and purge bitmaps under typed arena/subproc state.
        unsafe { mi_arena_purge(arena_ptr, range) };
        return;
    }

    let expire = crate::allocator::stats::_mi_clock_now() + delay as i64;
    let mut expire0 = 0i64;
    // SAFETY: purge scheduling records delayed purge windows and purge bitmaps under typed arena/subproc state.
    if unsafe {
        crate::core_types::AtomicI64Field::from_raw(
            ptr::addr_of_mut!((*arena.cast::<MiArenaPrefix>()).purge_expire).cast::<i64>(),
        )
        .compare_exchange_acq_rel(&mut expire0, expire)
    } {
        crate::internal::mi_assert_internal!(expire0 == 0);
        // SAFETY: purge scheduling records delayed purge windows and purge bitmaps under typed arena/subproc state.
        let subproc = unsafe { (*arena.cast::<MiArenaPrefix>()).subproc };
        unsafe {
            crate::core_types::AtomicI64Field::from_raw(
                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire).cast::<i64>(),
            )
            .compare_exchange_acq_rel(&mut expire0, expire)
        };
    }
    // SAFETY: purge scheduling records delayed purge windows and purge bitmaps under typed arena/subproc state.
    unsafe {
        mi_bitmap_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_purge,
            slice_index,
            slice_count,
            ptr::null_mut(),
        )
    };
}

/// # Safety
/// Internal arena free path matching the C ABI.
// SAFETY: arena free dispatch validates memid ranges, marks free slices, and schedules purge without changing allocation ownership rules.
pub(crate) unsafe extern "C" fn _mi_arenas_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    if p.is_null() || size == 0 {
        return;
    }

    // SAFETY: arena free dispatch validates memid ranges, marks free slices, and schedules purge without changing allocation ownership rules.
    unsafe { mi_track_mem_undefined(p, size) };
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Os(_) => {
            unsafe { _mi_os_free(p, size, memid) };
        }
        crate::core_types::MemIdView::Arena(arena_memid) => {
            let arena_ptr = arena_memid.arena();
            let arena = arena_ptr.as_ptr();
            let range = arena_memid.range();
            let slice_index = range.start_usize();
            let slice_count = range.count_usize();
            let slice_end = range.end_usize();
            crate::internal::mi_assert_internal!(size.is_multiple_of(MI_ARENA_SLICE_SIZE));
            crate::internal::mi_assert_internal!(slice_count * MI_ARENA_SLICE_SIZE == size);
            crate::internal::mi_assert_internal!(
                // SAFETY: arena free dispatch validates memid ranges, marks free slices, and schedules purge without changing allocation ownership rules.
                slice_end <= unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
            );
            crate::internal::mi_assert_internal!(unsafe {
                mi_arena_range_is_after_info_slices(arena_ptr, range)
            });
            // SAFETY: arena free dispatch validates memid ranges, marks free slices, and schedules purge without changing allocation ownership rules.
            if !unsafe { mi_arena_range_is_after_info_slices(arena_ptr, range) }
                || slice_end > unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
            {
                crate::allocator::options::_mi_error_message_fmt(
                    Errno::EINVAL,
                    format_args!(
                        "trying to free from an invalid arena block: {:p}, size {}\n",
                        p, size
                    ),
                );
                return;
            }

            // SAFETY: arena free dispatch validates memid ranges, marks free slices, and schedules purge without changing allocation ownership rules.
            if !unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
                unsafe { mi_arena_schedule_purge(ArenaPtr::new_unchecked(arena), range) };
            }

            let all_inuse = unsafe {
                mi_bbitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_free,
                    slice_index,
                    slice_count,
                )
            };
            if !all_inuse {
                crate::allocator::options::_mi_error_message_fmt(
                    Errno::EAGAIN,
                    format_args!(
                        "trying to free an already freed arena block: {:p}, size {}\n",
                        p, size
                    ),
                );
            }
        }
        crate::core_types::MemIdView::Meta(_) => {
            // SAFETY: arena free dispatch validates memid ranges, marks free slices, and schedules purge without changing allocation ownership rules.
            unsafe { _mi_meta_free(p, size, memid) };
        }
        crate::core_types::MemIdView::None
        | crate::core_types::MemIdView::External(_)
        | crate::core_types::MemIdView::Static => {}
        crate::core_types::MemIdView::HeapMain | crate::core_types::MemIdView::Unknown(_) => {
            crate::internal::mi_assert_internal!(false);
        }
    }
}

fn mi_arena_purge_delay() -> c_long {
    mi_option_get(mi_option_t::mi_option_purge_delay)
        * mi_option_get(mi_option_t::mi_option_arena_purge_mult)
}

#[inline(never)]
unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool {
    let arena = arena.as_ptr();
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    // SAFETY: direct arena purge updates committed-slice bitmaps around OS purge/decommit handoff.
    crate::internal::mi_assert_internal!(!unsafe {
        (*arena.cast::<MiArenaPrefix>()).memid.is_pinned
    });
    crate::internal::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });

    let size = mi_size_of_slices(slice_count);
    // SAFETY: direct arena purge updates committed-slice bitmaps around OS purge/decommit handoff.
    let p = unsafe {
        mi_arena_slice_start(ArenaPtr::new_unchecked(arena), range.start())
            .as_ptr()
            .cast::<c_void>()
    };
    let mut already_committed = 0usize;
    // SAFETY: direct arena purge updates committed-slice bitmaps around OS purge/decommit handoff.
    unsafe {
        mi_bitmap_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_committed,
            slice_index,
            slice_count,
            &mut already_committed,
        )
    };
    let all_committed = already_committed == slice_count;
    // SAFETY: direct arena purge updates committed-slice bitmaps around OS purge/decommit handoff.
    let needs_recommit = unsafe {
        crate::internal::_mi_os_purge_ex(
            p,
            size,
            all_committed,
            mi_size_of_slices(already_committed),
            (*arena.cast::<MiArenaPrefix>()).commit_fun,
            (*arena.cast::<MiArenaPrefix>()).commit_fun_arg,
        )
    };

    if needs_recommit || !all_committed {
        // SAFETY: direct arena purge updates committed-slice bitmaps around OS purge/decommit handoff.
        unsafe {
            mi_bitmap_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
    }

    needs_recommit
}

unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool {
    let arena = arena.as_ptr();
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    crate::internal::mi_assert!(slice_count < MI_BCHUNK_BITS);
    // SAFETY: range purge claims free-slice runs, falls back to single-slice ranges, and restores free bitmap state.
    if unsafe {
        mi_bbitmap_try_clearNC(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    } {
        // SAFETY: range purge claims free-slice runs, falls back to single-slice ranges, and restores free bitmap state.
        let decommitted = unsafe { mi_arena_purge(ArenaPtr::new_unchecked(arena), range) };
        crate::internal::mi_assert_internal!(
            !decommitted
                || unsafe {
                    mi_bitmap_is_clearN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY: range purge claims free-slice runs, falls back to single-slice ranges, and restores free bitmap state.
        unsafe {
            mi_bbitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        };
        true
    } else {
        false
    }
}

unsafe fn mi_arena_try_purge_visit(
    arena: ArenaPtr,
    range: crate::core_types::SliceRange,
    vinfo: &mut mi_purge_visit_info_t,
) -> bool {
    // SAFETY: range purge claims free-slice runs, falls back to single-slice ranges, and restores free bitmap state.
    if unsafe { mi_arena_try_purge_range(arena, range) } {
        vinfo.any_purged = true;
        vinfo.all_purged = true;
    } else if range.count_usize() > 1 {
        for i in 0..range.count_usize() {
            let Ok(single) = range.single_at_offset(i) else {
                crate::internal::mi_assert_internal!(false);
                vinfo.all_purged = false;
                continue;
            };
            // SAFETY: range purge claims free-slice runs, falls back to single-slice ranges, and restores free bitmap state.
            let purged = unsafe { mi_arena_try_purge_range(arena, single) };
            vinfo.any_purged = vinfo.any_purged || purged;
            vinfo.all_purged = vinfo.all_purged && purged;
        }
    }
    true
}

// SAFETY: purge visitor ABI validates raw slice ranges and callback state before forwarding typed purge work.
unsafe extern "C" fn mi_arena_try_purge_visitor(
    slice_index: usize,
    slice_count: usize,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let Some(arena_ptr) = ArenaPtr::new(arena) else {
        crate::internal::mi_assert_internal!(false);
        return true;
    };
    let Some(range) = crate::core_types::SliceRange::new(slice_index, slice_count, arena_ptr).ok()
    else {
        crate::internal::mi_assert_internal!(false);
        return true;
    };
    // SAFETY: purge visitor ABI validates raw slice ranges and callback state before forwarding typed purge work.
    let Some(vinfo) = (unsafe { arg.cast::<mi_purge_visit_info_t>().as_mut() }) else {
        crate::internal::mi_assert_internal!(false);
        return true;
    };

    // SAFETY: purge visitor ABI validates raw slice ranges and callback state before forwarding typed purge work.
    unsafe { mi_arena_try_purge_visit(arena_ptr, range, vinfo) }
}

unsafe fn mi_arena_try_purge(arena: ArenaPtr, now: mi_msecs_t, force: bool) -> c_int {
    let arena_ptr = arena;
    let arena = arena.as_ptr();
    // SAFETY: purge expiration policy reads and clears delayed purge clocks while accounting purge attempts.
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return -1;
    }

    let expire = unsafe {
        crate::core_types::AtomicI64Field::from_raw(
            ptr::addr_of_mut!((*arena.cast::<MiArenaPrefix>()).purge_expire).cast::<i64>(),
        )
        .load(crate::core_types::MemoryOrder::Relaxed)
    };
    if !force {
        if expire == 0 {
            return -1;
        }
        if expire > now {
            return 0;
        }
    }

    // SAFETY: purge expiration policy reads and clears delayed purge clocks while accounting purge attempts.
    unsafe {
        crate::core_types::AtomicI64Field::from_raw(
            ptr::addr_of_mut!((*arena.cast::<MiArenaPrefix>()).purge_expire).cast::<i64>(),
        )
        .store(0, crate::core_types::MemoryOrder::Release)
    };
    // SAFETY: purge expiration policy reads and clears delayed purge clocks while accounting purge attempts.
    unsafe {
        __mi_stat_counter_increase_mt(
            ptr::addr_of_mut!(
                (*(*arena.cast::<MiArenaPrefix>()).subproc)
                    .stats
                    .arena_purges
            ),
            1,
        )
    };

    let mut vinfo = mi_purge_visit_info_t {
        now,
        delay: mi_arena_purge_delay(),
        all_purged: true,
        any_purged: false,
    };
    let minslices = mi_slice_count_of_size(unsafe { _mi_os_minimal_purge_size() });
    // SAFETY: purge expiration policy reads and clears delayed purge clocks while accounting purge attempts.
    unsafe {
        _mi_bitmap_forall_setc_rangesn(
            (*arena.cast::<MiArenaPrefix>()).slices_purge,
            minslices,
            mi_arena_try_purge_visitor,
            arena_ptr.as_ptr(),
            ptr::addr_of_mut!(vinfo).cast(),
        )
    };

    if vinfo.any_purged { 1 } else { -1 }
}

unsafe fn mi_arenas_try_purge(
    force: bool,
    visit_all: bool,
    subproc: SubprocPtr,
    tseq: crate::core_types::ThreadSeq,
) {
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    let delay = mi_arena_purge_delay();
    if _mi_preloading() || delay <= 0 {
        return;
    }

    let now = crate::allocator::stats::_mi_clock_now();
    // SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
    let arenas_expire = unsafe {
        crate::core_types::AtomicI64Field::from_raw(
            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire).cast::<i64>(),
        )
        .load(crate::core_types::MemoryOrder::Acquire)
    };
    if !visit_all && !force && (arenas_expire == 0 || arenas_expire > now) {
        return;
    }

    // SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
    let max_arena = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    if max_arena == 0 {
        return;
    }

    static PURGE_GUARD: mi_atomic_guard_t = AtomicUsize::new(0);
    crate::atomic::mi_atomic_guard!(ptr::addr_of!(PURGE_GUARD).cast_mut(), {
        if arenas_expire > now {
            // SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
            unsafe {
                crate::core_types::AtomicI64Field::from_raw(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire)
                        .cast::<i64>(),
                )
                .store(
                    now + ((delay as mi_msecs_t) / 10),
                    crate::core_types::MemoryOrder::Release,
                )
            };
        }
        let arena_start = tseq.get() % max_arena;
        let mut max_purge_count = if visit_all {
            max_arena
        } else {
            (max_arena / 4) + 1
        };
        let mut all_visited = true;
        let mut any_purged = false;
        for arena_offset in 0..max_arena {
            let mut i = arena_offset + arena_start;
            if i >= max_arena {
                i -= max_arena;
            }
            let Some(arena_idx) = ArenaIndex::from_usize(i) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
            let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
            if !arena.is_null() {
                let purged =
                    unsafe { mi_arena_try_purge(ArenaPtr::new_unchecked(arena), now, force) };
                if purged >= 0 {
                    any_purged = true;
                    if purged >= 1 {
                        if max_purge_count <= 1 {
                            all_visited = false;
                            break;
                        }
                        max_purge_count -= 1;
                    }
                }
            }
        }
        if all_visited && !any_purged {
            // SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
            unsafe {
                crate::core_types::AtomicI64Field::from_raw(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire)
                        .cast::<i64>(),
                )
                .store(0, crate::core_types::MemoryOrder::Release)
            };
        }
    });
}

/// # Safety
/// Purges scheduled arena ranges for the current thread-local context.
// SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
pub(crate) unsafe extern "C" fn _mi_arenas_collect(
    force_purge: bool,
    visit_all: bool,
    tld: *mut mi_tld_t,
) {
    // SAFETY: arena collection uses the global purge guard and bounded arena visitation to update subproc purge state.
    unsafe {
        mi_arenas_try_purge(
            force_purge,
            visit_all,
            SubprocPtr::new_unchecked((*tld).subproc),
            crate::core_types::ThreadSeq::new((*tld).thread_seq),
        )
    };
}
