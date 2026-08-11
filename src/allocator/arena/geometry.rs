//! Arena slice geometry, containment, and heap page-table lookups.
//!
//! These helpers keep raw address projection and arena-page table access in one
//! narrow arena-local module while preserving the existing C ABI exports.

use super::*;

pub(super) unsafe fn mi_arena_numa_node(arena: *const mi_arena_t) -> crate::types::NumaNode {
    crate::types::NumaNode::new(unsafe { (*arena.cast::<MiArenaPrefix>()).numa_node })
}

pub(super) unsafe fn mi_arena_set_numa_node(
    arena: *mut mi_arena_t,
    numa_node: crate::types::NumaNode,
) {
    // SAFETY: arena suitability checks keep pinned, requested-arena, and NUMA predicates separate from table mutation.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).numa_node = numa_node.get();
    }
}

// SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_alignment() -> usize {
    MI_ARENA_SLICE_ALIGN
}

// SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_size() -> usize {
    MI_ARENA_MIN_SIZE
}

pub(super) unsafe fn mi_arena_info_slices(arena: ArenaPtr) -> usize {
    let arena = arena.as_ptr();
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { (*arena.cast::<MiArenaPrefix>()).info_slices }
}

pub(super) unsafe fn mi_arena_range_is_after_info_slices(
    arena: ArenaPtr,
    range: crate::types::SliceRange,
) -> bool {
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    range.start_usize() >= unsafe { mi_arena_info_slices(arena) }
}

pub(super) unsafe fn mi_heap_arena_pages(
    heap: BorrowedHeap,
    arena: ArenaPtr,
) -> *mut mi_arena_pages_t {
    let arena = arena.as_ptr();
    // SAFETY: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
    let arena_idx = unsafe { mi_arena_index(arena) };
    unsafe { mi_heap_arena_page_cell(heap, arena_idx).load_acquire() }.cast::<mi_arena_pages_t>()
}

/// An operation-scoped snapshot of a page's validated arena membership.
///
/// This value does not own or freeze the arena bitmap membership. Its copied
/// components become stale after the owning operation mutates that membership.
pub(super) struct PageArenaMembership {
    arena: ArenaPtr,
    range: crate::types::SliceRange,
    arena_pages: core::ptr::NonNull<mi_arena_pages_t>,
}

impl PageArenaMembership {
    /// # Safety
    /// `page` must identify a live owned page whose heap, arena MemId, and
    /// per-heap arena-page table remain stable while the returned value is used.
    pub(super) unsafe fn from_page(page: PagePtr) -> Option<Self> {
        let page_raw = page.as_ptr();
        // SAFETY: the caller provides live owned page metadata.
        crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page_raw) });
        let crate::types::MemIdView::Arena(arena_mem) =
            crate::types::MemId::view(unsafe { (*page_raw).memid })
        else {
            return None;
        };
        let arena = arena_mem.arena();
        // SAFETY: the page's live owning heap and decoded arena remain stable
        // under the caller's page ownership protocol.
        let arena_pages = unsafe {
            mi_heap_arena_pages(
                BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(mi_page_heap(page_raw))),
                arena,
            )
        };
        let arena_pages = core::ptr::NonNull::new(arena_pages)?;
        let range = arena_mem.range();
        // SAFETY: the table pointer is non-null and its bitmap remains live by
        // contract; the typed range supplies the checked start index.
        if !unsafe { mi_bitmap_is_setN((*arena_pages.as_ptr()).pages, range.start_usize(), 1) } {
            return None;
        }
        Some(Self {
            arena,
            range,
            arena_pages,
        })
    }

    pub(super) fn arena(&self) -> ArenaPtr {
        self.arena
    }

    pub(super) fn range(&self) -> crate::types::SliceRange {
        self.range
    }

    pub(super) fn arena_pages(&self) -> core::ptr::NonNull<mi_arena_pages_t> {
        self.arena_pages
    }
}

