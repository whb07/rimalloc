//! Abandoned-page ownership and per-heap arena page-table helpers.
//!
//! This module owns the small unsafe surface where abandoned pages are claimed
//! back into a thread heap and where the heap-local arena page table is lazily
//! published for those ownership checks.

use super::metadata::{mi_arena_pages_alloc, mi_arena_pages_size};
use super::*;

/// Release a claimed abandoned page, first collecting any remote frees.
///
/// # Safety
/// `page` must remain live, initialized, abandoned, and already carry the
/// claimed thread-free owner bit. `current_theap` must be the live current-thread
/// heap-local context responsible for its stats and any terminal free. The
/// caller must exclusively own page-local lists and metadata; protocol remote
/// atomic free publication may continue, but concurrent claim, reclaim, queue
/// mutation, or teardown is excluded.
pub(super) unsafe fn mi_abandoned_page_unown(
    owned_page: crate::types::OwnedAbandonedPage,
    current_theap: crate::types::ThreadHeapLocal,
) -> crate::types::PageAbandonOutcome {
    let page_ptr = owned_page.page();
    let page = page_ptr.as_ptr();
    let current_theap_raw = current_theap.ptr().as_ptr();
    // SAFETY: the caller keeps the claimed abandoned page's initialized atomic
    // owner word live for this invariant check.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    // SAFETY: the caller guarantees the live page remains in abandoned state
    // throughout collection and release.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    // SAFETY: current_theap and its TLD are caller-proven live initialized
    // current-thread metadata for this synchronous ownership transition.
    crate::internal::mi_assert_internal!(unsafe {
        _mi_thread_id() == (*(*current_theap_raw).tld).thread_id
    });
    // SAFETY: the caller keeps the page's aligned atomic thread-free field live
    // under the remote-publication protocol.
    let atomic_tf = unsafe { page_ptr.thread_free_cell() };
    let mut tf_old = atomic_tf.load_relaxed();
    loop {
        crate::internal::mi_assert_internal!(tf_old.is_owned());
        while tf_old.head().is_some() {
            // SAFETY: claimed ownership gives exclusive page-local metadata
            // access while remote producers touch only the atomic head.
            let local_free = unsafe { page_ptr.local_free() };
            // SAFETY: the live atomic field remains claimed; detaching preserves
            // the owner bit while transferring the observed remote chain.
            let detached = unsafe { atomic_tf.detach_head() };
            // SAFETY: owned_page proves exclusive abandoned collection; both
            // detached and local lists belong to this live page, whose block
            // geometry and encoding keys remain stable.
            let local_free = unsafe {
                match crate::allocator::page::collect_remote_for_owned_abandoned_page(
                    &owned_page,
                    detached,
                    local_free,
                ) {
                    Ok(local_free) => local_free,
                    Err(_) => page_ptr.local_free(),
                }
            };
            // SAFETY: collection retains exclusive access to this page's
            // initialized available-list head for the emptiness snapshot.
            if unsafe { page_ptr.free().is_empty() } && !local_free.is_empty() {
                // SAFETY: `owned_page` retains the claimed abandoned owner and
                // `local_free` was read from that same page immediately above.
                unsafe { owned_page.transfer_local_to_available(local_free) };
            }
            // SAFETY: after owned collection, the caller stabilizes used/free
            // accounting sufficiently to test the complete all-free state.
            if unsafe { mi_page_all_free(page) } {
                // SAFETY: current_theap_raw is the same live current-thread
                // local heap consumed for the checks above and remains valid.
                let current_theap = unsafe {
                    crate::types::ThreadHeapLocal::new_unchecked(
                        crate::types::ThreadHeapPtr::new_unchecked(current_theap_raw),
                    )
                };
                // SAFETY: the page is all-free with active claimed ownership;
                // competing mutation is excluded and current_theap owns stats.
                unsafe { _mi_arenas_page_unabandon_internal(&owned_page, Some(current_theap)) };
                let Ok(all_free) = owned_page.try_mark_all_free() else {
                    crate::internal::mi_assert_internal!(false);
                    return crate::types::PageAbandonOutcome::Abandoned;
                };
                // SAFETY: unabandon removed publication/accounting while the
                // retained claim refined into this detached all-free witness.
                let destroyed = unsafe { _mi_arenas_page_free_typed(all_free, current_theap) };
                let _ = destroyed.page();
                return crate::types::PageAbandonOutcome::Destroyed;
            }
            tf_old = atomic_tf.load_relaxed();
        }
        crate::internal::mi_assert_internal!(tf_old.head().is_none());
        // SAFETY: the live atomic remains claimed with the expected stable
        // snapshot; CAS releases only ownership while preserving remote head.
        if unsafe { atomic_tf.release_acq_rel(&mut tf_old) } {
            return crate::types::PageAbandonOutcome::Abandoned;
        }
    }
}

