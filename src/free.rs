//! Mechanical translation of the early `src/free.c` free-path helpers.

#![allow(
    dead_code,
    clippy::missing_safety_doc,
    clippy::if_same_then_else,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;
use core::{cmp, ptr};

use crate::arena::{
    _mi_arenas_page_free, _mi_arenas_page_try_reabandon_to_mapped, _mi_arenas_page_unabandon,
};
use crate::atomic::{mi_atomic_cas_weak_acq_rel, mi_atomic_load_explicit, mi_memory_order_e};
use crate::bits::MI_INTPTR_SIZE;
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_decrease, _mi_align_down_ptr,
    _mi_arena_memid_is_suitable, _mi_is_power_of_two, _mi_memset_aligned, _mi_option_get_fast,
    _mi_os_page_size, _mi_ptr_page, _mi_safe_ptr_page, _mi_theap_heap, _mi_theap_page_reclaim,
    EAGAIN, EFAULT, EINVAL, MI_BLOCK_TAG_GUARDED, MI_DEBUG, MI_ENCODE_FREELIST, MI_GUARDED,
    MI_LARGE_MAX_OBJ_SIZE, MI_MiB, MI_PADDING, MI_PADDING_CHECK, MI_SECURE, MI_TRACK_ENABLED,
    mi_block_next, mi_block_nextx, mi_block_ptr_is_guarded, mi_block_set_next, mi_is_in_same_page,
    mi_page_all_free, mi_page_block_size, mi_page_contains_address, mi_page_has_interior_pointers,
    mi_page_is_abandoned_mapped, mi_page_is_full, mi_page_is_mostly_used, mi_page_is_owned,
    mi_page_is_singleton, mi_page_queue, mi_page_start, mi_page_thread_free,
    mi_page_usable_block_size, mi_page_xthread_id, mi_ptr_encode_canary, mi_tf_block, mi_tf_create,
    mi_tf_is_owned, mi_theap_is_initialized,
};
use crate::options::{_mi_error_message_fmt, mi_option_is_enabled, mi_option_t};
use crate::page::{
    _mi_bin, _mi_page_free_collect, _mi_page_free_collect_partly, _mi_page_retire, _mi_page_unfull,
    MI_MEDIUM_MAX_OBJ_SIZE, mi_page_is_abandoned, mi_page_is_huge,
};
use crate::prim::{
    _mi_page_associated_theap_peek, _mi_prim_thread_id, _mi_theap_default,
    _mi_thread_is_initialized,
};
use crate::track::{mi_track_free_size, mi_track_mem_defined, mi_track_mem_noaccess};
use crate::types::{
    MI_DEBUG_FREED, MI_DEBUG_PADDING, MI_MAX_ALIGN_SIZE, MI_PAGE_FLAG_MASK,
    MI_PAGE_META_ALIGNED_FREE_SMALL, MI_SMALL_PAGE_SIZE, MI_SMALL_SIZE_MAX, mi_block_t,
    mi_padding_t, mi_page_t, mi_theap_t,
};

