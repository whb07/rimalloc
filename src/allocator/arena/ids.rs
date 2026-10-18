//! Arena id and memid compatibility helpers.
//!
//! This module keeps the raw arena-id shims and typed `MemIdView` routing out
//! of the main arena allocation flow without changing the C-shaped helper
//! boundaries they preserve.

use super::*;

/// Return the null arena-ID sentinel without accessing allocator metadata.
#[inline(never)]
pub(crate) extern "C" fn _mi_arena_id_none() -> mi_arena_id_t {
    ptr::null_mut()
}

/// # Safety
/// `id` must be either null or a valid top-level `mi_arena_t*`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arena_from_id(id: mi_arena_id_t) -> *mut mi_arena_t {
    let arena = id.cast::<mi_arena_t>();
    crate::internal::mi_assert_internal!(
        arena.is_null() || unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() }
    );
    arena
}

/// # Safety
/// `arena` must be null or a valid top-level `mi_arena_t*`.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_arena_id_from_arena(arena: *mut mi_arena_t) -> mi_arena_id_t {
    let Some(arena) = ArenaPtr::new(arena) else {
        return _mi_arena_id_none();
    };
    unsafe { mi_arena_id_from_arena_typed(arena) }
}

pub(super) unsafe fn mi_arena_id_from_arena_typed(arena: ArenaPtr) -> mi_arena_id_t {
    crate::internal::mi_assert_internal!(unsafe { arena.parent().is_none() });
    arena.as_ptr().cast::<c_void>()
}

/// # Safety
/// `arena` and an optional `req_arena` must remain live initialized registered
/// arenas with stable exclusive-role and parent-link metadata for the check.
/// Registry teardown or relinking must not race this operation.
pub(super) unsafe fn mi_arena_is_suitable(arena: ArenaPtr, req_arena: Option<ArenaPtr>) -> bool {
    if req_arena.is_some_and(|req_arena| arena.as_ptr() == req_arena.as_ptr()) {
        return true;
    }
    // SAFETY: the function contract keeps this registered arena's initialized
    // exclusive-role field live and stable for the read.
    if req_arena.is_none() && !unsafe { arena.is_exclusive() } {
        return true;
    }
    // SAFETY: the same registry epoch keeps this arena's initialized parent
    // field live and stable for the relationship check.
    let parent = unsafe { arena.parent() };
    parent.is_some_and(|parent| {
        req_arena.is_some_and(|req_arena| parent.as_ptr() == req_arena.as_ptr())
    })
}

/// # Safety
/// Any arena carried by `memid` and an optional `request_arena` must remain live
/// registered arenas with stable role and parent metadata. The decoded MemId
/// view must still describe that live allocation epoch; teardown or registry
/// relinking must not race this check.
unsafe fn mi_arena_memid_is_suitable_typed(
    memid: crate::core_types::MemIdView,
    request_arena: Option<ArenaPtr>,
) -> bool {
    match memid {
        // SAFETY: the Arena MemId variant carries the arena identity whose live
        // registration and stable relationship metadata the caller guarantees.
        crate::core_types::MemIdView::Arena(a) => unsafe {
            mi_arena_is_suitable(a.arena(), request_arena)
        },
        _ => false,
    }
}

/// # Safety
/// If `memid` encodes an arena allocation, that arena and a non-null
/// `request_arena` must remain live registered arenas with stable exclusive-role
/// and parent metadata. The MemId must still describe the live allocation epoch;
/// teardown or registry relinking must not race this check.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arena_memid_is_suitable(
    memid: mi_memid_t,
    request_arena: *mut mi_arena_t,
) -> bool {
    let request_arena = ArenaPtr::new(request_arena);
    let memid = crate::core_types::MemId::view(memid);
    // SAFETY: the raw contract keeps every arena identity decoded above live
    // with stable relationship metadata for the synchronous typed check.
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
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    crate::core_types::MemId::<crate::core_types::ArenaMem>::arena(
        unsafe { crate::core_types::ArenaPtr::new_unchecked(arena) },
        range,
        crate::core_types::InitiallyCommitted(false),
        crate::core_types::InitiallyZero(false),
        crate::core_types::Pinned(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned }),
    )
    .into_raw()
}

#[inline]
pub(super) unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool {
    matches!(
        crate::core_types::MemId::view(unsafe { (*page).memid }),
        crate::core_types::MemIdView::Arena(_)
    )
}

pub(super) unsafe fn mi_page_full_size(page: PagePtr) -> crate::core_types::ReservedBytes {
    let page_raw = page.as_ptr();
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