/// # Safety
/// `arena` must be aligned live initialized arena metadata with stable slice
/// count, page-metadata mode, and backing covering `slice_index`. That slice
/// must start a live abandoned page with a valid atomic thread-free field.
/// `keep_abandoned` remains exclusively borrowed for the result write.
/// Concurrent page destruction or metadata relocation is excluded; competing
/// claims may occur only through the atomic ownership protocol.
pub(super) unsafe fn mi_arena_try_claim_abandoned_at(
    slice_index: SliceIndex,
    arena: *mut mi_arena_t,
    keep_abandoned: &mut bool,
) -> bool {
    let slice_index = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY: the caller keeps the live arena's initialized slice count
        // stable for validation of the typed index.
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let page = {
        let index = slice_index;
        let page_addr: usize;
        // SAFETY: the caller guarantees the live arena prefix layout and stable
        // pages_meta value; the checked index selects either its metadata array
        // entry or the aligned arena slice without dereferencing the result.
        unsafe {
            core::arch::asm!(
                "mov {page_addr}, qword ptr [{arena} + {pages_meta_offset}]",
                "test {page_addr}, {page_addr}",
                "je 2f",
                "imul {index}, {index}, {page_size}",
                "add {page_addr}, {index}",
                "jmp 3f",
                "2:",
                "shl {index}, {slice_shift}",
                "lea {page_addr}, [{arena} + {index}]",
                "3:",
                arena = in(reg) arena,
                index = inout(reg) index => _,
                page_addr = out(reg) page_addr,
                pages_meta_offset = const offset_of!(MiArenaPrefix, pages_meta),
                page_size = const size_of::<mi_page_t>(),
                slice_shift = const MI_ARENA_SLICE_SIZE.trailing_zeros(),
                options(nostack, readonly)
            );
        }
        page_addr as *mut mi_page_t
    };
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    let page = {
        // SAFETY: the caller keeps the initialized pages_meta field readable
        // and stable for this projection mode snapshot.
        let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
        if !pages_meta.is_null() {
            // SAFETY: the checked slice index is in bounds for the caller-proven
            // live separate page-metadata array.
            unsafe { pages_meta.add(slice_index) }
        } else {
            // SAFETY: the caller-proven arena backing covers the checked whole
            // slice offset and colocates aligned live page metadata there.
            unsafe {
                arena
                    .cast::<u8>()
                    .add(slice_index * MI_ARENA_SLICE_SIZE)
                    .cast::<mi_page_t>()
            }
        }
    };
    // SAFETY: both projection modes produce non-null aligned live page metadata
    // at the caller-proven abandoned slice start.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: the caller guarantees this projected live page is initialized and
    // abandoned, with atomic ownership available for a competing claim.
    let abandoned = unsafe { crate::allocator::raw_bridge::assume_abandoned_page(page_ptr) };
    if abandoned.try_claim_ownership().is_err() {
        *keep_abandoned = true;
        false
    } else {
        *keep_abandoned = false;
        true
    }
}

/// # Safety
/// `arena` must satisfy `mi_arena_try_claim_abandoned_at`; the raw index must be
/// within its initialized slice count and identify an abandoned page start.
/// `keep_abandoned` must be non-null, aligned, and uniquely writable for one
/// bool. A true return transfers an unreleased page claim to the scanner, which
/// must complete the corresponding reclaim/unown protocol.
pub(super) unsafe extern "C" fn mi_arena_try_claim_abandoned(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_abandoned: *mut bool,
) -> bool {
    // SAFETY: the callback contract guarantees a non-null aligned uniquely
    // writable bool for this invocation; the reference does not escape.
    let keep_abandoned = unsafe { &mut *keep_abandoned };
    // SAFETY: the callback contract supplies the in-range raw index, live arena
    // backing, writable result flag, and atomic abandoned-claim protocol.
    unsafe { mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned) }
}

