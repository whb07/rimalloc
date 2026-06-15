//! Page management

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc,
    clippy::nonminimal_bool
)]

use core::ffi::{c_long, c_void};
use core::mem::size_of;
use core::ptr;

use crate::arena::{
    _mi_arenas_collect, _mi_arenas_page_abandon, _mi_arenas_page_alloc, _mi_arenas_page_free,
    MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE,
};
use crate::atomic::{
    EFAULT, mi_atomic_and_relaxed, mi_atomic_cas_weak_acq_rel, mi_atomic_cas_weak_release,
    mi_atomic_load_explicit, mi_atomic_load_ptr_acquire, mi_atomic_or_relaxed,
    mi_atomic_store_ptr_release, mi_memory_order_e,
};
use crate::bits::{MI_KiB, MI_SIZE_SIZE};
use crate::init::{_mi_page_empty, _mi_theap_empty_wrong};
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_increase, _mi_align_up, _mi_error_message,
    _mi_is_heap_main, _mi_os_commit, _mi_os_good_alloc_size, _mi_os_page_size,
    _mi_page_malloc_zero, _mi_ptr_page, _mi_theap_heap, _mi_theap_random_next, _mi_wsize_from_size,
    MI_DEBUG, MI_SECURE, mi_block_next, mi_block_set_next, mi_mem_is_zero, mi_memkind_t,
    mi_page_all_free, mi_page_area, mi_page_block_size, mi_page_committed,
    mi_page_contains_address, mi_page_flags, mi_page_has_interior_pointers,
    mi_page_immediate_available, mi_page_is_expandable, mi_page_is_mostly_used,
    mi_page_is_singleton, mi_page_queue, mi_page_set_has_interior_pointers, mi_page_set_keys,
    mi_page_slice_offset_of, mi_page_slice_start, mi_page_start, mi_page_thread_free,
    mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::options::{
    _mi_error_message_fmt, _mi_option_get_fast, mi_option_get_clamp, mi_option_t,
};
use crate::stats::{MI_BIN_HUGE, bin_block_size};
use crate::track::{
    MI_TRACK_ENABLED, mi_track_mem_defined, mi_track_mem_noaccess, mi_track_mem_undefined,
};
use crate::types::{
    ENOMEM, EOVERFLOW, MI_BIN_FULL, MI_ENABLE_LARGE_PAGES, MI_LARGE_MAX_OBJ_WSIZE,
    MI_PAGE_FLAG_MASK, MI_PAGE_IN_FULL_QUEUE, MI_PAGE_MIN_COMMIT_SIZE, MI_SMALL_SIZE_MAX, MI_STAT,
    MI_THREADID_ABANDONED, MI_THREADID_ABANDONED_MAPPED, mi_block_t, mi_deferred_free_fun,
    mi_page_flags_t, mi_page_queue_t, mi_page_t, mi_theap_t,
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

pub extern "C" fn mi_bin(size: usize) -> usize {
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

pub extern "C" fn _mi_bin(size: usize) -> usize {
    mi_bin(size)
}

pub extern "C" fn _mi_bin_size(bin: usize) -> usize {
    crate::mi_debug_assert!(bin <= MI_BIN_HUGE);
    bin_block_size(bin)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_good_size(size: usize) -> usize {
    if size <= MI_LARGE_MAX_OBJ_SIZE {
        _mi_bin_size(mi_bin(size + MI_PADDING_SIZE))
    } else if size <= MI_MAX_ALLOC_SIZE {
        align_up(size + MI_PADDING_SIZE, MI_OS_PAGE_SIZE)
    } else {
        size
    }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_is_huge(pq: *const mi_page_queue_t) -> bool {
    unsafe { (*pq).block_size == (MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>()) }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_is_full(pq: *const mi_page_queue_t) -> bool {
    unsafe { (*pq).block_size == (MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>())) }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_is_special(pq: *const mi_page_queue_t) -> bool {
    unsafe { (*pq).block_size > MI_LARGE_MAX_OBJ_SIZE }
}

/// # Safety
/// `pq` must point to a valid `mi_page_queue_t`.
pub unsafe extern "C" fn mi_page_queue_count(pq: *const mi_page_queue_t) -> usize {
    unsafe { (*pq).count }
}

#[inline]
pub unsafe fn mi_page_block_at(
    page: *const mi_page_t,
    page_start: *mut c_void,
    block_size: usize,
    i: usize,
) -> *mut mi_block_t {
    crate::MI_UNUSED!(page);
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_internal!(i <= (*page).reserved as usize);
    unsafe {
        page_start
            .cast::<u8>()
            .add(i * block_size)
            .cast::<mi_block_t>()
    }
}

#[inline]
pub unsafe fn mi_page_list_count(page: *mut mi_page_t, mut head: *mut mi_block_t) -> usize {
    crate::mi_assert_internal!(_mi_ptr_page((*page).page_start.cast()) == page);
    let slice_start = mi_page_slice_start(page);
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        slice_start.cast(),
        MI_PAGE_ALIGN
    ));
    let mut count = 0usize;
    while !head.is_null() {
        crate::mi_assert_internal!(
            (head as usize).wrapping_sub(slice_start as usize) > MI_LARGE_PAGE_SIZE
                || page == _mi_ptr_page(head.cast())
        );
        count += 1;
        head = mi_block_next(page, head);
    }
    count
}

#[inline]
pub unsafe fn mi_page_list_is_valid(page: *mut mi_page_t, mut p: *mut mi_block_t) -> bool {
    let mut psize = 0usize;
    let page_area = mi_page_area(page, &mut psize).cast::<mi_block_t>();
    let end = unsafe { page_area.cast::<u8>().add(psize).cast::<mi_block_t>() };
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
                ubsize - size_of::<mi_block_t>()
            ));
            block = mi_block_next(page, block);
        }
    }
    true
}

