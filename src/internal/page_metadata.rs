use crate::types::{mi_heap_t, mi_page_t, mi_tld_t};

#[inline]
pub(crate) unsafe fn mi_page_block_size_typed(
    page: *const mi_page_t,
) -> crate::core_types::BlockSize {
    unsafe { crate::core_types::PagePtr::new_unchecked(page.cast_mut()).block_size() }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_block_size(page: *const mi_page_t) -> usize {
    unsafe { mi_page_block_size_typed(page).get() }
}

/// # Safety
/// `page` must point to non-abandoned page metadata with a valid owning thread heap.
#[inline]
pub unsafe fn mi_page_tld(page: *const mi_page_t) -> *mut mi_tld_t {
    unsafe {
        crate::internal::mi_assert_internal!(!crate::allocator::page::mi_page_is_abandoned(page));
        crate::internal::mi_assert_internal!(!(*page).theap.is_null());
        (*(*page).theap).tld
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_heap(page: *const mi_page_t) -> *mut mi_heap_t {
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
pub unsafe fn mi_page_is_singleton(page: *const mi_page_t) -> bool {
    unsafe { crate::allocator::page::mi_page_reserved_count(page).get() == 1 }
}

#[inline]
pub(crate) unsafe fn mi_page_usable_block_size(
    page: *const mi_page_t,
) -> crate::core_types::UsableSize {
    unsafe { mi_page_block_size_typed(page).usable() }
}

/// Keep the infallible raw committed-size boundary on its historical
/// caller-proven multiplication. Propagating a fallible page-area result
/// through that compatibility surface is outside the PageArea migration.
#[inline]
unsafe fn mi_page_size_unchecked(page: *const mi_page_t) -> usize {
    // SAFETY: the caller proves initialized allocator page metadata whose
    // block size and reserved count describe a representable allocation.
    unsafe {
        mi_page_block_size(page) * crate::allocator::page::mi_page_reserved_count(page).as_usize()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub(crate) unsafe fn mi_page_committed_typed(
    page: *const mi_page_t,
) -> crate::core_types::CommittedBytes {
    unsafe {
        crate::core_types::CommittedBytes::new(if (*page).slice_committed == 0 {
            mi_page_size_unchecked(page)
        } else {
            (*page).slice_committed - super::page_location::mi_page_slice_offset_of(page, 0)
        })
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_committed(page: *const mi_page_t) -> usize {
    unsafe { mi_page_committed_typed(page).get() }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_all_free(page: *const mi_page_t) -> bool {
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::allocator::page::mi_page_used_count(page).get() == 0
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_immediate_available(page: *const mi_page_t) -> bool {
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
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let capacity = crate::allocator::page::mi_page_capacity_count(page);
        let reserved = crate::allocator::page::mi_page_reserved_count(page);
        crate::internal::mi_assert_internal!(capacity.get() <= reserved.get());
        capacity.get() < reserved.get()
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    unsafe {
        let full = crate::allocator::page::mi_page_reserved_count(page).get()
            == crate::allocator::page::mi_page_used_count(page).get();
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        crate::internal::mi_assert_internal!(!full || page_ptr.free().is_empty());
        full
    }
}

/// # Safety
/// `page` must be null or identify live initialized page metadata with stable,
/// readable `reserved` and `used` fields satisfying `used <= reserved`. The
/// caller must locally own the page or hold its abandoned-page claim while
/// reading these fields. Concurrent protocol remote-free publication is
/// permitted when it does not change either field.
#[inline]
pub unsafe fn mi_page_is_mostly_used(page: *const mi_page_t) -> bool {
    if page.is_null() {
        return true;
    }
    // SAFETY: the caller keeps the initialized page live and its count fields
    // stable while this snapshot projects them into typed scalar domains.
    let (reserved, used) = unsafe {
        let reserved = crate::allocator::page::mi_page_reserved_count(page);
        let used = crate::allocator::page::mi_page_used_count(page);
        (reserved, used)
    };
    let frac = reserved.get() / 8u16;
    reserved.get() - used.get() <= frac
}
