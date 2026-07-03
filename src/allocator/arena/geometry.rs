//! Arena slice geometry, containment, and heap page-table lookups.
//!
//! These helpers keep raw address projection and arena-page table access in one
//! narrow arena-local module while preserving the existing C ABI exports.

use super::*;

// SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_alignment() -> usize {
    MI_ARENA_SLICE_ALIGN
}

// SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_size() -> usize {
    MI_ARENA_MIN_SIZE
}

pub(super) unsafe fn mi_arena_info_slices(arena: ArenaPtr) -> usize {
    let arena = arena.as_ptr();
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { (*arena.cast::<MiArenaPrefix>()).info_slices }
}

pub(super) unsafe fn mi_arena_range_is_after_info_slices(
    arena: ArenaPtr,
    range: crate::core_types::SliceRange,
) -> bool {
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    range.start_usize() >= unsafe { mi_arena_info_slices(arena) }
}

pub(super) unsafe fn mi_heap_arena_pages(
    heap: BorrowedHeap,
    arena: ArenaPtr,
) -> *mut mi_arena_pages_t {
    let arena = arena.as_ptr();
    // SAFETY[AUDIT:arena-heap-page-table-boundary]: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
    let arena_idx = unsafe { mi_arena_index(arena) };
    unsafe { mi_heap_arena_page_cell(heap, arena_idx).load_acquire() }.cast::<mi_arena_pages_t>()
}

pub(super) unsafe fn mi_page_arena_pages(
    page: PagePtr,
    slice_index: *mut usize,
    slice_count: *mut usize,
    parena_pages: *mut *mut mi_arena_pages_t,
) -> *mut mi_arena_t {
    let page = page.as_ptr();
    // SAFETY[AUDIT:arena-heap-page-table-boundary]: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    let arena = unsafe { mi_arena_from_memid((*page).memid, slice_index, slice_count) };
    crate::internal::mi_assert_internal!(!arena.is_null());
    if !parena_pages.is_null() {
        let arena_pages = unsafe {
            mi_heap_arena_pages(
                BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(mi_page_heap(page))),
                ArenaPtr::new_unchecked(arena),
            )
        };
        crate::internal::mi_assert_internal!(!arena_pages.is_null());
        crate::internal::mi_assert_internal!(
            slice_index.is_null()
                // SAFETY[AUDIT:arena-heap-page-table-boundary]: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
                || unsafe { mi_bitmap_is_setN((*arena_pages).pages, *slice_index, 1) }
        );
        unsafe { *parena_pages = arena_pages };
    }
    arena
}

pub(super) unsafe fn mi_heap_has_page(heap: BorrowedHeap, arena: ArenaPtr, page: PagePtr) -> bool {
    let borrowed_heap = heap;
    let arena = arena.as_ptr();
    let page = page.as_ptr();
    // SAFETY[AUDIT:arena-heap-page-table-boundary]: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
    let arena_idx = unsafe { mi_arena_index(arena) };
    let arena_pages = unsafe { mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire() }
        .cast::<MiArenaPagesPrefix>();
    match crate::core_types::MemId::view(unsafe { (*page).memid }) {
        crate::core_types::MemIdView::Arena(a) => {
            a.arena().as_ptr() == arena
                && !arena_pages.is_null()
                // SAFETY[AUDIT:arena-heap-page-table-boundary]: heap arena-page table access validates arena indexes and bitmap ownership for page metadata lookup.
                && unsafe { mi_bitmap_is_setN((*arena_pages).pages, a.range().start_usize(), 1) }
        }
        _ => false,
    }
}

pub(super) unsafe fn mi_arena_size(arena: ArenaPtr) -> crate::core_types::ReservedBytes {
    let arena = arena.as_ptr();
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    crate::core_types::ReservedBytes::new(mi_size_of_slices(unsafe {
        (*arena.cast::<MiArenaPrefix>()).slice_count
    }))
}

pub(super) unsafe fn mi_arena_start(arena: ArenaPtr) -> crate::core_types::SliceStart {
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { crate::core_types::SliceStart::new_unchecked(arena.as_ptr().cast::<u8>()) }
}