/// # Safety
/// `heap` and `arena` must be non-null aligned live initialized metadata in the
/// same subproc. The arena must remain registered with stable index and slice
/// count. The heap's arena-page table and lock must remain live; all concurrent
/// table creation must use the same lock/publication protocol.
pub(super) unsafe fn mi_heap_ensure_arena_pages_from_raw(
    heap: *mut mi_heap_t,
    arena: *mut mi_arena_t,
) -> *mut mi_arena_pages_t {
    crate::internal::mi_assert_internal!(!arena.is_null());
    crate::internal::mi_assert_internal!(!heap.is_null());
    // SAFETY: the caller keeps the live registered arena's initialized index
    // stable and within the heap table domain.
    let arena_idx = unsafe { mi_arena_index(arena) };
    // SAFETY: the raw contract proves both pointers non-null and live in the
    // same subproc; the checked index matches arena registration.
    unsafe {
        mi_heap_ensure_arena_pages(
            BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)),
            ArenaPtr::new_unchecked(arena),
            arena_idx,
        )
    }
}

/// # Safety
/// `heap` and `arena` must remain live in one subproc, and `arena_idx` must be
/// the arena's stable registered index. The heap arena-page cell and lock must
/// be initialized; competing initialization must follow the same lock and
/// release-publication protocol. Concurrent heap/arena teardown is excluded.
unsafe fn mi_heap_ensure_arena_pages(
    heap: BorrowedHeap,
    arena: ArenaPtr,
    arena_idx: ArenaIndex,
) -> *mut mi_arena_pages_t {
    let heap_raw = heap.ptr().as_ptr();
    let arena_raw = arena.as_ptr();

    // SAFETY: the caller keeps the matching heap/arena/table live and stable
    // while the existing atomic table pointer is acquired.
    let mut arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
    if arena_pages.is_null() {
        // SAFETY: the caller keeps this heap's initialized arena-pages lock live;
        // holding it serializes table allocation and publication.
        let _lock = unsafe { MiLockGuard::acquire(heap.ptr().arena_pages_lock()) };
        // SAFETY: under the live heap lock, recheck the matching atomic table
        // cell after acquiring prior publication.
        arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
        if arena_pages.is_null() {
            // SAFETY: the caller keeps the initialized heap identity stable;
            // this predicate only compares it with the live main heap.
            if unsafe { _mi_is_heap_main(heap_raw) } {
                // SAFETY: the live main arena owns an initialized embedded page
                // table; lock ownership excludes concurrent publication.
                arena_pages =
                    unsafe { ptr::addr_of_mut!((*arena_raw.cast::<MiArenaPrefix>()).pages_main) };
            } else {
                // SAFETY: the caller keeps the live arena's initialized nonzero
                // slice count stable while constructing its complete table range.
                let slice_count = unsafe { arena.slice_count() };
                let Ok(range) = crate::types::SliceRange::new(0, slice_count.get() as usize, arena)
                else {
                    crate::internal::mi_assert_internal!(false);
                    return ptr::null_mut();
                };
                let (alloc_size, bitmap_base) = mi_arena_pages_size(slice_count);
                // SAFETY: the complete typed range matches the stable arena
                // slice count, and both layout values were derived from it;
                // lock ownership grants exclusive allocation/publication.
                arena_pages =
                    unsafe { mi_arena_pages_alloc(arena, range, alloc_size, bitmap_base) };
            }
            // SAFETY: this locked heap owns the initialized cell selected by
            // the matching registered index; release-store publishes null or
            // the fully initialized table exactly as the existing protocol.
            unsafe { mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast()) };
        }
    }
    // SAFETY: the live heap identity remains stable; main heaps must always use
    // their non-null embedded table after locked initialization.
    if unsafe { _mi_is_heap_main(heap_raw) } {
        crate::internal::mi_assert!(!arena_pages.is_null());
    }
    arena_pages
}

