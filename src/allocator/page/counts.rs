use core::mem::size_of;

use crate::core_types::{PagePtr, PageQueuePtr};
use crate::types::{mi_page_queue_t, mi_page_t};

use super::MI_LARGE_MAX_OBJ_SIZE;

/// # Safety
/// `pq` must be non-null and point to valid page-queue metadata.
#[inline(always)]
pub(super) unsafe fn page_queue_ptr(pq: *const mi_page_queue_t) -> PageQueuePtr {
    crate::internal::mi_assert_internal!(!pq.is_null());
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { PageQueuePtr::new_unchecked(pq.cast_mut()) }
}

/// # Safety
/// `page` must be non-null and point to valid page metadata.
#[inline(always)]
unsafe fn page_ptr(page: *const mi_page_t) -> PagePtr {
    crate::internal::mi_assert_internal!(!page.is_null());
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { PagePtr::new_unchecked(page.cast_mut()) }
}

/// # Safety
/// `pq` must point to valid page-queue metadata.
#[inline(always)]
pub(super) unsafe fn mi_page_queue_block_size_raw(pq: *const mi_page_queue_t) -> usize {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        page_queue_ptr(pq)
            .block_size()
            .map(crate::core_types::BlockSize::get)
            .unwrap_or(0)
    }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
// SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
pub unsafe extern "C" fn mi_page_queue_is_huge(pq: *const mi_page_queue_t) -> bool {
    unsafe { mi_page_queue_block_size_raw(pq) == (MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>()) }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
// SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
pub unsafe extern "C" fn mi_page_queue_is_full(pq: *const mi_page_queue_t) -> bool {
    unsafe {
        mi_page_queue_block_size_raw(pq) == (MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>()))
    }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
// SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
pub unsafe extern "C" fn mi_page_queue_is_special(pq: *const mi_page_queue_t) -> bool {
    unsafe { mi_page_queue_block_size_raw(pq) > MI_LARGE_MAX_OBJ_SIZE }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
// SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
pub unsafe extern "C" fn mi_page_queue_count(pq: *const mi_page_queue_t) -> usize {
    unsafe { mi_page_queue_count_typed(pq).get() }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
#[inline(always)]
pub(crate) unsafe fn mi_page_queue_count_typed(
    pq: *const mi_page_queue_t,
) -> crate::core_types::PageQueueCount {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_queue_ptr(pq).count() }
}

/// # Safety
/// `pq` must point to writable page-queue metadata.
#[inline(always)]
unsafe fn mi_page_queue_set_count(
    pq: *mut mi_page_queue_t,
    count: crate::core_types::PageQueueCount,
) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_queue_ptr(pq).set_count(count) };
}

/// # Safety
/// `pq` must point to writable page-queue metadata.
#[inline(always)]
pub(super) unsafe fn mi_page_queue_increment_count(pq: *mut mi_page_queue_t) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        mi_page_queue_set_count(pq, mi_page_queue_count_typed(pq).increment());
    }
}

/// # Safety
/// `pq` must point to writable page-queue metadata and have a nonzero count.
#[inline(always)]
pub(super) unsafe fn mi_page_queue_decrement_count(pq: *mut mi_page_queue_t) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        mi_page_queue_set_count(pq, mi_page_queue_count_typed(pq).decrement());
    }
}

/// # Safety
/// `pq` must point to writable page-queue metadata.
#[inline(always)]
pub(super) unsafe fn mi_page_queue_add_count(
    pq: *mut mi_page_queue_t,
    count: crate::core_types::PageQueueCount,
) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        mi_page_queue_set_count(pq, mi_page_queue_count_typed(pq).add(count));
    }
}

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_used_count(
    page: *const mi_page_t,
) -> crate::core_types::UsedBlockCount {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_ptr(page).used() }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_used_count(
    page: *mut mi_page_t,
    used: crate::core_types::UsedBlockCount,
) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_ptr(page).set_used(used) };
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_increment_used_count(
    page: *mut mi_page_t,
) -> crate::core_types::UsedBlockCount {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        let used = mi_page_used_count(page).increment_wrapping();
        mi_page_set_used_count(page, used);
        used
    }
}

/// # Safety
/// `page` must point to writable page metadata and have a nonzero used count.
#[inline(always)]
pub(crate) unsafe fn mi_page_decrement_used_count(
    page: *mut mi_page_t,
) -> crate::core_types::UsedBlockCount {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        let used = mi_page_used_count(page).decrement();
        mi_page_set_used_count(page, used);
        used
    }
}

/// # Safety
/// `page` must point to writable page metadata and have at least `count` used blocks.
#[inline(always)]
pub(crate) unsafe fn mi_page_subtract_used_count(
    page: *mut mi_page_t,
    count: u16,
) -> crate::core_types::UsedBlockCount {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        let used = mi_page_used_count(page).subtract(count);
        mi_page_set_used_count(page, used);
        used
    }
}

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_reserved_count(
    page: *const mi_page_t,
) -> crate::core_types::ReservedBlockCount {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_ptr(page).reserved() }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_reserved_count(
    page: *mut mi_page_t,
    reserved: crate::core_types::ReservedBlockCount,
) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_ptr(page).set_reserved(reserved) };
}

/// # Safety
/// `page` must point to valid page metadata satisfying `used <= capacity <= reserved`.
#[inline(always)]
pub(crate) unsafe fn mi_page_capacity_count(
    page: *const mi_page_t,
) -> crate::core_types::PageCapacity {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_ptr(page).capacity() }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_capacity_count(
    page: *mut mi_page_t,
    capacity: crate::core_types::PageCapacity,
) {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe { page_ptr(page).set_capacity(capacity) };
}

/// # Safety
/// `page` must point to writable page metadata, and `extend` must preserve
/// `used <= capacity + extend <= reserved`.
#[inline(always)]
pub(super) unsafe fn mi_page_add_capacity_count(
    page: *mut mi_page_t,
    extend: u16,
) -> crate::core_types::PageCapacity {
    // SAFETY[AUDIT:page-count-accessors]: Page queue/count helpers read and write allocator-owned page and queue metadata through typed counters.
    unsafe {
        let capacity = mi_page_capacity_count(page)
            .add(
                mi_page_used_count(page),
                extend,
                mi_page_reserved_count(page),
            )
            .unwrap_or_else(|| {
                crate::internal::mi_assert_internal!(false);
                crate::core_types::PageCapacity::new_unchecked(
                    mi_page_capacity_count(page).get().wrapping_add(extend),
                )
            });
        mi_page_set_capacity_count(page, capacity);
        capacity
    }
}