pub(super) unsafe fn mi_heap_has_page(heap: BorrowedHeap, arena: ArenaPtr, page: PagePtr) -> bool {
    let borrowed_heap = heap;
    let arena = arena.as_ptr();
    let page = page.as_ptr();
    // SAFETY: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
    let arena_idx = unsafe { mi_arena_index(arena) };
    let arena_pages = unsafe { mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire() }
        .cast::<MiArenaPagesPrefix>();
    match crate::types::MemId::view(unsafe { (*page).memid }) {
        crate::types::MemIdView::Arena(a) => {
            a.arena().as_ptr() == arena
                && !arena_pages.is_null()
                // SAFETY: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
                && unsafe { mi_bitmap_is_setN((*arena_pages).pages, a.range().start_usize(), 1) }
        }
        _ => false,
    }
}

pub(super) unsafe fn mi_arena_size(arena: ArenaPtr) -> crate::types::ReservedBytes {
    let arena = arena.as_ptr();
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    crate::types::ReservedBytes::new(mi_size_of_slices(unsafe {
        (*arena.cast::<MiArenaPrefix>()).slice_count
    }))
}

pub(super) unsafe fn mi_arena_start(arena: ArenaPtr) -> crate::types::SliceStart {
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { crate::types::SliceStart::new_unchecked(arena.as_ptr().cast::<u8>()) }
}

#[inline(never)]
pub(super) unsafe fn mi_arena_slice_start(
    arena: ArenaPtr,
    slice_index: crate::types::SliceIndex,
) -> crate::types::SliceStart {
    let arena_raw = arena.as_ptr();
    let slice_index = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        slice_index < unsafe { (*arena_raw.cast::<MiArenaPrefix>()).slice_count }
    );
    let start = unsafe {
        mi_arena_start(arena)
            .as_ptr()
            .add(mi_size_of_slices(slice_index))
    };
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { crate::types::SliceStart::new_unchecked(start) }
}

#[inline(never)]
pub(super) unsafe fn mi_arena_page_at_slice(
    arena: ArenaPtr,
    slice_index: crate::types::SliceIndex,
) -> PagePtr {
    let arena = arena.as_ptr();
    let slice_index_usize = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        slice_index_usize < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
    if !pages_meta.is_null() {
        return unsafe { PagePtr::new_unchecked(pages_meta.add(slice_index_usize)) };
    }
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let page = unsafe {
        mi_arena_slice_start(ArenaPtr::new_unchecked(arena), slice_index)
            .as_ptr()
            .cast::<mi_page_t>()
    };
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { PagePtr::new_unchecked(page) }
}

/// # Safety
/// `arena_id` must be null or a valid top-level arena id; `size` may be null.
// SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_area(arena_id: mi_arena_id_t, size: *mut usize) -> *mut c_void {
    if !size.is_null() {
        unsafe { *size = 0 };
    }
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        return ptr::null_mut();
    }
    if !size.is_null() {
        crate::internal::mi_assert_internal!(
            // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
            mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count })
                <= unsafe { (*arena.cast::<MiArenaPrefix>()).total_size }
        );
        unsafe { *size = (*arena.cast::<MiArenaPrefix>()).total_size };
    }
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe {
        mi_arena_start(ArenaPtr::new_unchecked(arena))
            .as_ptr()
            .cast::<c_void>()
    }
}

unsafe fn mi_arena_strictly_contains(
    arena: ArenaPtr,
    p: Option<InteriorUserPtr>,
    size: ReservedBytes,
) -> bool {
    let Some(p) = p else {
        return false;
    };
    let p = p.as_u8_ptr().cast_const();
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let start = unsafe { mi_arena_start(arena).as_ptr().cast_const() };
    let end = unsafe { start.add(size.get()) };
    start <= p && end > p
}

unsafe fn mi_arenas_contain_ex(p: Option<InteriorUserPtr>, parent: Option<ArenaPtr>) -> bool {
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let subproc = unsafe { _mi_subproc() };
    let subproc_ptr = unsafe { SubprocPtr::new_unchecked(subproc) };
    let max_arena = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    let parent_raw = parent.map(ArenaPtr::as_ptr);
    for i in 0..max_arena {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
        let Some(arena_ptr) = ArenaPtr::new(arena) else {
            continue;
        };
        if !arena.is_null()
            && (parent_raw.is_none()
                || Some(arena) == parent_raw
                // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
                || unsafe { Some((*arena.cast::<MiArenaPrefix>()).parent) == parent_raw })
            && unsafe { mi_arena_strictly_contains(arena_ptr, p, mi_arena_size(arena_ptr)) }
        {
            return true;
        }
    }
    false
}

