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
    theap: *mut mi_theap_t,
}

unsafe fn mi_heap_visit_page(page: PagePtr, vinfo: *mut mi_heap_visit_info_t) -> bool {
    let page = page.as_ptr();
    let mut area = mi_heap_area_t::default();
    // SAFETY[AUDIT:arena-heap-visit-callback-boundary]: `PagePtr` proves `page` is live
    // page metadata, and `vinfo` is the callback state owned by the visitor.
    unsafe { _mi_heap_area_init(ptr::addr_of_mut!(area), page) };
    crate::internal::mi_assert_internal!(unsafe { (*vinfo).heap.as_ptr() == mi_page_heap(page) });
    if !unsafe {
        (*vinfo).visitor.invoke(
            (*vinfo).heap,
            ptr::addr_of!(area),
            ptr::null_mut(),
            area.block_size,
            (*vinfo).arg,
        )
    } {
        return false;
    }
    // SAFETY[AUDIT:arena-heap-visit-callback-boundary]: the page area was initialized
    // above and the visitor callback is the caller-provided C ABI callback.
    if unsafe { (*vinfo).visit_blocks } {
        unsafe {
            _mi_theap_area_visit_blocks(ptr::addr_of!(area), page, (*vinfo).visitor, (*vinfo).arg)
        }
    } else {
        true
    }
}

unsafe fn mi_heap_visit_page_at_index(
    slice_index: SliceIndex,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let vinfo = arg.cast::<mi_heap_visit_info_t>();
    // SAFETY[AUDIT:arena-heap-visit-callback-boundary]: bitmap iteration supplies an
    // arena-owned slice index and the raw callback argument is visit state.
    let page = unsafe { mi_arena_page_at_slice(ArenaPtr::new_unchecked(arena), slice_index) };
    unsafe { mi_heap_visit_page(page, vinfo) }
}

// SAFETY[AUDIT:arena-heap-visit-callback-boundary]: this C ABI bitmap callback converts
// the raw slice index once before dispatching to the typed page visitor.
unsafe extern "C" fn mi_heap_visit_page_at(
    slice_index: usize,
    _slice_count: usize,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    // SAFETY[AUDIT:arena-heap-visit-callback-boundary]: `SliceIndex` preserves the
    // upstream callback index contract before page lookup.
    unsafe { mi_heap_visit_page_at_index(SliceIndex::new(slice_index), arena, arg) }
}

/// # Safety
/// Visits pages and optionally blocks for a heap using the C ABI callback contract.
// SAFETY[AUDIT:arena-heap-block-visit-boundary]: this internal C ABI shim accepts the
// upstream nullable heap convention and immediately converts non-null heaps.
pub(crate) unsafe extern "C" fn _mi_heap_visit_blocks(
    mut heap: *mut mi_heap_t,
    abandoned_only: bool,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    if heap.is_null() {
        // SAFETY[AUDIT:arena-heap-block-visit-boundary]: null heap follows the C ABI
        // convention for selecting the process main heap.
        heap = unsafe { mi_heap_main() };
    }

    let Some(heap_ptr) = HeapPtr::new(heap) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY[AUDIT:arena-heap-block-visit-boundary]: `HeapPtr::new` proved non-null
    // heap metadata before constructing the borrowed heap witness.
    unsafe {
        _mi_heap_visit_blocks_internal(
            BorrowedHeap::new_unchecked(heap_ptr),
            abandoned_only,
            visit_blocks,
            visitor,
            arg,
        )
    }
}

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
    // SAFETY[AUDIT:arena-heap-page-iteration-boundary]: `borrowed_heap` is a typed heap
    // witness; arena bitmap callbacks keep the visit state stack-local.
    unsafe {
        mi_forall_arenas(
            borrowed_heap,
            None,
            crate::core_types::ThreadSeq::new(0),
            |arena| {
                let arena_pages = mi_heap_arena_pages(borrowed_heap, arena);
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
                            if mi_heap_abandoned_count(heap, typed_bin).load_relaxed() > 0 {
                                ok = mi_bitmap_forall_set_typed(
                                    (*arena_pages).pages_abandoned[typed_bin.as_usize()],
                                    mi_heap_visit_page_at,
                                    Some(arena),
                                    ptr::addr_of_mut!(visit_info).cast(),
                                );
                            }
                        }
                    } else {
                        ok = mi_bitmap_forall_set_typed(
                            (*arena_pages).pages,
                            mi_heap_visit_page_at,
                            Some(arena),
                            ptr::addr_of_mut!(visit_info).cast(),
                        );
                    }
                }
                ok
            },
        );
    }
    if !ok {
        return false;
    }

    // SAFETY[AUDIT:arena-heap-abandoned-visit-boundary]: the helper snapshots the
    // abandoned-page list head while holding the heap abandoned-page lock.
    let page = unsafe { super::os_abandoned::first_snapshot(HeapPtr::new_unchecked(heap)) };
    let mut page = page;
    while let Some(current) = page.filter(|_| ok) {
        // SAFETY[AUDIT:arena-heap-abandoned-visit-boundary]: abandoned pages are linked
        // through allocator-owned page metadata during this traversal.
        let next = unsafe { super::os_abandoned::page_next(current) };
        ok = unsafe { mi_heap_visit_page(current, ptr::addr_of_mut!(visit_info)) };
        page = next;
    }

    ok
}