#[inline]
pub unsafe fn mi_page_is_valid_init(page: *mut mi_page_t) -> bool {
    crate::mi_assert_internal!(mi_page_block_size(page) > 0);
    crate::mi_assert_internal!((*page).used <= (*page).capacity);
    crate::mi_assert_internal!((*page).capacity <= (*page).reserved);
    crate::mi_assert_internal!(mi_page_list_is_valid(page, (*page).free));
    crate::mi_assert_internal!(mi_page_list_is_valid(page, (*page).local_free));

    if MI_DEBUG > 3 && (*page).free_is_zero {
        let ubsize = mi_page_usable_block_size(page);
        let mut block = (*page).free;
        while !block.is_null() {
            crate::mi_assert_expensive!(mi_mem_is_zero(
                block.add(1).cast(),
                ubsize - size_of::<mi_block_t>()
            ));
            block = mi_block_next(page, block);
        }
    }

    if MI_TRACK_ENABLED == 0 {
        let tfree = mi_page_thread_free(page);
        crate::mi_assert_internal!(mi_page_list_is_valid(page, tfree));
    }

    let free_count =
        mi_page_list_count(page, (*page).free) + mi_page_list_count(page, (*page).local_free);
    crate::mi_assert_internal!((*page).used as usize + free_count == (*page).capacity as usize);
    true
}

#[inline]
pub unsafe fn _mi_page_is_valid(page: *mut mi_page_t) -> bool {
    crate::mi_assert_internal!(mi_page_is_valid_init(page));
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    if MI_SECURE != 0 {
        crate::mi_assert_internal!((*page).keys[0] != 0);
    }
    if !mi_page_is_abandoned(page) {
        let pq = mi_page_queue_of(page);
        crate::mi_assert_internal!(mi_page_queue_contains(pq, page));
        crate::mi_assert_internal!(
            (*pq).block_size == mi_page_block_size(page)
                || mi_page_is_huge(page)
                || mi_page_is_in_full(page)
        );
        crate::mi_assert_internal!(mi_page_contains_address(page, page.cast()));
    }
    true
}

#[inline]
pub unsafe fn mi_page_thread_collect_to_local(page: *mut mi_page_t, head: *mut mi_block_t) {
    if head.is_null() {
        return;
    }

    let max_count = (*page).capacity as usize;
    let mut count = 1usize;
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
        _mi_error_message(EFAULT, c"corrupted thread-free list\n".as_ptr());
        return;
    } else if count > (*page).used as usize {
        _mi_error_message(
            EFAULT,
            c"corrupted meta-data in thread-free list\n".as_ptr(),
        );
        return;
    }

    mi_block_set_next(page, last, (*page).local_free);
    (*page).local_free = head;

    crate::mi_assert_internal!(u16::try_from(count).is_ok());
    crate::mi_assert_internal!((*page).used >= count as u16);
    (*page).used -= count as u16;
}

#[inline]
pub unsafe fn mi_page_thread_free_collect(page: *mut mi_page_t) {
    let mut tfree = mi_atomic_load_explicit(
        ptr::addr_of!((*page).xthread_free).cast(),
        mi_memory_order_e::mi_memory_order_relaxed,
    );
    let mut head;
    loop {
        head = crate::internal::mi_tf_block(tfree);
        if head.is_null() {
            return;
        }
        let tfreex =
            crate::internal::mi_tf_create(ptr::null_mut(), crate::internal::mi_tf_is_owned(tfree));
        if mi_atomic_cas_weak_acq_rel(
            ptr::addr_of_mut!((*page).xthread_free).cast(),
            &mut tfree,
            tfreex,
        ) {
            break;
        }
    }
    crate::mi_assert_internal!(!head.is_null());
    mi_page_thread_collect_to_local(page, head);
}

#[inline]
pub unsafe fn mi_page_free_quick_collect(page: *mut mi_page_t) -> bool {
    if !(*page).free.is_null() {
        return true;
    }
    if (*page).local_free.is_null() {
        return false;
    }
    (*page).free = (*page).local_free;
    (*page).local_free = ptr::null_mut();
    (*page).free_is_zero = false;
    true
}

#[inline]
pub unsafe fn _mi_page_free_collect(page: *mut mi_page_t, force: bool) {
    crate::mi_assert_internal!(!page.is_null());

    mi_page_thread_free_collect(page);

    if !(*page).local_free.is_null() {
        if (*page).free.is_null() {
            (*page).free = (*page).local_free;
            (*page).local_free = ptr::null_mut();
            (*page).free_is_zero = false;
        } else if force {
            let mut tail = (*page).local_free;
            loop {
                let next = mi_block_next(page, tail);
                if next.is_null() {
                    break;
                }
                tail = next;
            }
            mi_block_set_next(page, tail, (*page).free);
            (*page).free = (*page).local_free;
            (*page).local_free = ptr::null_mut();
            (*page).free_is_zero = false;
        }
    }

    crate::mi_assert_internal!(!force || (*page).local_free.is_null());
}

#[inline]
pub unsafe fn _mi_page_free_collect_partly(page: *mut mi_page_t, head: *mut mi_block_t) {
    if head.is_null() {
        return;
    }
    let next = mi_block_next(page, head);
    if !next.is_null() {
        mi_block_set_next(page, head, ptr::null());
        mi_page_thread_collect_to_local(page, next);
        if !(*page).local_free.is_null() && (*page).free.is_null() {
            (*page).free = (*page).local_free;
            (*page).local_free = ptr::null_mut();
            (*page).free_is_zero = false;
        }
    }
    if (*page).used == 1 {
        crate::mi_assert_internal!(
            crate::internal::mi_tf_block(mi_atomic_load_explicit(
                ptr::addr_of!((*page).xthread_free).cast(),
                mi_memory_order_e::mi_memory_order_relaxed,
            )) == head
        );
        crate::mi_assert_internal!(mi_block_next(page, head).is_null());
        _mi_page_free_collect(page, false);
    }
}