/// # Safety
/// `p` may be any pointer value.
// SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_contain(p: *const c_void) -> bool {
    unsafe { mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), None) }
}

/// # Safety
/// `arena_id` must be null or a valid top-level arena id; `p` may be any pointer value.
// SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_contains(arena_id: mi_arena_id_t, p: *const c_void) -> bool {
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        false
    // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    } else if unsafe {
        mi_arena_strictly_contains(
            ArenaPtr::new_unchecked(arena),
            InteriorUserPtr::new(p.cast_mut()),
            mi_arena_size(ArenaPtr::new_unchecked(arena)),
        )
    } {
        true
    } else {
        // SAFETY: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        unsafe {
            mi_arenas_contain_ex(
                InteriorUserPtr::new(p.cast_mut()),
                Some(ArenaPtr::new_unchecked(arena)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{mem::MaybeUninit, ptr, sync::atomic::Ordering};

    #[test]
    fn page_arena_membership_rejects_invalid_states_and_preserves_exact_range() {
        let mut bitmap = MaybeUninit::<mi_bitmap_t>::zeroed();
        let mut arena_pages = MaybeUninit::<mi_arena_pages_t>::zeroed();
        let mut arena = MaybeUninit::<mi_arena_t>::zeroed();
        let mut heap = MaybeUninit::<mi_heap_t>::zeroed();
        let mut page = MaybeUninit::<mi_page_t>::zeroed();

        // SAFETY: all fixtures are stack-local, initialize every field read by
        // membership construction, and remain live for the test.
        unsafe {
            let bitmap_raw = bitmap.as_mut_ptr();
            let arena_pages_raw = arena_pages.as_mut_ptr();
            let arena_raw = arena.as_mut_ptr();
            let heap_raw = heap.as_mut_ptr();
            let page_raw = page.as_mut_ptr();

            mi_bitmap_init(bitmap_raw, 16, true);
            ptr::addr_of_mut!((*arena_pages_raw).pages).write(bitmap_raw);
            (*arena_raw.cast::<MiArenaPrefix>()).arena_idx = 0;
            (*arena_raw.cast::<MiArenaPrefix>()).slice_count = 16;
            let arena_ptr = ArenaPtr::new_unchecked(arena_raw);
            let range = crate::types::SliceRange::new(5, 2, arena_ptr).unwrap();
            let arena_memid = crate::types::MemId::<crate::types::ArenaMem>::arena(
                arena_ptr,
                range,
                crate::types::InitiallyCommitted(false),
                crate::types::InitiallyZero(false),
                crate::types::Pinned(false),
            )
            .into_raw();
            ptr::addr_of_mut!((*page_raw).xthread_free).write(
                crate::types::ThreadFreeWord::from_parts(
                    None,
                    crate::types::ThreadFreeOwner::Owned,
                )
                .raw(),
            );
            ptr::addr_of_mut!((*page_raw).heap).write(heap_raw);
            let page_ptr = PagePtr::new_unchecked(page_raw);

            ptr::addr_of_mut!((*page_ptr.as_ptr()).memid)
                .write(crate::internal::memid::_mi_memid_none());
            assert!(PageArenaMembership::from_page(page_ptr).is_none());

            ptr::addr_of_mut!((*page_ptr.as_ptr()).memid).write(arena_memid);
            assert!(PageArenaMembership::from_page(page_ptr).is_none());

            (*heap_raw.cast::<MiHeapPrefix>()).arena_pages[0]
                .store(arena_pages_raw.cast(), Ordering::Release);
            assert!(PageArenaMembership::from_page(page_ptr).is_none());

            assert!(mi_bitmap_set(bitmap_raw, 5));
            let membership = PageArenaMembership::from_page(page_ptr)
                .expect("set start bit establishes exact membership");
            assert_eq!(membership.arena().as_ptr(), arena_raw);
            assert_eq!(membership.range().start_usize(), 5);
            assert_eq!(membership.range().count_usize(), 2);
            assert_eq!(membership.arena_pages().as_ptr(), arena_pages_raw);
        }
    }
}
