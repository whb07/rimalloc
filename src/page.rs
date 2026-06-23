//! Page management

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;

use crate::arena::{
    _mi_arenas_collect, _mi_arenas_page_abandon, _mi_arenas_page_alloc, _mi_arenas_page_free_typed,
    MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE,
};
use crate::bits::{MI_KiB, MI_SIZE_SIZE};
use crate::core_types::{
    AvailableFreeList, BlockPtr, Errno, ForceCollect, LocalFreeList, LocalPage, OwnedAbandonedPage,
    PageConstPtr, PageKeys, PagePtr, PageQueuePtr, RemoteFreeList,
};
use crate::init::{_mi_page_empty, _mi_theap_empty, _mi_theap_empty_wrong};
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_increase, _mi_align_up, _mi_is_heap_main, _mi_os_commit,
    _mi_os_good_alloc_size, _mi_os_page_size_typed, _mi_page_malloc_zero_typed, _mi_ptr_page,
    _mi_theap_heap, _mi_theap_random_next, _mi_wsize_from_size, MI_DEBUG, MI_SECURE, mi_block_next,
    mi_block_set_next, mi_direct_bin_from_size, mi_mem_is_zero, mi_page_all_free, mi_page_area,
    mi_page_block_size, mi_page_committed_typed, mi_page_contains_address, mi_page_flags,
    mi_page_has_interior_pointers, mi_page_immediate_available, mi_page_is_expandable,
    mi_page_is_mostly_used, mi_page_is_singleton, mi_page_queue, mi_page_set_has_interior_pointers,
    mi_page_set_keys, mi_page_slice_offset_of, mi_page_slice_start, mi_page_start,
    mi_page_thread_free, mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::options::{
    _mi_error_message_fmt, _mi_option_get_fast, mi_option_get_clamp, mi_option_t,
};
use crate::stats::MI_BIN_HUGE;
use crate::track::{
    MI_TRACK_ENABLED, mi_track_mem_defined, mi_track_mem_noaccess, mi_track_mem_undefined,
};
use crate::types::{
    MI_BIN_FULL, MI_ENABLE_LARGE_PAGES, MI_LARGE_MAX_OBJ_WSIZE, MI_PAGE_MIN_COMMIT_SIZE,
    MI_SMALL_SIZE_MAX, MI_STAT, mi_block_t, mi_deferred_free_fun, mi_page_flags_t, mi_page_queue_t,
    mi_page_t, mi_theap_t,
};

pub use crate::types::MI_PADDING_SIZE;

static mut deferred_free: *mut c_void = ptr::null_mut();
static mut deferred_arg: *mut c_void = ptr::null_mut();

pub const MI_OS_PAGE_SIZE: usize = 4096;
pub const MI_SMALL_PAGE_SIZE: usize = MI_ARENA_SLICE_SIZE;
pub const MI_MEDIUM_PAGE_SIZE: usize = 8 * MI_SMALL_PAGE_SIZE;
pub const MI_LARGE_PAGE_SIZE: usize = MI_SIZE_SIZE * MI_MEDIUM_PAGE_SIZE;
pub const MI_IN_PAGE_C: () = ();
pub const MI_MAX_SLICE_SHIFT: usize = 6;
pub const MI_MIN_SLICES: usize = 2;
pub const MI_MAX_EXTEND_SIZE: usize = 4 * 1024;
pub const MI_MIN_EXTEND: usize = if MI_SECURE != 0 { 8 * MI_SECURE } else { 1 };
pub const MI_PAGE_OSPAGE_BLOCK_ALIGN2: usize = 4 * MI_KiB;
pub const MI_SMALL_MAX_OBJ_SIZE: usize = (MI_SMALL_PAGE_SIZE - MI_PAGE_OSPAGE_BLOCK_ALIGN2) / 6;
pub const MI_MEDIUM_MAX_OBJ_SIZE: usize = if MI_ENABLE_LARGE_PAGES != 0 {
    (MI_MEDIUM_PAGE_SIZE - MI_PAGE_OSPAGE_BLOCK_ALIGN2) / 6
} else {
    MI_MEDIUM_PAGE_SIZE / 8
};
pub const MI_LARGE_MAX_OBJ_SIZE: usize = MI_LARGE_MAX_OBJ_WSIZE * size_of::<usize>();
pub const MI_MAX_RETIRE_SIZE: usize = MI_LARGE_MAX_OBJ_SIZE;
pub const MI_MAX_ALLOC_SIZE: usize = isize::MAX as usize;

const MI_ALIGN4W: bool = cfg!(target_pointer_width = "32");
const MI_ALIGN2W: bool = cfg!(target_pointer_width = "64");
const MI_PAGE_ALIGN: usize = MI_ARENA_SLICE_ALIGN;

fn align_up(size: usize, alignment: usize) -> usize {
    crate::mi_debug_assert!(alignment.is_power_of_two());
    (size + alignment - 1) & !(alignment - 1)
}

#[inline(always)]
pub(crate) fn mi_bin_impl(size: usize) -> usize {
    let mut wsize = size.div_ceil(size_of::<usize>());
    if MI_ALIGN4W {
        if wsize <= 4 {
            return if wsize <= 1 { 1 } else { (wsize + 1) & !1 };
        }
    } else if MI_ALIGN2W {
        if wsize <= 8 {
            return if wsize <= 1 { 1 } else { (wsize + 1) & !1 };
        }
    } else if wsize <= 8 {
        return if wsize == 0 { 1 } else { wsize };
    }

    if wsize > MI_LARGE_MAX_OBJ_WSIZE {
        return MI_BIN_HUGE;
    }

    if MI_ALIGN4W && wsize <= 16 {
        wsize = (wsize + 3) & !3;
    }

    wsize -= 1;
    let b = (usize::BITS as usize - 1) - wsize.leading_zeros() as usize;
    let bin = ((b << 2) + ((wsize >> (b - 2)) & 0x03)) - 3;
    crate::mi_debug_assert!(bin > 0 && bin < MI_BIN_HUGE);
    bin
}

#[inline(never)]
pub extern "C" fn mi_bin(size: usize) -> usize {
    mi_bin_impl(size)
}

#[inline(never)]
pub extern "C" fn _mi_bin(size: usize) -> usize {
    mi_bin_impl(size)
}

#[inline(always)]
fn mi_bin_from_raw_size(size: usize) -> crate::core_types::Bin {
    let bin = mi_bin_impl(size);
    crate::mi_debug_assert!(u16::try_from(bin).is_ok());
    crate::mi_debug_assert!(bin < crate::types::MI_BIN_COUNT);
    unsafe { crate::core_types::Bin::new_unchecked(bin as u16) }
}

#[inline(always)]
pub(crate) fn mi_bin_from_padded(padded: crate::core_types::PaddedSize) -> crate::core_types::Bin {
    mi_bin_from_raw_size(padded.get())
}

#[inline(always)]
pub(crate) fn mi_bin_from_block_size(
    block_size: crate::core_types::BlockSize,
) -> crate::core_types::Bin {
    mi_bin_from_raw_size(block_size.get())
}

#[inline(always)]
pub(crate) fn mi_bin_from_usable(usable: crate::core_types::UsableSize) -> crate::core_types::Bin {
    mi_bin_from_raw_size(usable.get())
}

/// # Safety
/// `pq` must be non-null and point to valid page-queue metadata.
#[inline(always)]
unsafe fn page_queue_ptr(pq: *const mi_page_queue_t) -> PageQueuePtr {
    crate::mi_assert_internal!(!pq.is_null());
    unsafe { PageQueuePtr::new_unchecked(pq.cast_mut()) }
}

/// # Safety
/// `pq` must point to valid page-queue metadata.
#[inline(always)]
unsafe fn mi_page_queue_block_size_raw(pq: *const mi_page_queue_t) -> usize {
    unsafe {
        page_queue_ptr(pq)
            .block_size()
            .map(crate::core_types::BlockSize::get)
            .unwrap_or(0)
    }
}

#[inline(never)]
pub extern "C" fn _mi_bin_size(bin: usize) -> usize {
    crate::mi_debug_assert!(bin <= MI_BIN_HUGE);
    unsafe { (*ptr::addr_of!(_mi_theap_empty).cast::<mi_theap_t>()).pages[bin].block_size }
}

#[inline(always)]
fn mi_good_size_small(padded: crate::core_types::PaddedSize) -> crate::core_types::GoodSize {
    crate::core_types::GoodSize::new(_mi_bin_size(mi_bin_from_padded(padded).as_usize()))
}

