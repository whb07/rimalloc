//! Arena slice geometry, containment, and heap page-table lookups.
//!
//! These helpers keep raw address projection and arena-page table access in one
//! narrow arena-local module while preserving the existing C ABI exports.

use super::*;

#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_alignment() -> usize {
    MI_ARENA_SLICE_ALIGN
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_size() -> usize {
    MI_ARENA_MIN_SIZE
}

/// # Safety
/// `arena` must remain live initialized arena metadata with a stable readable
/// `info_slices` field.
pub(super) unsafe fn mi_arena_info_slices(arena: ArenaPtr) -> usize {
    let arena = arena.as_ptr();
    // SAFETY: the caller keeps this arena prefix live and its initialized
    // information-slice count stable for the scalar read.
    unsafe { (*arena.cast::<MiArenaPrefix>()).info_slices }
}

/// # Safety
/// `arena` must satisfy `mi_arena_info_slices` and remain stable while the
/// typed range start is compared with that metadata snapshot.
pub(super) unsafe fn mi_arena_range_is_after_info_slices(
    arena: ArenaPtr,
    range: crate::types::SliceRange,
) -> bool {
    // SAFETY: the caller keeps the arena's initialized information-slice count
    // live and stable for this comparison.
    range.start_usize() >= unsafe { mi_arena_info_slices(arena) }
}

/// # Safety
/// `heap` and `arena` must remain live and belong to the same subproc. The arena
/// must have a stable registered index in bounds for the heap's initialized
/// atomic arena-page table. A non-null loaded result remains only a raw snapshot.
pub(super) unsafe fn mi_heap_arena_pages(
    heap: BorrowedHeap,
    arena: ArenaPtr,
) -> *mut mi_arena_pages_t {
    let arena = arena.as_ptr();
    // SAFETY: the caller keeps the registered arena prefix live with its
    // initialized table index stable and in range.
    let arena_idx = unsafe { mi_arena_index(arena) };
    // SAFETY: the borrowed heap remains live and owns the initialized atomic
    // cell at the checked arena index; acquire pairs with table publication.
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
        // SAFETY: the caller keeps the owned page metadata live with an
        // initialized atomic ownership word for this predicate.
        crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page_raw) });
        // SAFETY: the caller stabilizes this live page's initialized MemId for
        // the complete membership lookup.
        let crate::types::MemIdView::Arena(arena_mem) =
            crate::types::MemId::view(unsafe { (*page_raw).memid })
        else {
            return None;
        };
        let arena = arena_mem.arena();
        // SAFETY: the caller stabilizes the page's live initialized heap field;
        // the decoded arena belongs to that heap's subproc and stays registered.
        let arena_pages = unsafe {
            mi_heap_arena_pages(
                BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(mi_page_heap(page_raw))),
                arena,
            )
        };
        let arena_pages = core::ptr::NonNull::new(arena_pages)?;
        let range = arena_mem.range();
        // SAFETY: the non-null table and its published bitmap remain live; the
        // decoded typed range supplies its checked in-table start bit.
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

/// # Safety
/// `heap`, `arena`, and `page` must remain live within one subproc. The arena's
/// registered index and the page's initialized MemId must remain stable. Any
/// published arena-page table and bitmap must remain live and atomically
/// accessible for the lookup; concurrent destruction is excluded.
pub(super) unsafe fn mi_heap_has_page(heap: BorrowedHeap, arena: ArenaPtr, page: PagePtr) -> bool {
    let borrowed_heap = heap;
    let arena = arena.as_ptr();
    let page = page.as_ptr();
    // SAFETY: the caller keeps the registered live arena's initialized table
    // index stable and in bounds.
    let arena_idx = unsafe { mi_arena_index(arena) };
    // SAFETY: the borrowed live heap owns the initialized atomic cell selected
    // by the checked arena index; acquire observes table publication.
    let arena_pages = unsafe { mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire() }
        .cast::<MiArenaPagesPrefix>();
    // SAFETY: the caller keeps this live page's initialized MemId stable for
    // the arena identity and slice-start membership checks.
    match crate::types::MemId::view(unsafe { (*page).memid }) {
        crate::types::MemIdView::Arena(a) => {
            a.arena().as_ptr() == arena
                && !arena_pages.is_null()
                // SAFETY: the acquire-loaded non-null table and its bitmap stay
                // live, and the typed arena range supplies an in-table start bit.
                && unsafe { mi_bitmap_is_setN((*arena_pages).pages, a.range().start_usize(), 1) }
        }
        _ => false,
    }
}

/// # Safety
/// `arena` must remain live initialized metadata with a stable slice count that
/// is valid for `mi_size_of_slices` and whose byte conversion does not overflow.
pub(super) unsafe fn mi_arena_size(arena: ArenaPtr) -> crate::types::ReservedBytes {
    let arena = arena.as_ptr();
    // SAFETY: the caller keeps the live arena prefix and its initialized valid
    // slice count stable while the count is read and converted to bytes.
    crate::types::ReservedBytes::new(mi_size_of_slices(unsafe {
        (*arena.cast::<MiArenaPrefix>()).slice_count
    }))
}

