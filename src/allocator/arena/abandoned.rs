//! Abandoned-page ownership and per-heap arena page-table helpers.
//!
//! This module owns the small unsafe surface where abandoned pages are claimed
//! back into a thread heap and where the heap-local arena page table is lazily
//! published for those ownership checks.

use super::metadata::{mi_arena_pages_alloc, mi_arena_pages_size};
use super::*;

pub(super) unsafe fn mi_abandoned_page_unown(
    page: crate::types::AbandonedPage,
    current_theap: crate::types::ThreadHeapLocal,
) -> bool {
    let page_ptr = page.page();
    let page = page_ptr.as_ptr();
    let current_theap = current_theap.ptr().as_ptr();
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe {
        _mi_thread_id() == (*(*current_theap).tld).thread_id
    });
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    let owned_page = unsafe { crate::types::assume_owned_abandoned_page(page_ptr) };

    let atomic_tf = unsafe { page_ptr.thread_free_cell() };
    let mut tf_old = atomic_tf.load_relaxed();
    loop {
        crate::internal::mi_assert_internal!(tf_old.is_owned());
        while tf_old.head().is_some() {
            // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
            let local_free = unsafe { page_ptr.local_free() };
            let detached = unsafe { atomic_tf.detach_head() };
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
            // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
            if unsafe { page_ptr.free().is_empty() } && !local_free.is_empty() {
                // SAFETY: `owned_page` retains the claimed abandoned owner and
                // `local_free` was read from that same page immediately above.
                unsafe { owned_page.transfer_local_to_available(local_free) };
            }
            // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
            if unsafe { mi_page_all_free(page) } {
                unsafe { _mi_arenas_page_unabandon(page, current_theap) };
                unsafe { _mi_arenas_page_free(page, current_theap) };
                return true;
            }
            tf_old = atomic_tf.load_relaxed();
        }
        crate::internal::mi_assert_internal!(tf_old.head().is_none());
        // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
        if unsafe { atomic_tf.release_acq_rel(&mut tf_old) } {
            return false;
        }
    }
}

pub(super) unsafe fn mi_arena_try_claim_abandoned_at(
    slice_index: SliceIndex,
    arena: *mut mi_arena_t,
    keep_abandoned: *mut bool,
) -> bool {
    let slice_index = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let page = {
        let index = slice_index;
        let page_addr: usize;
        // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
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
        // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
        let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
        if !pages_meta.is_null() {
            unsafe { pages_meta.add(slice_index) }
        } else {
            unsafe {
                arena
                    .cast::<u8>()
                    .add(slice_index * MI_ARENA_SLICE_SIZE)
                    .cast::<mi_page_t>()
            }
        }
    };
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    let abandoned = unsafe { crate::types::assume_abandoned_page(page_ptr) };
    if unsafe { mi_page_claim_ownership(abandoned) }.is_none() {
        unsafe { *keep_abandoned = true };
        false
    } else {
        // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
        unsafe { *keep_abandoned = false };
        true
    }
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
}

pub(super) unsafe extern "C" fn mi_arena_try_claim_abandoned(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_abandoned: *mut bool,
) -> bool {
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    unsafe { mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned) }
}

pub(super) unsafe fn mi_heap_ensure_arena_pages_from_raw(
    heap: *mut mi_heap_t,
    arena: *mut mi_arena_t,
) -> *mut mi_arena_pages_t {
    crate::internal::mi_assert_internal!(!arena.is_null());
    crate::internal::mi_assert_internal!(!heap.is_null());
    // SAFETY: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
    let arena_idx = unsafe { mi_arena_index(arena) };
    unsafe {
        mi_heap_ensure_arena_pages(
            BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)),
            ArenaPtr::new_unchecked(arena),
            arena_idx,
        )
    }
}

unsafe fn mi_heap_ensure_arena_pages(
    heap: BorrowedHeap,
    arena: ArenaPtr,
    arena_idx: ArenaIndex,
) -> *mut mi_arena_pages_t {
    let heap_raw = heap.ptr().as_ptr();
    let arena_raw = arena.as_ptr();

    // SAFETY: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
    let mut arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
    if arena_pages.is_null() {
        unsafe { mi_lock_acquire(heap.ptr().arena_pages_lock()) };
        arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
        if arena_pages.is_null() {
            // SAFETY: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
            if unsafe { _mi_is_heap_main(heap_raw) } {
                arena_pages =
                    unsafe { ptr::addr_of_mut!((*arena_raw.cast::<MiArenaPrefix>()).pages_main) };
            } else {
                let slice_count = unsafe { arena.slice_count() };
                let Ok(range) = crate::types::SliceRange::new(0, slice_count.get() as usize, arena)
                else {
                    crate::internal::mi_assert_internal!(false);
                    // SAFETY: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
                    unsafe { mi_lock_release(heap.ptr().arena_pages_lock()) };
                    return ptr::null_mut();
                };
                let (alloc_size, bitmap_base) = unsafe { mi_arena_pages_size(slice_count) };
                arena_pages =
                    // SAFETY: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
                    unsafe { mi_arena_pages_alloc(arena, range, alloc_size, bitmap_base) };
            }
            unsafe { mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast()) };
        }
        unsafe { mi_lock_release(heap.ptr().arena_pages_lock()) };
    }
    // SAFETY: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
    if unsafe { _mi_is_heap_main(heap_raw) } {
        crate::internal::mi_assert!(!arena_pages.is_null());
    }
    arena_pages
}