pub(crate) fn mi_good_size_typed(
    size: crate::core_types::RequestedSize,
) -> crate::core_types::GoodSize {
    let sz = size.get();
    if sz <= MI_LARGE_MAX_OBJ_SIZE {
        // `sz <= MI_LARGE_MAX_OBJ_SIZE` is far below `usize::MAX`, so padding
        // cannot overflow; `checked_padded` makes that explicit rather than open-
        // coding `sz + MI_PADDING_SIZE`.
        let Ok(padded) = size.checked_padded() else {
            return crate::core_types::GoodSize::new(sz);
        };
        mi_good_size_small(padded)
    } else if sz <= MI_MAX_ALLOC_SIZE {
        let Ok(padded) = size.checked_padded() else {
            return crate::core_types::GoodSize::new(sz);
        };
        crate::core_types::GoodSize::new(align_up(padded.get(), crate::internal::MI_OS_PAGE_SIZE))
    } else {
        crate::core_types::GoodSize::new(sz)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_good_size(size: usize) -> usize {
    mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_is_huge(pq: *const mi_page_queue_t) -> bool {
    unsafe { mi_page_queue_block_size_raw(pq) == (MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>()) }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_is_full(pq: *const mi_page_queue_t) -> bool {
    unsafe {
        mi_page_queue_block_size_raw(pq) == (MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>()))
    }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_is_special(pq: *const mi_page_queue_t) -> bool {
    unsafe { mi_page_queue_block_size_raw(pq) > MI_LARGE_MAX_OBJ_SIZE }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_count(pq: *const mi_page_queue_t) -> usize {
    unsafe { mi_page_queue_count_typed(pq).get() }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
#[inline(always)]
pub(crate) unsafe fn mi_page_queue_count_typed(
    pq: *const mi_page_queue_t,
) -> crate::core_types::PageQueueCount {
    unsafe { page_queue_ptr(pq).count() }
}

/// # Safety
/// `pq` must point to writable page-queue metadata.
#[inline(always)]
unsafe fn mi_page_queue_set_count(
    pq: *mut mi_page_queue_t,
    count: crate::core_types::PageQueueCount,
) {
    unsafe { page_queue_ptr(pq).set_count(count) };
}

/// # Safety
/// `pq` must point to writable page-queue metadata.
#[inline(always)]
unsafe fn mi_page_queue_increment_count(pq: *mut mi_page_queue_t) {
    unsafe {
        mi_page_queue_set_count(pq, mi_page_queue_count_typed(pq).increment());
    }
}

/// # Safety
/// `pq` must point to writable page-queue metadata and have a nonzero count.
#[inline(always)]
unsafe fn mi_page_queue_decrement_count(pq: *mut mi_page_queue_t) {
    unsafe {
        mi_page_queue_set_count(pq, mi_page_queue_count_typed(pq).decrement());
    }
}

/// # Safety
/// `pq` must point to writable page-queue metadata.
#[inline(always)]
unsafe fn mi_page_queue_add_count(
    pq: *mut mi_page_queue_t,
    count: crate::core_types::PageQueueCount,
) {
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
    unsafe { crate::core_types::UsedBlockCount::new((*page).used) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_used_count(
    page: *mut mi_page_t,
    used: crate::core_types::UsedBlockCount,
) {
    unsafe {
        (*page).used = used.get();
    }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_increment_used_count(
    page: *mut mi_page_t,
) -> crate::core_types::UsedBlockCount {
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
    unsafe { crate::core_types::ReservedBlockCount::new((*page).reserved) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_reserved_count(
    page: *mut mi_page_t,
    reserved: crate::core_types::ReservedBlockCount,
) {
    unsafe {
        (*page).reserved = reserved.get();
    }
}

/// # Safety
/// `page` must point to valid page metadata satisfying `used <= capacity <= reserved`.
#[inline(always)]
pub(crate) unsafe fn mi_page_capacity_count(
    page: *const mi_page_t,
) -> crate::core_types::PageCapacity {
    unsafe {
        let used = mi_page_used_count(page);
        let reserved = mi_page_reserved_count(page);
        let capacity = (*page).capacity;
        crate::mi_assert_internal!(used.get() <= capacity);
        crate::mi_assert_internal!(capacity <= reserved.get());
        // SAFETY: asserted above; release keeps the same raw read shape.
        crate::core_types::PageCapacity::new(used, capacity, reserved)
            .unwrap_or_else(|| crate::core_types::PageCapacity::new_unchecked(capacity))
    }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_capacity_count(
    page: *mut mi_page_t,
    capacity: crate::core_types::PageCapacity,
) {
    unsafe {
        (*page).capacity = capacity.get();
    }
}

/// # Safety
/// `page` must point to writable page metadata, and `extend` must preserve
/// `used <= capacity + extend <= reserved`.
#[inline(always)]
unsafe fn mi_page_add_capacity_count(
    page: *mut mi_page_t,
    extend: u16,
) -> crate::core_types::PageCapacity {
    unsafe {
        let capacity = mi_page_capacity_count(page)
            .add(
                mi_page_used_count(page),
                extend,
                mi_page_reserved_count(page),
            )
            .unwrap_or_else(|| {
                crate::mi_assert_internal!(false);
                crate::core_types::PageCapacity::new_unchecked(
                    (*page).capacity.wrapping_add(extend),
                )
            });
        mi_page_set_capacity_count(page, capacity);
        capacity
    }
}

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_slice_committed(
    page: *const mi_page_t,
) -> crate::core_types::CommittedBytes {
    unsafe { crate::core_types::CommittedBytes::new((*page).slice_committed) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_slice_committed(
    page: *mut mi_page_t,
    committed: crate::core_types::CommittedBytes,
) {
    unsafe {
        (*page).slice_committed = committed.get();
    }
}

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_retire_expire(
    page: *const mi_page_t,
) -> crate::core_types::RetireExpire {
    unsafe { crate::core_types::RetireExpire::new((*page).retire_expire) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_retire_expire(
    page: *mut mi_page_t,
    expire: crate::core_types::RetireExpire,
) {
    unsafe {
        (*page).retire_expire = expire.get();
    }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_reset_retire_expire(page: *mut mi_page_t) {
    unsafe {
        mi_page_set_retire_expire(page, crate::core_types::RetireExpire::zero());
    }
}

/// # Safety
/// `page` must point to writable page metadata with a nonzero retire countdown.
#[inline(always)]
unsafe fn mi_page_decrement_retire_expire(page: *mut mi_page_t) -> crate::core_types::RetireExpire {
    unsafe {
        let expire = mi_page_retire_expire(page).decrement();
        mi_page_set_retire_expire(page, expire);
        expire
    }
}

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
    unsafe {
        crate::MI_UNUSED!(page);
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_internal!(i <= mi_page_reserved_count(page).as_usize());
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
    unsafe {
        let page = page.as_ptr();
        crate::mi_assert_internal!(_mi_ptr_page((*page).page_start.cast()) == page);
        let slice_start = mi_page_slice_start(page);
        crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
            slice_start.cast(),
            MI_PAGE_ALIGN
        ));
        let mut count = 0usize;
        while !head.is_null() {
            crate::mi_assert_internal!(
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
    unsafe {
        let page = page.as_ptr();
        let mut psize = 0usize;
        let page_area = mi_page_area(page, &mut psize).cast::<mi_block_t>();
        let end = page_area.cast::<u8>().add(psize).cast::<mi_block_t>();
        while !p.is_null() {
            if p < page_area || p >= end {
                return false;
            }
            p = mi_block_next(page, p);
        }
        if MI_DEBUG > 3 && (*page).free_is_zero {
            let ubsize = mi_page_usable_block_size(page);
            let mut block = (*page).free;
            while !block.is_null() {
                crate::mi_assert_expensive!(mi_mem_is_zero(
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
    unsafe {
        let page_ptr = page;
        let page = page.as_ptr();
        crate::mi_assert_internal!(mi_page_block_size(page) > 0);
        let used = mi_page_used_count(page);
        let capacity = mi_page_capacity_count(page);
        let reserved = mi_page_reserved_count(page);
        crate::mi_assert_internal!(used.get() <= capacity.get());
        crate::mi_assert_internal!(capacity.get() <= reserved.get());
        crate::mi_assert_internal!(mi_page_list_is_valid(page_ptr, (*page).free));
        crate::mi_assert_internal!(mi_page_list_is_valid(page_ptr, (*page).local_free));

        if MI_DEBUG > 3 && (*page).free_is_zero {
            let ubsize = mi_page_usable_block_size(page);
            let mut block = (*page).free;
            while !block.is_null() {
                crate::mi_assert_expensive!(mi_mem_is_zero(
                    block.add(1).cast(),
                    ubsize.get() - size_of::<mi_block_t>()
                ));
                block = mi_block_next(page, block);
            }
        }

        if MI_TRACK_ENABLED == 0 {
            let tfree = mi_page_thread_free(page);
            crate::mi_assert_internal!(mi_page_list_is_valid(page_ptr, tfree));
        }

        let free_count = mi_page_list_count(page_ptr, (*page).free)
            + mi_page_list_count(page_ptr, (*page).local_free);
        crate::mi_assert_internal!(
            mi_page_used_count(page).as_usize() + free_count == capacity.as_usize()
        );
        true
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn _mi_page_is_valid(page: *mut mi_page_t) -> bool {
    unsafe {
        let page_ptr = PagePtr::new_unchecked(page);
        let page_const = PageConstPtr::new_unchecked(page);
        let cpage = page_const.as_ptr();

        crate::mi_assert_internal!(mi_page_is_valid_init(page_ptr));
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        if MI_SECURE != 0 {
            let page = page_ptr.as_ptr();
            crate::mi_assert_internal!((*page).keys[0] != 0);
        }
        if !mi_page_is_abandoned(cpage) {
            let pq = mi_page_queue_of(cpage);
            crate::mi_assert_internal!(mi_page_queue_contains(pq, cpage));
            crate::mi_assert_internal!(
                mi_page_queue_block_size_raw(pq) == mi_page_block_size(cpage)
                    || mi_page_is_huge(cpage)
                    || mi_page_is_in_full(cpage)
            );
            crate::mi_assert_internal!(mi_page_contains_address(cpage, cpage.cast()));
        }
        true
    }
}

/// # Safety
/// `page` must be valid, and `head` must be a detached thread-free list for that page.
#[inline]
pub unsafe fn mi_page_thread_collect_to_local(page: *mut mi_page_t, head: *mut mi_block_t) {
    unsafe {
        if head.is_null() {
            return;
        }

        let page_ptr = PagePtr::new_unchecked(page);
        let remote =
            RemoteFreeList::from_detached_thread_free_head(Some(BlockPtr::new_unchecked(head)));
        let local = LocalFreeList::from_raw((*page).local_free);
        let owned_page = OwnedAbandonedPage::new_unchecked(page_ptr);
        let _ = mi_page_thread_collect_to_local_typed(owned_page, remote, local);
    }
}

/// # Safety
/// `page` must be valid, and `remote` must be a detached thread-free list for that page.
#[inline]
pub(crate) unsafe fn mi_page_thread_collect_to_local_typed(
    page: OwnedAbandonedPage,
    remote: RemoteFreeList,
    local: LocalFreeList,
) -> LocalFreeList {
    unsafe {
        let page_ptr = page.page();
        let page = page_ptr.as_ptr();
        let Some(head_block) = remote.head() else {
            return local;
        };

        let capacity = mi_page_capacity_count(page);
        let max_count = capacity.as_usize();
        let mut count = 1usize;
        let head = head_block.as_ptr();
        let mut last = head;
        loop {
            let next = mi_block_next(page, last);
            if next.is_null() || count > max_count {
                break;
            }
            count += 1;
            last = next;
        }

        if count > max_count {
            _mi_error_message_fmt(Errno::EFAULT, format_args!("corrupted thread-free list\n"));
            return local;
        }
        let used = mi_page_used_count(page);
        if count > used.as_usize() {
            _mi_error_message_fmt(
                Errno::EFAULT,
                format_args!("corrupted meta-data in thread-free list\n"),
            );
            return local;
        }

        mi_block_set_next(page, last, local.as_raw());
        let new_local = LocalFreeList(Some(head_block));
        (*page).local_free = new_local.as_raw();

        crate::mi_assert_internal!(u16::try_from(count).is_ok());
        crate::mi_assert_internal!(used.get() >= count as u16);
        mi_page_subtract_used_count(page, count as u16);
        new_local
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub unsafe fn mi_page_thread_free_collect(page: *mut mi_page_t) {
    unsafe {
        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = LocalPage::new_unchecked(page_ptr);
        let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(
            ptr::addr_of_mut!((*page).xthread_free),
        ));
        let remote = atomic_tf.collect_acq_rel();
        let Some(head) = remote.head().map(|b| b.as_ptr()) else {
            return;
        };
        crate::mi_assert_internal!(!head.is_null());
        let remote =
            RemoteFreeList::from_detached_thread_free_head(Some(BlockPtr::new_unchecked(head)));
        let _ = mi_page_thread_free_collect_typed(local_page, remote);
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub(crate) unsafe fn mi_page_thread_free_collect_typed(
    page: LocalPage,
    remote: RemoteFreeList,
) -> LocalFreeList {
    unsafe {
        let page_ptr = page.page();
        let page = page_ptr.as_ptr();
        let local = LocalFreeList::from_raw((*page).local_free);
        let owned_page = OwnedAbandonedPage::new_unchecked(page_ptr);
        mi_page_thread_collect_to_local_typed(owned_page, remote, local)
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub unsafe fn mi_page_free_quick_collect(page: *mut mi_page_t) -> bool {
    unsafe {
        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = LocalPage::new_unchecked(page_ptr);
        let available = AvailableFreeList::from_raw((*page).free);
        let local = LocalFreeList::from_raw((*page).local_free);
        mi_page_free_quick_collect_typed(local_page, available, local)
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub(crate) unsafe fn mi_page_free_quick_collect_typed(
    page: LocalPage,
    available: AvailableFreeList,
    local: LocalFreeList,
) -> bool {
    unsafe {
        if available.0.is_some() {
            return true;
        }
        let Some(local_head) = local.0 else {
            return false;
        };
        let page = page.page().as_ptr();
        let new_available = AvailableFreeList(Some(local_head));
        let new_local = LocalFreeList(None);
        (*page).free = new_available.as_raw();
        (*page).local_free = new_local.as_raw();
        (*page).free_is_zero = false;
        true
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_free_collect(page: *mut mi_page_t, force: bool) {
    unsafe {
        crate::mi_assert_internal!(!page.is_null());

        mi_page_thread_free_collect(page);

        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = LocalPage::new_unchecked(page_ptr);
        let available = AvailableFreeList::from_raw((*page).free);
        let local = LocalFreeList::from_raw((*page).local_free);
        let moved_local = local.0.is_some() && (available.0.is_none() || force);
        let force_collect = if force {
            ForceCollect::Yes
        } else {
            ForceCollect::No
        };
        let (available, local) = local.collect_into_available(local_page, available, force_collect);
        (*page).free = available.as_raw();
        (*page).local_free = local.as_raw();
        if moved_local {
            (*page).free_is_zero = false;
        }

        crate::mi_assert_internal!(!force || (*page).local_free.is_null());
    }
}

/// # Safety
/// `page` must be valid, and `head` must be null or a detached block list belonging to `page`.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_free_collect_partly(page: *mut mi_page_t, head: *mut mi_block_t) {
    unsafe {
        if head.is_null() {
            return;
        }
        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = LocalPage::new_unchecked(page_ptr);
        let remote_head =
            RemoteFreeList::from_detached_thread_free_head(Some(BlockPtr::new_unchecked(head)));
        _mi_page_free_collect_partly_typed(local_page, remote_head);
    }
}

/// # Safety
/// `page` must be valid, and `remote_head` must be a detached remote-free head for `page`.
#[inline]
pub(crate) unsafe fn _mi_page_free_collect_partly_typed(
    local_page: LocalPage,
    remote_head: RemoteFreeList,
) {
    unsafe {
        let page_ptr = local_page.page();
        let page = page_ptr.as_ptr();
        let Some(head_block) = remote_head.head() else {
            return;
        };
        let head = head_block.as_ptr();
        let next = mi_block_next(page, head);
        if !next.is_null() {
            mi_block_set_next(page, head, ptr::null());
            let remote_tail =
                RemoteFreeList::from_detached_thread_free_head(Some(BlockPtr::new_unchecked(next)));
            let local = LocalFreeList::from_raw((*page).local_free);
            let owned_page = OwnedAbandonedPage::new_unchecked(page_ptr);
            let _ = mi_page_thread_collect_to_local_typed(owned_page, remote_tail, local);
            if !(*page).local_free.is_null() && (*page).free.is_null() {
                let available = AvailableFreeList::from_raw((*page).free);
                let local = LocalFreeList::from_raw((*page).local_free);
                let _ = mi_page_free_quick_collect_typed(local_page, available, local);
            }
        }
        if mi_page_used_count(page).get() == 1 {
            let atomic_tf = crate::core_types::AtomicThreadFree(
                crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*page).xthread_free)),
            );
            crate::mi_assert_internal!(
                atomic_tf
                    .load_relaxed()
                    .head()
                    .map(|block| block.as_ptr())
                    .unwrap_or(ptr::null_mut())
                    == head
            );
            crate::mi_assert_internal!(mi_block_next(page, head).is_null());
            _mi_page_free_collect(page, false);
        }
    }
}

/// # Safety
/// `page` must point to a page currently owned by a full queue.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_unfull(page: *mut mi_page_t) {
    unsafe {
        let _ = _mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked(
            crate::core_types::PagePtr::new_unchecked(page),
        ));
    }
}

#[inline(never)]
pub(crate) unsafe fn _mi_page_unfull_internal(
    page_wrapper: crate::core_types::FullPage,
) -> crate::core_types::ActivePage {
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::mi_assert_internal!(mi_page_is_in_full(page));
        crate::mi_assert_internal!(!(*mi_page_theap(page)).allow_page_abandon);
        if !mi_page_is_in_full(page) {
            return crate::core_types::ActivePage::new_unchecked(page_wrapper.page());
        }

        let theap = mi_page_theap(page);
        let pqfull = ptr::addr_of_mut!((*theap).pages[MI_BIN_FULL]);
        mi_page_set_in_full(page, false);
        let pq = mi_theap_page_queue_of(theap, page);
        mi_page_set_in_full(page, true);
        let from = crate::core_types::FullPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pqfull),
        );
        let Some(queued) =
            crate::core_types::QueuedPage::<crate::core_types::FullQueue>::new(page_wrapper, from)
        else {
            crate::mi_assert_internal!(false);
            return crate::core_types::ActivePage::new_unchecked(page_wrapper.page());
        };
        let to = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        mi_page_unfull_typed(queued, to)
    }
}

#[inline]
unsafe fn mi_page_to_full(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    unsafe {
        crate::mi_assert_internal!(pq == mi_page_queue_of(page));
        crate::mi_assert_internal!(!mi_page_immediate_available(page));
        crate::mi_assert_internal!(!mi_page_is_in_full(page));

        let theap = mi_page_theap(page);
        if (*theap).allow_page_abandon {
            let active = crate::core_types::ActivePage::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let _ = _mi_page_abandon_internal(
                active.for_abandon(),
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            );
        } else if !mi_page_is_in_full(page) {
            let full = ptr::addr_of_mut!((*theap).pages[MI_BIN_FULL]);
            mi_page_move_to_full_typed(
                crate::core_types::ActivePage::new_unchecked(
                    crate::core_types::PagePtr::new_unchecked(page),
                ),
                crate::core_types::NormalPageQueue::new_unchecked(
                    crate::core_types::PageQueuePtr::new_unchecked(pq),
                ),
                crate::core_types::FullPageQueue::new_unchecked(
                    crate::core_types::PageQueuePtr::new_unchecked(full),
                ),
            );
            _mi_page_free_collect(page, false);
        }
    }
}

#[inline]
unsafe fn mi_page_move_to_full_typed(
    page: crate::core_types::ActivePage,
    from: crate::core_types::NormalPageQueue,
    to: crate::core_types::FullPageQueue,
) -> crate::core_types::FullPage {
    unsafe { page.to_full(from, to) }
}

#[inline]
unsafe fn mi_page_unfull_typed(
    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,
    to: crate::core_types::NormalPageQueue,
) -> crate::core_types::ActivePage {
    unsafe { mi_page_queue_enqueue_from_full(to, page) }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_free(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    unsafe {
        if mi_page_queue_is_full(pq) {
            if !mi_page_is_in_full(page) {
                mi_page_set_in_full(page, true);
            }
            let queue = crate::core_types::FullPageQueue::new_unchecked(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            );
            let full = crate::core_types::FullPage::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let Some(all_free) = full.mark_all_free() else {
                crate::mi_assert_internal!(false);
                return;
            };
            let _ = _mi_page_free_full_internal(all_free, queue);
            return;
        }

        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        let active = crate::core_types::ActivePage::new_unchecked(
            crate::core_types::PagePtr::new_unchecked(page),
        );
        let Some(queued) =
            crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new(active, queue)
        else {
            crate::mi_assert_internal!(false);
            return;
        };
        let Some(all_free) = active.mark_all_free() else {
            crate::mi_assert_internal!(false);
            return;
        };
        let _ = _mi_page_free_active_internal(all_free, queued);
    }
}

pub(crate) unsafe fn _mi_page_free_active_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,
    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,
) -> crate::core_types::DestroyedPage {
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_internal!(queued.page().as_ptr() == page);
        crate::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::mi_assert_internal!(mi_page_all_free(page));

        mi_page_set_has_interior_pointers(page_wrapper.page(), false);
        let _ = queued.remove();

        _mi_page_free_after_remove(page_wrapper)
    }
}

pub(crate) unsafe fn _mi_page_free_full_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,
    queue: crate::core_types::FullPageQueue,
) -> crate::core_types::DestroyedPage {
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let pq = queue.ptr().as_ptr();
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::mi_assert_internal!(pq == mi_page_queue_of(page));
        crate::mi_assert_internal!(mi_page_all_free(page));

        mi_page_set_has_interior_pointers(page_wrapper.page(), false);
        let full = crate::core_types::FullPage::new_unchecked(page_wrapper.page());
        let Some(queued) =
            crate::core_types::QueuedPage::<crate::core_types::FullQueue>::new(full, queue)
        else {
            crate::mi_assert_internal!(false);
            mi_page_queue_remove_raw(pq, page);
            return _mi_page_free_after_remove(page_wrapper);
        };
        let _ = queued.remove();

        _mi_page_free_after_remove(page_wrapper)
    }
}

unsafe fn _mi_page_free_after_remove<S>(
    page_wrapper: crate::core_types::AllFreePage<S>,
) -> crate::core_types::DestroyedPage {
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let theap = mi_page_theap(page);
        crate::mi_assert_internal!(!theap.is_null());
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_set_theap(page, ptr::null_mut());
        let destroyed = _mi_arenas_page_free_typed(page_wrapper, owner);
        _mi_arenas_collect(false, false, (*theap).tld);
        destroyed
    }
}

const MI_RETIRE_CYCLES: u8 = 16;

pub(crate) unsafe extern "C" fn _mi_page_retire(page: *mut mi_page_t) {
    unsafe {
        let active = crate::core_types::ActivePage::new_unchecked(
            crate::core_types::PagePtr::new_unchecked(page),
        );
        let Some(all_free) = active.mark_all_free() else {
            crate::mi_assert_internal!(false);
            return;
        };
        _mi_page_retire_internal(all_free);
    }
}

pub(crate) unsafe fn _mi_page_retire_internal(
    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,
) {
    unsafe {
        let page = page_wrapper.page().as_ptr();
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_expensive!(_mi_page_is_valid(page));
        crate::mi_assert_internal!(mi_page_all_free(page));

        if !mi_page_retire_expire(page).is_zero() {
            return;
        }
        mi_page_set_has_interior_pointers(page_wrapper.page(), false);

        let pq = mi_page_queue_of(page);
        let queue = page_queue_ptr(pq);
        if !mi_page_queue_is_special(pq)
            && queue
                .first()
                .is_some_and(|first| ptr::eq(first.as_ptr(), page))
            && queue
                .last()
                .is_some_and(|last| ptr::eq(last.as_ptr(), page))
        {
            let theap = mi_page_theap(page);
            if MI_STAT > 0 {
                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);
            }
            let retired = page_wrapper.retire();
            crate::mi_assert_internal!(ptr::eq(retired.page().as_ptr(), page));
            let bsize = mi_page_block_size(page);
            mi_page_set_retire_expire(
                page,
                crate::core_types::RetireExpire::new(if bsize <= MI_SMALL_MAX_OBJ_SIZE {
                    MI_RETIRE_CYCLES
                } else {
                    MI_RETIRE_CYCLES / 4
                }),
            );
            let index = pq.offset_from(ptr::addr_of!((*theap).pages[0])) as usize;
            crate::mi_assert_internal!(index < MI_BIN_FULL);
            if index < (*theap).page_retired_min {
                (*theap).page_retired_min = index;
            }
            if index > (*theap).page_retired_max {
                (*theap).page_retired_max = index;
            }
            crate::mi_assert_internal!(mi_page_all_free(page));
            return;
        }

        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        let active = crate::core_types::ActivePage::new_unchecked(page_wrapper.page());
        let Some(queued) =
            crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new(active, queue)
        else {
            if mi_page_queue_contains(pq, page) {
                mi_page_queue_remove_raw(pq, page);
                let _ = _mi_page_free_after_remove(page_wrapper);
            }
            return;
        };
        let _ = _mi_page_free_active_internal(page_wrapper, queued);
    }
}

pub(crate) unsafe extern "C" fn _mi_theap_collect_retired(theap: *mut mi_theap_t, force: bool) {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        _mi_theap_collect_retired_typed(theap, force);
    }
}

#[inline]
unsafe fn _mi_theap_collect_retired_typed(theap: crate::core_types::ThreadHeapLocal, force: bool) {
    unsafe {
        let theap = theap.ptr().as_ptr();
        let mut min = MI_BIN_FULL;
        let mut max = 0usize;
        for bin in (*theap).page_retired_min..=(*theap).page_retired_max {
            let pq = ptr::addr_of_mut!((*theap).pages[bin]);
            let page = page_queue_ptr(pq)
                .first()
                .map(PagePtr::as_ptr)
                .unwrap_or(ptr::null_mut());
            if !page.is_null() && !mi_page_retire_expire(page).is_zero() {
                if mi_page_all_free(page) {
                    let expire = mi_page_decrement_retire_expire(page);
                    if expire.is_zero() || force {
                        let active = crate::core_types::ActivePage::new_unchecked(
                            crate::core_types::PagePtr::new_unchecked(page),
                        );
                        let Some(all_free) = active.mark_all_free() else {
                            crate::mi_assert_internal!(false);
                            continue;
                        };
                        let queue = crate::core_types::NormalPageQueue::new_unchecked(
                            crate::core_types::PageQueuePtr::new_unchecked(pq),
                        );
                        let active = crate::core_types::ActivePage::new_unchecked(all_free.page());
                        let Some(queued) = crate::core_types::QueuedPage::<
                            crate::core_types::NormalQueue,
                        >::new(active, queue) else {
                            if mi_page_queue_contains(pq, page) {
                                mi_page_queue_remove_raw(pq, page);
                                let _ = _mi_page_free_after_remove(all_free);
                            }
                            continue;
                        };
                        let _ = _mi_page_free_active_internal(all_free, queued);
                    } else {
                        if bin < min {
                            min = bin;
                        }
                        if bin > max {
                            max = bin;
                        }
                    }
                } else {
                    mi_page_reset_retire_expire(page);
                }
            }
        }
        (*theap).page_retired_min = min;
        (*theap).page_retired_max = max;
    }
}

#[inline]
unsafe fn mi_page_free_list_extend_secure(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    bsize: usize,
    extend: usize,
) {
    unsafe {
        mi_page_free_list_extend_secure_typed(
            theap,
            crate::core_types::PagePtr::new_unchecked(page),
            crate::core_types::BlockSize::new_unchecked(bsize),
            extend,
        );
    }
}

#[inline]
unsafe fn mi_page_free_list_extend_secure_typed(
    theap: *mut mi_theap_t,
    page: crate::core_types::PagePtr,
    bsize: crate::core_types::BlockSize,
    extend: usize,
) {
    unsafe {
        let page_raw = page.as_ptr();
        let bsize = bsize.get();
        if MI_SECURE < 3 {
            crate::mi_assert_internal!(page.free().0.is_none());
            crate::mi_assert_internal!(page.local_free().0.is_none());
        }
        let capacity = page.capacity();
        crate::mi_assert_internal!(capacity.as_usize() + extend <= page.reserved().as_usize());
        crate::mi_assert_internal!(bsize == page.block_size().get());
        let page_area = mi_page_start(page_raw).cast::<c_void>();

        let mut shift = MI_MAX_SLICE_SHIFT;
        while (extend >> shift) == 0 {
            shift -= 1;
        }
        let slice_count = 1usize << shift;
        let slice_extend = extend / slice_count;
        crate::mi_assert_internal!(slice_extend >= 1);

        let mut blocks = [ptr::null_mut::<mi_block_t>(); 1 << MI_MAX_SLICE_SHIFT];
        let mut counts = [0usize; 1 << MI_MAX_SLICE_SHIFT];
        for i in 0..slice_count {
            blocks[i] = mi_page_block_at(
                page_raw,
                page_area,
                bsize,
                capacity.as_usize() + i * slice_extend,
            );
            counts[i] = slice_extend;
        }
        counts[slice_count - 1] += extend % slice_count;

        let r = _mi_theap_random_next(theap);
        let mut current = r % slice_count;
        counts[current] -= 1;
        let free_start = blocks[current];

        let mut rnd = crate::random::_mi_random_shuffle(r | 1);
        for i in 1..extend {
            let round = i % size_of::<usize>();
            if round == 0 {
                rnd = crate::random::_mi_random_shuffle(rnd);
            }
            let mut next = (rnd >> (8 * round)) & (slice_count - 1);
            while counts[next] == 0 {
                next += 1;
                if next == slice_count {
                    next = 0;
                }
            }
            counts[next] -= 1;
            let block = blocks[current];
            blocks[current] = block.cast::<u8>().add(bsize).cast::<mi_block_t>();
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(blocks[next]));
            current = next;
        }
        crate::core_types::BlockPtr::new_unchecked(blocks[current]).set_next(page, page.free().0);
        page.set_free(crate::core_types::AvailableFreeList(Some(
            crate::core_types::BlockPtr::new_unchecked(free_start),
        )));
    }
}

#[inline(never)]
unsafe extern "C" fn mi_page_free_list_extend(page: *mut mi_page_t, bsize: usize, extend: usize) {
    unsafe {
        mi_page_free_list_extend_typed(
            crate::core_types::PagePtr::new_unchecked(page),
            crate::core_types::BlockSize::new_unchecked(bsize),
            extend,
        );
    }
}

#[inline]
unsafe fn mi_page_free_list_extend_typed(
    page: crate::core_types::PagePtr,
    bsize: crate::core_types::BlockSize,
    extend: usize,
) {
    unsafe {
        let page_raw = page.as_ptr();
        let bsize = bsize.get();
        if MI_SECURE < 3 {
            crate::mi_assert_internal!(page.free().0.is_none());
            crate::mi_assert_internal!(page.local_free().0.is_none());
        }
        let capacity = page.capacity();
        crate::mi_assert_internal!(capacity.as_usize() + extend <= page.reserved().as_usize());
        crate::mi_assert_internal!(bsize == page.block_size().get());
        let page_area = mi_page_start(page_raw).cast::<c_void>();
        let start = mi_page_block_at(page_raw, page_area, bsize, capacity.as_usize());
        let last = mi_page_block_at(page_raw, page_area, bsize, capacity.as_usize() + extend - 1);
        let mut block = start;
        while block <= last {
            let next = block.cast::<u8>().add(bsize).cast::<mi_block_t>();
            crate::core_types::BlockPtr::new_unchecked(block)
                .set_next(page, crate::core_types::BlockPtr::new(next));
            block = next;
        }
        crate::core_types::BlockPtr::new_unchecked(last).set_next(page, page.free().0);
        page.set_free(crate::core_types::AvailableFreeList(Some(
            crate::core_types::BlockPtr::new_unchecked(start),
        )));
    }
}

#[used]
static MI_PAGE_FREE_LIST_EXTEND_KEEP: unsafe extern "C" fn(*mut mi_page_t, usize, usize) =
    mi_page_free_list_extend;

#[inline(never)]
unsafe extern "C" fn mi_page_extend_free(theap: *mut mi_theap_t, page: *mut mi_page_t) -> bool {
    unsafe { mi_page_extend_free_typed(theap, PagePtr::new_unchecked(page)) }
}

#[inline]
unsafe fn mi_page_extend_free_typed(theap: *mut mi_theap_t, page_ptr: PagePtr) -> bool {
    unsafe {
        let page = page_ptr.as_ptr();
        crate::mi_assert_expensive!(mi_page_is_valid_init(page_ptr));
        if MI_SECURE < 3 {
            crate::mi_assert!(page_ptr.free().0.is_none());
            crate::mi_assert!(page_ptr.local_free().0.is_none());
            if page_ptr.free().0.is_some() {
                return true;
            }
        }
        let capacity = page_ptr.capacity();
        let reserved = page_ptr.reserved();
        if capacity.get() >= reserved.get() {
            return true;
        }

        let bsize = page_ptr.block_size().get();
        let mut extend = reserved.as_usize() - capacity.as_usize();
        crate::mi_assert_internal!(extend > 0);

        let mut max_extend = if bsize >= MI_MAX_EXTEND_SIZE {
            MI_MIN_EXTEND
        } else {
            MI_MAX_EXTEND_SIZE / bsize
        };
        if max_extend < MI_MIN_EXTEND {
            max_extend = MI_MIN_EXTEND;
        }
        crate::mi_assert_internal!(max_extend > 0);
        if extend > max_extend {
            extend = max_extend;
        }

        crate::mi_assert_internal!(extend > 0);
        crate::mi_assert_internal!(extend + capacity.as_usize() <= reserved.as_usize());
        crate::mi_assert_internal!(u16::try_from(extend).is_ok());

        let committed = mi_page_slice_committed(page);
        if !committed.is_zero() {
            let needed_size = (capacity.as_usize() + extend) * bsize;
            let needed_commit = _mi_align_up(
                mi_page_slice_offset_of(page, needed_size),
                MI_PAGE_MIN_COMMIT_SIZE,
            );
            if needed_commit > committed.get() {
                crate::mi_assert_internal!(
                    (needed_commit - committed.get())
                        .is_multiple_of(_mi_os_page_size_typed().get())
                );
                if !_mi_os_commit(
                    mi_page_slice_start(page)
                        .cast::<u8>()
                        .add(committed.get())
                        .cast::<c_void>(),
                    needed_commit - committed.get(),
                    ptr::null_mut(),
                ) {
                    return false;
                }
                mi_page_set_slice_committed(
                    page,
                    crate::core_types::CommittedBytes::new(needed_commit),
                );
            }
        }

        if MI_STAT > 0 {
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);
        }

        if extend < MI_MIN_SLICES || MI_SECURE < 2 {
            mi_page_free_list_extend(page, bsize, extend);
        } else {
            mi_page_free_list_extend_secure(theap, page, bsize, extend);
        }
        mi_page_add_capacity_count(page, extend as u16);
        if MI_STAT > 0 {
            __mi_stat_increase(
                ptr::addr_of_mut!((*theap).stats.page_committed),
                crate::core_types::CommittedBytes::new(extend * bsize).get(),
            );
        }

        if MI_TRACK_ENABLED != 0 {
            let mut page_size = 0usize;
            let page_start = mi_page_area(page, &mut page_size);
            mi_track_mem_undefined(page_start.cast(), page_size);
            mi_track_mem_noaccess(page_start.cast(), page_size);
        }
        crate::mi_assert_expensive!(mi_page_is_valid_init(page_ptr));
        true
    }
}

#[used]
static MI_PAGE_EXTEND_FREE_KEEP: unsafe extern "C" fn(*mut mi_theap_t, *mut mi_page_t) -> bool =
    mi_page_extend_free;

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_abandon(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
        let queue = crate::core_types::PageQueuePtr::new_unchecked(pq);
        let _ = _mi_page_abandon_typed(page_ptr, queue);
    }
}

#[inline]
unsafe fn _mi_page_abandon_typed(
    page_ptr: crate::core_types::PagePtr,
    queue: crate::core_types::PageQueuePtr,
) -> crate::core_types::PageAbandonOutcome {
    unsafe {
        let page_wrapper = if mi_page_is_in_full(page_ptr.as_ptr()) {
            crate::core_types::FullPage::new_unchecked(page_ptr).for_abandon()
        } else {
            crate::core_types::ActivePage::new_unchecked(page_ptr).for_abandon()
        };
        _mi_page_abandon_internal(page_wrapper, queue)
    }
}

pub(crate) unsafe fn _mi_page_abandon_internal(
    page_wrapper: crate::core_types::PageForAbandon,
    queue: crate::core_types::PageQueuePtr,
) -> crate::core_types::PageAbandonOutcome {
    unsafe {
        let page_ptr = match page_wrapper {
            crate::core_types::PageForAbandon::Active(page) => page.page(),
            crate::core_types::PageForAbandon::Full(page) => page.page(),
        };
        let page = page_ptr.as_ptr();
        let pq = queue.as_ptr();
        _mi_page_free_collect(page, false);
        if mi_page_all_free(page) {
            let destroyed = match page_wrapper {
                crate::core_types::PageForAbandon::Full(page) => {
                    let Some(all_free) = page.mark_all_free() else {
                        crate::mi_assert_internal!(false);
                        return crate::core_types::PageAbandonOutcome::Abandoned(
                            crate::core_types::AbandonedPage::new_unchecked(page_ptr),
                        );
                    };
                    let queue = crate::core_types::FullPageQueue::new_unchecked(queue);
                    _mi_page_free_full_internal(all_free, queue)
                }
                crate::core_types::PageForAbandon::Active(page) => {
                    let Some(all_free) = page.mark_all_free() else {
                        crate::mi_assert_internal!(false);
                        return crate::core_types::PageAbandonOutcome::Abandoned(
                            crate::core_types::AbandonedPage::new_unchecked(page_ptr),
                        );
                    };
                    if let Some(queue) = crate::core_types::NormalPageQueue::new(queue) {
                        if let Some(queued) = crate::core_types::QueuedPage::<
                            crate::core_types::NormalQueue,
                        >::new(page, queue)
                        {
                            _mi_page_free_active_internal(all_free, queued)
                        } else {
                            crate::mi_assert_internal!(false);
                            mi_page_queue_remove_raw(pq, page.page().as_ptr());
                            _mi_page_free_after_remove(all_free)
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                        _mi_page_free_after_remove(all_free)
                    }
                }
            };
            crate::core_types::PageAbandonOutcome::Destroyed(destroyed)
        } else {
            match page_wrapper {
                crate::core_types::PageForAbandon::Full(page) => {
                    if let Some(queue) = crate::core_types::FullPageQueue::new(queue) {
                        if let Some(queued) = crate::core_types::QueuedPage::<
                            crate::core_types::FullQueue,
                        >::new(page, queue)
                        {
                            let _ = queued.remove();
                        } else {
                            crate::mi_assert_internal!(false);
                            mi_page_queue_remove_raw(pq, page.page().as_ptr());
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                    }
                }
                crate::core_types::PageForAbandon::Active(page) => {
                    if let Some(queue) = crate::core_types::NormalPageQueue::new(queue) {
                        if let Some(queued) = crate::core_types::QueuedPage::<
                            crate::core_types::NormalQueue,
                        >::new(page, queue)
                        {
                            let _ = queued.remove();
                        } else {
                            crate::mi_assert_internal!(false);
                            mi_page_queue_remove_raw(pq, page.page().as_ptr());
                        }
                    } else {
                        mi_page_queue_remove_raw(pq, page.page().as_ptr());
                    }
                }
            }
            let theap = (*page).theap;
            mi_page_set_theap(page, ptr::null_mut());
            // Preserve the original theap pointer so same-theap reclaim-on-free can still work.
            (*page).theap = theap;
            _mi_arenas_page_abandon(page, theap);
            _mi_arenas_collect(false, false, (*theap).tld);
            crate::core_types::PageAbandonOutcome::Abandoned(
                crate::core_types::AbandonedPage::new_unchecked(page_ptr),
            )
        }
    }
}

/// # Safety
/// `theap` must be valid. If non-null, `pq` must be a page queue belonging to `theap`.
#[inline(never)]
pub unsafe extern "C" fn mi_page_fresh_alloc(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    block_size: usize,
    page_alignment: usize,
) -> *mut mi_page_t {
    unsafe {
        crate::mi_assert_internal!(pq.is_null() || mi_theap_contains_queue(theap, pq));
        crate::mi_assert_internal!(
            page_alignment > 0
                || block_size > MI_LARGE_MAX_OBJ_SIZE
                || pq.is_null()
                || block_size == mi_page_queue_block_size_raw(pq)
        );
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
            crate::core_types::PaddedSize::new_unchecked(block_size),
            crate::core_types::Zeroed::No,
        );
        let Some(page_alignment) = crate::core_types::MaybeAlignment::new(page_alignment) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let Some(page) = mi_page_fresh_alloc_typed(owner, pq, layout, page_alignment) else {
            return ptr::null_mut();
        };
        page.page().as_ptr()
    }
}

#[inline]
unsafe fn mi_page_fresh_alloc_typed(
    owner: crate::core_types::ThreadHeapLocal,
    pq: *mut mi_page_queue_t,
    layout: crate::core_types::LayoutRequest,
    page_alignment: crate::core_types::MaybeAlignment,
) -> Option<crate::core_types::ActivePage> {
    unsafe {
        let theap = owner.ptr().as_ptr();
        let block_size = layout.padded().get();
        let page = _mi_arenas_page_alloc(theap, block_size, page_alignment.get());
        if page.is_null() {
            return None;
        }
        let page = if mi_page_is_abandoned(page) {
            let page = _mi_theap_page_reclaim(
                crate::core_types::OwnedAbandonedPage::new_unchecked(
                    crate::core_types::PagePtr::new_unchecked(page),
                ),
                owner,
            );
            if !mi_page_immediate_available(page.page().as_ptr()) {
                if mi_page_is_expandable(page.page().as_ptr()) {
                    if !mi_page_extend_free(theap, page.page().as_ptr()) {
                        return None;
                    }
                } else {
                    crate::mi_assert!(false);
                    return None;
                }
            }
            page
        } else if !pq.is_null() {
            let page = crate::core_types::ActivePage::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            if mi_page_queue_is_huge(pq) {
                mi_page_queue_push_raw(theap, pq, page.page().as_ptr());
            } else {
                let _ = mi_page_queue_push_typed(
                    owner,
                    crate::core_types::NormalPageQueue::new_unchecked(
                        crate::core_types::PageQueuePtr::new_unchecked(pq),
                    ),
                    page,
                );
            }
            page
        } else {
            crate::core_types::ActivePage::new_unchecked(crate::core_types::PagePtr::new_unchecked(
                page,
            ))
        };
        crate::mi_assert_internal!(
            pq.is_null() || mi_page_block_size(page.page().as_ptr()) >= block_size
        );
        crate::mi_assert_expensive!(_mi_page_is_valid(page.page().as_ptr()));
        Some(page)
    }
}

#[used]
static MI_PAGE_FRESH_ALLOC_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    *mut mi_page_queue_t,
    usize,
    usize,
) -> *mut mi_page_t = mi_page_fresh_alloc;

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_init(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
) -> bool {
    unsafe {
        crate::mi_assert!(!page.is_null());
        crate::mi_assert!(!theap.is_null());
        _mi_page_init_typed(
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
            crate::core_types::Page::<crate::core_types::Uninit>::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            ),
        )
    }
}

#[inline]
unsafe fn _mi_page_init_typed(
    owner: crate::core_types::ThreadHeapLocal,
    page_uninit: crate::core_types::Page<crate::core_types::Uninit>,
) -> bool {
    unsafe {
        let theap = owner.ptr().as_ptr();
        let page = page_uninit.ptr().as_ptr();
        let heap = _mi_theap_heap(theap);
        (*page).heap = if _mi_is_heap_main(heap) {
            ptr::null_mut()
        } else {
            heap
        };
        mi_page_set_theap(page, theap);

        let mut page_size = 0usize;
        let page_start = mi_page_area(page, &mut page_size);
        mi_track_mem_noaccess(page_start.cast(), page_size);
        crate::mi_assert_internal!(page_size / mi_page_block_size(page) < (1usize << 16));
        crate::mi_assert_internal!(mi_page_reserved_count(page).get() > 0);

        mi_page_set_keys(
            PagePtr::new_unchecked(page),
            PageKeys([_mi_theap_random_next(theap), _mi_theap_random_next(theap)]),
        );

        if MI_DEBUG > 2 && (*page).memid.initially_zero {
            let committed = mi_page_committed_typed(page);
            mi_track_mem_defined((*page).page_start.cast(), committed.get());
            crate::mi_assert_expensive!(mi_mem_is_zero(page_start.cast(), committed.get()));
        }

        crate::mi_assert_internal!(!(*page).theap.is_null());
        crate::mi_assert_internal!((*page).theap == mi_page_theap(page));
        crate::mi_assert_internal!(mi_page_capacity_count(page).get() == 0);
        crate::mi_assert_internal!((*page).free.is_null());
        crate::mi_assert_internal!(mi_page_used_count(page).get() == 0);
        crate::mi_assert_internal!(crate::internal::mi_page_is_owned(page));
        let thread_free = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(
            ptr::addr_of_mut!((*page).xthread_free),
        ))
        .load_relaxed();
        crate::mi_assert_internal!(
            thread_free
                == crate::core_types::ThreadFreeWord::from_parts(
                    None,
                    crate::core_types::ThreadFreeOwner::Owned
                )
        );
        crate::mi_assert_internal!((*page).next.is_null());
        crate::mi_assert_internal!((*page).prev.is_null());
        crate::mi_assert_internal!(mi_page_retire_expire(page).is_zero());
        crate::mi_assert_internal!(!mi_page_has_interior_pointers(page));
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        {
            crate::mi_assert_internal!((*page).keys[0] != 0);
            crate::mi_assert_internal!((*page).keys[1] != 0);
        }
        crate::mi_assert_expensive!(mi_page_is_valid_init(page_uninit.ptr()));

        if !mi_page_extend_free(theap, page) {
            return false;
        }
        let active = page_uninit.initialize(owner);
        crate::mi_assert!(mi_page_immediate_available(active.page().as_ptr()));
        true
    }
}

/// # Safety
/// `theap` must be valid, and `pq` must be a page queue belonging to `theap`.
#[inline]
pub unsafe fn mi_page_fresh(theap: *mut mi_theap_t, pq: *mut mi_page_queue_t) -> *mut mi_page_t {
    unsafe {
        crate::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let queue = crate::core_types::NormalPageQueue::new_unchecked(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        );
        let block_size = mi_page_queue_block_size_raw(pq);
        let block_size_typed = crate::core_types::BlockSize::new_unchecked(block_size);
        let Some(slice_count) = crate::core_types::SliceCount::from_usize(
            crate::internal::mi_slice_count_of_size(block_size),
        ) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let Some(page) = mi_page_fresh_typed(owner, queue, block_size_typed, slice_count) else {
            return ptr::null_mut();
        };
        page.page().as_ptr()
    }
}

#[inline]
unsafe fn mi_page_fresh_typed(
    owner: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size_typed: crate::core_types::BlockSize,
    slice_count: crate::core_types::SliceCount,
) -> Option<crate::core_types::ActivePage> {
    unsafe {
        let theap = owner.ptr().as_ptr();
        let pq = queue.ptr().as_ptr();
        let block_size = block_size_typed.get();
        crate::mi_assert_internal!(slice_count.get() > 0);
        let page = mi_page_fresh_alloc(theap, pq, block_size, 0);
        if page.is_null() {
            return None;
        }
        crate::mi_assert_internal!(block_size == mi_page_block_size(page));
        crate::mi_assert_internal!(pq == mi_theap_page_queue_of(theap, page));
        Some(crate::core_types::ActivePage::new_unchecked(
            crate::core_types::PagePtr::new_unchecked(page),
        ))
    }
}

#[inline(never)]
pub(crate) unsafe fn try_reclaim_abandoned(
    page_wrapper: crate::core_types::OwnedAbandonedPage,
    target: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::ActivePage {
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let theap = target.ptr().as_ptr();
        crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
            mi_page_slice_start(page).cast(),
            MI_PAGE_ALIGN,
        ));
        crate::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);
        crate::mi_assert_internal!(crate::internal::mi_page_is_owned(page));
        crate::mi_assert_internal!(mi_page_is_abandoned(page));

        mi_page_set_theap(page, theap);

        let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(
            ptr::addr_of_mut!((*page).xthread_free),
        ));
        let remote = atomic_tf.collect_acq_rel();
        let local = crate::core_types::LocalFreeList::from_raw((*page).local_free);
        remote.collect_into_local(page_wrapper, local);

        _mi_page_free_collect(page, false);
        let pq = mi_theap_page_queue_of(theap, page);
        mi_page_queue_push_at_end_typed(
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
            crate::core_types::NormalPageQueue::new_unchecked(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ),
            page_wrapper.reclaim(target),
        );
        crate::mi_assert_expensive!(_mi_page_is_valid(page));
        page_wrapper.reclaim(target)
    }
}

#[inline(never)]
pub(crate) unsafe fn _mi_theap_page_reclaim(
    page: crate::core_types::OwnedAbandonedPage,
    target: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::ActivePage {
    unsafe { try_reclaim_abandoned(page, target) }
}

/// # Safety
/// C-ABI-shaped keep boundary for page reclaim. The caller must ensure `theap`
/// and `page` are valid, and that `page` is an owned abandoned page.
#[inline(never)]
unsafe extern "C" fn mi_theap_page_reclaim_raw(theap: *mut mi_theap_t, page: *mut mi_page_t) {
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
        let wrapper = crate::core_types::OwnedAbandonedPage::new_unchecked(page_ptr);
        let theap_ptr = crate::core_types::ThreadHeapPtr::new_unchecked(theap);
        let theap_wrapper = crate::core_types::ThreadHeapLocal::new_unchecked(theap_ptr);
        _mi_theap_page_reclaim(wrapper, theap_wrapper);
    }
}

#[used]
static MI_THEAP_PAGE_RECLAIM_KEEP: unsafe extern "C" fn(*mut mi_theap_t, *mut mi_page_t) =
    mi_theap_page_reclaim_raw;

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_huge(page: *const mi_page_t) -> bool {
    unsafe {
        if !mi_page_is_singleton(page) {
            return false;
        }
        if mi_page_block_size(page) > MI_LARGE_MAX_OBJ_SIZE {
            return true;
        }
        match crate::core_types::MemId::view((*page).memid) {
            crate::core_types::MemIdView::Os(os) => os
                .base
                .is_some_and(|base| base.as_ptr() < page.cast_mut().cast::<c_void>()),
            _ => false,
        }
    }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_flags_set(page: *mut mi_page_t, set: bool, newflag: mi_page_flags_t) -> bool {
    unsafe {
        let Some(flag) = crate::core_types::PageFlags::new(newflag) else {
            crate::mi_assert_internal!(false);
            return false;
        };
        let page = crate::core_types::PagePtr::new_unchecked(page);
        mi_page_flags_set_typed(page, set, flag)
    }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
unsafe fn mi_page_flags_set_typed(
    page: crate::core_types::PagePtr,
    set: bool,
    flag: crate::core_types::PageFlags,
) -> bool {
    unsafe {
        let atomic_tid = page.thread_id_cell();
        let old = atomic_tid.set_flags(flag, set);
        old.flags().contains(flag)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_in_full(page: *const mi_page_t) -> bool {
    unsafe {
        crate::core_types::PageFlags::from_low_bits(mi_page_flags(page))
            .contains(crate::core_types::PageFlags::IN_FULL_QUEUE)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    unsafe { !mi_page_immediate_available(page) && !mi_page_is_expandable(page) }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_set_in_full(page: *mut mi_page_t, in_full: bool) {
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
        let was_in_full = mi_page_flags_set_typed(
            page_ptr,
            in_full,
            crate::core_types::PageFlags::IN_FULL_QUEUE,
        );
        if was_in_full != in_full {
            let theap = (*page).theap;
            crate::mi_assert_internal!(!theap.is_null());
            if !theap.is_null() {
                let size = mi_page_capacity_count(page).as_usize() * mi_page_block_size(page);
                if in_full {
                    (*theap).pages_full_size += size;
                } else {
                    crate::mi_assert_internal!(size <= (*theap).pages_full_size);
                    (*theap).pages_full_size -= size;
                }
            }
        }
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_abandoned(page: *const mi_page_t) -> bool {
    unsafe {
        let atomic_tid =
            crate::core_types::AtomicPageThreadId(crate::core_types::AtomicUsizeField(
                ptr::addr_of_mut!((*(page as *mut mi_page_t)).xthread_id),
            ));
        let tid = atomic_tid.load();
        tid.is_abandoned() || tid.is_abandoned_mapped()
    }
}

/// # Safety
/// `page` must be valid. `theap` must be null or a valid thread heap for this allocator.
#[inline]
pub unsafe fn mi_page_set_theap(page: *mut mi_page_t, theap: *mut mi_theap_t) {
    unsafe {
        let page = crate::core_types::PagePtr::new_unchecked(page);
        let owner = if theap.is_null() {
            None
        } else {
            let theap = crate::core_types::ThreadHeapPtr::new_unchecked(theap);
            Some(crate::core_types::ThreadHeapLocal::new_unchecked(theap))
        };
        mi_page_set_theap_typed(page, owner);
    }
}

/// # Safety
/// `page` must be valid. `owner` must be the local owner for `page`, or `None`
/// when transitioning the page to abandoned ownership.
#[inline]
unsafe fn mi_page_set_theap_typed(
    page: crate::core_types::PagePtr,
    owner: Option<crate::core_types::ThreadHeapLocal>,
) {
    unsafe {
        let theap = owner
            .map(|owner| owner.ptr().as_ptr())
            .unwrap_or(ptr::null_mut());
        (*page.as_ptr()).theap = theap;
        let owner = if let Some(owner) = owner {
            let tid = (*(*owner.ptr().as_ptr()).tld).thread_id;
            let thread_id = crate::core_types::ThreadId::new(tid).unwrap_or_else(|| {
                crate::mi_assert_internal!(false);
                crate::core_types::ThreadId::new_unchecked(tid)
            });
            crate::core_types::PageOwnerId::Thread(thread_id)
        } else {
            crate::core_types::PageOwnerId::Abandoned
        };

        let atomic_tid = page.thread_id_cell();
        let mut xtid_old = atomic_tid.load();
        loop {
            let xtid = xtid_old.with_owner(owner);
            if atomic_tid.compare_exchange_owner(&mut xtid_old, xtid) {
                break;
            }
        }
    }
}

/// # Safety
/// `page` must be valid and must not be abandoned.
#[inline]
pub unsafe fn mi_page_theap(page: *const mi_page_t) -> *mut mi_theap_t {
    unsafe {
        crate::mi_assert_internal!(!mi_page_is_abandoned(page));
        crate::mi_assert_internal!(!(*page).theap.is_null());
        (*page).theap
    }
}

/// # Safety
/// `queue` must be valid, and `page` must be null or page metadata compatible with that queue.
#[inline]
pub unsafe fn mi_page_queue_contains(queue: *mut mi_page_queue_t, page: *const mi_page_t) -> bool {
    unsafe {
        crate::mi_assert_internal!(!page.is_null());
        let queue_ptr = page_queue_ptr(queue);
        let mut list = queue_ptr
            .first()
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        while !list.is_null() {
            crate::mi_assert_internal!((*list).next.is_null() || (*(*list).next).prev == list);
            crate::mi_assert_internal!((*list).prev.is_null() || (*(*list).prev).next == list);
            if ptr::eq(list, page.cast_mut()) {
                break;
            }
            list = (*list).next;
        }
        ptr::eq(list, page.cast_mut())
    }
}

/// # Safety
/// `theap` must point to a valid thread heap, and `pq` must be a queue pointer to compare against
/// that heap's page queues.
#[inline]
pub unsafe fn mi_theap_contains_queue(
    theap: *const mi_theap_t,
    pq: *const mi_page_queue_t,
) -> bool {
    unsafe {
        let start = crate::core_types::ptr_exposed_addr(ptr::addr_of!((*theap).pages[0]));
        let end = crate::core_types::ptr_exposed_addr(ptr::addr_of!((*theap).pages[MI_BIN_FULL]));
        let queue = crate::core_types::ptr_exposed_addr(pq);
        queue >= start && queue <= end
    }
}

/// # Safety
/// `theap` must be valid, and `pq` must point to a page queue belonging to that heap.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_queue_is_valid(
    theap: *mut mi_theap_t,
    pq: *const mi_page_queue_t,
) -> bool {
    unsafe {
        if pq.is_null() {
            return false;
        }
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let pq = crate::core_types::PageQueuePtr::new_unchecked(pq.cast_mut());
        if let Some(queue) = crate::core_types::NormalPageQueue::new(pq) {
            mi_normal_page_queue_is_valid(theap, queue)
        } else if let Some(queue) = crate::core_types::HugePageQueue::new(pq) {
            mi_huge_page_queue_is_valid(theap, queue)
        } else if let Some(queue) = crate::core_types::FullPageQueue::new(pq) {
            mi_full_page_queue_is_valid(theap, queue)
        } else {
            false
        }
    }
}

#[inline]
pub(crate) unsafe fn mi_normal_page_queue_is_valid(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
) -> bool {
    unsafe { mi_page_queue_is_valid_typed(theap, queue.ptr()) }
}

#[inline]
pub(crate) unsafe fn mi_huge_page_queue_is_valid(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::HugePageQueue,
) -> bool {
    unsafe { mi_page_queue_is_valid_typed(theap, queue.ptr()) }
}

#[inline]
pub(crate) unsafe fn mi_full_page_queue_is_valid(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::FullPageQueue,
) -> bool {
    unsafe { mi_page_queue_is_valid_typed(theap, queue.ptr()) }
}

#[inline]
unsafe fn mi_page_queue_is_valid_typed(
    theap: crate::core_types::ThreadHeapLocal,
    pq: crate::core_types::PageQueuePtr,
) -> bool {
    unsafe {
        let theap = theap.ptr().as_ptr();
        let pq = pq.as_ptr();
        crate::MI_UNUSED_RELEASE!(theap);
        let mut count = 0usize;
        crate::MI_UNUSED_RELEASE!(count);
        let mut prev = ptr::null_mut();
        crate::MI_UNUSED_RELEASE!(prev);
        let queue = page_queue_ptr(pq);
        let block_size = mi_page_queue_block_size_raw(pq);
        let mut page = queue
            .first()
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        while !page.is_null() {
            crate::mi_assert_internal!((*page).prev == prev);
            if mi_page_is_in_full(page) {
                crate::mi_assert_internal!(
                    _mi_wsize_from_size(block_size) == MI_LARGE_MAX_OBJ_WSIZE + 2
                );
            } else if mi_page_is_huge(page) {
                crate::mi_assert_internal!(
                    _mi_wsize_from_size(block_size) == MI_LARGE_MAX_OBJ_WSIZE + 1
                );
            } else {
                crate::mi_assert_internal!(mi_page_block_size(page) == block_size);
            }
            crate::mi_assert_internal!((*page).theap == theap);
            if (*page).next.is_null() {
                crate::mi_assert_internal!(
                    queue
                        .last()
                        .map(crate::core_types::PagePtr::as_ptr)
                        .unwrap_or(ptr::null_mut())
                        == page
                );
            }
            count += 1;
            prev = page;
            page = (*page).next;
        }
        crate::mi_assert_internal!(mi_page_queue_count_typed(pq).get() == count);
        true
    }
}

#[used]
static MI_PAGE_QUEUE_IS_VALID_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    *const mi_page_queue_t,
) -> bool = _mi_page_queue_is_valid;

#[inline]
pub(crate) unsafe fn mi_page_bin_typed(page: *const mi_page_t) -> crate::core_types::Bin {
    unsafe {
        let bin = if mi_page_is_in_full(page) {
            MI_BIN_FULL
        } else if mi_page_is_huge(page) {
            MI_BIN_HUGE
        } else {
            mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(
                mi_page_block_size(page),
            ))
            .as_usize()
        };
        crate::mi_assert_internal!(bin <= MI_BIN_FULL);
        crate::mi_debug_assert!(u16::try_from(bin).is_ok());
        // SAFETY: `MI_BIN_FULL` is the last valid bin, and the assertion above
        // proves `bin < MI_BIN_COUNT`.
        crate::core_types::Bin::new_unchecked(bin as u16)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_bin(page: *const mi_page_t) -> usize {
    unsafe { mi_page_bin_typed(page).as_usize() }
}

#[inline]
pub(crate) unsafe fn _mi_page_stats_bin_typed(
    page: *const mi_page_t,
) -> crate::core_types::StatsBin {
    unsafe {
        let bin = if mi_page_is_huge(page) {
            MI_BIN_HUGE
        } else {
            mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(
                mi_page_block_size(page),
            ))
            .as_usize()
        };
        crate::mi_assert_internal!(bin <= MI_BIN_HUGE);
        crate::mi_debug_assert!(u16::try_from(bin).is_ok());
        // SAFETY: the assertion above proves `bin <= MI_BIN_HUGE`.
        crate::core_types::StatsBin::new_unchecked(bin as u16)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_stats_bin(page: *const mi_page_t) -> usize {
    unsafe {
        let bin = _mi_page_stats_bin_typed(page).as_usize();
        crate::mi_assert_internal!(bin <= MI_BIN_HUGE);
        bin
    }
}

/// # Safety
/// `theap` must be valid, and `page` must point to page metadata owned by that heap.
#[inline]
pub(crate) unsafe fn mi_theap_page_queue_of_typed(
    theap: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
) -> crate::core_types::PageQueuePtr {
    unsafe {
        let bin = mi_bin_from_block_size(block_size);
        crate::mi_assert_internal!(bin.as_usize() <= MI_BIN_HUGE);
        theap.ptr().page_queue(bin)
    }
}

/// # Safety
/// `theap` must be valid. `page` must point to page metadata owned by `theap`.
#[inline]
pub unsafe fn mi_theap_page_queue_of(
    theap: *mut mi_theap_t,
    page: *const mi_page_t,
) -> *mut mi_page_queue_t {
    unsafe {
        crate::mi_assert_internal!(!theap.is_null());
        let bin = mi_page_bin_typed(page).as_usize();
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let block_size = crate::core_types::BlockSize::new_unchecked(mi_page_block_size(page));
        let pq = if mi_page_is_in_full(page) {
            crate::core_types::PageQueuePtr::new_unchecked(ptr::addr_of_mut!(
                (*theap.ptr().as_ptr()).pages[bin]
            ))
        } else {
            mi_theap_page_queue_of_typed(theap, block_size)
        };
        let pq = pq.as_ptr();
        crate::mi_assert_internal!(
            (mi_page_block_size(page) == mi_page_queue_block_size_raw(pq))
                || (mi_page_is_huge(page) && mi_page_queue_is_huge(pq))
                || (mi_page_is_in_full(page) && mi_page_queue_is_full(pq))
        );
        pq
    }
}

/// # Safety
/// `page` must point to non-abandoned page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_queue_of(page: *const mi_page_t) -> *mut mi_page_queue_t {
    unsafe {
        let theap = mi_page_theap(page);
        let pq = mi_theap_page_queue_of(theap, page);
        crate::mi_assert_expensive!(mi_page_queue_contains(pq, page));
        pq
    }
}

/// # Safety
/// `theap` must be valid, and `pq` must point to one of its page queues.
pub unsafe fn mi_theap_queue_first_update(theap: *mut mi_theap_t, pq: *const mi_page_queue_t) {
    unsafe {
        crate::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
        let queue = page_queue_ptr(pq);
        if let Some(queue) = crate::core_types::NormalPageQueue::new(queue) {
            let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            );
            mi_theap_queue_first_update_typed(theap, queue);
        }
    }
}

#[inline]
pub(crate) unsafe fn mi_theap_queue_first_update_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
) {
    unsafe {
        let pq = queue.ptr().as_ptr();
        let Some(block_size) = queue.ptr().block_size() else {
            crate::mi_assert_internal!(false);
            return;
        };
        let size = block_size.get();
        if size > MI_SMALL_SIZE_MAX {
            return;
        }

        let page = queue
            .ptr()
            .first()
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or_else(|| ptr::addr_of!(_mi_page_empty).cast_mut());

        let direct_idx = mi_direct_bin_from_size(size);
        let idx = direct_idx.as_usize();
        crate::mi_debug_assert!(idx < crate::types::MI_PAGES_DIRECT);
        let theap_ptr = theap.ptr();
        let theap_raw = theap_ptr.as_ptr();
        if theap_ptr
            .direct_page(direct_idx)
            .map(crate::core_types::PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut())
            == page
        {
            return;
        }

        let start = if idx <= 1 {
            0
        } else {
            crate::mi_assert_internal!(pq > ptr::addr_of_mut!((*theap_raw).pages[0]));
            let bin = mi_bin_from_block_size(block_size).as_usize();
            let mut prev = pq.sub(1);
            while bin
                == mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(
                    mi_page_queue_block_size_raw(prev),
                ))
                .as_usize()
                && prev > ptr::addr_of_mut!((*theap_raw).pages[0])
            {
                prev = prev.sub(1);
            }
            let start = 1 + _mi_wsize_from_size(mi_page_queue_block_size_raw(prev));
            start.min(idx)
        };

        crate::mi_assert_internal!(start <= idx);
        let direct_start = crate::core_types::DirectBin::new_unchecked(start as u16);
        theap_ptr.fill_direct_pages(
            direct_start,
            direct_idx,
            Some(crate::core_types::PagePtr::new_unchecked(page)),
        );
    }
}

/// # Safety
/// `queue` and `page` must be valid, and `page` must currently be linked into `queue`.
pub(crate) unsafe fn mi_page_queue_remove_raw(queue: *mut mi_page_queue_t, page: *mut mi_page_t) {
    unsafe {
        let queue = PageQueuePtr::new_unchecked(queue);
        let page = PagePtr::new_unchecked(page);
        mi_page_queue_remove_typed(queue, page);
    }
}

/// # Safety
/// `queue` and `page` must be valid, and `page` must currently be linked into `queue`.
#[inline]
pub(crate) unsafe fn mi_page_queue_remove_typed(queue: PageQueuePtr, page: PagePtr) {
    unsafe {
        let queue_ptr = queue;
        let queue = queue.as_ptr();
        let page = page.as_ptr();
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_expensive!(mi_page_queue_contains(queue, page));
        crate::mi_assert_internal!(mi_page_queue_count_typed(queue).get() >= 1);
        crate::mi_assert_internal!(
            mi_page_block_size(page) == mi_page_queue_block_size_raw(queue)
                || (mi_page_is_huge(page) && mi_page_queue_is_huge(queue))
                || (mi_page_is_in_full(page) && mi_page_queue_is_full(queue))
        );
        let theap = mi_page_theap(page);
        if !(*page).prev.is_null() {
            (*(*page).prev).next = (*page).next;
        }
        if !(*page).next.is_null() {
            (*(*page).next).prev = (*page).prev;
        }
        if queue_ptr
            .last()
            .is_some_and(|last| ptr::eq(page, last.as_ptr()))
        {
            queue_ptr.set_last(PagePtr::new((*page).prev));
        }
        if queue_ptr
            .first()
            .is_some_and(|first| ptr::eq(page, first.as_ptr()))
        {
            queue_ptr.set_first(PagePtr::new((*page).next));
            crate::mi_assert_internal!(mi_theap_contains_queue(theap, queue));
            mi_theap_queue_first_update(theap, queue);
        }
        (*theap).page_count -= 1;
        mi_page_queue_decrement_count(queue);
        (*page).next = ptr::null_mut();
        (*page).prev = ptr::null_mut();
        mi_page_set_in_full(page, false);
    }
}

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
unsafe fn mi_page_queue_push_raw(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_queue_push_internal(
            theap,
            PageQueuePtr::new_unchecked(queue),
            PagePtr::new_unchecked(page),
        );
    }
}

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
#[inline]
unsafe fn mi_page_queue_push_internal(
    theap: crate::core_types::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
) {
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        let queue_raw = queue.as_ptr();
        let page_raw = page.as_ptr();
        crate::mi_assert_internal!(mi_page_theap(page_raw) == theap_raw);
        crate::mi_assert_internal!(!mi_page_queue_contains(queue_raw, page_raw));
        crate::mi_assert_internal!(
            mi_page_block_size(page_raw) == mi_page_queue_block_size_raw(queue_raw)
                || (mi_page_is_huge(page_raw) && mi_page_queue_is_huge(queue_raw))
                || (mi_page_is_in_full(page_raw) && mi_page_queue_is_full(queue_raw))
        );

        mi_page_set_in_full(page_raw, mi_page_queue_is_full(queue_raw));

        (*page_raw).next = queue
            .first()
            .map(PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        (*page_raw).prev = ptr::null_mut();
        if let Some(first) = queue.first() {
            crate::mi_assert_internal!((*first.as_ptr()).prev.is_null());
            (*first.as_ptr()).prev = page_raw;
            queue.set_first(PagePtr::new(page_raw));
        } else {
            queue.set_first(PagePtr::new(page_raw));
            queue.set_last(PagePtr::new(page_raw));
        }
        mi_page_queue_increment_count(queue_raw);
        mi_theap_queue_first_update(theap_raw, queue_raw);
        (*theap_raw).page_count += 1;
    }
}

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
unsafe fn mi_page_queue_push_at_end_raw(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_queue_push_at_end_internal(
            theap,
            PageQueuePtr::new_unchecked(queue),
            PagePtr::new_unchecked(page),
        );
    }
}

/// # Safety
/// `theap`, `queue`, and `page` must be valid. `queue` must belong to `theap`, and `page` must not
/// already be linked into it.
#[inline]
unsafe fn mi_page_queue_push_at_end_internal(
    theap: crate::core_types::ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
) {
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        let queue_raw = queue.as_ptr();
        let page_raw = page.as_ptr();
        crate::mi_assert_internal!(mi_page_theap(page_raw) == theap_raw);
        crate::mi_assert_internal!(!mi_page_queue_contains(queue_raw, page_raw));
        crate::mi_assert_internal!(
            mi_page_block_size(page_raw) == mi_page_queue_block_size_raw(queue_raw)
                || (mi_page_is_huge(page_raw) && mi_page_queue_is_huge(queue_raw))
                || (mi_page_is_in_full(page_raw) && mi_page_queue_is_full(queue_raw))
        );

        mi_page_set_in_full(page_raw, mi_page_queue_is_full(queue_raw));

        (*page_raw).prev = queue.last().map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        (*page_raw).next = ptr::null_mut();
        if let Some(last) = queue.last() {
            crate::mi_assert_internal!((*last.as_ptr()).next.is_null());
            (*last.as_ptr()).next = page_raw;
            queue.set_last(PagePtr::new(page_raw));
        } else {
            queue.set_first(PagePtr::new(page_raw));
            queue.set_last(PagePtr::new(page_raw));
        }
        mi_page_queue_increment_count(queue_raw);

        if queue
            .first()
            .is_some_and(|first| ptr::eq(first.as_ptr(), page_raw))
        {
            mi_theap_queue_first_update(theap_raw, queue_raw);
        }
        (*theap_raw).page_count += 1;
    }
}

#[inline]
unsafe fn mi_page_queue_push_at_end_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    page: crate::core_types::ActivePage,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    unsafe {
        mi_page_queue_push_at_end_internal(theap, queue.ptr(), page.page());
        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(page, queue)
    }
}

#[inline]
unsafe fn mi_page_queue_push_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    page: crate::core_types::ActivePage,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    unsafe {
        mi_page_queue_push_internal(theap, queue.ptr(), page.page());
        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(page, queue)
    }
}

#[inline]
unsafe fn mi_page_queue_move_to_front_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,
) -> crate::core_types::QueuedPage<crate::core_types::NormalQueue> {
    unsafe {
        let page = queued.page().as_ptr();
        crate::mi_assert_internal!(mi_page_theap(page) == theap.ptr().as_ptr());
        let queue_ptr = queue.ptr();
        if queue_ptr
            .first()
            .is_some_and(|first| ptr::eq(first.as_ptr(), page))
        {
            return queued;
        }
        let active = queued.remove();
        let queued = mi_page_queue_push_typed(theap, queue, active);
        crate::mi_assert_internal!(
            queue_ptr
                .first()
                .is_some_and(|first| ptr::eq(first.as_ptr(), page))
        );
        queued
    }
}

pub(crate) unsafe fn mi_page_queue_enqueue_from_raw(
    to: crate::core_types::PageQueuePtr,
    from: crate::core_types::PageQueuePtr,
    enqueue_at_end: bool,
    page: crate::core_types::PagePtr,
) {
    unsafe {
        let to_ptr = to;
        let from_ptr = from;
        let to = to.as_ptr();
        let from = from.as_ptr();
        let page = page.as_ptr();
        crate::mi_assert_internal!(!page.is_null());
        crate::mi_assert_internal!(mi_page_queue_count_typed(from).get() >= 1);
        crate::mi_assert_expensive!(mi_page_queue_contains(from, page));
        crate::mi_assert_expensive!(!mi_page_queue_contains(to, page));
        let bsize = mi_page_block_size(page);
        crate::MI_UNUSED!(bsize);
        crate::mi_assert_internal!(
            (bsize == mi_page_queue_block_size_raw(to)
                && (bsize == mi_page_queue_block_size_raw(from) || mi_page_queue_is_full(from)))
                || (bsize == mi_page_queue_block_size_raw(from) && mi_page_queue_is_full(to))
                || (mi_page_is_huge(page)
                    && (mi_page_queue_is_huge(to) || mi_page_queue_is_full(to)))
        );

        let theap = mi_page_theap(page);

        if !(*page).prev.is_null() {
            (*(*page).prev).next = (*page).next;
        }
        if !(*page).next.is_null() {
            (*(*page).next).prev = (*page).prev;
        }
        if from_ptr
            .last()
            .is_some_and(|last| ptr::eq(page, last.as_ptr()))
        {
            from_ptr.set_last(PagePtr::new((*page).prev));
        }
        if from_ptr
            .first()
            .is_some_and(|first| ptr::eq(page, first.as_ptr()))
        {
            from_ptr.set_first(PagePtr::new((*page).next));
            crate::mi_assert_internal!(mi_theap_contains_queue(theap, from));
            mi_theap_queue_first_update(theap, from);
        }
        mi_page_queue_decrement_count(from);

        mi_page_queue_increment_count(to);
        if enqueue_at_end {
            (*page).prev = to_ptr
                .last()
                .map(PagePtr::as_ptr)
                .unwrap_or(ptr::null_mut());
            (*page).next = ptr::null_mut();
            if let Some(last) = to_ptr.last() {
                crate::mi_assert_internal!(theap == mi_page_theap(last.as_ptr()));
                (*last.as_ptr()).next = page;
                to_ptr.set_last(PagePtr::new(page));
            } else {
                to_ptr.set_first(PagePtr::new(page));
                to_ptr.set_last(PagePtr::new(page));
                mi_theap_queue_first_update(theap, to);
            }
        } else if let Some(first) = to_ptr.first() {
            crate::mi_assert_internal!(theap == mi_page_theap(first.as_ptr()));
            let next = (*first.as_ptr()).next;
            (*page).prev = first.as_ptr();
            (*page).next = next;
            (*first.as_ptr()).next = page;
            if !next.is_null() {
                (*next).prev = page;
            } else {
                to_ptr.set_last(PagePtr::new(page));
            }
        } else {
            (*page).prev = ptr::null_mut();
            (*page).next = ptr::null_mut();
            to_ptr.set_first(PagePtr::new(page));
            to_ptr.set_last(PagePtr::new(page));
            mi_theap_queue_first_update(theap, to);
        }
    }
}

#[inline]
pub(crate) unsafe fn mi_page_queue_enqueue_from(
    to: crate::core_types::FullPageQueue,
    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,
) -> crate::core_types::FullPage {
    unsafe { page.into_full(to) }
}

#[inline]
pub(crate) unsafe fn mi_page_queue_enqueue_from_full(
    to: crate::core_types::NormalPageQueue,
    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,
) -> crate::core_types::ActivePage {
    unsafe { page.unfull(to) }
}

/// # Safety
/// `theap`, `pq`, and `append` must be valid page-queue structures with compatible block sizes.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_queue_append(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    append: *mut mi_page_queue_t,
) -> usize {
    unsafe {
        let queue = page_queue_ptr(pq);
        let append_queue = page_queue_ptr(append);
        crate::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
        crate::mi_assert_internal!(
            mi_page_queue_block_size_raw(pq) == mi_page_queue_block_size_raw(append)
        );

        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        if let (Some(queue), Some(append_queue)) = (
            crate::core_types::NormalPageQueue::new(queue),
            crate::core_types::NormalPageQueue::new(append_queue),
        ) {
            mi_page_queue_append_normal_typed(theap, queue, append_queue)
        } else if let (Some(queue), Some(append_queue)) = (
            crate::core_types::HugePageQueue::new(queue),
            crate::core_types::HugePageQueue::new(append_queue),
        ) {
            mi_page_queue_append_huge_typed(theap, queue, append_queue)
        } else if let (Some(queue), Some(append_queue)) = (
            crate::core_types::FullPageQueue::new(queue),
            crate::core_types::FullPageQueue::new(append_queue),
        ) {
            mi_page_queue_append_full_typed(theap, queue, append_queue)
        } else {
            crate::mi_assert_internal!(false);
            0
        }
    }
}

#[inline]
unsafe fn mi_page_queue_append_normal_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    append: crate::core_types::NormalPageQueue,
) -> usize {
    unsafe { mi_page_queue_append_typed(theap, queue.ptr(), append.ptr(), Some(queue)) }
}

#[inline]
unsafe fn mi_page_queue_append_huge_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::HugePageQueue,
    append: crate::core_types::HugePageQueue,
) -> usize {
    unsafe { mi_page_queue_append_typed(theap, queue.ptr(), append.ptr(), None) }
}

#[inline]
unsafe fn mi_page_queue_append_full_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::FullPageQueue,
    append: crate::core_types::FullPageQueue,
) -> usize {
    unsafe { mi_page_queue_append_typed(theap, queue.ptr(), append.ptr(), None) }
}

#[inline]
unsafe fn mi_page_queue_append_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::PageQueuePtr,
    append_queue: crate::core_types::PageQueuePtr,
    first_update_queue: Option<crate::core_types::NormalPageQueue>,
) -> usize {
    unsafe {
        if append_queue.first().is_none() {
            return 0;
        }

        let mut count = 0usize;
        let mut page = append_queue
            .first()
            .map(PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        while !page.is_null() {
            mi_page_set_theap(page, theap.ptr().as_ptr());
            count += 1;
            page = (*page).next;
        }
        let append_count = append_queue.count();
        crate::mi_assert_internal!(count == append_count.get());

        if queue.last().is_none() {
            crate::mi_assert_internal!(queue.first().is_none());
            queue.set_first(append_queue.first());
            queue.set_last(append_queue.last());
            if let Some(queue) = first_update_queue {
                mi_theap_queue_first_update_typed(theap, queue);
            }
        } else {
            let Some(last) = queue.last() else {
                crate::mi_assert_internal!(false);
                return count;
            };
            let Some(append_first) = append_queue.first() else {
                crate::mi_assert_internal!(false);
                return count;
            };
            (*last.as_ptr()).next = append_first.as_ptr();
            (*append_first.as_ptr()).prev = last.as_ptr();
            queue.set_last(append_queue.last());
        }
        queue.set_count(queue.count().add(append_count));

        count
    }
}

#[used]
static MI_PAGE_QUEUE_APPEND_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    *mut mi_page_queue_t,
    *mut mi_page_queue_t,
) -> usize = _mi_page_queue_append;

#[inline]
unsafe fn mi_page_queue_find_free_ex(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    first_try: bool,
) -> *mut mi_page_t {
    unsafe {
        let queue = page_queue_ptr(pq);
        let Some(queue) = crate::core_types::NormalPageQueue::new(queue) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let Some(block_size) = queue.ptr().block_size() else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_page_queue_find_free_ex_typed(theap, queue, block_size, first_try)
            .map(|page| page.page().as_ptr())
            .unwrap_or(ptr::null_mut())
    }
}

#[inline]
unsafe fn mi_page_queue_find_free_ex_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
    first_try: bool,
) -> Option<crate::core_types::ActivePage> {
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        let pq = queue.ptr().as_ptr();
        let mut count = crate::core_types::PageQueueCount::new(0);
        let mut candidate_limit = crate::core_types::PageReclaimLimit::new(0);
        let mut page_full_retain = if block_size.get() > MI_SMALL_MAX_OBJ_SIZE {
            crate::core_types::PageReclaimLimit::new(0)
        } else {
            crate::core_types::PageReclaimLimit::new((*theap_raw).page_full_retain)
        };
        let mut page_candidate = ptr::null_mut::<mi_page_t>();
        let mut page = queue
            .ptr()
            .first()
            .map(PagePtr::as_ptr)
            .unwrap_or(ptr::null_mut());

        while !page.is_null() {
            let next = (*page).next;
            count = count.increment();
            candidate_limit = candidate_limit.decrement();

            let mut immediate_available = mi_page_immediate_available(page);
            if !immediate_available {
                _mi_page_free_collect(page, false);
                immediate_available = mi_page_immediate_available(page);
            }

            if !immediate_available && !mi_page_is_expandable(page) {
                page_full_retain = page_full_retain.decrement();
                if page_full_retain.is_negative() {
                    crate::mi_assert_internal!(
                        !mi_page_is_in_full(page) && !mi_page_immediate_available(page)
                    );
                    mi_page_to_full(page, pq);
                }
            } else {
                if page_candidate.is_null() {
                    page_candidate = page;
                    candidate_limit = crate::core_types::PageReclaimLimit::new(
                        _mi_option_get_fast(mi_option_t::mi_option_page_max_candidates),
                    );
                } else if mi_page_all_free(page_candidate) {
                    let active = crate::core_types::ActivePage::new_unchecked(
                        crate::core_types::PagePtr::new_unchecked(page_candidate),
                    );
                    let all_free = crate::core_types::AllFreePage::active_unchecked(active);
                    let queued =
                        crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(
                            active, queue,
                        );
                    let _ = _mi_page_free_active_internal(all_free, queued);
                    page_candidate = page;
                } else if mi_page_used_count(page) >= mi_page_used_count(page_candidate)
                    && !mi_page_is_mostly_used(page)
                {
                    page_candidate = page;
                }
                if immediate_available || candidate_limit.is_exhausted() {
                    crate::mi_assert_internal!(!page_candidate.is_null());
                    break;
                }
            }

            page = next;
        }

        __mi_stat_counter_increase(
            ptr::addr_of_mut!((*theap_raw).stats.page_searches),
            count.get(),
        );
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap_raw).stats.page_searches_count), 1);

        if !page_candidate.is_null() {
            page = page_candidate;
        }
        if !page.is_null() {
            if !mi_page_immediate_available(page) {
                crate::mi_assert_internal!(mi_page_is_expandable(page));
                if !mi_page_extend_free(theap_raw, page) {
                    page = ptr::null_mut();
                }
            }
            crate::mi_assert_internal!(page.is_null() || mi_page_immediate_available(page));
        }

        if page.is_null() {
            _mi_theap_collect_retired_typed(theap, false);
            let Some(slice_count) = crate::core_types::SliceCount::from_usize(
                crate::internal::mi_slice_count_of_size(block_size.get()),
            ) else {
                crate::mi_assert_internal!(false);
                return None;
            };
            if let Some(fresh) = mi_page_fresh_typed(theap, queue, block_size, slice_count) {
                crate::mi_assert_internal!(mi_page_immediate_available(fresh.page().as_ptr()));
                return Some(fresh);
            }
            if first_try {
                let page = mi_page_queue_find_free_ex_typed(theap, queue, block_size, false);
                crate::mi_assert_internal!(
                    page.is_none_or(|page| mi_page_immediate_available(page.page().as_ptr()))
                );
                return page;
            }
        } else {
            crate::mi_assert_internal!(mi_page_immediate_available(page));
            let active = crate::core_types::ActivePage::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            );
            let queued =
                crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new_unchecked(
                    active, queue,
                );
            let _ = mi_page_queue_move_to_front_typed(theap, queue, queued);
            mi_page_reset_retire_expire(page);
        }
        crate::mi_assert_internal!(page.is_null() || mi_page_immediate_available(page));

        crate::core_types::PagePtr::new(page)
            .map(|page| crate::core_types::ActivePage::new_unchecked(page))
    }
}