/// # Safety
/// `arena` must be the non-null slice-aligned base of a live arena backing
/// allocation, not merely an arbitrary non-null metadata pointer.
pub(super) unsafe fn mi_arena_start(arena: ArenaPtr) -> crate::types::SliceStart {
    // SAFETY: the caller proves the arena base has the non-null alignment and
    // backing identity required by SliceStart; this conversion does not read it.
    unsafe { crate::types::SliceStart::new_unchecked(arena.as_ptr().cast::<u8>()) }
}

#[inline(never)]
/// # Safety
/// `arena` must remain live with stable initialized slice count and backing
/// geometry. `slice_index` must be in range, and the backing allocation must
/// cover the computed whole-slice offset without address overflow.
pub(super) unsafe fn mi_arena_slice_start(
    arena: ArenaPtr,
    slice_index: crate::types::SliceIndex,
) -> crate::types::SliceStart {
    let arena_raw = arena.as_ptr();
    let slice_index = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY: the caller keeps the initialized live arena prefix and its
        // slice count stable for the bounds assertion.
        slice_index < unsafe { (*arena_raw.cast::<MiArenaPrefix>()).slice_count }
    );
    // SAFETY: the caller proves the arena is a valid SliceStart and that the
    // checked slice index's byte offset remains within its backing allocation.
    let start = unsafe {
        mi_arena_start(arena)
            .as_ptr()
            .add(mi_size_of_slices(slice_index))
    };
    // SAFETY: adding a whole-slice offset to the aligned arena base preserves
    // non-null slice alignment within the caller-proven backing allocation.
    unsafe { crate::types::SliceStart::new_unchecked(start) }
}

#[inline(never)]
/// # Safety
/// `arena` must remain live with stable initialized slice count, `pages_meta`,
/// and backing geometry. `slice_index` must be in range. A non-null metadata
/// array must contain that element; otherwise the corresponding arena slice
/// must contain live aligned page metadata.
pub(super) unsafe fn mi_arena_page_at_slice(
    arena: ArenaPtr,
    slice_index: crate::types::SliceIndex,
) -> PagePtr {
    let arena = arena.as_ptr();
    let slice_index_usize = slice_index.get();
    crate::internal::mi_assert_internal!(
        // SAFETY: the caller keeps the live arena's initialized slice count
        // stable for this bounds assertion.
        slice_index_usize < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    // SAFETY: the caller keeps the live arena prefix and its initialized
    // separate-metadata pointer stable for this snapshot.
    let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
    if !pages_meta.is_null() {
        // SAFETY: the caller proves the non-null metadata array contains one
        // aligned page entry for every valid slice index.
        return unsafe { PagePtr::new_unchecked(pages_meta.add(slice_index_usize)) };
    }
    // SAFETY: the caller proves the arena backing and checked index identify a
    // live aligned page-metadata slice at the computed address.
    let page = unsafe {
        mi_arena_slice_start(ArenaPtr::new_unchecked(arena), slice_index)
            .as_ptr()
            .cast::<mi_page_t>()
    };
    // SAFETY: the computed slice address is caller-proven non-null aligned live
    // page metadata in the arena backing allocation.
    unsafe { PagePtr::new_unchecked(page) }
}

/// # Safety
/// `arena_id` must be null or a valid registered top-level arena id. If `size`
/// is non-null, it must be properly aligned and writable for one `usize` for
/// the duration of the call. The selected arena metadata must remain live and
/// stable while its geometry is read.
// SAFETY: this exported symbol intentionally preserves the upstream C ABI and
// its raw arena-id/output-pointer signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_area(arena_id: mi_arena_id_t, size: *mut usize) -> *mut c_void {
    if !size.is_null() {
        // SAFETY: the caller guarantees a non-null `size` points to writable
        // initialized-compatible storage for the output value.
        unsafe { *size = 0 };
    }
    // SAFETY: the caller supplies null or a valid registered top-level arena id.
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        return ptr::null_mut();
    }
    if !size.is_null() {
        crate::internal::mi_assert_internal!(
            // SAFETY: id lookup returned live initialized arena metadata whose
            // slice count and total size remain stable for this consistency check.
            mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count })
                // SAFETY: the same live arena prefix keeps total_size readable
                // and stable for the paired comparison.
                <= unsafe { (*arena.cast::<MiArenaPrefix>()).total_size }
        );
        // SAFETY: the caller's non-null output pointer remains writable, and
        // id lookup keeps this arena's initialized total size readable.
        unsafe { *size = (*arena.cast::<MiArenaPrefix>()).total_size };
    }
    // SAFETY: id lookup returned this live arena's non-null slice-aligned base.
    unsafe {
        mi_arena_start(ArenaPtr::new_unchecked(arena))
            .as_ptr()
            .cast::<c_void>()
    }
}

