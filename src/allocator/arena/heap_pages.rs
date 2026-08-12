//! Heap page visitation and heap-owned page transfer helpers.
//!
//! This module keeps heap visit/delete/move/destroy helpers out of the main
//! arena allocation flow while preserving the C-shaped entry points and local
//! callback contracts.

use super::*;

#[allow(non_camel_case_types)]
#[repr(C)]
struct mi_heap_visit_info_t {
    heap: HeapPtr,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
    visit_blocks: bool,
}

#[allow(non_camel_case_types)]
#[repr(C)]
struct mi_heap_delete_visit_info_t {
    heap_target: *mut mi_heap_t,
    theap_target: *mut mi_theap_t,
}

/// # Safety
/// `page` must identify a live initialized page owned by `vinfo.heap`, with stable metadata, block
/// backing, geometry, and heap association for this synchronous visit. The callback and opaque
/// argument must remain valid and must not unwind. A callback returning true may allocate/change
/// availability, but must not invalidate the current page/backing, change geometry, or mutate the
/// area descriptor before block traversal returns. The stack-local area pointer must not be
/// retained after its callback invocation; the opaque argument remains caller-governed.
unsafe fn mi_heap_visit_page(page: PagePtr, vinfo: &mut mi_heap_visit_info_t) -> bool {
    // SAFETY: the page visitor keeps this page's initialized geometry and
    // counts stable while projecting the callback value, and keeps its page
    // metadata and block backing live with stable geometry through both
    // callback invocations below.
    let Some(area) = (unsafe { heap_area_for_page(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    let page_raw = page.as_ptr();
    // SAFETY: the contract keeps this live page's initialized heap association stable throughout
    // the synchronous visit.
    crate::internal::mi_assert_internal!(unsafe { vinfo.heap.as_ptr() == mi_page_heap(page_raw) });
    // SAFETY: the callback and opaque argument remain valid and non-unwinding; `area` is initialized
    // stack-local callback data for this invocation and cannot be retained by contract.
    if !unsafe {
        vinfo.visitor.invoke(
            vinfo.heap,
            ptr::addr_of!(area),
            ptr::null_mut(),
            area.block_size,
            vinfo.arg,
        )
    } {
        return false;
    }
    if vinfo.visit_blocks {
        // SAFETY: area and page are live validated witnesses from the heap-visit protocol; visit_page_blocks only traverses linked blocks.
        unsafe {
            // SAFETY: `area` matches this live page, and a continuing callback preserved its
            // backing, geometry, callback, and opaque argument for synchronous block traversal.
            visit_page_blocks(&area, page, vinfo.visitor, vinfo.arg)
        }
    } else {
        true
    }
}

/// # Safety
/// The bitmap visitor must call this synchronously with `arena` naming a live registered arena,
/// `slice_index` naming an initialized page start owned by the visited heap, and `arg` non-null,
/// aligned, initialized, and uniquely writable as `mi_heap_visit_info_t`. Page/arena backing and
/// callback state must remain live and stable, and callbacks must not unwind.
unsafe extern "C" fn mi_heap_visit_page_at(
    slice_index: usize,
    _slice_count: usize,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let Some(arena) = ArenaPtr::new(arena) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY: the bitmap callback contract supplies an in-bounds arena-owned slice index and a
    // uniquely writable stack-local visit argument for this synchronous invocation.
    let Some(vinfo) = (unsafe { arg.cast::<mi_heap_visit_info_t>().as_mut() }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY: bitmap traversal keeps the registered arena and page metadata live, and the callback
    // index identifies an initialized page start in this arena.
    let page = unsafe { mi_arena_page_at_slice(arena, SliceIndex::new(slice_index)) };
    // SAFETY: the live page belongs to the visited heap; `vinfo` remains exclusive and its callback
    // state remains valid for this synchronous dispatch.
    unsafe { mi_heap_visit_page(page, vinfo) }
}

/// # Safety
/// `heap` must remain a live initialized borrowed heap with stable subprocess registry,
/// arena-page tables, OS-abandoned list, and visited page backing for this synchronous traversal.
/// `visitor` and `arg` must satisfy the block-visit callback contract and must not unwind. Callback
/// allocator reentry may change availability but must not destroy/relink the traversed heap/page or
/// change current page geometry before traversal advances. Each stack-local area descriptor may be
/// used only during its callback and must not be retained; opaque `arg` is passed through unchanged.
unsafe fn _mi_heap_visit_blocks_internal(
    heap: BorrowedHeap,
    abandoned_only: bool,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    let borrowed_heap = heap;
    let heap_ptr = heap.ptr();
    let heap = heap_ptr.as_ptr();
    let mut visit_info = mi_heap_visit_info_t {
        heap: heap_ptr,
        visitor: BlockVisitCallback::new(Some(visitor)),
        arg: BlockVisitCallbackArg::from_raw(arg),
        visit_blocks,
    };
    let mut ok = true;
    let mut visit_arena = |arena| {
        // SAFETY: the internal visitor contract keeps this registered arena and borrowed heap live
        // in one subprocess, with a stable arena index/table publication.
        let arena_pages = unsafe { mi_heap_arena_pages(borrowed_heap, arena) };
        if ok && !arena_pages.is_null() {
            if abandoned_only {
                for bin in 0..MI_BIN_COUNT {
                    if !ok {
                        break;
                    }
                    let Some(typed_bin) = crate::core_types::Bin::new(bin as u16) else {
                        crate::internal::mi_assert_internal!(false);
                        break;
                    };
                    // SAFETY: the borrowed heap remains live and `typed_bin` bounds its initialized
                    // atomic abandoned counter; relaxed load is the traversal skip hint.
                    if unsafe { mi_heap_abandoned_count(heap, typed_bin).load_relaxed() } > 0 {
                        // SAFETY: the acquire-published non-null table and selected per-bin bitmap
                        // remain live; synchronous traversal keeps `visit_info` uniquely writable.
                        ok = unsafe {
                            mi_bitmap_forall_set_typed(
                                (*arena_pages).pages_abandoned[typed_bin.as_usize()],
                                mi_heap_visit_page_at,
                                Some(arena),
                                ptr::addr_of_mut!(visit_info).cast(),
                            )
                        };
                    }
                }
            } else {
                // SAFETY: the acquire-published non-null table and its page bitmap remain live;
                // synchronous traversal keeps `visit_info` uniquely writable for each callback.
                ok = unsafe {
                    mi_bitmap_forall_set_typed(
                        (*arena_pages).pages,
                        mi_heap_visit_page_at,
                        Some(arena),
                        ptr::addr_of_mut!(visit_info).cast(),
                    )
                };
            }
        }
        ok
    };
    // SAFETY: the borrowed heap's live stable subprocess registry bounds arena enumeration; the
    // synchronous closure does not retain arena pointers or callback state.
    unsafe {
        mi_forall_arenas(
            borrowed_heap,
            None,
            crate::core_types::ThreadSeq::new(0),
            &mut visit_arena,
        );
    }
    if !ok {
        return false;
    }

    // SAFETY: the borrowed heap remains live; this helper takes the OS-abandoned-list lock only for
    // the head snapshot. The outer exclusive/no-relink contract stabilizes subsequent unlocked walk.
    let page = unsafe { super::os_abandoned::first_snapshot(heap_ptr) };
    let mut page = page;
    while let Some(current) = page.filter(|_| ok) {
        // SAFETY: the exclusive traversal contract keeps the current page and its initialized OS
        // abandoned-list link live and stable even though the snapshot lock is no longer held.
        let next = unsafe { super::os_abandoned::page_next(current) };
        // SAFETY: `current` belongs to this live borrowed heap and its backing/geometry remain
        // stable; exclusive traversal and callback rules satisfy the page visitor.
        ok = unsafe { mi_heap_visit_page(current, &mut visit_info) };
        page = next;
    }

    ok
}

/// # Safety
/// A non-null `heap` must remain live/initialized for synchronous traversal; null selects the live
/// main heap. `visitor` and `arg` must remain valid and must not unwind. A callback returning true
/// may allocate/change availability, but must not invalidate the current page backing, change its
/// geometry, mutate or retain the stack-local current area descriptor, or destroy/relink traversal
/// state before return. Opaque `arg` is passed through and remains caller-governed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_visit_blocks(
    heap: *mut mi_heap_t,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    let heap = if heap.is_null() {
        crate::allocator::init::mi_heap_main()
    } else {
        heap
    };
    let Some(heap_ptr) = HeapPtr::new(heap) else {
        return false;
    };
    // SAFETY: the public contract keeps this non-null heap and traversal state live; the borrowed
    // witness and synchronous callback arguments satisfy the internal visitor.
    unsafe {
        _mi_heap_visit_blocks_internal(
            BorrowedHeap::new_unchecked(heap_ptr),
            false,
            visit_blocks,
            visitor,
            arg,
        )
    }
}

/// # Safety
/// A non-null `heap` must remain live/initialized for synchronous abandoned-page traversal; null
/// selects the live main heap. `visitor` and `arg` must remain valid and must not unwind. A callback
/// returning true may allocate/change availability, but must not invalidate current page backing,
/// change geometry, mutate or retain the stack-local area descriptor, or destroy/relink traversal
/// state before return. Opaque `arg` is passed through and remains caller-governed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_visit_abandoned_blocks(
    heap: *mut mi_heap_t,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    let heap = if heap.is_null() {
        crate::allocator::init::mi_heap_main()
    } else {
        heap
    };
    let Some(heap_ptr) = HeapPtr::new(heap) else {
        return false;
    };
    // SAFETY: the public contract keeps this non-null heap and abandoned traversal state live; the
    // borrowed witness and synchronous callback arguments satisfy the internal visitor.
    unsafe {
        _mi_heap_visit_blocks_internal(
            BorrowedHeap::new_unchecked(heap_ptr),
            true,
            visit_blocks,
            visitor,
            arg,
        )
    }
}

/// # Safety
/// This callback may only be invoked synchronously by the exclusive heap-delete traversal. `heap`
/// must name its live source heap; `area` must be non-null/aligned and describe a live source page
/// in `reserved1`; `arg` must be non-null/aligned/initialized and uniquely writable delete state.
/// The source/target heap metadata, arena tables, page backing, stats, and locks must remain live;
/// no concurrent teardown, queue/list relink, heap reassignment, or other non-protocol metadata
/// mutation may race this non-unwinding callback. Remote-free head publication remains permitted
/// only through the page's atomic protocol.
unsafe extern "C" fn mi_heap_delete_page(
    heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    _block: *mut c_void,
    _block_size: usize,
    arg: *mut c_void,
) -> bool {
    let info = arg.cast::<mi_heap_delete_visit_info_t>();
    // SAFETY: the callback contract makes `info` aligned, initialized, uniquely writable, and live
    // for this invocation; copying both target fields ends raw delete-state access.
    let (mut heap_target, mut theap_target) =
        unsafe { ((*info).heap_target, (*info).theap_target) };
    let current_theap = ptr::null_mut::<mi_theap_t>();
    // SAFETY: the callback contract makes `area` aligned and initialized, with `reserved1` storing
    // this visited page's live metadata pointer.
    let page = unsafe { (*area).reserved1.cast::<mi_page_t>() };

    // SAFETY: the callback contract guarantees that `reserved1` contains the non-null live page
    // metadata pointer for this source-heap area.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: exclusive source-heap deletion keeps this page's initialized owner state stable for
    // classification before any queue/list mutation.
    let is_abandoned = unsafe { mi_page_is_abandoned(page) };
    if is_abandoned {
        // SAFETY: classification above established abandoned state for this live page; exclusive
        // deletion excludes a competing non-protocol metadata transition.
        let abandoned = unsafe { crate::core_types::assume_abandoned_page(page_ptr) };
        let Ok(owned) = abandoned.try_claim_ownership() else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };
        // SAFETY: the claimed page retains stable heap/MemId/list state; null current thread heap
        // selects atomic source stats while abandoned publication is removed.
        unsafe { _mi_arenas_page_unabandon_internal(&owned, None) };
    } else {
        // SAFETY: the live page's initialized thread-free atomic remains stable under exclusive
        // heap deletion; this RMW must acquire its owner bit before any metadata mutation.
        let atomic = unsafe { page_ptr.thread_free_cell() };
        // SAFETY: `atomic` is the matching live thread-free field. This idempotent fetch-or retains
        // an existing local owner bit or sets it; exclusive heap deletion excludes a competing
        // owner transition while protocol remote-head publication remains permitted.
        let _ = unsafe { atomic.claim_acq_rel() };
        // SAFETY: the active owner bit plus exclusive heap deletion authorizes detaching the live
        // page's queue links and publishing its abandoned thread-heap state.
        unsafe {
            page_ptr.set_queue_next(None);
            page_ptr.set_queue_prev(None);
            mi_page_set_theap(page, ptr::null_mut());
        }
    }
    // SAFETY: the abandoned branch claimed ownership before typed unabandon, while the local branch
    // idempotently set the owner bit before detaching and marking the page abandoned.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    // SAFETY: exclusive deletion keeps the initialized atomic owner bit set after either transition.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    // SAFETY: active claimed ownership keeps this page's initialized used count stable while the
    // exclusive delete path chooses release, destruction, or transfer.
    if unsafe { crate::allocator::page::mi_page_used_count(page).get() == 0 } {
        // SAFETY: the page is all-free, detached, abandoned, and still carries the active owner bit;
        // null current thread heap selects the atomic release/accounting path.
        unsafe { _mi_arenas_page_free(page, current_theap) };
    } else if heap_target.is_null() {
        // SAFETY: destroying a non-empty page
        // clears its used count before returning the owned page to the arena.
        unsafe {
            crate::allocator::page::mi_page_set_used_count(
                page,
                crate::core_types::UsedBlockCount::new(0),
            )
        };
        // SAFETY: used was reset under exclusive ownership, so the detached abandoned page is now
        // all-free and releasable through the null current-thread path.
        unsafe { _mi_arenas_page_free(page, current_theap) };
    } else {
        // SAFETY: active ownership keeps this page's initialized block-size classification stable
        // while selecting its source and target stats bin.
        let sbin = unsafe { _mi_page_stats_bin(page) };
        // SAFETY: exclusive deletion and the active owner bit keep the live page, MemId, heap
        // association, arena table, and start bit stable through this operation-scoped snapshot.
        let Some(membership) = (unsafe { PageArenaMembership::from_page(page_ptr) }) else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };
        let arena_id = membership.arena().as_ptr();
        let slice_index = membership.range().start_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: the operation-scoped membership snapshot validated this exact live source table
        // and start bit; exclusive transfer owns clearing its page publication.
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        let heap = heap as *mut mi_heap_t;
        // SAFETY: exclusive deletion keeps the live source heap and its aligned initialized stats
        // storage valid; MT helpers preserve the shared stats protocol.
        unsafe {
            __mi_stat_decrease_mt(ptr::addr_of_mut!((*heap).stats.page_bins[sbin]), 1);
            __mi_stat_decrease_mt(ptr::addr_of_mut!((*heap).stats.pages), 1);
        }
        // SAFETY: the non-null live target heap and registered arena remain stable; this resolves
        // or creates the target's initialized arena-page table before transfer publication.
        let mut arena_pages_target =
            unsafe { mi_heap_ensure_arena_pages_from_raw(heap_target, arena_id) };
        if arena_pages_target.is_null() {
            heap_target = crate::allocator::init::mi_heap_main();
            // SAFETY: the fallback main heap is live initialized allocator metadata, so its owning
            // thread-heap prefix is readable for target stats.
            theap_target = unsafe { mi_heap_theap(heap_target) };
            // SAFETY: the live fallback main heap can resolve/create the table for this registered
            // arena; transfer remains exclusive and arena identity stable.
            arena_pages_target =
                unsafe { mi_heap_ensure_arena_pages_from_raw(heap_target, arena_id) };
            crate::internal::mi_assert_internal!(!arena_pages_target.is_null());
        }
        // SAFETY: the non-null target table and bitmap remain live and cover `slice_index`; this
        // reads the required clear state before publication.
        crate::internal::mi_assert_internal!(unsafe {
            !mi_bitmap_is_set((*arena_pages_target).pages, slice_index)
        });
        // SAFETY: the live initialized target bitmap covers this validated start bit and is clear;
        // exclusive transfer publishes it before page ownership changes.
        unsafe { mi_bitmap_set((*arena_pages_target).pages, slice_index) };
        // SAFETY: the claimed detached page metadata is exclusively writable and the target heap
        // remains live; bitmap publication above precedes reassignment.
        unsafe { (*page).heap = heap_target };
        // SAFETY: the live target thread heap owns aligned initialized stats storage; exclusive
        // transfer updates the matching bin and page totals after heap reassignment.
        unsafe {
            __mi_stat_increase(ptr::addr_of_mut!((*theap_target).stats.page_bins[sbin]), 1);
            __mi_stat_increase(ptr::addr_of_mut!((*theap_target).stats.pages), 1);
        }
        // SAFETY: the page retains its active owner bit after reassignment and target publication;
        // the live target thread heap owns stats while raw abandon completes mapped publication.
        let _ = unsafe { _mi_arenas_page_abandon(page, theap_target) };
    }
    true
}