#[inline]
unsafe fn mi_find_free_page(theap: *mut mi_theap_t, pq: *mut mi_page_queue_t) -> *mut mi_page_t {
    unsafe {
        let queue = page_queue_ptr(pq);
        let Some(queue) = crate::core_types::NormalPageQueue::new(queue) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let Some(block_size) = queue.ptr().block_size() else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_find_free_page_typed(theap, queue, block_size)
            .map(|page| page.page().as_ptr())
            .unwrap_or(ptr::null_mut())
    }
}

#[inline]
#[allow(clippy::absurd_extreme_comparisons)]
unsafe fn mi_find_free_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    queue: crate::core_types::NormalPageQueue,
    block_size: crate::core_types::BlockSize,
) -> Option<crate::core_types::ActivePage> {
    unsafe {
        let page = queue.ptr().first();
        if let Some(page) = page.filter(|page| mi_page_free_quick_collect(page.as_ptr())) {
            let page_raw = page.as_ptr();
            if MI_SECURE >= 2
                && mi_page_capacity_count(page_raw).get() < mi_page_reserved_count(page_raw).get()
                && ((_mi_theap_random_next(theap.ptr().as_ptr()) & 1) == 1)
            {
                let _ = mi_page_extend_free(theap.ptr().as_ptr(), page_raw);
                crate::mi_assert_internal!(mi_page_immediate_available(page_raw));
            }
            mi_page_reset_retire_expire(page_raw);
            Some(crate::core_types::ActivePage::new_unchecked(page))
        } else {
            mi_page_queue_find_free_ex_typed(theap, queue, block_size, true)
        }
    }
}

