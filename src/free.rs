//! Mechanical translation of the early `src/free.c` free-path helpers.

#![allow(
    dead_code,
    clippy::missing_safety_doc,
    clippy::if_same_then_else,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;
use core::{cmp, ptr};

use crate::atomic::{mi_atomic_cas_weak_acq_rel, mi_atomic_load_explicit, mi_memory_order_e};
use crate::internal::{
    __mi_stat_decrease, _mi_error_message, _mi_memset_aligned, _mi_os_page_size, EAGAIN, EFAULT,
    MI_DEBUG, MI_ENCODE_FREELIST, MI_LARGE_MAX_OBJ_SIZE, MI_MiB, MI_PADDING, MI_PADDING_CHECK,
    MI_TRACK_ENABLED, mi_block_next, mi_block_nextx, mi_block_set_next, mi_is_in_same_page,
    mi_page_block_size, mi_page_thread_free, mi_page_usable_block_size, mi_ptr_encode_canary,
    mi_tf_block, mi_tf_create, mi_tf_is_owned, mi_theap_is_initialized,
};
use crate::page::{
    _mi_bin, _mi_page_retire, _mi_page_unfull, mi_page_is_abandoned, mi_page_is_huge,
};
use crate::prim::_mi_theap_default;
use crate::track::{mi_track_free_size, mi_track_mem_defined, mi_track_mem_noaccess};
use crate::types::{
    MI_DEBUG_FREED, MI_DEBUG_PADDING, MI_INTPTR_SIZE, MI_MAX_ALIGN_SIZE, mi_block_t, mi_padding_t,
    mi_page_t, mi_theap_t,
};

unsafe extern "C" {
    fn mi_free_try_collect_mt(page: *mut mi_page_t, mt_free: *mut mi_block_t);
}

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

unsafe fn mi_check_is_double_freex(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    if mi_list_contains(page, (*page).free, block)
        || mi_list_contains(page, (*page).local_free, block)
        || mi_list_contains(page, mi_page_thread_free(page), block)
    {
        _mi_error_message(
            EAGAIN,
            b"double free detected of block %p with size %zu\n\0"
                .as_ptr()
                .cast(),
            block,
            mi_page_block_size(page),
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
        padding.cast::<c_void>(),
        core::mem::size_of::<mi_padding_t>(),
    );
    *delta = (*padding).delta as usize;
    let canary = (*padding).canary;
    let keys = [(*page).keys[0], (*page).keys[1]];
    let ok = mi_ptr_encode_canary(page.cast::<c_void>(), block.cast::<c_void>(), &keys) == canary
        && *delta <= *bsize;
    mi_track_mem_noaccess(
        padding.cast::<c_void>(),
        core::mem::size_of::<mi_padding_t>(),
    );
    ok
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
        mi_track_mem_defined(fill.cast::<c_void>(), maxpad);
        for i in 0..maxpad {
            if *fill.add(i) != MI_DEBUG_PADDING {
                *wrong = bsize - delta + i;
                ok = false;
                break;
            }
        }
        mi_track_mem_noaccess(fill.cast::<c_void>(), maxpad);
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
        _mi_error_message(
            EFAULT,
            b"buffer overflow in theap block %p of size %zu: write after %zu bytes\n\0"
                .as_ptr()
                .cast(),
            block,
            size,
            wrong,
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
    if MI_DEBUG > 0 && MI_TRACK_ENABLED == 0 && !was_guarded {
        _mi_memset_aligned(
            block.cast::<c_void>(),
            MI_DEBUG_FREED as i32,
            mi_page_block_size(page),
        );
    }
    if track_stats {
        mi_track_free_size(
            block.cast::<c_void>(),
            mi_page_usable_size_of(page, block, was_guarded),
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
