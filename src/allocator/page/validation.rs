use core::ffi::c_void;
use core::mem::size_of;

use crate::core_types::{PageConstPtr, PagePtr};
use crate::internal::{
    _mi_ptr_page, MI_DEBUG, mi_block_next, mi_mem_is_zero, mi_page_block_size,
    mi_page_contains_address, mi_page_slice_start, mi_page_thread_free, mi_page_usable_block_size,
};
use crate::track::MI_TRACK_ENABLED;
use crate::types::{mi_block_t, mi_page_t};

use super::{
    MI_LARGE_PAGE_SIZE, MI_PAGE_ALIGN, mi_page_capacity_count, mi_page_is_abandoned,
    mi_page_is_huge, mi_page_is_in_full, mi_page_queue_block_size_raw, mi_page_queue_contains,
    mi_page_queue_of, mi_page_reserved_count, mi_page_used_count,
};

/// # Safety
/// `page` must be valid, `page_start` must be that page's block area, and `i * block_size` must
/// stay within the reserved block range.
#[inline]
pub unsafe fn mi_page_block_at(
    page: *const mi_page_t,
    page_start: *mut c_void,
    block_size: usize,
    i: usize,
) -> *mut mi_block_t {
    // SAFETY[AUDIT:page-validation-helpers]: callers supply page metadata and
    // a block-area start for the same page; reserved-count bounds protect the
    // raw block offset calculation.
    unsafe {
        crate::internal::MI_UNUSED(&page);
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_internal!(i <= mi_page_reserved_count(page).as_usize());
        page_start
            .cast::<u8>()
            .add(i * block_size)
            .cast::<mi_block_t>()
    }
}

/// # Safety
/// `page` must be valid, and `head` must be null or a block list belonging to `page`.
#[inline]
pub(crate) unsafe fn mi_page_list_count(page: PagePtr, mut head: *mut mi_block_t) -> usize {
    // SAFETY[AUDIT:page-validation-helpers]: callers provide a typed page
    // witness and a block list for that page; traversal follows encoded links
    // only after page ownership assertions.
    unsafe {
        let page = page.as_ptr();
        crate::internal::mi_assert_internal!(_mi_ptr_page((*page).page_start.cast()) == page);
        let slice_start = mi_page_slice_start(page);
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
            slice_start.cast(),
            MI_PAGE_ALIGN
        ));
        let mut count = 0usize;
        while !head.is_null() {
            crate::internal::mi_assert_internal!(
                crate::core_types::ptr_addr_diff(head, slice_start) > MI_LARGE_PAGE_SIZE
                    || page == _mi_ptr_page(head.cast())
            );
            count += 1;
            head = mi_block_next(page, head);
        }
        count
    }
}

/// # Safety
/// `page` must be valid, and `p` must be null or a block list belonging to `page`.
#[inline]
pub(crate) unsafe fn mi_page_list_is_valid(page: PagePtr, mut p: *mut mi_block_t) -> bool {
    // SAFETY[AUDIT:page-validation-helpers]: callers provide a typed page
    // witness and a candidate block list; the helper bounds every block against
    // the page area before following the next link.
    unsafe {
        let page_ptr = page;
        let page = page_ptr.as_ptr();
        let psize = page_ptr.byte_size();
        let page_area = page_ptr.start().as_ptr().cast::<mi_block_t>();
        let end = page_area.cast::<u8>().add(psize).cast::<mi_block_t>();
        while !p.is_null() {
            if p < page_area || p >= end {
                return false;
            }
            p = mi_block_next(page, p);
        }
        if MI_DEBUG > 3 && (*page).free_is_zero {
            let ubsize = mi_page_usable_block_size(page);
            let mut block = page_ptr.free().as_raw();
            while !block.is_null() {
                crate::internal::mi_assert_expensive!(mi_mem_is_zero(
                    block.add(1).cast(),
                    ubsize.get() - size_of::<mi_block_t>()
                ));
                block = mi_block_next(page, block);
            }
        }
        true
    }
}

/// # Safety
/// `page` must point to page metadata initialized or being initialized by this allocator.
#[inline]
pub(crate) unsafe fn mi_page_is_valid_init(page: PagePtr) -> bool {
    // SAFETY[AUDIT:page-validation-helpers]: callers provide initialized page
    // metadata; the helper checks typed capacity/used/reserved domains and all
    // local block lists before reporting validity.
    unsafe {
        let page_ptr = page;
        let page = page.as_ptr();
        crate::internal::mi_assert_internal!(mi_page_block_size(page) > 0);
        let used = mi_page_used_count(page);
        let capacity = mi_page_capacity_count(page);
        let reserved = mi_page_reserved_count(page);
        crate::internal::mi_assert_internal!(used.get() <= capacity.get());
        crate::internal::mi_assert_internal!(capacity.get() <= reserved.get());
        let available = page_ptr.free();
        let local = page_ptr.local_free();
        crate::internal::mi_assert_internal!(mi_page_list_is_valid(page_ptr, available.as_raw()));
        crate::internal::mi_assert_internal!(mi_page_list_is_valid(page_ptr, local.as_raw()));

        if MI_DEBUG > 3 && (*page).free_is_zero {
            let ubsize = mi_page_usable_block_size(page);
            let mut block = available.as_raw();
            while !block.is_null() {
                crate::internal::mi_assert_expensive!(mi_mem_is_zero(
                    block.add(1).cast(),
                    ubsize.get() - size_of::<mi_block_t>()
                ));
                block = mi_block_next(page, block);
            }
        }

        if MI_TRACK_ENABLED == 0 {
            let tfree = mi_page_thread_free(page);
            crate::internal::mi_assert_internal!(mi_page_list_is_valid(page_ptr, tfree));
        }

        let free_count = mi_page_list_count(page_ptr, available.as_raw())
            + mi_page_list_count(page_ptr, local.as_raw());
        crate::internal::mi_assert_internal!(
            mi_page_used_count(page).as_usize() + free_count == capacity.as_usize()
        );
        true
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn _mi_page_is_valid(page: *mut mi_page_t) -> bool {
    // SAFETY[AUDIT:page-validation-helpers]: callers provide allocator-owned
    // page metadata; this wraps the pointer once, validates local invariants,
    // then checks owner queue membership for non-abandoned pages.
    unsafe {
        let page_ptr = PagePtr::new_unchecked(page);
        let page_const = PageConstPtr::new_unchecked(page);
        let cpage = page_const.as_ptr();

        crate::internal::mi_assert_internal!(mi_page_is_valid_init(page_ptr));
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        if crate::internal::MI_SECURE != 0 {
            let page = page_ptr.as_ptr();
            crate::internal::mi_assert_internal!((*page).keys[0] != 0);
        }
        if !mi_page_is_abandoned(cpage) {
            let pq = mi_page_queue_of(cpage);
            crate::internal::mi_assert_internal!(mi_page_queue_contains(pq, cpage));
            crate::internal::mi_assert_internal!(
                mi_page_queue_block_size_raw(pq) == mi_page_block_size(cpage)
                    || mi_page_is_huge(cpage)
                    || mi_page_is_in_full(cpage)
            );
            crate::internal::mi_assert_internal!(mi_page_contains_address(cpage, cpage.cast()));
        }
        true
    }
}
