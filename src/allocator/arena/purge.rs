//! Arena purge scheduling, free dispatch, and collection helpers.
//!
//! These helpers keep delayed purge state and OS decommit handoff together
//! while the parent arena module retains page allocation and lifecycle logic.

use super::*;

/// # Safety
/// `arena` must identify live initialized arena metadata whose slice/free/purge bitmaps cover
/// `range`. The range must still be allocated (clear in `slices_free`), the arena's subprocess and
/// expiration fields must remain live, and teardown must not race this atomic publication.
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
    // SAFETY: the initialized free bitmap covers `range`; scheduling begins while its allocated
    // slices remain clear under the caller-owned arena transition.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });
    if delay == 0 {
        // SAFETY: the live non-pinned arena and allocated in-bounds range satisfy immediate purge;
        // teardown and reuse remain excluded by the scheduling contract.
        unsafe { mi_arena_purge(arena_ptr, range) };
        return;
    }

    let expire = crate::allocator::stats::_mi_clock_now() + delay as i64;
    let mut expire0 = 0i64;
    // SAFETY: the arena expiration field is aligned, initialized, and live; this atomic CAS
    // publishes the first delayed expiration.
    if unsafe {
        crate::core_types::AtomicI64Field::from_cell(&(*arena.cast::<MiArenaPrefix>()).purge_expire)
            .compare_exchange_acq_rel(&mut expire0, expire)
    } {
        crate::internal::mi_assert_internal!(expire0 == 0);
        // SAFETY: the live arena metadata names its live subprocess under the no-teardown contract.
        let subproc = unsafe { (*arena.cast::<MiArenaPrefix>()).subproc };
        // SAFETY: the subprocess expiration field is aligned and initialized; this atomic update
        // publishes the arena's first expiration through the shared scheduling protocol.
        unsafe {
            crate::core_types::AtomicI64Field::from_cell(
                &(*subproc.cast::<MiSubprocPrefix>()).purge_expire,
            )
            .compare_exchange_acq_rel(&mut expire0, expire)
        };
    }
    // SAFETY: the initialized purge bitmap covers `range`; this atomic set publishes exactly the
    // scheduled slices while teardown is excluded.
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
/// When `p` is non-null and `size` is nonzero, `p`, `size`, and `memid` must describe the same live
/// allocation returned by this allocator and not yet freed. The caller must own its free transition,
/// and any arena, subprocess, metadata, or OS allocation named by `memid` must remain initialized
/// and stable for the call.
pub(crate) unsafe extern "C" fn _mi_arenas_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    if p.is_null() || size == 0 {
        return;
    }

    // SAFETY: `(p, size)` is the live allocation range from the contract; tracking is invalidated
    // before its backing is returned.
    unsafe { mi_track_mem_undefined(p, size) };
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Os(_) => {
            // SAFETY: the decoded OS memid and `(p, size)` still identify the caller-owned live
            // allocation; this branch consumes its free transition.
            unsafe { _mi_os_free(p, size, memid) };
        }
        crate::core_types::MemIdView::Arena(arena_memid) => {
            let arena_ptr = arena_memid.arena();
            let arena = arena_ptr.as_ptr();
            let range = arena_memid.range();
            let slice_index = range.start_usize();
            let slice_count = range.count_usize();
            let slice_end = range.end_usize();
            // SAFETY: the arena memid resolves live initialized metadata, whose stable
            // `slice_count` bounds every valid allocation range.
            let arena_slice_count = unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count };
            crate::internal::mi_assert_internal!(size.is_multiple_of(MI_ARENA_SLICE_SIZE));
            crate::internal::mi_assert_internal!(slice_count * MI_ARENA_SLICE_SIZE == size);
            crate::internal::mi_assert_internal!(slice_end <= arena_slice_count);
            // SAFETY: the decoded arena and range remain live; this reads only the initialized,
            // stable information-slice boundary.
            crate::internal::mi_assert_internal!(unsafe {
                mi_arena_range_is_after_info_slices(arena_ptr, range)
            });
            // SAFETY: the decoded arena remains live; this reads its stable information-slice
            // boundary solely to reject an invalid range before mutation.
            if !unsafe { mi_arena_range_is_after_info_slices(arena_ptr, range) }
                || slice_end > arena_slice_count
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

            // SAFETY: the arena metadata remains live and stable, so its immutable pin policy is
            // readable before scheduling.
            if !unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
                // SAFETY: validation above proves this allocated range belongs to the live arena;
                // its free bits remain clear until publication below and teardown cannot race.
                unsafe { mi_arena_schedule_purge(arena_ptr, range) };
            }

            // SAFETY: the initialized free bitmap covers the validated range, and the caller owns
            // this atomic free publication.
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
            // SAFETY: the metadata memid and `(p, size)` identify the same caller-owned live
            // metadata allocation, whose free transition is consumed here.
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
/// # Safety
/// `arena` must identify a live initialized non-pinned arena whose free and committed bitmaps cover
/// `range`. The range must remain claimed/clear in `slices_free`; its backing and optional commit
/// callback state must remain valid, stable, and non-unwinding through the synchronous OS purge.
unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool {
    let arena_ptr = arena;
    let arena = arena_ptr.as_ptr();
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    // SAFETY: the contract guarantees live initialized arena metadata and a non-pinned policy.
    crate::internal::mi_assert_internal!(!unsafe {
        (*arena.cast::<MiArenaPrefix>()).memid.is_pinned
    });
    // SAFETY: the initialized free bitmap covers `range`, whose bits remain clear under the purge
    // contract for this assertion and subsequent OS handoff.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });

    let size = mi_size_of_slices(slice_count);
    // SAFETY: the live arena backing covers the validated range, so projecting its first slice
    // yields the exact OS purge address.
    let p = unsafe {
        mi_arena_slice_start(arena_ptr, range.start())
            .as_ptr()
            .cast::<c_void>()
    };
    let mut already_committed = 0usize;
    // SAFETY: the initialized committed bitmap covers the free-bitmap-claimed `range`; the atomic
    // set temporarily normalizes its commitment and reports the prior committed count locally.
    unsafe {
        mi_bitmap_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_committed,
            slice_index,
            slice_count,
            &mut already_committed,
        )
    };
    let all_committed = already_committed == slice_count;
    // SAFETY: `p` and `size` name the validated live backing range, committed accounting comes from
    // the bitmap snapshot, and the arena's callback/argument remain valid and non-unwinding.
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
        // SAFETY: the initialized committed bitmap still covers the range; purge either requires
        // recommit or found a partial prior commitment, so clearing publishes the exact state.
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

