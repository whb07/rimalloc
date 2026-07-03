use core::ptr;

use crate::types::{mi_block_t, mi_page_flags_t, mi_page_t, mi_threadid_t};

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_xthread_id(page: *const mi_page_t) -> mi_threadid_t {
    // SAFETY[AUDIT:internal-page-flag-owner-boundary]: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    unsafe {
        let atomic_tid =
            crate::core_types::AtomicPageThreadId(crate::core_types::AtomicUsizeField::from_raw(
                ptr::addr_of_mut!((*(page as *mut mi_page_t)).xthread_id),
            ));
        atomic_tid.load().raw()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_thread_id(page: *const mi_page_t) -> mi_threadid_t {
    // SAFETY[AUDIT:internal-page-flag-owner-boundary]: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    let xthread_id = crate::core_types::PageThreadId::new(unsafe { mi_page_xthread_id(page) });
    xthread_id.owner_bits()
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_flags(page: *const mi_page_t) -> mi_page_flags_t {
    // SAFETY[AUDIT:internal-page-flag-owner-boundary]: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    let xthread_id = crate::core_types::PageThreadId::new(unsafe { mi_page_xthread_id(page) });
    xthread_id.flags().bits()
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_has_interior_pointers(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:internal-page-flag-owner-boundary]: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    crate::core_types::PageFlags::from_low_bits(unsafe { mi_page_flags(page) })
        .contains(crate::core_types::PageFlags::HAS_INTERIOR_POINTERS)
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_abandoned_mapped(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:internal-page-flag-owner-boundary]: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    crate::core_types::PageThreadId::new(unsafe { mi_page_xthread_id(page) }).is_abandoned_mapped()
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_thread_free(page: *const mi_page_t) -> *mut mi_block_t {
    // SAFETY[AUDIT:internal-thread-free-boundary]: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        let atomic_tf =
            crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField::from_raw(
                ptr::addr_of_mut!((*(page as *mut mi_page_t)).xthread_free),
            ));
        atomic_tf.load_relaxed().head_raw()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_has_any_available(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:internal-thread-free-boundary]: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let reserved = crate::page::mi_page_reserved_count(page);
        crate::internal::mi_assert_internal!(reserved.get() > 0);
        crate::page::mi_page_used_count(page).get() < reserved.get()
            || !mi_page_thread_free(page).is_null()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_owned(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:internal-thread-free-boundary]: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        let atomic_tf =
            crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField::from_raw(
                ptr::addr_of_mut!((*(page as *mut mi_page_t)).xthread_free),
            ));
        atomic_tf.load_relaxed().is_owned()
    }
}