/// # Safety
/// `theap` must point to writable thread-heap metadata.
#[inline(always)]
unsafe fn mi_theap_increment_heartbeat(theap: *mut mi_theap_t) -> crate::core_types::Heartbeat {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_theap_increment_heartbeat_typed(theap)
    }
}

/// # Safety
/// `theap` must identify writable local thread-heap metadata.
#[inline(always)]
unsafe fn mi_theap_increment_heartbeat_typed(
    theap: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::Heartbeat {
    unsafe {
        let heartbeat = theap.ptr().heartbeat().increment_wrapping();
        theap.ptr().set_heartbeat(heartbeat);
        heartbeat
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_deferred_free(theap: *mut mi_theap_t, force: bool) {
    unsafe {
        let heartbeat = mi_theap_increment_heartbeat(theap);
        let fun_ptr = crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_free))
            .load(crate::core_types::MemoryOrder::Acquire);
        let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_deferred_free_typed(theap, force, heartbeat, callback);
    }
}

/// # Safety
/// `theap` must identify the live local thread heap whose deferred-free callback
/// is being dispatched.
#[inline(always)]
unsafe fn mi_deferred_free_typed(
    theap: crate::core_types::ThreadHeapLocal,
    force: bool,
    heartbeat: crate::core_types::Heartbeat,
    callback: crate::core_types::DeferredFreeCallback,
) {
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        if callback.is_some() && !(*(*theap_raw).tld).recurse {
            (*(*theap_raw).tld).recurse = true;
            let arg = crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_arg))
                .load(crate::core_types::MemoryOrder::Acquire);
            let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);
            callback.invoke(force, heartbeat, arg);
            (*(*theap_raw).tld).recurse = false;
        }
    }
}