#[inline]
pub unsafe fn _mi_page_unfull(page: *mut mi_page_t) {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
    crate::mi_assert_internal!(mi_page_is_in_full(page));
    crate::mi_assert_internal!(!(*mi_page_theap(page)).allow_page_abandon);
    if !mi_page_is_in_full(page) {
        return;
    }

    let theap = mi_page_theap(page);
    let pqfull = ptr::addr_of_mut!((*theap).pages[MI_BIN_FULL]);
    mi_page_set_in_full(page, false);
    let pq = mi_theap_page_queue_of(theap, page);
    mi_page_set_in_full(page, true);
    mi_page_queue_enqueue_from_full(pq, pqfull, page);
}

#[allow(dead_code)]
#[inline]
unsafe fn mi_page_to_full(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    crate::mi_assert_internal!(pq == mi_page_queue_of(page));
    crate::mi_assert_internal!(!mi_page_immediate_available(page));
    crate::mi_assert_internal!(!mi_page_is_in_full(page));

    let theap = mi_page_theap(page);
    if (*theap).allow_page_abandon {
        _mi_page_abandon(page, pq);
    } else if !mi_page_is_in_full(page) {
        let full = ptr::addr_of_mut!((*theap).pages[MI_BIN_FULL]);
        mi_page_queue_enqueue_from(full, pq, page);
        _mi_page_free_collect(page, false);
    }
}

pub(crate) unsafe extern "C" fn _mi_page_free(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
    crate::mi_assert_internal!(pq == mi_page_queue_of(page));
    crate::mi_assert_internal!(mi_page_all_free(page));

    mi_page_set_has_interior_pointers(page, false);
    mi_page_queue_remove(pq, page);

    let theap = mi_page_theap(page);
    crate::mi_assert_internal!(!theap.is_null());
    mi_page_set_theap(page, ptr::null_mut());
    _mi_arenas_page_free(page, theap);
    _mi_arenas_collect(false, false, (*theap).tld);
}

const MI_RETIRE_CYCLES: u8 = 16;

pub(crate) unsafe extern "C" fn _mi_page_retire(page: *mut mi_page_t) {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
    crate::mi_assert_internal!(mi_page_all_free(page));

    if (*page).retire_expire != 0 {
        return;
    }
    mi_page_set_has_interior_pointers(page, false);

    let pq = mi_page_queue_of(page);
    if !mi_page_queue_is_special(pq) && ptr::eq((*pq).first, page) && ptr::eq((*pq).last, page) {
        let theap = mi_page_theap(page);
        if MI_STAT > 0 {
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);
        }
        let bsize = mi_page_block_size(page);
        (*page).retire_expire = if bsize <= MI_SMALL_MAX_OBJ_SIZE {
            MI_RETIRE_CYCLES
        } else {
            MI_RETIRE_CYCLES / 4
        };
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

    _mi_page_free(page, pq);
}

pub(crate) unsafe extern "C" fn _mi_theap_collect_retired(theap: *mut mi_theap_t, force: bool) {
    let mut min = MI_BIN_FULL;
    let mut max = 0usize;
    for bin in (*theap).page_retired_min..=(*theap).page_retired_max {
        let pq = ptr::addr_of_mut!((*theap).pages[bin]);
        let page = (*pq).first;
        if !page.is_null() && (*page).retire_expire != 0 {
            if mi_page_all_free(page) {
                (*page).retire_expire -= 1;
                if (*page).retire_expire == 0 || force {
                    _mi_page_free(page, pq);
                } else {
                    if bin < min {
                        min = bin;
                    }
                    if bin > max {
                        max = bin;
                    }
                }
            } else {
                (*page).retire_expire = 0;
            }
        }
    }
    (*theap).page_retired_min = min;
    (*theap).page_retired_max = max;
}

#[inline]
unsafe fn mi_page_free_list_extend_secure(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    bsize: usize,
    extend: usize,
) {
    if MI_SECURE < 3 {
        crate::mi_assert_internal!((*page).free.is_null());
        crate::mi_assert_internal!((*page).local_free.is_null());
    }
    crate::mi_assert_internal!((*page).capacity as usize + extend <= (*page).reserved as usize);
    crate::mi_assert_internal!(bsize == mi_page_block_size(page));
    let page_area = mi_page_start(page).cast::<c_void>();

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
            page,
            page_area,
            bsize,
            (*page).capacity as usize + i * slice_extend,
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
        mi_block_set_next(page, block, blocks[next]);
        current = next;
    }
    mi_block_set_next(page, blocks[current], (*page).free);
    (*page).free = free_start;
}

#[inline]
unsafe fn mi_page_free_list_extend(page: *mut mi_page_t, bsize: usize, extend: usize) {
    if MI_SECURE < 3 {
        crate::mi_assert_internal!((*page).free.is_null());
        crate::mi_assert_internal!((*page).local_free.is_null());
    }
    crate::mi_assert_internal!((*page).capacity as usize + extend <= (*page).reserved as usize);
    crate::mi_assert_internal!(bsize == mi_page_block_size(page));
    let page_area = mi_page_start(page).cast::<c_void>();
    let start = mi_page_block_at(page, page_area, bsize, (*page).capacity as usize);
    let last = mi_page_block_at(
        page,
        page_area,
        bsize,
        (*page).capacity as usize + extend - 1,
    );
    let mut block = start;
    while block <= last {
        let next = block.cast::<u8>().add(bsize).cast::<mi_block_t>();
        mi_block_set_next(page, block, next);
        block = next;
    }
    mi_block_set_next(page, last, (*page).free);
    (*page).free = start;
}