/// # Safety
/// `heap` must retain exclusive ownership of a live non-main source heap and all its page/list/table
/// transitions for the complete drain. A non-null `heap_target` must identify a distinct live
/// initialized target heap with live thread-heap stats and arena tables. Source/target teardown,
/// queue/list relinking, heap reassignment, or other non-protocol metadata mutation must not race;
/// remote frees may publish only through the page atomic protocol. Internal callbacks execute
/// synchronously and do not unwind.
unsafe fn mi_heap_delete_pages(heap: OwnedHeap, heap_target: *mut mi_heap_t) {
    let heap_ptr = heap.ptr();
    // SAFETY: the owned witness keeps this source heap live and exclusively stable for the
    // synchronous borrowed traversal.
    let borrowed_heap = unsafe { BorrowedHeap::new_unchecked(heap_ptr) };
    let heap = heap_ptr.as_ptr();
    let theap_target = if heap_target.is_null() {
        ptr::null_mut()
    } else {
        // SAFETY: the helper contract keeps this non-null target heap live and initialized, so its
        // owning thread-heap prefix remains readable for target stats updates.
        unsafe { mi_heap_theap(heap_target) }
    };
    let mut info = mi_heap_delete_visit_info_t {
        heap_target,
        theap_target,
    };
    // SAFETY: source ownership stabilizes its registry/tables/lists and the target contract keeps
    // target state live; synchronous traversal keeps unique stack-local delete state valid.
    let _ = unsafe {
        _mi_heap_visit_blocks_internal(
            borrowed_heap,
            false,
            false,
            mi_heap_delete_page,
            ptr::addr_of_mut!(info).cast(),
        )
    };
    if MI_DEBUG > 1 {
        for i in 0..MI_ARENA_BIN_COUNT {
            let Some(arena_idx) = ArenaIndex::from_usize(i) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY: `arena_idx` is within the initialized arena-page table of the still-live
            // exclusively drained heap; acquire observes any table publication.
            let arena_pages =
                unsafe { mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire() };
            if !arena_pages.is_null() {
                // SAFETY: the acquire-loaded table remains live under source ownership, and its
                // page bitmap is initialized for this post-drain emptiness check.
                crate::internal::mi_assert_internal!(unsafe {
                    mi_bitmap_is_all_clear((*arena_pages).pages)
                });
            }
        }
        // SAFETY: the live drained heap owns this initialized list/lock; the helper holds the lock
        // while checking the head and source ownership excludes concurrent relinking.
        crate::internal::mi_assert_internal!(unsafe { super::os_abandoned::is_empty(heap_ptr) });
        for i in 0..MI_BIN_COUNT {
            let Some(bin) = crate::core_types::Bin::new(i as u16) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY: the live heap's initialized abandoned counter array is indexed by checked
            // `bin`; atomic relaxed load is sufficient for this exclusive post-drain assertion.
            crate::internal::mi_assert_internal!(unsafe {
                mi_heap_abandoned_count(heap, bin).load_relaxed() == 0
            });
        }
    }
}

