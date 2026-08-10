use crate::types::{mi_block_t, mi_page_flags_t, mi_page_t, mi_threadid_t};

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_xthread_id(page: *const mi_page_t) -> mi_threadid_t {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    unsafe {
        let page = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        let atomic_tid = page.thread_id_cell();
        atomic_tid.load().raw()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_thread_id(page: *const mi_page_t) -> mi_threadid_t {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    let xthread_id = crate::core_types::PageThreadId::new(unsafe { mi_page_xthread_id(page) });
    xthread_id.owner_bits()
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_flags(page: *const mi_page_t) -> mi_page_flags_t {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    let xthread_id = crate::core_types::PageThreadId::new(unsafe { mi_page_xthread_id(page) });
    xthread_id.flags().bits()
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_has_interior_pointers(page: *const mi_page_t) -> bool {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    crate::core_types::PageFlags::from_low_bits(unsafe { mi_page_flags(page) })
        .contains(crate::core_types::PageFlags::HAS_INTERIOR_POINTERS)
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_abandoned_mapped(page: *const mi_page_t) -> bool {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    crate::core_types::PageThreadId::new(unsafe { mi_page_xthread_id(page) }).is_abandoned_mapped()
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_thread_free(page: *const mi_page_t) -> *mut mi_block_t {
    // SAFETY: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        let page = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        let atomic_tf = page.thread_free_cell();
        atomic_tf.load_relaxed().head_raw()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_has_any_available(page: *const mi_page_t) -> bool {
    // SAFETY: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let reserved = crate::allocator::page::mi_page_reserved_count(page);
        crate::internal::mi_assert_internal!(reserved.get() > 0);
        crate::allocator::page::mi_page_used_count(page).get() < reserved.get()
            || !mi_page_thread_free(page).is_null()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_owned(page: *const mi_page_t) -> bool {
    // SAFETY: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        let page = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        let atomic_tf = page.thread_free_cell();
        atomic_tf.load_relaxed().is_owned()
    }
}