#[inline]
unsafe fn mi_page_extend_free(theap: *mut mi_theap_t, page: *mut mi_page_t) -> bool {
    crate::mi_assert_expensive!(mi_page_is_valid_init(page));
    if MI_SECURE < 3 {
        crate::mi_assert!((*page).free.is_null());
        crate::mi_assert!((*page).local_free.is_null());
        if !(*page).free.is_null() {
            return true;
        }
    }
    if (*page).capacity >= (*page).reserved {
        return true;
    }

    let bsize = mi_page_block_size(page);
    let mut extend = (*page).reserved as usize - (*page).capacity as usize;
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
    crate::mi_assert_internal!(extend + (*page).capacity as usize <= (*page).reserved as usize);
    crate::mi_assert_internal!(u16::try_from(extend).is_ok());

    if (*page).slice_committed > 0 {
        let needed_size = ((*page).capacity as usize + extend) * bsize;
        let needed_commit = _mi_align_up(
            mi_page_slice_offset_of(page, needed_size),
            MI_PAGE_MIN_COMMIT_SIZE,
        );
        if needed_commit > (*page).slice_committed {
            crate::mi_assert_internal!(
                (needed_commit - (*page).slice_committed).is_multiple_of(_mi_os_page_size())
            );
            if !_mi_os_commit(
                mi_page_slice_start(page)
                    .cast::<u8>()
                    .add((*page).slice_committed)
                    .cast::<c_void>(),
                needed_commit - (*page).slice_committed,
                ptr::null_mut(),
            ) {
                return false;
            }
            (*page).slice_committed = needed_commit;
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
    (*page).capacity += extend as u16;
    if MI_STAT > 0 {
        __mi_stat_increase(
            ptr::addr_of_mut!((*theap).stats.page_committed),
            extend * bsize,
        );
    }

    if MI_TRACK_ENABLED != 0 {
        let mut page_size = 0usize;
        let page_start = mi_page_area(page, &mut page_size);
        mi_track_mem_undefined(page_start.cast(), page_size);
        mi_track_mem_noaccess(page_start.cast(), page_size);
    }
    crate::mi_assert_expensive!(mi_page_is_valid_init(page));
    true
}

pub(crate) unsafe extern "C" fn _mi_page_abandon(page: *mut mi_page_t, pq: *mut mi_page_queue_t) {
    _mi_page_free_collect(page, false);
    if mi_page_all_free(page) {
        _mi_page_free(page, pq);
    } else {
        mi_page_queue_remove(pq, page);
        let theap = (*page).theap;
        mi_page_set_theap(page, ptr::null_mut());
        // Preserve the original theap pointer so same-theap reclaim-on-free can still work.
        (*page).theap = theap;
        _mi_arenas_page_abandon(page, theap);
        _mi_arenas_collect(false, false, (*theap).tld);
    }
}

#[inline]
pub unsafe fn mi_page_fresh_alloc(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    block_size: usize,
    page_alignment: usize,
) -> *mut mi_page_t {
    crate::mi_assert_internal!(pq.is_null() || mi_theap_contains_queue(theap, pq));
    crate::mi_assert_internal!(
        page_alignment > 0
            || block_size > MI_LARGE_MAX_OBJ_SIZE
            || pq.is_null()
            || block_size == (*pq).block_size
    );
    let page = _mi_arenas_page_alloc(theap, block_size, page_alignment);
    if page.is_null() {
        return ptr::null_mut();
    }
    if mi_page_is_abandoned(page) {
        _mi_theap_page_reclaim(theap, page);
        if !mi_page_immediate_available(page) {
            if mi_page_is_expandable(page) {
                if !mi_page_extend_free(theap, page) {
                    return ptr::null_mut();
                }
            } else {
                crate::mi_assert!(false);
                return ptr::null_mut();
            }
        }
    } else if !pq.is_null() {
        mi_page_queue_push(theap, pq, page);
    }
    crate::mi_assert_internal!(pq.is_null() || mi_page_block_size(page) >= block_size);
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
    page
}

pub(crate) unsafe extern "C" fn _mi_page_init(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
) -> bool {
    crate::mi_assert!(!page.is_null());
    crate::mi_assert!(!theap.is_null());

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
    crate::mi_assert_internal!((*page).reserved > 0);

    mi_page_set_keys(
        page,
        _mi_theap_random_next(theap),
        _mi_theap_random_next(theap),
    );

    if MI_DEBUG > 2 && (*page).memid.initially_zero {
        mi_track_mem_defined((*page).page_start.cast(), mi_page_committed(page));
        crate::mi_assert_expensive!(mi_mem_is_zero(page_start.cast(), mi_page_committed(page)));
    }

    crate::mi_assert_internal!(!(*page).theap.is_null());
    crate::mi_assert_internal!((*page).theap == mi_page_theap(page));
    crate::mi_assert_internal!((*page).capacity == 0);
    crate::mi_assert_internal!((*page).free.is_null());
    crate::mi_assert_internal!((*page).used == 0);
    crate::mi_assert_internal!(crate::internal::mi_page_is_owned(page));
    crate::mi_assert_internal!((*page).xthread_free == 1);
    crate::mi_assert_internal!((*page).next.is_null());
    crate::mi_assert_internal!((*page).prev.is_null());
    crate::mi_assert_internal!((*page).retire_expire == 0);
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
    crate::mi_assert_expensive!(mi_page_is_valid_init(page));

    if !mi_page_extend_free(theap, page) {
        return false;
    }
    crate::mi_assert!(mi_page_immediate_available(page));
    true
}

#[inline]
pub unsafe fn mi_page_fresh(theap: *mut mi_theap_t, pq: *mut mi_page_queue_t) -> *mut mi_page_t {
    crate::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
    let page = mi_page_fresh_alloc(theap, pq, (*pq).block_size, 0);
    if page.is_null() {
        return ptr::null_mut();
    }
    crate::mi_assert_internal!((*pq).block_size == mi_page_block_size(page));
    crate::mi_assert_internal!(pq == mi_theap_page_queue_of(theap, page));
    page
}

#[inline]
pub unsafe fn _mi_theap_page_reclaim(theap: *mut mi_theap_t, page: *mut mi_page_t) {
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        mi_page_slice_start(page).cast(),
        MI_PAGE_ALIGN,
    ));
    crate::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);
    crate::mi_assert_internal!(crate::internal::mi_page_is_owned(page));
    crate::mi_assert_internal!(mi_page_is_abandoned(page));

    mi_page_set_theap(page, theap);
    _mi_page_free_collect(page, false);
    let pq = mi_theap_page_queue_of(theap, page);
    mi_page_queue_push_at_end(theap, pq, page);
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
}