/// # Safety
/// Public C API wrapper over `_mi_heap_visit_blocks`.
// SAFETY[AUDIT:arena-heap-block-visit-boundary]: exported C ABI wrapper preserves the
// public heap visit symbol while narrowing non-null heaps to typed witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_visit_blocks(
    heap: *mut mi_heap_t,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    let heap = if heap.is_null() {
        // SAFETY[AUDIT:arena-heap-block-visit-boundary]: null heap follows the public
        // C ABI convention for selecting the process main heap.
        unsafe { mi_heap_main() }
    } else {
        heap
    };
    let Some(heap_ptr) = HeapPtr::new(heap) else {
        return false;
    };
    // SAFETY[AUDIT:arena-heap-block-visit-boundary]: non-null heap metadata is wrapped
    // before internal traversal uses arena bitmaps and callbacks.
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
/// Public C API wrapper over `_mi_heap_visit_blocks` for abandoned pages only.
// SAFETY[AUDIT:arena-heap-abandoned-visit-boundary]: exported C ABI wrapper preserves the
// abandoned-page visit symbol while narrowing non-null heaps to typed witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_visit_abandoned_blocks(
    heap: *mut mi_heap_t,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    let heap = if heap.is_null() {
        // SAFETY[AUDIT:arena-heap-abandoned-visit-boundary]: null heap follows the public
        // C ABI convention for selecting the process main heap.
        unsafe { mi_heap_main() }
    } else {
        heap
    };
    let Some(heap_ptr) = HeapPtr::new(heap) else {
        return false;
    };
    // SAFETY[AUDIT:arena-heap-abandoned-visit-boundary]: non-null heap metadata is wrapped
    // before internal traversal uses arena bitmaps and callbacks.
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