/// # Safety
/// `fn_`, when present, must remain callable for the lifetime of its registration. `arg` is stored
/// opaquely and passed back to that callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_register_deferred_free(
    fn_: Option<mi_deferred_free_fun>,
    arg: *mut c_void,
) {
    unsafe {
        let callback = crate::core_types::DeferredFreeCallback::new(fn_);
        let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);
        crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_arg))
            .store(arg.as_ptr(), crate::core_types::MemoryOrder::Release);
        let f = callback.storage_ptr();
        crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_free))
            .store(f, crate::core_types::MemoryOrder::Release);
    }
}

#[inline]
unsafe fn mi_huge_page_alloc(
    theap: *mut mi_theap_t,
    size: usize,
    page_alignment: usize,
    pq: *mut mi_page_queue_t,
) -> *mut mi_page_t {
    unsafe {
        let block_size = _mi_os_good_alloc_size(size);
        crate::mi_assert_internal!(mi_page_queue_is_huge(pq));
        let owner = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let Some(queue) = crate::core_types::HugePageQueue::new(
            crate::core_types::PageQueuePtr::new_unchecked(pq),
        ) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
            crate::core_types::PaddedSize::new_unchecked(block_size),
            crate::core_types::Zeroed::No,
        );
        let Some(page_alignment) = crate::core_types::MaybeAlignment::new(page_alignment) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        mi_huge_page_alloc_typed(owner, layout, page_alignment, queue)
            .map(|page| page.page().as_ptr())
            .unwrap_or(ptr::null_mut())
    }
}