#[inline]
pub unsafe fn mi_page_is_huge(page: *const mi_page_t) -> bool {
    mi_page_is_singleton(page)
        && (mi_page_block_size(page) > MI_LARGE_MAX_OBJ_SIZE
            || (matches!(
                (*page).memid.memkind,
                mi_memkind_t::MI_MEM_OS
                    | mi_memkind_t::MI_MEM_OS_HUGE
                    | mi_memkind_t::MI_MEM_OS_REMAP
            ) && unsafe { (*page).memid.mem.os.base } < page.cast_mut().cast::<c_void>()))
}

#[inline]
pub unsafe fn mi_page_flags_set(page: *mut mi_page_t, set: bool, newflag: mi_page_flags_t) -> bool {
    let old = if set {
        mi_atomic_or_relaxed(&mut (*page).xthread_id, newflag)
    } else {
        mi_atomic_and_relaxed(&mut (*page).xthread_id, !newflag)
    };
    (old & newflag) == newflag
}

#[inline]
pub unsafe fn mi_page_is_in_full(page: *const mi_page_t) -> bool {
    (mi_page_flags(page) & MI_PAGE_IN_FULL_QUEUE) != 0
}

#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    !mi_page_immediate_available(page) && !mi_page_is_expandable(page)
}

#[inline]
pub unsafe fn mi_page_set_in_full(page: *mut mi_page_t, in_full: bool) {
    let was_in_full = mi_page_flags_set(page, in_full, MI_PAGE_IN_FULL_QUEUE);
    if was_in_full != in_full {
        let theap = (*page).theap;
        crate::mi_assert_internal!(!theap.is_null());
        if !theap.is_null() {
            let size = (*page).capacity as usize * mi_page_block_size(page);
            if in_full {
                (*theap).pages_full_size += size;
            } else {
                crate::mi_assert_internal!(size <= (*theap).pages_full_size);
                (*theap).pages_full_size -= size;
            }
        }
    }
}

#[inline]
pub unsafe fn mi_page_is_abandoned(page: *const mi_page_t) -> bool {
    let thread_id = (*page).xthread_id & !MI_PAGE_FLAG_MASK;
    thread_id <= MI_THREADID_ABANDONED_MAPPED
}

#[inline]
pub unsafe fn mi_page_set_theap(page: *mut mi_page_t, theap: *mut mi_theap_t) {
    (*page).theap = theap;
    let tid = if theap.is_null() {
        MI_THREADID_ABANDONED
    } else {
        (*(*theap).tld).thread_id
    };
    crate::mi_assert_internal!((tid & MI_PAGE_FLAG_MASK) == 0);

    let mut xtid_old = (*page).xthread_id;
    loop {
        let xtid = tid | (xtid_old & MI_PAGE_FLAG_MASK);
        if mi_atomic_cas_weak_release(&mut (*page).xthread_id, &mut xtid_old, xtid) {
            break;
        }
    }
}

#[inline]
pub unsafe fn mi_page_theap(page: *const mi_page_t) -> *mut mi_theap_t {
    crate::mi_assert_internal!(!mi_page_is_abandoned(page));
    crate::mi_assert_internal!(!(*page).theap.is_null());
    (*page).theap
}