// SAFETY[AUDIT:arena-heap-delete-boundary]: the visit callback receives heap,
// area, and argument pointers from `_mi_heap_visit_blocks` while deleting pages.
unsafe extern "C" fn mi_heap_delete_page(
    heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    _block: *mut c_void,
    _block_size: usize,
    arg: *mut c_void,
) -> bool {
    let info = arg.cast::<mi_heap_delete_visit_info_t>();
    // SAFETY[AUDIT:arena-heap-delete-boundary]: delete traversal passes live
    // stack-local delete state and an area whose reserved slot stores the page.
    let mut heap_target = unsafe { (*info).heap_target };
    let theap = ptr::null_mut::<mi_theap_t>();
    let page = unsafe { (*area).reserved1.cast::<mi_page_t>() };

    // SAFETY[AUDIT:arena-heap-delete-boundary]: the area reserved page belongs to
    // the heap traversal and is claimed before ownership-sensitive mutation.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    let abandoned = unsafe { crate::core_types::AbandonedPage::new_unchecked(page_ptr) };
    let _owned = unsafe { mi_page_claim_ownership(abandoned) };
    if unsafe { mi_page_is_abandoned(page) } {
        // SAFETY[AUDIT:arena-heap-delete-boundary]: owned abandoned pages can be
        // unlinked from arena abandoned state before deletion or transfer.
        unsafe { _mi_arenas_page_unabandon(page, theap) };
    } else {
        // SAFETY[AUDIT:arena-heap-delete-boundary]: owned non-abandoned pages can
        // be detached from heap page links before deletion or transfer.
        unsafe {
            (*page).next = ptr::null_mut();
            (*page).prev = ptr::null_mut();
            mi_page_set_theap(page, ptr::null_mut());
        }
    }
    // SAFETY[AUDIT:arena-heap-delete-boundary]: ownership was claimed above and
    // the page should now be in abandoned-owned state for the following cases.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    // SAFETY[AUDIT:arena-heap-delete-boundary]: owned empty pages can be released
    // directly; non-empty destroy paths first reset used-count metadata.
    if unsafe { crate::page::mi_page_used_count(page).get() == 0 } {
        unsafe { _mi_arenas_page_free(page, theap) };
    } else if heap_target.is_null() {
        // SAFETY[AUDIT:arena-heap-delete-boundary]: destroying a non-empty page
        // clears its used count before returning the owned page to the arena.
        unsafe {
            crate::page::mi_page_set_used_count(page, crate::core_types::UsedBlockCount::new(0))
        };
        unsafe { _mi_arenas_page_free(page, theap) };
    } else {
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: transfer uses the page's arena
        // location and typed page witness to update source/target heap tables.
        let sbin = unsafe { _mi_page_stats_bin(page) };
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: the page witness is
        // allocator-owned and the out-pointers are stack-local transfer state.
        let arena_id = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: `mi_page_arena_pages` filled
        // arena table outputs for the source heap before bitmap mutation.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_set((*arena_pages).pages, slice_index)
        });
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        if !theap.is_null() {
            // SAFETY[AUDIT:arena-heap-transfer-boundary]: page-bin stats are adjusted
            // on the owning thread heap before the page moves.
            unsafe {
                __mi_stat_decrease(ptr::addr_of_mut!((*theap).stats.page_bins[sbin]), 1);
                __mi_stat_decrease(ptr::addr_of_mut!((*theap).stats.pages), 1);
            }
        } else {
            let heap = heap as *mut mi_heap_t;
            // SAFETY[AUDIT:arena-heap-transfer-boundary]: cross-thread heap stats use
            // the existing atomic/stat helpers for the source heap.
            unsafe {
                __mi_stat_decrease_mt(&mut (*heap).stats.page_bins[sbin], 1);
                __mi_stat_decrease_mt(&mut (*heap).stats.pages, 1);
            }
        }
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: delete state owns the target
        // thread heap pointer selected before traversal.
        let mut theap_target = unsafe { (*info).theap_target };

        // SAFETY[AUDIT:arena-heap-transfer-boundary]: target heap arena-page tables
        // are created or resolved for the same arena id before transfer.
        let mut arena_pages_target =
            unsafe { mi_heap_ensure_arena_pages_from_raw(heap_target, arena_id) };
        if arena_pages_target.is_null() {
            // SAFETY[AUDIT:arena-heap-transfer-boundary]: fallback to the main heap
            // preserves transfer when the requested target lacks arena tables.
            heap_target = unsafe { mi_heap_main() };
            theap_target = unsafe { mi_heap_theap(heap_target) };
            arena_pages_target =
                unsafe { mi_heap_ensure_arena_pages_from_raw(heap_target, arena_id) };
            crate::internal::mi_assert_internal!(!arena_pages_target.is_null());
        }
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: source bitmap cleared this
        // slice, and the target bitmap must be empty before publishing it.
        crate::internal::mi_assert_internal!(unsafe {
            !mi_bitmap_is_set((*arena_pages_target).pages, slice_index)
        });
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: target table publication
        // happens before the page heap pointer is reassigned.
        unsafe { mi_bitmap_set((*arena_pages_target).pages, slice_index) };
        unsafe { (*page).heap = heap_target };
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: target heap stats and abandon
        // state are updated after the page's heap pointer is reassigned.
        unsafe {
            __mi_stat_increase(ptr::addr_of_mut!((*theap_target).stats.page_bins[sbin]), 1);
            __mi_stat_increase(ptr::addr_of_mut!((*theap_target).stats.pages), 1);
        }
        // SAFETY[AUDIT:arena-heap-transfer-boundary]: the moved page is now tracked
        // as abandoned by the target thread heap.
        unsafe { _mi_arenas_page_abandon(page, theap_target) };
    }
    true
}