#[inline]
unsafe fn mi_huge_page_alloc_typed(
    owner: crate::core_types::ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    page_alignment: crate::core_types::MaybeAlignment,
    queue: crate::core_types::HugePageQueue,
) -> Option<crate::core_types::ActivePage> {
    unsafe {
        let theap = owner.ptr().as_ptr();
        let pq = queue.ptr().as_ptr();
        let block_size = layout.padded().get();
        let page_alignment = page_alignment.get();
        let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);
        if !page.is_null() {
            crate::mi_assert_internal!(mi_page_block_size(page) >= layout.padded().get());
            crate::mi_assert_internal!(mi_page_immediate_available(page));
            crate::mi_assert_internal!(mi_page_is_huge(page));
            crate::mi_assert_internal!(mi_page_is_singleton(page));
            if MI_STAT > 0 {
                __mi_stat_increase(
                    ptr::addr_of_mut!((*theap).stats.malloc_huge),
                    mi_page_block_size(page),
                );
                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_huge_count), 1);
            }
        }
        crate::core_types::PagePtr::new(page)
            .map(|page| crate::core_types::ActivePage::new_unchecked(page))
    }
}

#[inline(always)]
unsafe fn mi_find_page(
    theap: *mut mi_theap_t,
    size: usize,
    huge_alignment: usize,
) -> *mut mi_page_t {
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        let size = crate::core_types::PaddedSize::new_unchecked(size);
        let Some(huge_alignment) = crate::core_types::MaybeAlignment::new(huge_alignment) else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        mi_find_page_typed(theap, size, huge_alignment)
            .map(|page| page.page().as_ptr())
            .unwrap_or(ptr::null_mut())
    }
}