#[inline]
pub unsafe fn mi_page_queue_contains(queue: *mut mi_page_queue_t, page: *const mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    let mut list = (*queue).first;
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

#[inline]
pub unsafe fn mi_theap_contains_queue(
    theap: *const mi_theap_t,
    pq: *const mi_page_queue_t,
) -> bool {
    let start = ptr::addr_of!((*theap).pages[0]) as usize;
    let end = ptr::addr_of!((*theap).pages[MI_BIN_FULL]) as usize;
    let queue = pq as usize;
    queue >= start && queue <= end
}

pub unsafe fn _mi_page_queue_is_valid(theap: *mut mi_theap_t, pq: *const mi_page_queue_t) -> bool {
    crate::MI_UNUSED_RELEASE!(theap);
    if pq.is_null() {
        return false;
    }
    let mut count = 0usize;
    crate::MI_UNUSED_RELEASE!(count);
    let mut prev = ptr::null_mut();
    crate::MI_UNUSED_RELEASE!(prev);
    let mut page = (*pq).first;
    while !page.is_null() {
        crate::mi_assert_internal!((*page).prev == prev);
        if mi_page_is_in_full(page) {
            crate::mi_assert_internal!(
                _mi_wsize_from_size((*pq).block_size) == MI_LARGE_MAX_OBJ_WSIZE + 2
            );
        } else if mi_page_is_huge(page) {
            crate::mi_assert_internal!(
                _mi_wsize_from_size((*pq).block_size) == MI_LARGE_MAX_OBJ_WSIZE + 1
            );
        } else {
            crate::mi_assert_internal!(mi_page_block_size(page) == (*pq).block_size);
        }
        crate::mi_assert_internal!((*page).theap == theap);
        if (*page).next.is_null() {
            crate::mi_assert_internal!((*pq).last == page);
        }
        count += 1;
        prev = page;
        page = (*page).next;
    }
    crate::mi_assert_internal!((*pq).count == count);
    true
}

#[inline]
pub unsafe fn mi_page_bin(page: *const mi_page_t) -> usize {
    let bin = if mi_page_is_in_full(page) {
        MI_BIN_FULL
    } else if mi_page_is_huge(page) {
        MI_BIN_HUGE
    } else {
        mi_bin(mi_page_block_size(page))
    };
    crate::mi_assert_internal!(bin <= MI_BIN_FULL);
    bin
}

pub unsafe fn _mi_page_stats_bin(page: *const mi_page_t) -> usize {
    let bin = if mi_page_is_huge(page) {
        MI_BIN_HUGE
    } else {
        mi_bin(mi_page_block_size(page))
    };
    crate::mi_assert_internal!(bin <= MI_BIN_HUGE);
    bin
}

#[inline]
pub unsafe fn mi_theap_page_queue_of(
    theap: *mut mi_theap_t,
    page: *const mi_page_t,
) -> *mut mi_page_queue_t {
    crate::mi_assert_internal!(!theap.is_null());
    let bin = mi_page_bin(page);
    let pq = ptr::addr_of_mut!((*theap).pages[bin]);
    crate::mi_assert_internal!(
        (mi_page_block_size(page) == (*pq).block_size)
            || (mi_page_is_huge(page) && mi_page_queue_is_huge(pq))
            || (mi_page_is_in_full(page) && mi_page_queue_is_full(pq))
    );
    pq
}

#[inline]
pub unsafe fn mi_page_queue_of(page: *const mi_page_t) -> *mut mi_page_queue_t {
    let theap = mi_page_theap(page);
    let pq = mi_theap_page_queue_of(theap, page);
    crate::mi_assert_expensive!(mi_page_queue_contains(pq, page));
    pq
}

pub unsafe fn mi_theap_queue_first_update(theap: *mut mi_theap_t, pq: *const mi_page_queue_t) {
    crate::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
    let size = (*pq).block_size;
    if size > MI_SMALL_SIZE_MAX {
        return;
    }

    let mut page = (*pq).first;
    if page.is_null() {
        page = ptr::addr_of!(_mi_page_empty).cast_mut();
    }

    let idx = _mi_wsize_from_size(size);
    let pages_free = ptr::addr_of_mut!((*theap).pages_free_direct).cast::<*mut mi_page_t>();
    if *pages_free.add(idx) == page {
        return;
    }

    let start = if idx <= 1 {
        0
    } else {
        crate::mi_assert_internal!(pq > ptr::addr_of!((*theap).pages[0]));
        let bin = mi_bin(size);
        let mut prev = pq.sub(1);
        while bin == mi_bin((*prev).block_size) && prev > ptr::addr_of!((*theap).pages[0]) {
            prev = prev.sub(1);
        }
        let start = 1 + _mi_wsize_from_size((*prev).block_size);
        start.min(idx)
    };

    crate::mi_assert_internal!(start <= idx);
    for sz in start..=idx {
        *pages_free.add(sz) = page;
    }
}

pub unsafe fn mi_page_queue_remove(queue: *mut mi_page_queue_t, page: *mut mi_page_t) {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_expensive!(mi_page_queue_contains(queue, page));
    crate::mi_assert_internal!((*queue).count >= 1);
    crate::mi_assert_internal!(
        mi_page_block_size(page) == (*queue).block_size
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
    if ptr::eq(page, (*queue).last) {
        (*queue).last = (*page).prev;
    }
    if ptr::eq(page, (*queue).first) {
        (*queue).first = (*page).next;
        crate::mi_assert_internal!(mi_theap_contains_queue(theap, queue));
        mi_theap_queue_first_update(theap, queue);
    }
    (*theap).page_count -= 1;
    (*queue).count -= 1;
    (*page).next = ptr::null_mut();
    (*page).prev = ptr::null_mut();
    mi_page_set_in_full(page, false);
}

pub unsafe fn mi_page_queue_push(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    crate::mi_assert_internal!(mi_page_theap(page) == theap);
    crate::mi_assert_internal!(!mi_page_queue_contains(queue, page));
    crate::mi_assert_internal!(
        mi_page_block_size(page) == (*queue).block_size
            || (mi_page_is_huge(page) && mi_page_queue_is_huge(queue))
            || (mi_page_is_in_full(page) && mi_page_queue_is_full(queue))
    );

    mi_page_set_in_full(page, mi_page_queue_is_full(queue));

    (*page).next = (*queue).first;
    (*page).prev = ptr::null_mut();
    if !(*queue).first.is_null() {
        crate::mi_assert_internal!((*(*queue).first).prev.is_null());
        (*(*queue).first).prev = page;
        (*queue).first = page;
    } else {
        (*queue).first = page;
        (*queue).last = page;
    }
    (*queue).count += 1;
    mi_theap_queue_first_update(theap, queue);
    (*theap).page_count += 1;
}

pub unsafe fn mi_page_queue_push_at_end(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    crate::mi_assert_internal!(mi_page_theap(page) == theap);
    crate::mi_assert_internal!(!mi_page_queue_contains(queue, page));
    crate::mi_assert_internal!(
        mi_page_block_size(page) == (*queue).block_size
            || (mi_page_is_huge(page) && mi_page_queue_is_huge(queue))
            || (mi_page_is_in_full(page) && mi_page_queue_is_full(queue))
    );

    mi_page_set_in_full(page, mi_page_queue_is_full(queue));

    (*page).prev = (*queue).last;
    (*page).next = ptr::null_mut();
    if !(*queue).last.is_null() {
        crate::mi_assert_internal!((*(*queue).last).next.is_null());
        (*(*queue).last).next = page;
        (*queue).last = page;
    } else {
        (*queue).first = page;
        (*queue).last = page;
    }
    (*queue).count += 1;

    if ptr::eq((*queue).first, page) {
        mi_theap_queue_first_update(theap, queue);
    }
    (*theap).page_count += 1;
}

pub unsafe fn mi_page_queue_move_to_front(
    theap: *mut mi_theap_t,
    queue: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    crate::mi_assert_internal!(mi_page_theap(page) == theap);
    crate::mi_assert_internal!(mi_page_queue_contains(queue, page));
    if ptr::eq((*queue).first, page) {
        return;
    }
    mi_page_queue_remove(queue, page);
    mi_page_queue_push(theap, queue, page);
    crate::mi_assert_internal!(ptr::eq((*queue).first, page));
}

pub unsafe fn mi_page_queue_enqueue_from_ex(
    to: *mut mi_page_queue_t,
    from: *mut mi_page_queue_t,
    enqueue_at_end: bool,
    page: *mut mi_page_t,
) {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_internal!((*from).count >= 1);
    crate::mi_assert_expensive!(mi_page_queue_contains(from, page));
    crate::mi_assert_expensive!(!mi_page_queue_contains(to, page));
    let bsize = mi_page_block_size(page);
    crate::MI_UNUSED!(bsize);
    crate::mi_assert_internal!(
        (bsize == (*to).block_size && bsize == (*from).block_size)
            || (bsize == (*to).block_size && mi_page_queue_is_full(from))
            || (bsize == (*from).block_size && mi_page_queue_is_full(to))
            || (mi_page_is_huge(page) && mi_page_queue_is_huge(to))
            || (mi_page_is_huge(page) && mi_page_queue_is_full(to))
    );

    let theap = mi_page_theap(page);

    if !(*page).prev.is_null() {
        (*(*page).prev).next = (*page).next;
    }
    if !(*page).next.is_null() {
        (*(*page).next).prev = (*page).prev;
    }
    if ptr::eq(page, (*from).last) {
        (*from).last = (*page).prev;
    }
    if ptr::eq(page, (*from).first) {
        (*from).first = (*page).next;
        crate::mi_assert_internal!(mi_theap_contains_queue(theap, from));
        mi_theap_queue_first_update(theap, from);
    }
    (*from).count -= 1;

    (*to).count += 1;
    if enqueue_at_end {
        (*page).prev = (*to).last;
        (*page).next = ptr::null_mut();
        if !(*to).last.is_null() {
            crate::mi_assert_internal!(theap == mi_page_theap((*to).last));
            (*(*to).last).next = page;
            (*to).last = page;
        } else {
            (*to).first = page;
            (*to).last = page;
            mi_theap_queue_first_update(theap, to);
        }
    } else if !(*to).first.is_null() {
        crate::mi_assert_internal!(theap == mi_page_theap((*to).first));
        let next = (*(*to).first).next;
        (*page).prev = (*to).first;
        (*page).next = next;
        (*(*to).first).next = page;
        if !next.is_null() {
            (*next).prev = page;
        } else {
            (*to).last = page;
        }
    } else {
        (*page).prev = ptr::null_mut();
        (*page).next = ptr::null_mut();
        (*to).first = page;
        (*to).last = page;
        mi_theap_queue_first_update(theap, to);
    }

    mi_page_set_in_full(page, mi_page_queue_is_full(to));
}

#[inline]
pub unsafe fn mi_page_queue_enqueue_from(
    to: *mut mi_page_queue_t,
    from: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    mi_page_queue_enqueue_from_ex(to, from, true, page);
}

#[inline]
pub unsafe fn mi_page_queue_enqueue_from_full(
    to: *mut mi_page_queue_t,
    from: *mut mi_page_queue_t,
    page: *mut mi_page_t,
) {
    mi_page_queue_enqueue_from_ex(to, from, true, page);
}

pub unsafe fn _mi_page_queue_append(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    append: *mut mi_page_queue_t,
) -> usize {
    crate::mi_assert_internal!(mi_theap_contains_queue(theap, pq));
    crate::mi_assert_internal!((*pq).block_size == (*append).block_size);

    if (*append).first.is_null() {
        return 0;
    }

    let mut count = 0usize;
    let mut page = (*append).first;
    while !page.is_null() {
        mi_page_set_theap(page, theap);
        count += 1;
        page = (*page).next;
    }
    crate::mi_assert_internal!(count == (*append).count);

    if (*pq).last.is_null() {
        crate::mi_assert_internal!((*pq).first.is_null());
        (*pq).first = (*append).first;
        (*pq).last = (*append).last;
        mi_theap_queue_first_update(theap, pq);
    } else {
        crate::mi_assert_internal!(!(*pq).last.is_null());
        crate::mi_assert_internal!(!(*append).first.is_null());
        (*(*pq).last).next = (*append).first;
        (*(*append).first).prev = (*pq).last;
        (*pq).last = (*append).last;
    }
    (*pq).count += (*append).count;

    count
}

#[inline]
unsafe fn mi_page_queue_find_free_ex(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    first_try: bool,
) -> *mut mi_page_t {
    let mut count = 0usize;
    let mut candidate_limit: c_long = 0;
    let mut page_full_retain = if (*pq).block_size > MI_SMALL_MAX_OBJ_SIZE {
        0
    } else {
        (*theap).page_full_retain
    };
    let mut page_candidate = ptr::null_mut::<mi_page_t>();
    let mut page = (*pq).first;

    while !page.is_null() {
        let next = (*page).next;
        count += 1;
        candidate_limit -= 1;

        let mut immediate_available = mi_page_immediate_available(page);
        if !immediate_available {
            _mi_page_free_collect(page, false);
            immediate_available = mi_page_immediate_available(page);
        }

        if !immediate_available && !mi_page_is_expandable(page) {
            page_full_retain -= 1;
            if page_full_retain < 0 {
                crate::mi_assert_internal!(
                    !mi_page_is_in_full(page) && !mi_page_immediate_available(page)
                );
                mi_page_to_full(page, pq);
            }
        } else {
            if page_candidate.is_null() {
                page_candidate = page;
                candidate_limit = _mi_option_get_fast(mi_option_t::mi_option_page_max_candidates);
            } else if mi_page_all_free(page_candidate) {
                _mi_page_free(page_candidate, pq);
                page_candidate = page;
            } else if (*page).used >= (*page_candidate).used && !mi_page_is_mostly_used(page) {
                page_candidate = page;
            }
            if immediate_available || candidate_limit <= 0 {
                crate::mi_assert_internal!(!page_candidate.is_null());
                break;
            }
        }

        page = next;
    }

    __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.page_searches), count);
    __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.page_searches_count), 1);

    if !page_candidate.is_null() {
        page = page_candidate;
    }
    if !page.is_null() {
        if !mi_page_immediate_available(page) {
            crate::mi_assert_internal!(mi_page_is_expandable(page));
            if !mi_page_extend_free(theap, page) {
                page = ptr::null_mut();
            }
        }
        crate::mi_assert_internal!(page.is_null() || mi_page_immediate_available(page));
    }

    if page.is_null() {
        _mi_theap_collect_retired(theap, false);
        page = mi_page_fresh(theap, pq);
        crate::mi_assert_internal!(page.is_null() || mi_page_immediate_available(page));
        if page.is_null() && first_try {
            page = mi_page_queue_find_free_ex(theap, pq, false);
            crate::mi_assert_internal!(page.is_null() || mi_page_immediate_available(page));
        }
    } else {
        crate::mi_assert_internal!(mi_page_immediate_available(page));
        mi_page_queue_move_to_front(theap, pq, page);
        (*page).retire_expire = 0;
    }
    crate::mi_assert_internal!(page.is_null() || mi_page_immediate_available(page));

    page
}