unsafe fn mi_list_contains(
    page: *const mi_page_t,
    mut list: *const mi_block_t,
    elem: *const mi_block_t,
) -> bool {
    while !list.is_null() {
        if ptr::eq(elem, list) {
            return true;
        }
        list = mi_block_next(page, list);
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_page_ptr_unalign(
    page: *const mi_page_t,
    p: *const c_void,
) -> *mut mi_block_t {
    crate::mi_assert_internal!(!page.is_null() && !p.is_null());
    let diff = p.cast::<u8>().offset_from(mi_page_start(page)) as usize;
    let block_size = mi_page_block_size(page);
    let adjust = if _mi_is_power_of_two(block_size) {
        diff & (block_size - 1)
    } else {
        diff % block_size
    };
    p.cast_mut().cast::<u8>().sub(adjust).cast::<mi_block_t>()
}

unsafe fn mi_check_is_double_freex(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    if mi_list_contains(page, (*page).free, block)
        || mi_list_contains(page, (*page).local_free, block)
        || mi_list_contains(page, mi_page_thread_free(page), block)
    {
        let block_size = mi_page_block_size(page);
        _mi_error_message_fmt(
            EAGAIN,
            format_args!("double free detected of block {block:p} with size {block_size}\n"),
        );
        return true;
    }
    false
}

unsafe fn mi_page_decode_padding(
    page: *const mi_page_t,
    block: *const mi_block_t,
    delta: *mut usize,
    bsize: *mut usize,
) -> bool {
    *bsize = mi_page_usable_block_size(page);
    let padding = (block.cast::<u8>()).add(*bsize).cast::<mi_padding_t>();
    mi_track_mem_defined(
        padding.cast_mut().cast::<c_void>(),
        core::mem::size_of::<mi_padding_t>(),
    );
    *delta = (*padding).delta as usize;
    let canary = (*padding).canary;
    let keys = [(*page).keys[0], (*page).keys[1]];
    let ok = mi_ptr_encode_canary(page.cast::<c_void>(), block.cast::<c_void>(), &keys) == canary
        && *delta <= *bsize;
    mi_track_mem_noaccess(
        padding.cast_mut().cast::<c_void>(),
        core::mem::size_of::<mi_padding_t>(),
    );
    ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_padding_shrink(
    page: *const mi_page_t,
    block: *const mi_block_t,
    min_size: usize,
) {
    if MI_PADDING == 0 {
        let _ = page;
        let _ = block;
        let _ = min_size;
        return;
    }

    let mut bsize = 0usize;
    let mut delta = 0usize;
    let ok = mi_page_decode_padding(page, block, &mut delta, &mut bsize);
    crate::mi_assert_internal!(ok);
    if !ok || (bsize - delta) >= min_size {
        return;
    }

    crate::mi_assert_internal!(bsize >= min_size);
    if bsize < min_size {
        return;
    }

    let new_delta = bsize - min_size;
    crate::mi_assert_internal!(new_delta < bsize);
    let padding = (block.cast::<u8>())
        .add(bsize)
        .cast_mut()
        .cast::<mi_padding_t>();
    mi_track_mem_defined(
        padding.cast::<c_void>(),
        core::mem::size_of::<mi_padding_t>(),
    );
    (*padding).delta = new_delta as u32;
    mi_track_mem_noaccess(
        padding.cast::<c_void>(),
        core::mem::size_of::<mi_padding_t>(),
    );
}

unsafe fn mi_verify_padding(
    page: *const mi_page_t,
    block: *const mi_block_t,
    size: *mut usize,
    wrong: *mut usize,
) -> bool {
    let mut bsize = 0usize;
    let mut delta = 0usize;
    let mut ok = mi_page_decode_padding(page, block, &mut delta, &mut bsize);
    *size = bsize;
    *wrong = bsize;
    if !ok {
        return false;
    }
    crate::mi_assert_internal!(bsize >= delta);
    *size = bsize - delta;
    if !mi_page_is_huge(page) {
        let fill = block.cast::<u8>().add(bsize - delta);
        let maxpad = cmp::min(delta, MI_MAX_ALIGN_SIZE);
        mi_track_mem_defined(fill.cast_mut().cast::<c_void>(), maxpad);
        for i in 0..maxpad {
            if *fill.add(i) != MI_DEBUG_PADDING {
                *wrong = bsize - delta + i;
                ok = false;
                break;
            }
        }
        mi_track_mem_noaccess(fill.cast_mut().cast::<c_void>(), maxpad);
    }
    ok
}

pub unsafe fn mi_check_padding(page: *const mi_page_t, block: *const mi_block_t) {
    if MI_PADDING == 0 || MI_PADDING_CHECK == 0 {
        return;
    }

    let mut size = 0usize;
    let mut wrong = 0usize;
    if !mi_verify_padding(page, block, &mut size, &mut wrong) {
        _mi_error_message_fmt(
            EFAULT,
            format_args!(
                "buffer overflow in theap block {block:p} of size {size}: write after {wrong} bytes\n"
            ),
        );
    }
}

pub unsafe fn mi_check_is_double_free(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    if MI_ENCODE_FREELIST == 0 || (crate::internal::MI_SECURE < 4 && MI_DEBUG == 0) {
        return false;
    }

    let n = mi_block_nextx(page.cast::<c_void>(), block, &(*page).keys);
    if ((n as usize) & (MI_INTPTR_SIZE - 1)) == 0
        && (n.is_null() || mi_is_in_same_page(block.cast::<c_void>(), n.cast::<c_void>()))
    {
        return mi_check_is_double_freex(page, block);
    }
    false
}

pub unsafe fn mi_page_usable_size_of(
    page: *const mi_page_t,
    block: *const mi_block_t,
    is_guarded: bool,
) -> usize {
    if MI_PADDING == 0 {
        let _ = block;
        let _ = is_guarded;
        return mi_page_usable_block_size(page);
    }

    if is_guarded {
        let bsize = mi_page_block_size(page);
        bsize - _mi_os_page_size()
    } else {
        let mut bsize = 0usize;
        let mut delta = 0usize;
        let ok = mi_page_decode_padding(page, block, &mut delta, &mut bsize);
        crate::mi_assert_internal!(ok);
        crate::mi_assert_internal!(delta <= bsize);
        if ok { bsize - delta } else { 0 }
    }
}

pub unsafe fn mi_stat_free(page: *const mi_page_t, block: *const mi_block_t) {
    let _ = block;
    if crate::types::MI_STAT == 0 {
        return;
    }

    let theap: *mut mi_theap_t = _mi_theap_default();
    if !mi_theap_is_initialized(theap) {
        return;
    }

    let bsize = mi_page_usable_block_size(page);
    if bsize <= MI_LARGE_MAX_OBJ_SIZE {
        __mi_stat_decrease(&mut (*theap).stats.malloc_normal, bsize);
        if crate::types::MI_STAT > 1 {
            __mi_stat_decrease(&mut (*theap).stats.malloc_bins[_mi_bin(bsize)], 1);
        }
    } else {
        let bpsize = mi_page_block_size(page);
        __mi_stat_decrease(&mut (*theap).stats.malloc_huge, bpsize);
    }
}

pub unsafe fn mi_free_block_local(
    page: *mut mi_page_t,
    block: *mut mi_block_t,
    was_guarded: bool,
    track_stats: bool,
    check_full: bool,
) {
    if mi_check_is_double_free(page, block) {
        return;
    }
    if !was_guarded {
        mi_check_padding(page, block);
    }
    if track_stats {
        mi_stat_free(page, block);
    }
    if track_stats {
        mi_track_free_size(
            block.cast::<c_void>(),
            mi_page_usable_size_of(page, block, was_guarded),
        );
    }
    if MI_DEBUG > 0 && MI_TRACK_ENABLED == 0 && !was_guarded {
        _mi_memset_aligned(
            block.cast::<c_void>(),
            MI_DEBUG_FREED as i32,
            mi_page_block_size(page),
        );
    }

    mi_block_set_next(page, block, (*page).local_free);
    (*page).local_free = block;
    (*page).used -= 1;
    if (*page).used == 0 {
        if (*page).retire_expire == 0 {
            _mi_page_retire(page);
        }
    } else if check_full && crate::page::mi_page_is_in_full(page) {
        _mi_page_unfull(page);
    }
}

pub unsafe fn mi_free_block_mt(page: *mut mi_page_t, block: *mut mi_block_t, was_guarded: bool) {
    mi_stat_free(page, block);
    mi_track_free_size(
        block.cast::<c_void>(),
        mi_page_usable_size_of(page, block, was_guarded),
    );

    if MI_DEBUG > 0 && MI_TRACK_ENABLED == 0 && !was_guarded {
        let mut dbgsize = crate::mimalloc::mi_usable_size(block.cast::<c_void>());
        if dbgsize > MI_MiB {
            dbgsize = MI_MiB;
        }
        _mi_memset_aligned(block.cast::<c_void>(), MI_DEBUG_FREED as i32, dbgsize);
    }

    let mut tf_old = mi_atomic_load_explicit(
        ptr::addr_of!((*page).xthread_free).cast(),
        mi_memory_order_e::mi_memory_order_relaxed,
    );
    loop {
        mi_block_set_next(page, block, mi_tf_block(tf_old));
        let tf_new = mi_tf_create(block, true);
        if mi_atomic_cas_weak_acq_rel(
            ptr::addr_of_mut!((*page).xthread_free).cast(),
            &mut tf_old,
            tf_new,
        ) {
            break;
        }
    }

    let is_owned_now = !mi_tf_is_owned(tf_old);
    if is_owned_now {
        crate::mi_assert_internal!(mi_page_is_abandoned(page));
        mi_free_try_collect_mt(page, block);
    }
}

unsafe fn mi_block_unguard(page: *mut mi_page_t, block: *mut mi_block_t, p: *mut c_void) {
    if MI_GUARDED == 0 {
        let _ = page;
        let _ = block;
        let _ = p;
        return;
    }

    crate::mi_assert_internal!(mi_block_ptr_is_guarded(block, p));
    crate::mi_assert_internal!(mi_page_has_interior_pointers(page));
    crate::mi_assert_internal!(
        p.cast::<u8>().offset_from(block.cast::<u8>())
            >= core::mem::size_of::<mi_block_t>() as isize
    );
    crate::mi_assert_internal!((*block).next == MI_BLOCK_TAG_GUARDED);

    let bsize = mi_page_block_size(page);
    let psize = _mi_os_page_size();
    crate::mi_assert_internal!(bsize > psize);
    crate::mi_assert_internal!(!(*page).memid.is_pinned);

    let gpage = block.cast::<u8>().add(bsize - psize).cast::<c_void>();
    crate::mi_assert_internal!((gpage as usize).is_multiple_of(psize));
    crate::os::_mi_os_unprotect(gpage, psize);
}

pub unsafe fn mi_block_check_unguard(
    page: *mut mi_page_t,
    block: *mut mi_block_t,
    p: *mut c_void,
) -> bool {
    if MI_GUARDED != 0 && mi_block_ptr_is_guarded(block, p) {
        mi_block_unguard(page, block, p);
        true
    } else {
        let _ = page;
        let _ = block;
        let _ = p;
        false
    }
}

#[allow(clippy::absurd_extreme_comparisons)]
pub unsafe fn mi_validate_block_from_ptr(
    page: *const mi_page_t,
    p: *mut c_void,
) -> *mut mi_block_t {
    crate::mi_assert!(_mi_page_ptr_unalign(page, p) == p.cast());
    if MI_SECURE > 0 {
        _mi_page_ptr_unalign(page, p)
    } else {
        p.cast()
    }
}

pub unsafe fn mi_free_generic_local(page: *mut mi_page_t, p: *mut c_void) {
    crate::mi_assert_internal!(!p.is_null() && !page.is_null());
    let block = if mi_page_has_interior_pointers(page) {
        _mi_page_ptr_unalign(page, p)
    } else {
        mi_validate_block_from_ptr(page, p)
    };
    let was_guarded = mi_block_check_unguard(page, block, p);
    mi_free_block_local(page, block, was_guarded, true, true);
}

pub unsafe fn mi_free_generic_mt(page: *mut mi_page_t, p: *mut c_void) {
    crate::mi_assert_internal!(!p.is_null() && !page.is_null());
    let block = if mi_page_has_interior_pointers(page) {
        _mi_page_ptr_unalign(page, p)
    } else {
        mi_validate_block_from_ptr(page, p)
    };
    let was_guarded = mi_block_check_unguard(page, block, p);
    mi_free_block_mt(page, block, was_guarded);
}

pub(crate) unsafe extern "C" fn _mi_free_generic(
    page: *mut mi_page_t,
    is_local: bool,
    p: *mut c_void,
) {
    if is_local {
        mi_free_generic_local(page, p);
    } else {
        mi_free_generic_mt(page, p);
    }
}

unsafe fn mi_validate_ptr_page(p: *const c_void, msg: *const i8) -> *mut mi_page_t {
    if MI_DEBUG != 0 || cfg!(feature = "secure") {
        if !p.is_null()
            && ((p as usize) & (MI_INTPTR_SIZE - 1)) != 0
            && !mi_option_is_enabled(mi_option_t::mi_option_guarded_precise)
        {
            let msg = std::ffi::CStr::from_ptr(msg).to_string_lossy();
            _mi_error_message_fmt(
                EINVAL,
                format_args!("{msg}: invalid (unaligned) pointer: {p:p}\n"),
            );
            return ptr::null_mut();
        }
        let mut page = _mi_safe_ptr_page(p);
        if !page.is_null() && !mi_page_contains_address(page, p) {
            page = ptr::null_mut();
        }
        if !p.is_null() && page.is_null() {
            let msg = std::ffi::CStr::from_ptr(msg).to_string_lossy();
            _mi_error_message_fmt(EINVAL, format_args!("{msg}: invalid pointer: {p:p}\n"));
        }
        page
    } else {
        _mi_ptr_page(p)
    }
}

#[inline(always)]
pub unsafe fn mi_free_ex(p: *mut c_void, usable: *mut usize, page: *mut mi_page_t) {
    if page.is_null() {
        return;
    }
    crate::mi_assert_internal!(!p.is_null() && !page.is_null());
    if !usable.is_null() {
        *usable = mi_page_usable_block_size(page);
    }

    let xtid = _mi_prim_thread_id() ^ mi_page_xthread_id(page);
    if xtid == 0 {
        let block = mi_validate_block_from_ptr(page, p);
        mi_free_block_local(page, block, false, true, false);
        return;
    }
    mi_free_ex_slow(p, page, xtid);
}

#[cold]
#[inline(never)]
unsafe fn mi_free_ex_slow(p: *mut c_void, page: *mut mi_page_t, xtid: usize) {
    if xtid <= MI_PAGE_FLAG_MASK {
        mi_free_generic_local(page, p);
    } else if (xtid & MI_PAGE_FLAG_MASK) == 0 {
        let block = mi_validate_block_from_ptr(page, p);
        mi_free_block_mt(page, block, false);
    } else {
        mi_free_generic_mt(page, p);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_free(p: *mut c_void) {
    let page = mi_validate_ptr_page(p, c"mi_free".as_ptr());
    mi_free_ex(p, ptr::null_mut(), page);
}

unsafe fn mi_abandoned_page_try_free(page: *mut mi_page_t) -> bool {
    if !mi_page_all_free(page) {
        return false;
    }
    _mi_arenas_page_unabandon(page, ptr::null_mut());
    _mi_arenas_page_free(page, ptr::null_mut());
    true
}

unsafe fn mi_abandoned_page_try_reabandon_to_mapped(page: *mut mi_page_t) -> bool {
    if mi_page_is_mostly_used(page) {
        return false;
    }
    if (*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA
        || mi_page_is_abandoned_mapped(page)
    {
        return false;
    }

    crate::mi_assert!(!mi_page_is_full(page));
    _mi_arenas_page_try_reabandon_to_mapped(page)
}

unsafe fn mi_abandoned_page_unown_from_free(
    page: *mut mi_page_t,
    expected_thread_free: *mut mi_block_t,
) {
    crate::mi_assert_internal!(mi_page_is_owned(page));
    crate::mi_assert_internal!(mi_page_is_abandoned(page));
    crate::mi_assert_internal!(!mi_page_all_free(page));

    let mut tf_expect = mi_tf_create(expected_thread_free, true);
    let mut tf_new = mi_tf_create(expected_thread_free, false);
    while !mi_atomic_cas_weak_acq_rel(
        ptr::addr_of_mut!((*page).xthread_free).cast(),
        &mut tf_expect,
        tf_new,
    ) {
        crate::mi_assert_internal!(mi_tf_is_owned(tf_expect));
        while !mi_tf_block(tf_expect).is_null() {
            _mi_page_free_collect(page, false);
            if mi_abandoned_page_try_free(page) {
                return;
            }
            if mi_abandoned_page_try_reabandon_to_mapped(page) {
                return;
            }
            tf_expect = mi_atomic_load_explicit(
                ptr::addr_of!((*page).xthread_free).cast(),
                mi_memory_order_e::mi_memory_order_relaxed,
            );
        }
        crate::mi_assert_internal!(mi_tf_block(tf_expect).is_null());
        tf_new = crate::internal::mi_tf_create(ptr::null_mut(), false);
    }
}

unsafe fn mi_page_queue_len_is_atmost(
    theap: *mut mi_theap_t,
    block_size: usize,
    atmost: core::ffi::c_long,
) -> bool {
    if atmost < 0 {
        return false;
    }
    let pq = mi_page_queue(theap, block_size);
    crate::mi_assert_internal!(!pq.is_null());
    (*pq).count <= atmost as usize
}

unsafe fn mi_abandoned_page_try_reclaim(
    page: *mut mi_page_t,
    reclaim_on_free: core::ffi::c_long,
) -> bool {
    crate::mi_assert_internal!(mi_page_is_owned(page));
    crate::mi_assert_internal!(mi_page_is_abandoned(page));
    crate::mi_assert_internal!(!mi_page_all_free(page));
    crate::mi_assert_internal!((*page).block_size <= MI_MEDIUM_MAX_OBJ_SIZE);
    crate::mi_assert_internal!(reclaim_on_free >= 0);

    if !_mi_thread_is_initialized() {
        return false;
    }

    let theap = _mi_page_associated_theap_peek(page);
    if theap.is_null() || !(*theap).allow_page_reclaim {
        return false;
    }

    let mut max_reclaim = 0;
    if theap == (*page).theap {
        max_reclaim = _mi_option_get_fast(if (*(*theap).tld).is_in_threadpool {
            mi_option_t::mi_option_page_cross_thread_max_reclaim
        } else {
            mi_option_t::mi_option_page_max_reclaim
        });
    } else if reclaim_on_free == 1
        && !(*(*theap).tld).is_in_threadpool
        && !mi_page_is_mostly_used(page)
        && _mi_arena_memid_is_suitable((*page).memid, (*_mi_theap_heap(theap)).exclusive_arena)
    {
        max_reclaim = _mi_option_get_fast(mi_option_t::mi_option_page_cross_thread_max_reclaim);
    }

    if max_reclaim >= 0 && !mi_page_queue_len_is_atmost(theap, (*page).block_size, max_reclaim) {
        return false;
    }

    _mi_arenas_page_unabandon(page, theap);
    _mi_theap_page_reclaim(theap, page);
    __mi_stat_counter_increase(&mut (*theap).stats.pages_reclaim_on_free, 1);
    true
}

unsafe fn mi_free_try_collect_mt(page: *mut mi_page_t, mut mt_free: *mut mi_block_t) {
    crate::mi_assert_internal!(mi_page_is_owned(page));
    crate::mi_assert_internal!(mi_page_is_abandoned(page));
    crate::mi_assert_internal!(!mt_free.is_null());

    if (*page).block_size <= MI_SMALL_SIZE_MAX {
        crate::mi_assert_internal!((*page).reserved >= 16);
        _mi_page_free_collect_partly(page, mt_free);
    } else {
        _mi_page_free_collect(page, false);
        mt_free = ptr::null_mut();
    }

    let reclaim_on_free = _mi_option_get_fast(mi_option_t::mi_option_page_reclaim_on_free);
    if MI_DEBUG > 1 {
        if mi_page_is_singleton(page) {
            crate::mi_assert_internal!(mi_page_all_free(page));
        }
        if mi_page_is_full(page) {
            crate::mi_assert!(mi_page_is_mostly_used(page));
        }
    }

    if mi_abandoned_page_try_free(page) {
        return;
    }
    if (*page).block_size <= MI_MEDIUM_MAX_OBJ_SIZE
        && reclaim_on_free >= 0
        && mi_abandoned_page_try_reclaim(page, reclaim_on_free)
    {
        return;
    }
    if mi_abandoned_page_try_reabandon_to_mapped(page) {
        return;
    }

    mi_abandoned_page_unown_from_free(page, mt_free);
}

pub unsafe fn mi_page_usable_aligned_size_of(page: *const mi_page_t, p: *const c_void) -> usize {
    let block = _mi_page_ptr_unalign(page, p);
    let is_guarded = mi_block_ptr_is_guarded(block, p.cast_mut());
    let size = mi_page_usable_size_of(page, block, is_guarded);
    let adjust = p.cast::<u8>().offset_from(block.cast::<u8>());
    crate::mi_assert_internal!(adjust >= 0 && adjust as usize <= size);
    size - adjust as usize
}

pub unsafe fn _mi_usable_size(p: *const c_void, page: *const mi_page_t) -> usize {
    if page.is_null() {
        return 0;
    }
    if !mi_page_has_interior_pointers(page) {
        mi_page_usable_size_of(page, p.cast(), false)
    } else {
        mi_page_usable_aligned_size_of(page, p)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_usable_size(p: *const c_void) -> usize {
    let page = mi_validate_ptr_page(p, c"mi_usable_size".as_ptr());
    _mi_usable_size(p, page)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_free_size(p: *mut c_void, size: usize) {
    let _ = size;
    if MI_DEBUG != 0 {
        let page = mi_validate_ptr_page(p, c"mi_free_size".as_ptr());
        let available = _mi_usable_size(p, page);
        crate::mi_assert!(p.is_null() || size <= available || available == 0);
    }
    mi_free(p);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_free_size_aligned(p: *mut c_void, size: usize, alignment: usize) {
    crate::mi_assert!(alignment != 0 && (p as usize).is_multiple_of(alignment));
    mi_free_size(p, size);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_free_aligned(p: *mut c_void, alignment: usize) {
    crate::mi_assert!(alignment != 0 && (p as usize).is_multiple_of(alignment));
    mi_free(p);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_ufree(p: *mut c_void, usable: *mut usize) {
    let page = mi_validate_ptr_page(p, c"mi_ufree".as_ptr());
    mi_free_ex(p, usable, page);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_free_small(p: *mut c_void) {
    if MI_PAGE_META_ALIGNED_FREE_SMALL == 0 || MI_GUARDED != 0 {
        mi_free(p);
        return;
    }

    let page = _mi_align_down_ptr(p, MI_SMALL_PAGE_SIZE).cast::<mi_page_t>();
    crate::mi_assert!(page == mi_validate_ptr_page(p, c"mi_free_small".as_ptr()));
    crate::mi_assert!(
        page.cast::<c_void>() == _mi_align_down_ptr(mi_page_start(page).cast(), MI_SMALL_PAGE_SIZE)
    );
    crate::mi_assert!((*page).block_size <= MI_SMALL_SIZE_MAX);
    mi_free_ex(p, ptr::null_mut(), page);
}
