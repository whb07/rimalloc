use crate::types::{mi_heap_t, mi_page_t, mi_tld_t};

#[inline]
pub(crate) unsafe fn mi_page_block_size_typed(
    page: *const mi_page_t,
) -> crate::core_types::BlockSize {
    // SAFETY[AUDIT:internal-page-size-count-boundary]: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        crate::internal::mi_assert_internal!((*page).block_size > 0);
        crate::core_types::BlockSize::new_unchecked((*page).block_size)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_block_size(page: *const mi_page_t) -> usize {
    // SAFETY[AUDIT:internal-page-size-count-boundary]: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe { mi_page_block_size_typed(page).get() }
}

/// # Safety
/// `page` must point to non-abandoned page metadata with a valid owning thread heap.
#[inline]
pub unsafe fn mi_page_tld(page: *const mi_page_t) -> *mut mi_tld_t {
    // SAFETY[AUDIT:internal-page-owner-heap-boundary]: page owner helpers resolve heap, thread heap, and queue pointers from caller-proven metadata.
    unsafe {
        crate::internal::mi_assert_internal!(!crate::page::mi_page_is_abandoned(page));
        crate::internal::mi_assert_internal!(!(*page).theap.is_null());
        (*(*page).theap).tld
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_heap(page: *const mi_page_t) -> *mut mi_heap_t {
    // SAFETY[AUDIT:internal-page-owner-heap-boundary]: page owner helpers resolve heap, thread heap, and queue pointers from caller-proven metadata.
    unsafe {
        let mut heap = (*page).heap;
        if crate::internal::mi_likely(heap.is_null()) {
            heap = crate::mimalloc::mi_heap_main();
        }
        crate::internal::mi_assert_internal!(!heap.is_null());
        heap
    }
}