#[inline]
/// # Safety
/// `page` must be non-null aligned live initialized page metadata whose stable
/// `reserved`, `used`, and available-list fields satisfy `used <= reserved`.
/// The caller must hold local or claimed-abandoned ownership preventing
/// concurrent mutation of those fields; protocol remote publication may touch
/// only its separate atomic thread-free state.
pub(super) unsafe fn mi_page_is_full_local(page: *const mi_page_t) -> bool {
    // SAFETY: the caller keeps this page's initialized reserved and used counts
    // stable and ordered while both scalar values are copied.
    let full = unsafe {
        crate::allocator::page::mi_page_reserved_count(page).get()
            == crate::allocator::page::mi_page_used_count(page).get()
    };
    // SAFETY: the caller guarantees non-null aligned live page metadata and
    // stable readable available-list head under local/claimed ownership.
    let free_is_empty = unsafe {
        crate::types::PagePtr::new_unchecked(page.cast_mut())
            .free()
            .is_empty()
    };
    crate::internal::mi_assert_internal!(!full || free_is_empty);
    full
}

/// Publish a claimed abandoned page into its arena/heap abandoned structures,
/// then release the remote-free ownership claim.
///
/// # Safety
/// `page` must be live, detached from all queues, non-all-free, already marked
/// abandoned, and retain active claimed thread-free ownership for this
/// transition, with competing mutation excluded.
/// `current_theap` must be the live current-thread local heap formerly owning
/// the page and its stats. Page geometry, MemId, lists, heap and arena table
/// membership must remain stable; the caller excludes concurrent relink,
/// reclaim, or teardown while protocol remote frees may publish atomically.
pub(crate) unsafe fn _mi_arenas_page_abandon_internal(
    page: crate::types::OwnedAbandonedPage,
    current_theap: crate::types::ThreadHeapLocal,
) -> crate::types::PageAbandonOutcome {
    let page_ptr = page.page();
    let page_raw = page_ptr.as_ptr();
    let current_theap_raw = current_theap.ptr().as_ptr();
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: the owned witness keeps initialized page geometry live and
        // stable; its slice start is readable for the alignment invariant.
        unsafe { mi_page_slice_start(page_raw) }.cast(),
        MI_PAGE_ALIGN
    ));
    // SAFETY: stable live page geometry provides a valid start address in the
    // page-map domain and must resolve back to this exact metadata.
    crate::internal::mi_assert_internal!(
        unsafe { crate::internal::_mi_ptr_page(mi_page_start(page_raw).cast()) } == page_raw
    );
    // SAFETY: OwnedAbandonedPage proves the live atomic owner claim remains set.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page_raw) });
    // SAFETY: the witness and caller contract keep the page's abandoned owner
    // state stable through publication.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page_raw) });
    // SAFETY: the caller stabilizes page-local accounting and guarantees this
    // claimed abandon path is not the all-free terminal transition.
    crate::internal::mi_assert_internal!(!unsafe { mi_page_all_free(page_raw) });
    // SAFETY: the caller detached this live page before creating the owned
    // abandon witness and keeps both initialized queue links stable and empty.
    crate::internal::mi_assert_internal!(unsafe {
        page_ptr.queue_next().is_none() && page_ptr.queue_prev().is_none()
    });

    // SAFETY: the live claimed page retains its initialized former heap pointer
    // until abandon publication and stats reconciliation complete.
    let heap = unsafe { mi_page_heap(page_raw) };
    // SAFETY: current_theap is live initialized local metadata and its owning
    // heap association remains stable during this transition.
    crate::internal::mi_assert_internal!(heap == unsafe { _mi_theap_heap(current_theap_raw) });
    // SAFETY: the caller keeps the page's initialized MemId stable until the
    // corresponding arena/OS abandoned publication branch completes.
    let is_arena = unsafe { mi_page_memid_is_arena(page_raw) };
    let is_full = if is_arena {
        // SAFETY: claimed ownership stabilizes used/reserved/free fields for
        // the full-page decision; remote frees use only the atomic remote head.
        unsafe { mi_page_is_full_local(page_raw) }
    } else {
        false
    };
    if is_arena && !is_full {
        // SAFETY: the live page's initialized block size remains stable and
        // valid for bin classification while it is claimed.
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page_raw) });
        // SAFETY: the owned page, stable MemId/heap, and claimed state keep its
        // arena range/table membership live for the publication snapshot.
        let Some(membership) = (unsafe { PageArenaMembership::from_page(page_ptr) }) else {
            crate::internal::mi_assert_internal!(false);
            // SAFETY: invalid membership is outside the function contract, but the claimed page and
            // current thread heap remain live; release the claim rather than leaking ownership in a
            // non-asserting build.
            let current_theap = unsafe {
                crate::types::ThreadHeapLocal::new_unchecked(
                    crate::types::ThreadHeapPtr::new_unchecked(current_theap_raw),
                )
            };
            // SAFETY: the same active claim and live thread heap described above remain valid for
            // the synchronous claim release or terminal all-free cleanup.
            return unsafe { mi_abandoned_page_unown(page, current_theap) };
        };
        let arena = membership.arena().as_ptr();
        let range = membership.range();
        let slice_index = range.start_usize();
        let slice_count = range.count_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: claimed stable page metadata remains readable for the
        // singleton invariant before publishing a normal abandoned-page bit.
        crate::internal::mi_assert_internal!(!unsafe { mi_page_is_singleton(page_raw) });
        // SAFETY: the validated membership range indexes the live arena free
        // bbitmap, which remains stable and must still mark these slices in use.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY: claimed ownership stabilizes this page's initialized committed
        // byte count for comparison with arena commitment publication.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page_raw) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                // SAFETY: the validated live arena committed bitmap covers this
                // exact membership range and remains readable during claim.
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY: the validated live arena dirty bitmap covers this exact
        // membership range and remains readable during publication.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index,
                slice_count,
            )
        });

        // SAFETY: the owned page keeps its aligned atomic thread-id field live;
        // this owner publication precedes the arena abandoned bit.
        let atomic_tid = unsafe { page_ptr.thread_id_cell() };
        // SAFETY: the page is claimed, detached, and exclusively transitioning;
        // publishing AbandonedMapped preserves flags under the atomic protocol.
        unsafe { atomic_tid.store_owner(crate::types::PageOwnerId::AbandonedMapped) };
        // SAFETY: validated membership supplies the live per-bin bitmap and
        // in-range start bit; this transition owns its clear-to-set publication.
        let was_clear =
            unsafe { mi_bitmap_set((*arena_pages).pages_abandoned[bin.as_usize()], slice_index) };
        crate::internal::MI_UNUSED(&was_clear);
        crate::internal::mi_assert_internal!(was_clear);
        // SAFETY: heap and checked bin remain live; this atomic field is the
        // matching per-heap mapped-abandoned counter.
        unsafe { mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1) };
        // SAFETY: current_theap is the live current-thread stats owner and this
        // transition has exclusive non-atomic access to its counter.
        unsafe {
            __mi_stat_increase(
                ptr::addr_of_mut!((*current_theap_raw).stats.pages_abandoned),
                1,
            )
        };
    } else {
        if !is_arena && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned) {
            // SAFETY: `heap` owns this live detached non-arena page; the helper
            // serializes its OS-abandoned list and keeps backing stable.
            unsafe { os_abandoned::push_front(HeapPtr::new_unchecked(heap), page_ptr) };
        }
        // SAFETY: current_theap remains the live current-thread stats owner and
        // exclusively updates its local abandoned-page count.
        unsafe {
            __mi_stat_increase(
                ptr::addr_of_mut!((*current_theap_raw).stats.pages_abandoned),
                1,
            )
        };
    }
    // SAFETY: current_theap_raw is the same live current-thread local heap
    // consumed at entry and remains valid through synchronous publication.
    let current_theap = unsafe {
        crate::types::ThreadHeapLocal::new_unchecked(crate::types::ThreadHeapPtr::new_unchecked(
            current_theap_raw,
        ))
    };
    // SAFETY: active claimed page ownership is carried through all bitmap,
    // list, and stats publication; competing mutation is excluded and
    // current_theap remains live for release.
    unsafe { mi_abandoned_page_unown(page, current_theap) }
}

