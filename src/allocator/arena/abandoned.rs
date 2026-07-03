//! Abandoned-page ownership and per-heap arena page-table helpers.
//!
//! This module owns the small unsafe surface where abandoned pages are claimed
//! back into a thread heap and where the heap-local arena page table is lazily
//! published for those ownership checks.

use super::*;

pub(super) unsafe fn mi_abandoned_page_unown(
    page: crate::core_types::AbandonedPage,
    current_theap: crate::core_types::ThreadHeapLocal,
) -> bool {
    let page_ptr = page.page();
    let page = page_ptr.as_ptr();
    let current_theap = current_theap.ptr().as_ptr();
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe {
        _mi_thread_id() == (*(*current_theap).tld).thread_id
    });
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    let owned_page = unsafe { crate::core_types::OwnedAbandonedPage::new_unchecked(page_ptr) };

    let atomic_tf = crate::core_types::AtomicThreadFree::from_field(
        crate::core_types::AtomicUsizeField::from_raw(unsafe {
            ptr::addr_of_mut!((*page).xthread_free)
        }),
    );
    let mut tf_old = atomic_tf.load_relaxed();
    loop {
        crate::internal::mi_assert_internal!(tf_old.is_owned());
        while tf_old.head().is_some() {
            // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
            let local_free = unsafe { page_ptr.local_free() };
            let local_free = unsafe {
                atomic_tf
                    .collect_acq_rel()
                    .collect_into_local(owned_page, local_free)
            };
            // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
            if unsafe { page_ptr.free().is_empty() } && !local_free.is_empty() {
                unsafe {
                    page_ptr.set_free(local_free.into_available());
                    page_ptr.set_local_free(crate::core_types::LocalFreeList::empty());
                }
            }
            // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
            if unsafe { mi_page_all_free(page) } {
                unsafe { _mi_arenas_page_unabandon(page, current_theap) };
                unsafe { _mi_arenas_page_free(page, current_theap) };
                return true;
            }
            tf_old = atomic_tf.load_relaxed();
        }
        crate::internal::mi_assert_internal!(tf_old.head().is_none());
        // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
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
        // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let page = {
        let index = slice_index;
        let page_addr: usize;
        // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
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
        // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
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
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    let abandoned = unsafe { crate::core_types::AbandonedPage::new_unchecked(page_ptr) };
    if unsafe { mi_page_claim_ownership(abandoned) }.is_none() {
        unsafe { *keep_abandoned = true };
        false
    } else {
        // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
        unsafe { *keep_abandoned = false };
        true
    }
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
}

pub(super) unsafe extern "C" fn mi_arena_try_claim_abandoned(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_abandoned: *mut bool,
) -> bool {
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    unsafe { mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned) }
}

pub(super) unsafe fn mi_heap_ensure_arena_pages_from_raw(
    heap: *mut mi_heap_t,
    arena: *mut mi_arena_t,
) -> *mut mi_arena_pages_t {
    crate::internal::mi_assert_internal!(!arena.is_null());
    crate::internal::mi_assert_internal!(!heap.is_null());
    // SAFETY[AUDIT:arena-abandoned-page-table-boundary]: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
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

    // SAFETY[AUDIT:arena-abandoned-page-table-boundary]: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
    let mut arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
    if arena_pages.is_null() {
        unsafe { mi_lock_acquire(heap.ptr().arena_pages_lock()) };
        arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
        if arena_pages.is_null() {
            // SAFETY[AUDIT:arena-abandoned-page-table-boundary]: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
            if unsafe { _mi_is_heap_main(heap_raw) } {
                arena_pages =
                    unsafe { ptr::addr_of_mut!((*arena_raw.cast::<MiArenaPrefix>()).pages_main) };
            } else {
                let slice_count = unsafe { arena.slice_count() };
                let Ok(range) =
                    crate::core_types::SliceRange::new(0, slice_count.get() as usize, arena)
                else {
                    crate::internal::mi_assert_internal!(false);
                    // SAFETY[AUDIT:arena-abandoned-page-table-boundary]: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
                    unsafe { mi_lock_release(heap.ptr().arena_pages_lock()) };
                    return ptr::null_mut();
                };
                let (alloc_size, bitmap_base) = unsafe { mi_arena_pages_size(slice_count) };
                arena_pages =
                    // SAFETY[AUDIT:arena-abandoned-page-table-boundary]: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
                    unsafe { mi_arena_pages_alloc(arena, range, alloc_size, bitmap_base) };
            }
            unsafe { mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast()) };
        }
        unsafe { mi_lock_release(heap.ptr().arena_pages_lock()) };
    }
    // SAFETY[AUDIT:arena-abandoned-page-table-boundary]: abandoned-page lookup maintains per-heap arena-page tables before ownership transfer.
    if unsafe { _mi_is_heap_main(heap_raw) } {
        crate::internal::mi_assert!(!arena_pages.is_null());
    }
    arena_pages
}

#[inline]
pub(super) unsafe fn mi_page_is_full_local(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:arena-page-ownership-transition-boundary]: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    let full = unsafe {
        crate::page::mi_page_reserved_count(page).get()
            == crate::page::mi_page_used_count(page).get()
    };
    crate::internal::mi_assert_internal!(!full || unsafe { (*page).free.is_null() });
    full
}
