use crate::types::{mi_heap_t, mi_page_t, mi_tld_t};

#[inline]
pub(crate) unsafe fn mi_page_block_size_typed(
    page: *const mi_page_t,
) -> crate::core_types::BlockSize {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe { crate::core_types::PagePtr::new_unchecked(page.cast_mut()).block_size() }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_block_size(page: *const mi_page_t) -> usize {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe { mi_page_block_size_typed(page).get() }
}

/// # Safety
/// `page` must point to non-abandoned page metadata with a valid owning thread heap.
#[inline]
pub unsafe fn mi_page_tld(page: *const mi_page_t) -> *mut mi_tld_t {
    // SAFETY: page owner helpers resolve heap, thread heap, and queue pointers from caller-proven metadata.
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
    // SAFETY: page owner helpers resolve heap, thread heap, and queue pointers from caller-proven metadata.
    unsafe {
        let mut heap = (*page).heap;
        if crate::internal::mi_likely(heap.is_null()) {
            heap = crate::mimalloc::mi_heap_main();
        }
        crate::internal::mi_assert_internal!(!heap.is_null());
        heap
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_size(page: *const mi_page_t) -> usize {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        let block_size = mi_page_block_size(page);
        block_size * crate::page::mi_page_reserved_count(page).as_usize()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_singleton(page: *const mi_page_t) -> bool {
    // SAFETY: page location helpers derive arena, slice, and address ranges from allocator metadata.
    unsafe { crate::page::mi_page_reserved_count(page).get() == 1 }
}

#[inline]
pub(crate) unsafe fn mi_page_usable_block_size(
    page: *const mi_page_t,
) -> crate::core_types::UsableSize {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe { mi_page_block_size_typed(page).usable() }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub(crate) unsafe fn mi_page_committed_typed(
    page: *const mi_page_t,
) -> crate::core_types::CommittedBytes {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        crate::core_types::CommittedBytes::new(if (*page).slice_committed == 0 {
            mi_page_size(page)
        } else {
            (*page).slice_committed - super::page_location::mi_page_slice_offset_of(page, 0)
        })
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_committed(page: *const mi_page_t) -> usize {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe { mi_page_committed_typed(page).get() }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_all_free(page: *const mi_page_t) -> bool {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::page::mi_page_used_count(page).get() == 0
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_immediate_available(page: *const mi_page_t) -> bool {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        !crate::core_types::PagePtr::new_unchecked(page.cast_mut())
            .free()
            .is_empty()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_expandable(page: *const mi_page_t) -> bool {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let capacity = crate::page::mi_page_capacity_count(page);
        let reserved = crate::page::mi_page_reserved_count(page);
        crate::internal::mi_assert_internal!(capacity.get() <= reserved.get());
        capacity.get() < reserved.get()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        let full = crate::page::mi_page_reserved_count(page).get()
            == crate::page::mi_page_used_count(page).get();
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        crate::internal::mi_assert_internal!(!full || page_ptr.free().is_empty());
        full
    }
}

/// # Safety
/// `page` must be null or point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_mostly_used(page: *const mi_page_t) -> bool {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        if page.is_null() {
            return true;
        }
        let reserved = crate::page::mi_page_reserved_count(page);
        let used = crate::page::mi_page_used_count(page);
        let frac = reserved.get() / 8u16;
        reserved.get() - used.get() <= frac
    }
}

/// # Safety
/// `page` must be null or point to page metadata owned by this allocator. `n` must be nonzero.
#[inline]
pub unsafe fn mi_page_is_used_at_frac(page: *const mi_page_t, n: u16) -> bool {
    // SAFETY: page size/count accessors project caller-proven metadata into typed scalar domains.
    unsafe {
        if page.is_null() {
            return true;
        }
        let reserved = crate::page::mi_page_reserved_count(page);
        let used = crate::page::mi_page_used_count(page);
        let frac = reserved.get() / n;
        reserved.get() - used.get() <= frac
    }
}