/// Remove a claimed abandoned page from mapped/OS abandoned publication and
/// reconcile its abandoned-page accounting.
///
/// # Safety
/// `page` must carry active claimed ownership for live initialized abandoned
/// page metadata, with competing mutation excluded and stable heap, MemId,
/// geometry, block size, and arena membership.
/// If supplied, `current_theap` must be the live current-thread local stats owner
/// associated with the page; otherwise the heap's atomic stats remain live.
/// The caller excludes concurrent reclaim, list relink, or teardown while
/// protocol remote frees may continue through the page's atomic remote head.
/// This operation does not release the thread-free claim; the borrowed witness
/// remains the active ownership capability after publication removal.
pub(crate) unsafe fn _mi_arenas_page_unabandon_internal(
    page: &crate::types::OwnedAbandonedPage,
    current_theap: Option<crate::types::ThreadHeapLocal>,
) {
    let page_ptr = page.page();
    let page = page_ptr.as_ptr();
    let current_theapx = current_theap
        .map(|theap| theap.ptr().as_ptr())
        .unwrap_or(ptr::null_mut());

    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: the owned witness keeps initialized page geometry live and
        // stable; its slice start is readable for the alignment invariant.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    // SAFETY: stable page geometry provides a valid start in the page-map
    // domain and must resolve to this exact live metadata pointer.
    crate::internal::mi_assert_internal!(
        unsafe { crate::internal::_mi_ptr_page(mi_page_start(page).cast()) } == page
    );
    // SAFETY: OwnedAbandonedPage proves the live atomic owner claim remains set.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    // SAFETY: the witness and caller contract keep abandoned owner state stable
    // until list/bitmap/accounting removal completes.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(
        current_theapx.is_null()
            // SAFETY: a supplied current_theap and TLD are live initialized
            // current-thread metadata for this synchronous transition.
            || unsafe { _mi_thread_id() == (*(*current_theapx).tld).thread_id }
    );

    // SAFETY: claimed live page metadata retains its initialized owning heap
    // pointer until abandoned publication and stats are removed.
    let heap = unsafe { mi_page_heap(page) };
    // SAFETY: the caller stabilizes the claimed page's initialized MemId for
    // choosing the mapped or non-arena removal path.
    let is_arena = unsafe { mi_page_memid_is_arena(page) };
    // SAFETY: active claimed ownership stabilizes the atomic owner word while
    // competing mutation is excluded for the mapped-abandoned state check.
    if unsafe { mi_page_is_abandoned_mapped(page) } {
        crate::internal::mi_assert_internal!(is_arena);
        // SAFETY: the claimed page's initialized block size remains stable for
        // the matching per-bin abandoned counter and bitmap selection.
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        // SAFETY: claimed page ownership and stable MemId/heap keep its arena
        // range, start bit, and owning table live for removal.
        let Some(membership) = (unsafe { PageArenaMembership::from_page(page_ptr) }) else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        let arena = membership.arena().as_ptr();
        let range = membership.range();
        let slice_index = range.start_usize();
        let slice_count = range.count_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: validated membership indexes the live arena free bbitmap,
        // which must still mark the owned page's slices unavailable.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY: claimed ownership stabilizes the initialized committed-byte
        // snapshot for comparison with arena commitment publication.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                // SAFETY: the validated live committed bitmap covers this exact
                // membership range and remains readable during removal.
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY: validated membership selects the live per-bin abandoned
        // bitmap and in-range start bit, which this owned transition clears once.
        unsafe {
            mi_bitmap_clear_once_set((*arena_pages).pages_abandoned[bin.as_usize()], slice_index)
        };
        // SAFETY: the claimed live page's aligned atomic owner field remains
        // stable; clearing mapped state preserves its abandoned ownership.
        unsafe { mi_page_clear_abandoned_mapped(page_ptr) };
        // SAFETY: the live heap and checked bin select the matching atomic
        // mapped-abandoned counter decremented after bitmap removal.
        unsafe { mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1) };
    } else if !is_arena && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned) {
        // SAFETY: `heap` owns this live claimed non-arena page; the helper
        // serializes removal from its OS-abandoned list before claim release.
        unsafe { os_abandoned::remove(HeapPtr::new_unchecked(heap), page_ptr) };
    }

    if !current_theapx.is_null() {
        // SAFETY: current_theapx is the live current-thread stats owner and this
        // transition exclusively updates its non-atomic abandoned count.
        unsafe {
            __mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.pages_abandoned),
                1,
            )
        };
    } else {
        // SAFETY: without a current local owner, the live heap's aligned shared
        // abandoned counter is updated only through the MT atomic helper.
        unsafe { __mi_stat_decrease_mt(ptr::addr_of_mut!((*heap).stats.pages_abandoned), 1) };
    }
}
