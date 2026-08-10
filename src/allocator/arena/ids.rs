//! Arena id and memid compatibility helpers.
//!
//! This module keeps the raw arena-id shims and typed `MemIdView` routing out
//! of the main arena allocation flow without changing the C-shaped helper
//! boundaries they preserve.

use super::*;

/// # Safety
/// Matches the C internal API and returns the null arena-id sentinel.
#[inline(never)]
// SAFETY: this raw C ABI shim returns the
// null arena-id sentinel and does not dereference allocator metadata.
pub(crate) unsafe extern "C" fn _mi_arena_id_none() -> mi_arena_id_t {
    ptr::null_mut()
}

/// # Safety
/// `id` must be either null or a valid top-level `mi_arena_t*`.
#[inline(never)]
// SAFETY: this raw C ABI shim checks the arena
// prefix parent field before returning the arena pointer unchanged.
pub(crate) unsafe extern "C" fn _mi_arena_from_id(id: mi_arena_id_t) -> *mut mi_arena_t {
    let arena = id.cast::<mi_arena_t>();
    crate::internal::mi_assert_internal!(
        // SAFETY: non-null arena ids point to
        // initialized top-level arena metadata by the internal arena-id contract.
        arena.is_null() || unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() }
    );
    arena
}

/// # Safety
/// `arena` must be null or a valid top-level `mi_arena_t*`.
// SAFETY: this exported arena-id converter keeps
// C ABI shape and wraps non-null arenas before typed validation.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_arena_id_from_arena(arena: *mut mi_arena_t) -> mi_arena_id_t {
    let Some(arena) = ArenaPtr::new(arena) else {
        // SAFETY: null arena input maps to the
        // null arena-id sentinel through the internal C ABI shim.
        return unsafe { _mi_arena_id_none() };
    };
    // SAFETY: `ArenaPtr::new` proved the pointer
    // is non-null; the typed helper checks top-level arena state.
    unsafe { mi_arena_id_from_arena_typed(arena) }
}

pub(super) unsafe fn mi_arena_id_from_arena_typed(arena: ArenaPtr) -> mi_arena_id_t {
    // SAFETY: `ArenaPtr` is a non-null arena
    // witness and `parent()` only reads the arena prefix parent field.
    crate::internal::mi_assert_internal!(unsafe { arena.parent().is_none() });
    arena.as_ptr().cast::<c_void>()
}

pub(super) unsafe fn mi_arena_is_suitable(arena: ArenaPtr, req_arena: Option<ArenaPtr>) -> bool {
    if req_arena.is_some_and(|req_arena| arena.as_ptr() == req_arena.as_ptr()) {
        return true;
    }
    // SAFETY: arena suitability helpers read typed arena role and parent metadata.
    // witness and `is_exclusive()` reads the arena prefix role flag.
    if req_arena.is_none() && !unsafe { arena.is_exclusive() } {
        return true;
    }
    // SAFETY: arena suitability helpers read typed arena role and parent metadata.
    // witness and `parent()` reads the arena prefix parent link.
    let parent = unsafe { arena.parent() };
    parent.is_some_and(|parent| {
        req_arena.is_some_and(|req_arena| parent.as_ptr() == req_arena.as_ptr())
    })
}

unsafe fn mi_arena_memid_is_suitable_typed(
    memid: crate::core_types::MemIdView,
    request_arena: Option<ArenaPtr>,
) -> bool {
    match memid {
        // SAFETY: `MemIdView::Arena` carries the
        // typed arena witness extracted from the raw memid.
        crate::core_types::MemIdView::Arena(a) => unsafe {
            mi_arena_is_suitable(a.arena(), request_arena)
        },
        _ => false,
    }
}

/// # Safety
/// `memid` and `request_arena` must follow the internal arena allocation invariants.
#[inline(never)]
// SAFETY: this raw C ABI shim converts nullable
// arena requests and raw memids into typed views before suitability checks.
pub(crate) unsafe extern "C" fn _mi_arena_memid_is_suitable(
    memid: mi_memid_t,
    request_arena: *mut mi_arena_t,
) -> bool {
    let request_arena = ArenaPtr::new(request_arena);
    let memid = crate::core_types::MemId::view(memid);
    // SAFETY: the raw inputs have been converted
    // to typed/optional views before the suitability helper inspects metadata.
    unsafe { mi_arena_memid_is_suitable_typed(memid, request_arena) }
}