#[inline(never)]
pub(super) unsafe fn mi_arena_slice_start(
    arena: ArenaPtr,
    slice_index: crate::core_types::SliceIndex,
) -> crate::core_types::SliceStart {
    let arena_raw = arena.as_ptr();
    let slice_index = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        slice_index < unsafe { (*arena_raw.cast::<MiArenaPrefix>()).slice_count }
    );
    let start = unsafe {
        mi_arena_start(arena)
            .as_ptr()
            .add(mi_size_of_slices(slice_index))
    };
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { crate::core_types::SliceStart::new_unchecked(start) }
}

#[inline(never)]
pub(super) unsafe fn mi_arena_page_at_slice(
    arena: ArenaPtr,
    slice_index: crate::core_types::SliceIndex,
) -> PagePtr {
    let arena = arena.as_ptr();
    let slice_index_usize = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        slice_index_usize < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
    if !pages_meta.is_null() {
        return unsafe { PagePtr::new_unchecked(pages_meta.add(slice_index_usize)) };
    }
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let page = unsafe {
        mi_arena_slice_start(ArenaPtr::new_unchecked(arena), slice_index)
            .as_ptr()
            .cast::<mi_page_t>()
    };
    // SAFETY[AUDIT:arena-slice-geometry-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    unsafe { PagePtr::new_unchecked(page) }
}

/// # Safety
/// `arena_id` must be null or a valid top-level arena id; `size` may be null.
// SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_area(arena_id: mi_arena_id_t, size: *mut usize) -> *mut c_void {
    if !size.is_null() {
        unsafe { *size = 0 };
    }
    // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        return ptr::null_mut();
    }
    if !size.is_null() {
        crate::internal::mi_assert_internal!(
            // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
            mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count })
                <= unsafe { (*arena.cast::<MiArenaPrefix>()).total_size }
        );
        unsafe { *size = (*arena.cast::<MiArenaPrefix>()).total_size };
    }
    // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
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
    // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let start = unsafe { mi_arena_start(arena).as_ptr().cast_const() };
    let end = unsafe { start.add(size.get()) };
    start <= p && end > p
}

unsafe fn mi_arenas_contain_ex(p: Option<InteriorUserPtr>, parent: Option<ArenaPtr>) -> bool {
    // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let subproc = unsafe { _mi_subproc() };
    let subproc_ptr = unsafe { SubprocPtr::new_unchecked(subproc) };
    let max_arena = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    let parent_raw = parent.map(ArenaPtr::as_ptr);
    for i in 0..max_arena {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
        let Some(arena_ptr) = ArenaPtr::new(arena) else {
            continue;
        };
        if !arena.is_null()
            && (parent_raw.is_none()
                || Some(arena) == parent_raw
                // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
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
// SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_contain(p: *const c_void) -> bool {
    unsafe { mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), None) }
}

// SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[used]
static MI_ARENAS_CONTAIN_KEEP: unsafe extern "C" fn(*const c_void) -> bool = _mi_arenas_contain;

/// # Safety
/// `arena_id` must be null or a valid top-level arena id; `p` may be any pointer value.
// SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_contains(arena_id: mi_arena_id_t, p: *const c_void) -> bool {
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        false
    // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    } else if unsafe {
        mi_arena_strictly_contains(
            ArenaPtr::new_unchecked(arena),
            InteriorUserPtr::new(p.cast_mut()),
            mi_arena_size(ArenaPtr::new_unchecked(arena)),
        )
    } {
        true
    } else {
        // SAFETY[AUDIT:arena-slice-containment-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        unsafe {
            mi_arenas_contain_ex(
                InteriorUserPtr::new(p.cast_mut()),
                Some(ArenaPtr::new_unchecked(arena)),
            )
        }
    }
}

#[used]
static MI_ARENA_SLICE_START_KEEP: unsafe fn(ArenaPtr, SliceIndex) -> crate::core_types::SliceStart =
    mi_arena_slice_start;
#[used]
static MI_ARENA_PAGE_AT_SLICE_KEEP: unsafe fn(ArenaPtr, crate::core_types::SliceIndex) -> PagePtr =
    mi_arena_page_at_slice;