unsafe fn mi_heap_delete_pages(heap: OwnedHeap, heap_target: *mut mi_heap_t) {
    // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: `OwnedHeap` can be borrowed for
    // traversal while this helper drains pages from that heap.
    let borrowed_heap = unsafe { BorrowedHeap::new_unchecked(heap.ptr()) };
    let heap = heap.ptr().as_ptr();
    let theap_target = if heap_target.is_null() {
        ptr::null_mut()
    } else {
        // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: non-null target heaps use the
        // allocator-owned thread-heap prefix for target stats updates.
        unsafe { mi_heap_theap(heap_target) }
    };
    let mut info = mi_heap_delete_visit_info_t {
        heap_target,
        theap_target,
        theap: ptr::null_mut(),
    };
    // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: page deletion is implemented as a
    // heap visit with stack-local delete state and the delete-page callback.
    let _ = unsafe {
        _mi_heap_visit_blocks(
            heap,
            false,
            false,
            mi_heap_delete_page,
            ptr::addr_of_mut!(info).cast(),
        )
    };
    if MI_DEBUG > 1 {
        for i in 0..MI_ARENA_BIN_COUNT {
            // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: debug-only verification
            // checks every arena-page table attached to the drained heap.
            let arena_idx = unsafe { ArenaIndex::new_unchecked(i as u16) };
            let arena_pages =
                unsafe { mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire() };
            if !arena_pages.is_null() {
                // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: debug-only bitmap
                // verification reads arena-page metadata still owned by heap.
                crate::internal::mi_assert_internal!(unsafe {
                    mi_bitmap_is_all_clear((*arena_pages).pages)
                });
            }
        }
        // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: the abandoned-page lock is
        // held while checking that the drained heap has no abandoned pages.
        crate::internal::mi_assert_internal!(unsafe {
            super::os_abandoned::is_empty(HeapPtr::new_unchecked(heap))
        });
        for i in 0..MI_BIN_COUNT {
            let Some(bin) = crate::core_types::Bin::new(i as u16) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]: typed bin construction
            // bounds the debug-only abandoned count lookup.
            crate::internal::mi_assert_internal!(unsafe {
                mi_heap_abandoned_count(heap, bin).load_relaxed() == 0
            });
        }
    }
}

/// # Safety
/// Moves all pages from one heap to another following the original internal ABI.
// SAFETY[AUDIT:arena-heap-move-boundary]: internal C ABI shim converts raw heap
// handles to typed source/target heap witnesses before moving pages.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_move_pages(
    heap_from: *mut mi_heap_t,
    mut heap_to: *mut mi_heap_t,
) {
    let Some(heap_from) = HeapPtr::new(heap_from) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:arena-heap-move-boundary]: the source heap must be uniquely
    // owned while pages are drained from it.
    let Some(heap_from) = (unsafe { OwnedHeap::new(heap_from) }) else {
        return;
    };
    if heap_to.is_null() {
        // SAFETY[AUDIT:arena-heap-move-boundary]: null target follows the
        // internal ABI convention for moving pages to the main heap.
        heap_to = unsafe { mi_heap_main() };
    }
    let Some(heap_to) = HeapPtr::new(heap_to) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:arena-heap-move-boundary]: typed source ownership and target
    // borrow witnesses bound the internal page transfer.
    unsafe {
        _mi_heap_move_pages_internal(heap_from, BorrowedHeap::new_unchecked(heap_to));
    }
}

unsafe fn _mi_heap_move_pages_internal(heap_from: OwnedHeap, heap_to: BorrowedHeap) {
    // SAFETY[AUDIT:arena-heap-move-boundary]: page deletion handles transfer
    // into a non-null target heap when a target pointer is supplied.
    unsafe { mi_heap_delete_pages(heap_from, heap_to.ptr().as_ptr()) };
}

/// # Safety
/// Destroys all pages owned by a non-main heap following the original internal ABI.
// SAFETY[AUDIT:arena-heap-destroy-boundary]: internal C ABI shim rejects the main
// heap and converts the source heap to an owned witness before destruction.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_destroy_pages(heap_from: *mut mi_heap_t) {
    // SAFETY[AUDIT:arena-heap-destroy-boundary]: main heap pages are never drained
    // through this destroy helper.
    if unsafe { _mi_is_heap_main(heap_from) } {
        return;
    }
    let Some(heap_from) = HeapPtr::new(heap_from) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:arena-heap-destroy-boundary]: non-main source heap ownership is
    // required before destructive page draining.
    let Some(heap_from) = (unsafe { OwnedHeap::new(heap_from) }) else {
        return;
    };
    // SAFETY[AUDIT:arena-heap-destroy-boundary]: owned non-main heap can be drained
    // into the free path with no target heap.
    unsafe { _mi_heap_destroy_pages_internal(heap_from) };
}

unsafe fn _mi_heap_destroy_pages_internal(heap_from: OwnedHeap) {
    // SAFETY[AUDIT:arena-heap-destroy-boundary]: null target selects destruction
    // instead of page transfer.
    unsafe { mi_heap_delete_pages(heap_from, ptr::null_mut()) };
}