fn mi_arena_strictly_contains(
    arena: ArenaPtr,
    p: Option<InteriorUserPtr>,
    size: ReservedBytes,
) -> bool {
    let Some(p) = p else {
        return false;
    };
    let p = crate::core_types::ptr_exposed_addr(p.as_u8_ptr());
    let start = crate::core_types::ptr_exposed_addr(arena.as_ptr());
    start <= p && start.checked_add(size.get()).is_some_and(|end| end > p)
}

/// # Safety
/// The current subproc and its arena registry must remain initialized and live
/// for the traversal. Published arena pointers, their parent links, and arena
/// size fields must remain stable and readable; concurrent arena destruction or
/// reparenting is excluded. A supplied `parent` must belong to that registry.
unsafe fn mi_arenas_contain_ex(p: Option<InteriorUserPtr>, parent: Option<ArenaPtr>) -> bool {
    // The noninitializing lookup selects the current or process-main subproc;
    // the caller keeps that registry live for the ensuing traversal.
    let subproc = _mi_subproc();
    // SAFETY: the initialized current subproc identity is non-null by contract.
    let subproc_ptr = unsafe { SubprocPtr::new_unchecked(subproc) };
    // SAFETY: the caller stabilizes this live subproc's published arena count
    // and table for the bounded traversal.
    let max_arena = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    let parent_raw = parent.map(ArenaPtr::as_ptr);
    for i in 0..max_arena {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY: the caller keeps the live subproc arena table stable; the
        // typed index is within the count snapshot and table capacity.
        let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
        let Some(arena_ptr) = ArenaPtr::new(arena) else {
            continue;
        };
        if !arena.is_null()
            && (parent_raw.is_none()
                || Some(arena) == parent_raw
                // SAFETY: the published live arena prefix and its parent link
                // remain readable and stable for this identity comparison.
                || unsafe { Some((*arena.cast::<MiArenaPrefix>()).parent) == parent_raw })
            // SAFETY: the published arena remains live with stable initialized
            // slice count valid for the byte-size conversion.
            && mi_arena_strictly_contains(arena_ptr, p, unsafe { mi_arena_size(arena_ptr) })
        {
            return true;
        }
    }
    false
}

/// # Safety
/// `arena_id` must be null or a valid registered top-level arena id; `p` may be
/// any pointer value. The current subproc registry and all traversed arena
/// metadata must remain live and stable for the synchronous lookup.
// SAFETY: this exported symbol intentionally preserves the upstream C ABI and
// its raw arena-id/address signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_contains(arena_id: mi_arena_id_t, p: *const c_void) -> bool {
    // SAFETY: the caller supplies null or a valid registered top-level arena id
    // while the current registry remains stable.
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        false
    } else {
        // SAFETY: id lookup returned non-null live registered arena metadata.
        let arena_ptr = unsafe { ArenaPtr::new_unchecked(arena) };
        // SAFETY: the caller keeps this arena's initialized slice count stable
        // and valid for the byte-size conversion.
        let arena_size = unsafe { mi_arena_size(arena_ptr) };
        if mi_arena_strictly_contains(arena_ptr, InteriorUserPtr::new(p.cast_mut()), arena_size) {
            true
        } else {
            // SAFETY: the caller keeps the current registry and this parent
            // arena's published child relationships stable for traversal.
            unsafe { mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), Some(arena_ptr)) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{mem::MaybeUninit, ptr, sync::atomic::Ordering};

    #[test]
    fn strict_arena_contains_uses_checked_half_open_geometry() {
        let mut arena = MaybeUninit::<mi_arena_t>::zeroed();
        let arena_raw = arena.as_mut_ptr();
        let arena_ptr = ArenaPtr::new(arena_raw).unwrap();
        let byte_len = core::mem::size_of::<mi_arena_t>();
        let size = crate::types::ReservedBytes::new(byte_len);

        // SAFETY: the stack allocation contains `byte_len` bytes, so the last
        // byte and one-past addresses are valid pointer-arithmetic results.
        let (last, one_past) = unsafe {
            (
                arena_raw.cast::<u8>().add(byte_len - 1),
                arena_raw.cast::<u8>().add(byte_len),
            )
        };
        assert!(mi_arena_strictly_contains(
            arena_ptr,
            InteriorUserPtr::new(arena_raw.cast()),
            size,
        ));
        assert!(mi_arena_strictly_contains(
            arena_ptr,
            InteriorUserPtr::new(last.cast()),
            size,
        ));
        assert!(!mi_arena_strictly_contains(
            arena_ptr,
            InteriorUserPtr::new(one_past.cast()),
            size,
        ));
        assert!(!mi_arena_strictly_contains(arena_ptr, None, size));

        let near_max = ptr::without_provenance_mut::<mi_arena_t>(usize::MAX - 7);
        let near_max = ArenaPtr::new(near_max).unwrap();
        assert!(!mi_arena_strictly_contains(
            near_max,
            InteriorUserPtr::new(near_max.as_ptr().cast()),
            crate::types::ReservedBytes::new(16),
        ));
    }

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