/// # Safety
/// `heap_from` must identify a live non-main heap exclusively owned for page draining. A non-null
/// `heap_to` must identify a distinct live initialized target heap; null selects the live main heap.
/// Both heaps' arena tables, stats, page backing, and list/lock state must remain valid, and no
/// concurrent teardown, queue/list relink, heap reassignment, or other non-protocol metadata
/// mutation may race this synchronous transfer. Remote frees may publish only through the page's
/// atomic protocol.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_move_pages(
    heap_from: *mut mi_heap_t,
    mut heap_to: *mut mi_heap_t,
) {
    let Some(heap_from) = HeapPtr::new(heap_from) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: the source heap must be uniquely
    // owned while pages are drained from it.
    let Some(heap_from) = (unsafe { OwnedHeap::new(heap_from) }) else {
        return;
    };
    if heap_to.is_null() {
        heap_to = crate::allocator::init::mi_heap_main();
    }
    let Some(heap_to) = HeapPtr::new(heap_to) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: source ownership remains exclusive and the target heap stays live for the complete
    // transfer; page deletion consumes the source pages into this non-null target.
    unsafe { mi_heap_delete_pages(heap_from, heap_to.as_ptr()) };
}

/// # Safety
/// `heap_from` must identify a live initialized heap. The main heap is accepted as a no-op; a
/// non-main heap must be exclusively owned for destructive page draining. Its arena tables, stats,
/// page backing, and list/lock state must remain valid, and no concurrent teardown, page mutation,
/// or relink may race this synchronous destruction.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_destroy_pages(heap_from: *mut mi_heap_t) {
    // SAFETY: main heap pages are never drained
    // through this destroy helper.
    if unsafe { _mi_is_heap_main(heap_from) } {
        return;
    }
    let Some(heap_from) = HeapPtr::new(heap_from) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: non-main source heap ownership is
    // required before destructive page draining.
    let Some(heap_from) = (unsafe { OwnedHeap::new(heap_from) }) else {
        return;
    };
    // SAFETY: owned non-main heap can be drained into the free path; null target selects
    // destruction rather than transfer.
    unsafe { mi_heap_delete_pages(heap_from, ptr::null_mut()) };
}