/// # Safety
/// `arena` must identify a live initialized arena whose free, purge, and committed bitmaps cover
/// `range`. The range must be scheduled and free, its backing/callback state must satisfy
/// `mi_arena_purge`, and arena teardown must not race. Concurrent allocation attempts are allowed:
/// a successful free-bitmap claim gives this call exclusive range ownership until restoration.
unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool {
    let arena_ptr = arena;
    let arena = arena_ptr.as_ptr();
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    crate::internal::mi_assert!(slice_count < MI_BCHUNK_BITS);
    // SAFETY: the initialized free bitmap covers `range`; this conditional atomic clear claims the
    // otherwise-free run exclusively for the purge attempt.
    if unsafe {
        mi_bbitmap_try_clearNC(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    } {
        // SAFETY: the successful clear makes the range exclusively claimed, and the contract keeps
        // its arena backing, bitmaps, and callback state valid through purge.
        let decommitted = unsafe { mi_arena_purge(arena_ptr, range) };
        crate::internal::mi_assert_internal!(
            !decommitted
                // SAFETY: the initialized committed bitmap covers the exclusively claimed range;
                // this reads the required post-decommit state before restoring its free bits.
                || unsafe {
                    mi_bitmap_is_clearN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY: this restores exactly the free bits atomically claimed above before releasing the
        // purge attempt.
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

/// # Safety
/// `arena` and every subrange of `range` must satisfy `mi_arena_try_purge_range`. `any_purged` must
/// be exclusively writable for this synchronous visit.
unsafe fn mi_arena_try_purge_visit(
    arena: ArenaPtr,
    range: crate::core_types::SliceRange,
    any_purged: &mut bool,
) -> bool {
    // SAFETY: the caller supplies the live arena/bitmap/backing epoch for the complete range.
    if unsafe { mi_arena_try_purge_range(arena, range) } {
        *any_purged = true;
    } else if range.count_usize() > 1 {
        for i in 0..range.count_usize() {
            let Ok(single) = range.single_at_offset(i) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY: `single` is an in-bounds subrange and inherits the caller's live arena,
            // backing, bitmap, and no-teardown guarantees.
            let purged = unsafe { mi_arena_try_purge_range(arena, single) };
            *any_purged = *any_purged || purged;
        }
    }
    true
}

/// # Safety
/// The bitmap visitor must call this synchronously with `arena` naming its live initialized arena,
/// an in-bounds scheduled range, and `arg` non-null, aligned, initialized, and uniquely writable as
/// a `bool` for the callback duration. Arena state must not be torn down concurrently, and the
/// callback chain must not unwind.
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
    // SAFETY: the callback retains the initialized arena and its stable extent.
    let arena_slices = unsafe { arena_ptr.slice_count() };
    let Some(range) =
        crate::core_types::SliceRange::new(slice_index, slice_count, arena_slices).ok()
    else {
        crate::internal::mi_assert_internal!(false);
        return true;
    };
    // SAFETY: the callback contract makes `arg` aligned, initialized, and uniquely writable for
    // this invocation; null is still rejected defensively.
    let Some(any_purged) = (unsafe { arg.cast::<bool>().as_mut() }) else {
        crate::internal::mi_assert_internal!(false);
        return true;
    };

    // SAFETY: validated typed arena/range and exclusive visitor state retain the callback's live
    // bitmap/backing epoch for this synchronous forwarding call.
    unsafe { mi_arena_try_purge_visit(arena_ptr, range, any_purged) }
}

/// # Safety
/// `arena` must identify live initialized arena/subprocess metadata with initialized expiration and
/// purge bitmaps. Scheduled ranges, their backing, and optional callback state must remain valid
/// through synchronous visitation; teardown/backing or callback invalidation must not race, and
/// callbacks must not unwind. Concurrent allocation attempts are serialized by free-bitmap claims.
unsafe fn mi_arena_try_purge(arena: ArenaPtr, now: mi_msecs_t, force: bool) -> c_int {
    let arena_ptr = arena;
    let arena = arena.as_ptr();
    // SAFETY: the contract keeps initialized arena policy metadata live and readable.
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return -1;
    }

    // SAFETY: the contract keeps the aligned initialized expiration atomic live; this relaxed load
    // is the scheduling-policy snapshot.
    let expire = unsafe {
        crate::core_types::AtomicI64Field::from_cell(&(*arena.cast::<MiArenaPrefix>()).purge_expire)
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

    // SAFETY: the aligned live expiration field is cleared atomically before scheduled ranges are
    // visited.
    unsafe {
        crate::core_types::AtomicI64Field::from_cell(&(*arena.cast::<MiArenaPrefix>()).purge_expire)
            .store(0, crate::core_types::MemoryOrder::Release)
    };
    // SAFETY: the live arena names live subprocess stats; this aligned initialized counter is
    // updated only through the MT atomic helper.
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

    let mut any_purged = false;
    // SAFETY: OS purge configuration is initialized and stable during arena collection.
    let minslices = mi_slice_count_of_size(unsafe { _mi_os_minimal_purge_size() });
    // SAFETY: the initialized purge bitmap belongs to this live arena; visitation is synchronous,
    // and `any_purged`, arena backing, and callback state remain valid throughout.
    unsafe {
        _mi_bitmap_forall_setc_rangesn(
            (*arena.cast::<MiArenaPrefix>()).slices_purge,
            minslices,
            mi_arena_try_purge_visitor,
            arena_ptr.as_ptr(),
            ptr::addr_of_mut!(any_purged).cast(),
        )
    };

    if any_purged { 1 } else { -1 }
}

/// # Safety
/// `subproc` must identify live initialized subprocess metadata, including its arena registry and
/// expiration atomic. Registered arenas and their backing must remain live during guarded
/// visitation; registry teardown/destruction must not race, and `tseq` must be its valid seed.
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
    // SAFETY: the contract keeps the aligned initialized subprocess expiration field live; acquire
    // observes its expiration publication. Purge bitmap traversal uses its own atomic protocol.
    let arenas_expire = unsafe {
        crate::core_types::AtomicI64Field::from_cell(
            &(*subproc.cast::<MiSubprocPrefix>()).purge_expire,
        )
        .load(crate::core_types::MemoryOrder::Acquire)
    };
    if !visit_all && !force && (arenas_expire == 0 || arenas_expire > now) {
        return;
    }

    // SAFETY: the live subprocess registry remains initialized and stable; this typed count reads
    // its current bounded high-water mark.
    let max_arena = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    if max_arena == 0 {
        return;
    }

    static PURGE_GUARD: mi_atomic_guard_t = AtomicUsize::new(0);
    crate::atomic::mi_atomic_guard!(ptr::addr_of!(PURGE_GUARD).cast_mut(), {
        if arenas_expire > now {
            // SAFETY: the subprocess expiration field stays live; the purge guard serializes this
            // policy update and release publishes the shortened delay.
            unsafe {
                crate::core_types::AtomicI64Field::from_cell(
                    &(*subproc.cast::<MiSubprocPrefix>()).purge_expire,
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
            // SAFETY: `arena_idx` is below the registry count snapshot, and the subprocess/guard
            // contract keeps registry storage stable during this slot load.
            let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
            if let Some(arena) = ArenaPtr::new(arena) {
                // SAFETY: the registry entry is live under the subprocess/guard contract; its
                // purge bitmap, backing, and callback state remain stable for this visit.
                let purged = unsafe { mi_arena_try_purge(arena, now, force) };
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
            // SAFETY: the aligned live subprocess expiration field is cleared atomically only after
            // no arena reported pending or due work during this guarded traversal.
            unsafe {
                crate::core_types::AtomicI64Field::from_cell(
                    &(*subproc.cast::<MiSubprocPrefix>()).purge_expire,
                )
                .store(0, crate::core_types::MemoryOrder::Release)
            };
        }
    });
}

/// # Safety
/// `tld` must be non-null, aligned, and point to live initialized current-thread TLD metadata. Its
/// subprocess association/registry and thread sequence must remain stable; teardown or arena
/// destruction/relink must not race this synchronous collection.
pub(crate) unsafe extern "C" fn _mi_arenas_collect(
    force_purge: bool,
    visit_all: bool,
    tld: *mut mi_tld_t,
) {
    // SAFETY: the wrapper contract keeps TLD fields readable and its associated subprocess live;
    // the extracted association and sequence satisfy the synchronous guarded traversal.
    unsafe {
        mi_arenas_try_purge(
            force_purge,
            visit_all,
            SubprocPtr::new_unchecked((*tld).subproc),
            crate::core_types::ThreadSeq::new((*tld).thread_seq),
        )
    };
}