#[inline]
pub(super) unsafe fn mi_page_is_full_local(page: *const mi_page_t) -> bool {
    // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    let full = unsafe {
        crate::allocator::page::mi_page_reserved_count(page).get()
            == crate::allocator::page::mi_page_used_count(page).get()
    };
    // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    let free_is_empty = unsafe {
        crate::types::PagePtr::new_unchecked(page.cast_mut())
            .free()
            .is_empty()
    };
    crate::internal::mi_assert_internal!(!full || free_is_empty);
    full
}

pub(super) unsafe fn _mi_arenas_page_abandon_internal(
    page_wrapper: crate::types::PageForAbandon,
    current_theap: crate::types::ThreadHeapLocal,
) {
    let page = match page_wrapper {
        crate::types::PageForAbandon::Active(page) => page.page().as_ptr(),
        crate::types::PageForAbandon::Full(page) => page.page().as_ptr(),
    };
    let current_theap = current_theap.ptr().as_ptr();
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_all_free(page) });
    // SAFETY: `page` came from the typed
    // page-for-abandon wrapper above.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    crate::internal::mi_assert_internal!(unsafe {
        page_ptr.queue_next().is_none() && page_ptr.queue_prev().is_none()
    });

    // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    let heap = unsafe { mi_page_heap(page) };
    crate::internal::mi_assert_internal!(heap == unsafe { _mi_theap_heap(current_theap) });
    if unsafe { mi_page_memid_is_arena(page) } && !unsafe { mi_page_is_full_local(page) } {
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        let Some(membership) =
            (unsafe { PageArenaMembership::from_page(PagePtr::new_unchecked(page)) })
        else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        let arena = membership.arena().as_ptr();
        let range = membership.range();
        let slice_index = range.start_usize();
        let slice_count = range.count_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        crate::internal::mi_assert_internal!(!unsafe { mi_page_is_singleton(page) });
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index,
                slice_count,
            )
        });

        let atomic_tid =
            // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
            unsafe { PagePtr::new_unchecked(page).thread_id_cell() };
        unsafe { atomic_tid.store_owner(crate::types::PageOwnerId::AbandonedMapped) };
        let was_clear =
            // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
            unsafe { mi_bitmap_set((*arena_pages).pages_abandoned[bin.as_usize()], slice_index) };
        crate::internal::MI_UNUSED(&was_clear);
        crate::internal::mi_assert_internal!(was_clear);
        unsafe { mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1) };
        unsafe { __mi_stat_increase(ptr::addr_of_mut!((*current_theap).stats.pages_abandoned), 1) };
    } else {
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        if !unsafe { mi_page_memid_is_arena(page) }
            && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned)
        {
            // SAFETY: arena page abandon links non-arena pages into the owning heap OS-abandoned list under its lock.
            unsafe {
                os_abandoned::push_front(HeapPtr::new_unchecked(heap), PagePtr::new_unchecked(page))
            };
        }
        // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        unsafe { __mi_stat_increase(ptr::addr_of_mut!((*current_theap).stats.pages_abandoned), 1) };
    }
    unsafe {
        mi_abandoned_page_unown(
            crate::types::assume_abandoned_page(PagePtr::new_unchecked(page)),
            crate::types::ThreadHeapLocal::new_unchecked(
                crate::types::ThreadHeapPtr::new_unchecked(current_theap),
            ),
        )
    };
}

pub(super) unsafe fn _mi_arenas_page_unabandon_internal(
    page: crate::types::OwnedAbandonedPage,
    current_theap: Option<crate::types::ThreadHeapLocal>,
) {
    let page = page.page().as_ptr();
    let current_theapx = current_theap
        .map(|theap| theap.ptr().as_ptr())
        .unwrap_or(ptr::null_mut());

    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: unabandon entry validates raw page and thread-heap witnesses before cleanup.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY: unabandon entry validates raw page and thread-heap witnesses before cleanup.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(
        current_theapx.is_null()
            || unsafe { _mi_thread_id() == (*(*current_theapx).tld).thread_id }
    );

    // SAFETY: unabandon entry validates raw page and thread-heap witnesses before cleanup.
    let heap = unsafe { mi_page_heap(page) };
    if unsafe { mi_page_is_abandoned_mapped(page) } {
        crate::internal::mi_assert_internal!(unsafe { mi_page_memid_is_arena(page) });
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        // SAFETY: mapped unabandon clears arena abandoned bitmaps and heap counters.
        let Some(membership) =
            (unsafe { PageArenaMembership::from_page(PagePtr::new_unchecked(page)) })
        else {
            crate::internal::mi_assert_internal!(false);
            return;
        };
        let arena = membership.arena().as_ptr();
        let range = membership.range();
        let slice_index = range.start_usize();
        let slice_count = range.count_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: mapped unabandon clears arena abandoned bitmaps and heap counters.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY: mapped unabandon clears arena abandoned bitmaps and heap counters.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY: mapped unabandon clears arena abandoned bitmaps and heap counters.
        unsafe {
            mi_bitmap_clear_once_set((*arena_pages).pages_abandoned[bin.as_usize()], slice_index)
        };
        unsafe { mi_page_clear_abandoned_mapped(PagePtr::new_unchecked(page)) };
        unsafe { mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1) };
    // SAFETY: mapped unabandon clears arena abandoned bitmaps and heap counters.
    } else if !unsafe { mi_page_memid_is_arena(page) }
        && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned)
    {
        // SAFETY: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
        unsafe { os_abandoned::remove(HeapPtr::new_unchecked(heap), PagePtr::new_unchecked(page)) };
    }

    if !current_theapx.is_null() {
        // SAFETY: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
        unsafe {
            __mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.pages_abandoned),
                1,
            )
        };
    } else {
        // SAFETY: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
        unsafe { __mi_stat_decrease_mt(ptr::addr_of_mut!((*heap).stats.pages_abandoned), 1) };
    }
}