const _: unsafe fn(*mut mi_page_queue_t, crate::core_types::PageQueueCount) =
    mi_page_queue_add_count;
const _: unsafe fn(*mut mi_page_t) -> crate::core_types::UsedBlockCount =
    mi_page_increment_used_count;
const _: unsafe fn(*mut mi_theap_t, *mut mi_page_queue_t, *mut mi_page_t) =
    mi_page_queue_push_at_end_raw;
const _: unsafe fn(
    crate::core_types::FullPageQueue,
    crate::core_types::QueuedPage<crate::core_types::NormalQueue>,
) -> crate::core_types::FullPage = mi_page_queue_enqueue_from;
const _: unsafe fn(*mut mi_theap_t, *mut mi_page_queue_t, bool) -> *mut mi_page_t =
    mi_page_queue_find_free_ex;
const _: unsafe fn(*mut mi_theap_t, *mut mi_page_queue_t) -> *mut mi_page_t = mi_find_free_page;
const _: unsafe fn(*mut mi_theap_t, usize, usize, *mut mi_page_queue_t) -> *mut mi_page_t =
    mi_huge_page_alloc;
const _: unsafe fn(*mut mi_theap_t, usize, usize) -> *mut mi_page_t = mi_find_page;

#[inline(always)]
unsafe fn mi_find_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    size: crate::core_types::PaddedSize,
    huge_alignment: crate::core_types::MaybeAlignment,
) -> Option<crate::core_types::ActivePage> {
    unsafe {
        let req_size = size.get().wrapping_sub(MI_PADDING_SIZE);
        if req_size > MI_MAX_ALLOC_SIZE {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!("allocation request is too large ({req_size} bytes)\n"),
            );
            return None;
        }

        let queue_block_size = if huge_alignment.get() > 0 {
            MI_LARGE_MAX_OBJ_SIZE + 1
        } else {
            size.get()
        };
        let pq = mi_page_queue(
            theap.ptr().as_ptr(),
            crate::core_types::BlockSize::new_unchecked(queue_block_size),
        );
        if mi_page_queue_is_huge(pq) || req_size > MI_MAX_ALLOC_SIZE {
            let Some(queue) = crate::core_types::HugePageQueue::new(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ) else {
                crate::mi_assert_internal!(false);
                return None;
            };
            let block_size = _mi_os_good_alloc_size(size.get());
            let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
                crate::core_types::PaddedSize::new_unchecked(block_size),
                crate::core_types::Zeroed::No,
            );
            mi_huge_page_alloc_typed(theap, layout, huge_alignment, queue)
        } else {
            let Some(queue) = crate::core_types::NormalPageQueue::new(
                crate::core_types::PageQueuePtr::new_unchecked(pq),
            ) else {
                crate::mi_assert_internal!(false);
                return None;
            };
            let Some(block_size) = queue.ptr().block_size() else {
                crate::mi_assert_internal!(false);
                return None;
            };
            mi_find_free_page_typed(theap, queue, block_size)
        }
    }
}