pub(super) unsafe fn mi_memid_create_arena(
    arena: *mut mi_arena_t,
    range: crate::core_types::SliceRange,
) -> mi_memid_t {
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    crate::internal::mi_assert_internal!(slice_count > 0);
    crate::internal::mi_assert_internal!(
        // SAFETY: arena allocation callers pass
        // initialized arena metadata and a range validated for that arena.
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    crate::core_types::MemId::<crate::core_types::ArenaMem>::arena(
        // SAFETY: `arena` is allocator-owned
        // arena metadata selected before this memid construction boundary.
        unsafe { crate::core_types::ArenaPtr::new_unchecked(arena) },
        range,
        crate::core_types::InitiallyCommitted(false),
        crate::core_types::InitiallyZero(false),
        // SAFETY: the arena prefix memid owns the
        // pinned flag copied into the allocation memid.
        crate::core_types::Pinned(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned }),
    )
    .into_raw()
}

pub(super) unsafe fn mi_arena_from_memid(
    memid: mi_memid_t,
    slice_index: *mut usize,
    slice_count: *mut usize,
) -> *mut mi_arena_t {
    // SAFETY: the helper only returns an arena
    // pointer when the raw memid decodes to an arena MemIdView.
    let Some((arena, range)) = (unsafe { mi_arena_range_from_memid(memid) }) else {
        return core::ptr::null_mut();
    };
    if !slice_index.is_null() {
        // SAFETY: caller provided a non-null
        // out-pointer for the arena slice start.
        unsafe { *slice_index = range.start_usize() };
    }
    if !slice_count.is_null() {
        // SAFETY: caller provided a non-null
        // out-pointer for the arena slice count.
        unsafe { *slice_count = range.count_usize() };
    }
    arena.as_ptr()
}

unsafe fn mi_arena_range_from_memid(
    memid: mi_memid_t,
) -> Option<(crate::core_types::ArenaPtr, crate::core_types::SliceRange)> {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(a) => Some((a.arena(), a.range())),
        _ => {
            crate::internal::mi_assert_internal!(false);
            None
        }
    }
}

#[inline]
pub(super) unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool {
    matches!(
        // SAFETY: callers pass live page metadata
        // before classifying the page memid view.
        crate::core_types::MemId::view(unsafe { (*page).memid }),
        crate::core_types::MemIdView::Arena(_)
    )
}

pub(super) fn mi_memid_is_os(memid: mi_memid_t) -> bool {
    matches!(
        crate::core_types::MemId::view(memid),
        crate::core_types::MemIdView::Os(_)
    )
}

pub(super) unsafe fn mi_page_full_size(page: PagePtr) -> crate::core_types::ReservedBytes {
    let page_raw = page.as_ptr();
    // SAFETY: page memid size helpers project arena, OS, and external allocation extents.
    // witness and the memid field is read-only for size classification here.
    let memid = unsafe { (*page_raw).memid };
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(arena_memid) => {
            let range = arena_memid.range();
            crate::core_types::ReservedBytes::new(range.count_usize() * MI_ARENA_SLICE_SIZE)
        }
        crate::core_types::MemIdView::Os(os) => {
            let base = os
                .base
                .map(|p| p.as_ptr())
                .unwrap_or(core::ptr::null_mut())
                .cast::<u8>();
            let os_size = os.size;

            let page_ptr = page_raw.cast::<u8>();
            crate::internal::mi_assert_internal!(base <= page_ptr);
            // SAFETY: the base and page pointer
            // come from the same OS allocation memid range.
            let presize = unsafe { page_ptr.offset_from(base) };
            crate::internal::mi_assert_internal!(os_size.get() as isize >= presize);
            if presize > os_size.get() as isize {
                crate::core_types::ReservedBytes::new(0)
            } else {
                crate::core_types::ReservedBytes::new(os_size.get() - presize as usize)
            }
        }
        crate::core_types::MemIdView::External(external) => {
            let base = external
                .base
                .map(|p| p.as_ptr())
                .unwrap_or(core::ptr::null_mut())
                .cast::<u8>();
            let os_size = external.size.get();

            let page_ptr = page_raw.cast::<u8>();
            crate::internal::mi_assert_internal!(base <= page_ptr);
            // SAFETY: the base and page pointer
            // come from the same external allocation memid range.
            let presize = unsafe { page_ptr.offset_from(base) };
            crate::internal::mi_assert_internal!(os_size as isize >= presize);
            if presize > os_size as isize {
                crate::core_types::ReservedBytes::new(0)
            } else {
                crate::core_types::ReservedBytes::new(os_size - presize as usize)
            }
        }
        _ => crate::core_types::ReservedBytes::new(0),
    }
}