#[inline]
#[allow(clippy::absurd_extreme_comparisons)]
unsafe fn mi_find_free_page(theap: *mut mi_theap_t, pq: *mut mi_page_queue_t) -> *mut mi_page_t {
    crate::mi_assert_internal!(!mi_page_queue_is_huge(pq));

    let page = (*pq).first;
    if !page.is_null() && mi_page_free_quick_collect(page) {
        if MI_SECURE >= 2
            && (*page).capacity < (*page).reserved
            && ((_mi_theap_random_next(theap) & 1) == 1)
        {
            let _ = mi_page_extend_free(theap, page);
            crate::mi_assert_internal!(mi_page_immediate_available(page));
        }
        (*page).retire_expire = 0;
        page
    } else {
        mi_page_queue_find_free_ex(theap, pq, true)
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_deferred_free(theap: *mut mi_theap_t, force: bool) {
    (*theap).heartbeat = (*theap).heartbeat.wrapping_add(1);
    let fun_ptr = mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_free));
    if !fun_ptr.is_null() && !(*(*theap).tld).recurse {
        (*(*theap).tld).recurse = true;
        let arg = mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_arg));
        let fun: mi_deferred_free_fun = core::mem::transmute(fun_ptr);
        fun(force, (*theap).heartbeat, arg);
        (*(*theap).tld).recurse = false;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_register_deferred_free(
    fn_: Option<mi_deferred_free_fun>,
    arg: *mut c_void,
) {
    mi_atomic_store_ptr_release(ptr::addr_of_mut!(deferred_arg), arg);
    let f = fn_.map_or(ptr::null_mut(), |fun| fun as *const () as *mut c_void);
    mi_atomic_store_ptr_release(ptr::addr_of_mut!(deferred_free), f);
}