pub(crate) unsafe extern "C" fn _mi_malloc_generic(
    theap: *mut mi_theap_t,
    size: usize,
    zero_huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        // SAFETY: `zero_huge_alignment` is the C-compatible packed flag word:
        // bit 0 is zeroing and the remaining bits are the huge-alignment payload.
        let flags = crate::core_types::MallocGenericFlags::from_raw(zero_huge_alignment);
        if size == 0 {
            return ptr::null_mut();
        }
        let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
            crate::core_types::PaddedSize::new_unchecked(size),
            flags.zeroed(),
        );
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        _mi_malloc_generic_typed(theap, layout, flags, usable)
    }
}

#[inline(always)]
pub(crate) unsafe fn _mi_theap_or_default_typed(
    mut theap: *mut mi_theap_t,
) -> Option<crate::core_types::ThreadHeapPtr> {
    unsafe {
        if !mi_theap_is_initialized(theap) {
            if ptr::eq(theap, ptr::addr_of!(_mi_theap_empty_wrong).cast_mut()) {
                return None;
            }
            crate::mimalloc::mi_thread_init();
            theap = crate::prim::_mi_theap_default();
            if !mi_theap_is_initialized(theap) {
                return None;
            }
            crate::mi_assert_internal!(crate::prim::_mi_theap_default() == theap);
        }
        crate::mi_assert_internal!(mi_theap_is_initialized(theap));

        Some(crate::core_types::ThreadHeapPtr::new_unchecked(theap))
    }
}

pub(crate) unsafe fn _mi_malloc_generic_typed(
    theap: crate::core_types::ThreadHeapPtr,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let local_theap = crate::core_types::ThreadHeapLocal::new_unchecked(theap);
        let theap = local_theap.ptr().as_ptr();
        let size = layout.padded().get();
        let zero = flags.zeroed() == crate::core_types::Zeroed::Yes;
        let Some(huge_alignment) =
            crate::core_types::MaybeAlignment::new(flags.huge_alignment().get())
        else {
            crate::mi_assert_internal!(false);
            return ptr::null_mut();
        };

        (*theap).generic_count += 1;
        if (*theap).generic_count >= 1000 {
            (*theap).generic_collect_count += (*theap).generic_count;
            (*theap).generic_count = 0;
            _mi_deferred_free(theap, false);
            _mi_theap_collect_retired(theap, false);

            let generic_collect =
                mi_option_get_clamp(mi_option_t::mi_option_generic_collect, 1, 1_000_000);
            if (*theap).generic_collect_count >= generic_collect {
                (*theap).generic_collect_count = 0;
                crate::mimalloc::mi_theap_collect(theap, false);
            }
        }

        let mut active = mi_find_page_typed(local_theap, layout.padded(), huge_alignment);
        if active.is_none() {
            crate::mimalloc::mi_theap_collect(theap, true);
            active = mi_find_page_typed(local_theap, layout.padded(), huge_alignment);
        }

        let Some(active) = active else {
            let req_size = size.wrapping_sub(MI_PADDING_SIZE);
            _mi_error_message_fmt(
                Errno::ENOMEM,
                format_args!("unable to allocate memory ({req_size} bytes)\n"),
            );
            return ptr::null_mut();
        };
        let page = active.page().as_ptr();

        crate::mi_assert_internal!(mi_page_immediate_available(page));
        crate::mi_assert_internal!(mi_page_block_size(page) >= size);
        crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
            mi_page_slice_start(page).cast(),
            MI_PAGE_ALIGN
        ));
        crate::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);

        if !usable.is_null() {
            *usable = mi_page_usable_block_size(page).get();
        }
        let available = active.available().unwrap_unchecked();
        let p =
            _mi_page_malloc_zero_typed(theap, available, layout.padded(), zero, ptr::null_mut());
        crate::mi_assert_internal!(!p.is_null());

        if mi_page_block_size(page) > MI_SMALL_MAX_OBJ_SIZE
            && !mi_page_immediate_available(page)
            && !mi_page_is_expandable(page)
        {
            mi_page_to_full(page, mi_page_queue_of(page));
        }
        p
    }
}