#[inline]
unsafe fn mi_huge_page_alloc(
    theap: *mut mi_theap_t,
    size: usize,
    page_alignment: usize,
    pq: *mut mi_page_queue_t,
) -> *mut mi_page_t {
    let block_size = _mi_os_good_alloc_size(size);
    crate::mi_assert_internal!(mi_page_queue_is_huge(pq));
    let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);
    if !page.is_null() {
        crate::mi_assert_internal!(mi_page_block_size(page) >= size);
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
    page
}

#[inline(always)]
unsafe fn mi_find_page(
    theap: *mut mi_theap_t,
    size: usize,
    huge_alignment: usize,
) -> *mut mi_page_t {
    let req_size = size.wrapping_sub(MI_PADDING_SIZE);
    if req_size > MI_MAX_ALLOC_SIZE {
        _mi_error_message_fmt(
            EOVERFLOW,
            format_args!("allocation request is too large ({req_size} bytes)\n"),
        );
        return ptr::null_mut();
    }

    let pq = mi_page_queue(
        theap,
        if huge_alignment > 0 {
            MI_LARGE_MAX_OBJ_SIZE + 1
        } else {
            size
        },
    );
    if mi_page_queue_is_huge(pq) || req_size > MI_MAX_ALLOC_SIZE {
        mi_huge_page_alloc(theap, size, huge_alignment, pq)
    } else {
        mi_find_free_page(theap, pq)
    }
}

pub(crate) unsafe extern "C" fn _mi_malloc_generic(
    mut theap: *mut mi_theap_t,
    size: usize,
    zero_huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    let zero = (zero_huge_alignment & 1) != 0;
    let huge_alignment = zero_huge_alignment & !1;

    if !mi_theap_is_initialized(theap) {
        if ptr::eq(theap, ptr::addr_of!(_mi_theap_empty_wrong).cast_mut()) {
            return ptr::null_mut();
        }
        crate::mimalloc::mi_thread_init();
        theap = crate::prim::_mi_theap_default();
        if !mi_theap_is_initialized(theap) {
            return ptr::null_mut();
        }
        crate::mi_assert_internal!(crate::prim::_mi_theap_default() == theap);
    }
    crate::mi_assert_internal!(mi_theap_is_initialized(theap));

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

    let mut page = mi_find_page(theap, size, huge_alignment);
    if page.is_null() {
        crate::mimalloc::mi_theap_collect(theap, true);
        page = mi_find_page(theap, size, huge_alignment);
    }

    if page.is_null() {
        let req_size = size.wrapping_sub(MI_PADDING_SIZE);
        _mi_error_message_fmt(
            ENOMEM,
            format_args!("unable to allocate memory ({req_size} bytes)\n"),
        );
        return ptr::null_mut();
    }

    crate::mi_assert_internal!(mi_page_immediate_available(page));
    crate::mi_assert_internal!(mi_page_block_size(page) >= size);
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        mi_page_slice_start(page).cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);

    if !usable.is_null() {
        *usable = mi_page_usable_block_size(page);
    }
    let p = _mi_page_malloc_zero(theap, page, size, zero);
    crate::mi_assert_internal!(!p.is_null());

    if mi_page_block_size(page) > MI_SMALL_MAX_OBJ_SIZE
        && !mi_page_immediate_available(page)
        && !mi_page_is_expandable(page)
    {
        mi_page_to_full(page, mi_page_queue_of(page));
    }
    p
}
