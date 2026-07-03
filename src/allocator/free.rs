//! Mechanical translation of the early `src/free.c` free-path helpers.

mod abandoned;

use core::ffi::{CStr, c_char, c_void};
use core::{cmp, ptr};

use crate::arena::{
    _mi_arenas_page_free, _mi_arenas_page_try_reabandon_to_mapped_arena, _mi_arenas_page_unabandon,
};
use crate::bits::MI_INTPTR_SIZE;
use crate::core_types::Errno;
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_decrease, _mi_align_down_ptr,
    _mi_arena_memid_is_suitable, _mi_is_power_of_two, _mi_memset_aligned, _mi_option_get_fast,
    _mi_os_page_size_typed, _mi_ptr_page, _mi_safe_ptr_page_registered, _mi_theap_heap, MI_DEBUG,
    MI_ENCODE_FREELIST, MI_GUARDED, MI_LARGE_MAX_OBJ_SIZE, MI_MiB, MI_PADDING, MI_PADDING_CHECK,
    MI_SECURE, MI_TRACK_ENABLED, mi_block_next, mi_block_nextx, mi_block_ptr_is_guarded,
    mi_block_set_next_typed, mi_is_in_same_page, mi_page_all_free, mi_page_block_size,
    mi_page_has_interior_pointers, mi_page_is_abandoned_mapped, mi_page_is_full,
    mi_page_is_mostly_used, mi_page_is_owned, mi_page_is_singleton, mi_page_keys, mi_page_queue,
    mi_page_start, mi_page_thread_free, mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::options::{_mi_error_message_fmt, mi_option_is_enabled, mi_option_t};
use crate::page::{
    _mi_page_free_collect, _mi_page_free_collect_partly, _mi_page_retire_internal,
    _mi_page_unfull_internal, _mi_theap_page_reclaim, MI_MEDIUM_MAX_OBJ_SIZE, mi_bin_from_usable,
    mi_page_is_abandoned, mi_page_is_huge, mi_page_queue_count_typed,
};
use crate::prim::{
    _mi_page_associated_theap_peek, _mi_prim_thread_id, _mi_theap_default,
    _mi_thread_is_initialized,
};
use crate::track::{mi_track_free_size, mi_track_mem_defined, mi_track_mem_noaccess};
use crate::types::{
    MI_DEBUG_FREED, MI_DEBUG_PADDING, MI_MAX_ALIGN_SIZE, MI_PAGE_META_ALIGNED_FREE_SMALL,
    MI_SMALL_PAGE_SIZE, MI_SMALL_SIZE_MAX, mi_block_t, mi_page_t, mi_theap_t,
};

use abandoned::mi_free_try_collect_mt_typed;

// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[cfg(mi_c_malloc_fast)]
core::arch::global_asm!(
    ".hidden _mi_padding_shrink",
    ".type _mi_padding_shrink,@function",
    "_mi_padding_shrink:",
    "ret",
    ".size _mi_padding_shrink, .-_mi_padding_shrink",
);

// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[cfg(mi_c_malloc_fast)]
core::arch::global_asm!(
    ".globl mi_free_size",
    ".type mi_free_size,@function",
    ".p2align 4",
    "mi_free_size:",
    "endbr64",
    "jmp mi_free",
    ".size mi_free_size, .-mi_free_size",
    ".globl mi_free_size_aligned",
    ".type mi_free_size_aligned,@function",
    ".p2align 4",
    "mi_free_size_aligned:",
    "endbr64",
    "jmp mi_free_size",
    ".size mi_free_size_aligned, .-mi_free_size_aligned",
    ".globl mi_free_aligned",
    ".type mi_free_aligned,@function",
    ".p2align 4",
    "mi_free_aligned:",
    "endbr64",
    "jmp mi_free",
    ".size mi_free_aligned, .-mi_free_aligned",
);

unsafe fn mi_list_contains(
    page: *const mi_page_t,
    mut list: *const mi_block_t,
    elem: *const mi_block_t,
) -> bool {
    // SAFETY[AUDIT:free-double-check-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        while !list.is_null() {
            if ptr::eq(elem, list) {
                return true;
            }
            list = mi_block_next(page, list);
        }
        false
    }
}

// SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_ptr_unalign(
    page: *const mi_page_t,
    p: *const c_void,
) -> *mut mi_block_t {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null() && !p.is_null());
        let diff = p.cast::<u8>().offset_from(mi_page_start(page)) as usize;
        let block_size = mi_page_block_size(page);
        let adjust = if _mi_is_power_of_two(block_size) {
            diff & (block_size - 1)
        } else {
            diff % block_size
        };
        p.cast_mut().cast::<u8>().sub(adjust).cast::<mi_block_t>()
    }
}

unsafe fn mi_check_is_double_freex(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    // SAFETY[AUDIT:free-double-check-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        if mi_list_contains(page, (*page).free, block)
            || mi_list_contains(page, (*page).local_free, block)
            || mi_list_contains(page, mi_page_thread_free(page), block)
        {
            let block_size = mi_page_block_size(page);
            _mi_error_message_fmt(
                Errno::EAGAIN,
                format_args!("double free detected of block {block:p} with size {block_size}\n"),
            );
            return true;
        }
        false
    }
}

unsafe fn mi_page_decode_padding(
    page: *const mi_page_t,
    block: *const mi_block_t,
    delta: *mut usize,
    bsize: *mut usize,
) -> bool {
    // SAFETY[AUDIT:free-padding-boundary]: padding decode/shrink/check operations use typed padding metadata on validated blocks.
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        let block_ptr = crate::core_types::BlockPtr::new_unchecked(block.cast_mut());
        let usable_size = mi_page_usable_block_size(page);
        *bsize = usable_size.get();
        let padding = crate::core_types::PaddingPtr::from_block_usable(block_ptr, usable_size);
        let padding_ptr = padding.as_ptr();
        mi_track_mem_defined(
            padding_ptr.cast::<c_void>(),
            crate::core_types::PaddingPtr::SIZE,
        );
        let metadata = padding.read();
        *delta = metadata.delta().get() as usize;
        let canary = metadata.canary().get();
        let keys = crate::core_types::PageKeys(mi_page_keys(page));
        let ok = crate::core_types::PaddingCanary::from_page_block_keys(page_ptr, block_ptr, keys)
            .get()
            == canary
            && *delta <= *bsize;
        mi_track_mem_noaccess(
            padding_ptr.cast::<c_void>(),
            crate::core_types::PaddingPtr::SIZE,
        );
        ok
    }
}

// SAFETY[AUDIT:free-padding-boundary]: padding decode/shrink/check operations use typed padding metadata on validated blocks.
#[cfg(not(mi_c_malloc_fast))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_padding_shrink(
    page: *const mi_page_t,
    block: *const mi_block_t,
    min_size: usize,
) {
    // SAFETY[AUDIT:free-padding-boundary]: padding decode/shrink/check operations use typed padding metadata on validated blocks.
    unsafe {
        if MI_PADDING == 0 {
            let _ = page;
            let _ = block;
            let _ = min_size;
            return;
        }

        let mut bsize = 0usize;
        let mut delta = 0usize;
        let ok = mi_page_decode_padding(page, block, &mut delta, &mut bsize);
        crate::internal::mi_assert_internal!(ok);
        if !ok || (bsize - delta) >= min_size {
            return;
        }

        crate::internal::mi_assert_internal!(bsize >= min_size);
        if bsize < min_size {
            return;
        }

        let new_delta = bsize - min_size;
        crate::internal::mi_assert_internal!(new_delta < bsize);
        let padding = crate::core_types::PaddingPtr::from_block_usable(
            crate::core_types::BlockPtr::new_unchecked(block.cast_mut()),
            crate::core_types::UsableSize::new(bsize),
        );
        let padding_ptr = padding.as_ptr();
        mi_track_mem_defined(
            padding_ptr.cast::<c_void>(),
            crate::core_types::PaddingPtr::SIZE,
        );
        padding.write_delta(
            crate::core_types::PaddingDelta::for_usable(
                new_delta,
                crate::core_types::UsableSize::new(bsize),
            )
            .unwrap_or_else(|| {
                crate::internal::mi_assert_internal!(false);
                crate::core_types::PaddingDelta::new_unchecked(new_delta as u32)
            }),
        );
        mi_track_mem_noaccess(
            padding_ptr.cast::<c_void>(),
            crate::core_types::PaddingPtr::SIZE,
        );
    }
}

unsafe fn mi_verify_padding(
    page: *const mi_page_t,
    block: *const mi_block_t,
    size: *mut usize,
    wrong: *mut usize,
) -> bool {
    // SAFETY[AUDIT:free-padding-boundary]: padding decode/shrink/check operations use typed padding metadata on validated blocks.
    unsafe {
        let mut bsize = 0usize;
        let mut delta = 0usize;
        let mut ok = mi_page_decode_padding(page, block, &mut delta, &mut bsize);
        *size = bsize;
        *wrong = bsize;
        if !ok {
            return false;
        }
        crate::internal::mi_assert_internal!(bsize >= delta);
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
}

/// # Safety
/// `page` and `block` must refer to a valid block owned by that page.
pub unsafe fn mi_check_padding(page: *const mi_page_t, block: *const mi_block_t) {
    // SAFETY[AUDIT:free-padding-boundary]: padding decode/shrink/check operations use typed padding metadata on validated blocks.
    unsafe {
        if MI_PADDING == 0 || MI_PADDING_CHECK == 0 {
            return;
        }

        let mut size = 0usize;
        let mut wrong = 0usize;
        if !mi_verify_padding(page, block, &mut size, &mut wrong) {
            _mi_error_message_fmt(
                Errno::EFAULT,
                format_args!(
                    "buffer overflow in theap block {block:p} of size {size}: write after {wrong} bytes\n"
                ),
            );
        }
    }
}

/// # Safety
/// `page` and `block` must refer to a valid block owned by that page.
pub unsafe fn mi_check_is_double_free(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    // SAFETY[AUDIT:free-double-check-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        if MI_ENCODE_FREELIST == 0 || (crate::internal::MI_SECURE < 4 && MI_DEBUG == 0) {
            return false;
        }

        let keys = mi_page_keys(page);
        let n = mi_block_nextx(page.cast::<c_void>(), block, &keys);
        if ((n as usize) & (MI_INTPTR_SIZE - 1)) == 0
            && (n.is_null() || mi_is_in_same_page(block.cast::<c_void>(), n.cast::<c_void>()))
        {
            return mi_check_is_double_freex(page, block);
        }
        false
    }
}

pub(crate) unsafe fn mi_page_usable_size_of_typed(
    page: *const mi_page_t,
    block: *const mi_block_t,
    is_guarded: bool,
) -> crate::core_types::UsableSize {
    // SAFETY[AUDIT:free-usable-size-boundary]: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
    unsafe {
        if MI_PADDING == 0 {
            let _ = block;
            let _ = is_guarded;
            return mi_page_usable_block_size(page);
        }

        if is_guarded {
            let bsize = mi_page_block_size(page);
            crate::core_types::UsableSize::new(bsize)
                .checked_sub(_mi_os_page_size_typed().get())
                .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
        } else {
            let mut bsize = 0usize;
            let mut delta = 0usize;
            let ok = mi_page_decode_padding(page, block, &mut delta, &mut bsize);
            crate::internal::mi_assert_internal!(ok);
            crate::internal::mi_assert_internal!(delta <= bsize);
            if ok {
                crate::core_types::UsableSize::new(bsize)
                    .checked_sub(delta)
                    .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
            } else {
                crate::core_types::UsableSize::new(0)
            }
        }
    }
}

/// # Safety
/// `page` and `block` must refer to a valid block owned by that page.
pub unsafe fn mi_page_usable_size_of(
    page: *const mi_page_t,
    block: *const mi_block_t,
    is_guarded: bool,
) -> usize {
    // SAFETY[AUDIT:free-usable-size-boundary]: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
    unsafe { mi_page_usable_size_of_typed(page, block, is_guarded).get() }
}

/// # Safety
/// `page` and `block` must refer to a valid block being freed.
pub unsafe fn mi_stat_free(page: *const mi_page_t, block: *const mi_block_t) {
    // SAFETY[AUDIT:free-stats-boundary]: free stat updates mutate current thread-heap counters for caller-validated blocks.
    unsafe {
        let _ = block;
        if crate::types::MI_STAT == 0 {
            return;
        }

        let theap: *mut mi_theap_t = _mi_theap_default();
        if !mi_theap_is_initialized(theap) {
            return;
        }

        let bsize = mi_page_usable_block_size(page);
        if bsize.get() <= MI_LARGE_MAX_OBJ_SIZE {
            __mi_stat_decrease(ptr::addr_of_mut!((*theap).stats.malloc_normal), bsize.get());
            if crate::types::MI_STAT > 1 {
                __mi_stat_decrease(
                    ptr::addr_of_mut!(
                        (*theap).stats.malloc_bins[mi_bin_from_usable(bsize).as_usize()]
                    ),
                    1,
                );
            }
        } else {
            let page_block_size = mi_page_block_size(page);
            __mi_stat_decrease(
                ptr::addr_of_mut!((*theap).stats.malloc_huge),
                page_block_size,
            );
        }
    }
}

pub(crate) unsafe fn mi_free_block_local(
    page_wrapper: crate::core_types::LocalPage,
    block_wrapper: crate::core_types::BlockForFree,
    mode: crate::core_types::LocalFreeMode,
) {
    // SAFETY[AUDIT:free-local-list-boundary]: local frees mutate the owning page free list after validation and optional padding checks.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let block = block_wrapper.block().as_ptr();
        let was_guarded = block_wrapper.is_guarded();
        let check_full = mode.check_full();
        if mi_check_is_double_free(page, block) {
            return;
        }
        if !was_guarded {
            mi_check_padding(page, block);
        }
        mi_stat_free(page, block);
        mi_track_free_size(
            block.cast::<c_void>(),
            mi_page_usable_size_of(page, block, was_guarded),
        );
        if MI_DEBUG > 0 && MI_TRACK_ENABLED == 0 && !was_guarded {
            _mi_memset_aligned(
                block.cast::<c_void>(),
                MI_DEBUG_FREED as i32,
                mi_page_block_size(page),
            );
        }

        // Push the freed block onto the page's local free list via the typed
        // wrapper. Zero-cost: compiles to the same `mi_block_set_next` + store as
        // the raw form it replaces.
        // SAFETY: `page` is a valid local page; `block` is a valid block in it.
        let local = crate::core_types::LocalFreeList::from_raw((*page).local_free);
        let new_local = local.push(
            crate::core_types::LocalPage::new_unchecked(crate::core_types::PagePtr::new_unchecked(
                page,
            )),
            block_wrapper.into_free_block(),
        );
        (*page).local_free = new_local
            .0
            .map(|b| b.as_ptr())
            .unwrap_or(core::ptr::null_mut());
        let used = crate::page::mi_page_decrement_used_count(page);
        if used.get() == 0 {
            if crate::page::mi_page_retire_expire(page).is_zero() {
                let active = crate::core_types::ActivePage::new_unchecked(
                    crate::core_types::PagePtr::new_unchecked(page),
                );
                let Some(all_free) = active.mark_all_free() else {
                    crate::internal::mi_assert_internal!(false);
                    return;
                };
                _mi_page_retire_internal(all_free);
            }
        } else if check_full && crate::page::mi_page_is_in_full(page) {
            _mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            ));
        }
    }
}

pub(crate) unsafe fn mi_free_block_mt(
    page_wrapper: crate::core_types::ForeignPage,
    block_wrapper: crate::core_types::BlockForFree,
) {
    // SAFETY[AUDIT:free-remote-list-boundary]: remote frees publish blocks through the typed atomic thread-free word.
    unsafe {
        let page_ptr = page_wrapper.page();
        let page = page_ptr.as_ptr();
        let bp = block_wrapper.block();
        let block = bp.as_ptr();
        let was_guarded = block_wrapper.is_guarded();
        mi_stat_free(page, block);
        mi_track_free_size(
            block.cast::<c_void>(),
            mi_page_usable_size_of_typed(page, block, was_guarded).get(),
        );

        if MI_DEBUG > 0 && MI_TRACK_ENABLED == 0 && !was_guarded {
            let mut dbgsize = crate::mimalloc::mi_usable_size(block.cast::<c_void>());
            if dbgsize > MI_MiB {
                dbgsize = MI_MiB;
            }
            _mi_memset_aligned(block.cast::<c_void>(), MI_DEBUG_FREED as i32, dbgsize);
        }

        let atomic_tf = crate::core_types::AtomicThreadFree::from_field(
            crate::core_types::AtomicUsizeField::from_raw(ptr::addr_of_mut!((*page).xthread_free)),
        );

        let mut tf_old = atomic_tf.load_relaxed();
        loop {
            mi_block_set_next_typed(page_ptr, bp, tf_old.head());
            match atomic_tf.push_remote_acq_rel(&mut tf_old, bp) {
                Ok(_) => break,
                Err(_) => continue, // tf_old is updated by the atomic wrapper
            }
        }

        if !tf_old.is_owned() {
            crate::internal::mi_assert_internal!(mi_page_is_abandoned(page));
            mi_free_try_collect_mt_typed(
                crate::core_types::OwnedAbandonedPage::new_unchecked(
                    crate::core_types::PagePtr::new_unchecked(page),
                ),
                crate::core_types::RemoteFreeList::from_detached_head(bp),
            );
        }
    }
}

unsafe fn mi_block_unguard(
    page: crate::core_types::PagePtr,
    block: crate::core_types::GuardedBlock,
    p: crate::core_types::UserPtr,
) {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let page = page.as_ptr();
        let block = block.block().as_ptr();
        let p = p.as_ptr();
        if MI_GUARDED == 0 {
            let _ = page;
            let _ = block;
            let _ = p;
            return;
        }

        crate::internal::mi_assert_internal!(mi_block_ptr_is_guarded(block, p));
        crate::internal::mi_assert_internal!(mi_page_has_interior_pointers(page));
        crate::internal::mi_assert_internal!(
            p.cast::<u8>().offset_from(block.cast::<u8>())
                >= core::mem::size_of::<mi_block_t>() as isize
        );
        crate::internal::mi_assert_internal!(crate::core_types::BlockLink::is_guarded_raw(block));

        let bsize = mi_page_block_size(page);
        let psize = _mi_os_page_size_typed().get();
        crate::internal::mi_assert_internal!(bsize > psize);
        crate::internal::mi_assert_internal!(!(*page).memid.is_pinned);

        let gpage = block.cast::<u8>().add(bsize - psize).cast::<c_void>();
        crate::internal::mi_assert_internal!((gpage as usize).is_multiple_of(psize));
        crate::os::_mi_os_unprotect(gpage, psize);
    }
}

/// # Safety
/// `page`, `block`, and `p` must describe the same allocated block.
pub(crate) unsafe fn mi_block_check_unguard(
    page: crate::core_types::PagePtr,
    block: crate::core_types::BlockForFree,
    p: crate::core_types::UserPtr,
) -> bool {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let block_ptr = block.block();
        let user = p.as_ptr();
        if MI_GUARDED != 0
            && block.is_guarded()
            && mi_block_ptr_is_guarded(block_ptr.as_ptr(), user)
        {
            mi_block_unguard(
                page,
                crate::core_types::GuardedBlock::after_guarded_detection(block_ptr),
                p,
            );
            true
        } else {
            let _ = page;
            let _ = block;
            let _ = p;
            false
        }
    }
}

#[allow(clippy::absurd_extreme_comparisons)]
pub(crate) unsafe fn mi_validate_block_from_ptr(
    page: crate::core_types::PagePtr,
    user: crate::core_types::UserPtr,
) -> crate::core_types::PointerLookup {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let location = mi_pointer_location(page, user, MI_SECURE > 0, true);

        crate::core_types::PointerLookup::valid(user, page, location)
    }
}

unsafe fn mi_lookup_pointer_page(
    user: crate::core_types::UserPtr,
) -> Option<crate::core_types::PagePtr> {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let p = user.as_ptr();
        if MI_DEBUG != 0 || cfg!(feature = "secure") {
            let registered = _mi_safe_ptr_page_registered(p);
            match registered {
                Some(registered) if registered.page().contains_user(user) => {
                    Some(registered.page())
                }
                _ => None,
            }
        } else {
            crate::core_types::PagePtr::new(_mi_ptr_page(p))
        }
    }
}

unsafe fn mi_pointer_location(
    page: crate::core_types::PagePtr,
    user: crate::core_types::UserPtr,
    force_unalign: bool,
    assert_block_start: bool,
) -> crate::core_types::PointerLocation {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let page_raw = page.as_ptr();
        let user_raw = user.as_ptr();

        let block = if mi_page_has_interior_pointers(page_raw) || force_unalign {
            _mi_page_ptr_unalign(page_raw, user_raw)
        } else {
            user_raw.cast()
        };
        if assert_block_start && !mi_page_has_interior_pointers(page_raw) {
            crate::internal::mi_assert!(
                _mi_page_ptr_unalign(page_raw, user_raw) == user_raw.cast()
            );
        }

        let block = crate::core_types::BlockPtr::new_unchecked(block);
        let offset =
            crate::core_types::BlockOffset((user_raw as usize) - (block.as_ptr() as usize));
        if MI_GUARDED != 0 && mi_block_ptr_is_guarded(block.as_ptr(), user_raw) {
            crate::core_types::PointerLocation::Guarded {
                block: crate::core_types::GuardedBlock::after_guarded_detection(block),
                offset,
            }
        } else if offset.0 != 0 {
            crate::core_types::PointerLocation::Interior { block, offset }
        } else {
            crate::core_types::PointerLocation::BlockStart(block)
        }
    }
}

/// # Safety
/// `lookup` must come from validating the same page/user pair that is about to
/// be freed by the generic free path. The caller must not separately unguard
/// the block; this boundary consumes the lookup and performs any required
/// guarded-block unprotection before returning the freeable block witness.
unsafe fn mi_block_for_free_from_lookup(
    lookup: crate::core_types::PointerLookup,
) -> Option<crate::core_types::BlockForFree> {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe { mi_prepare_validated_free(lookup).map(crate::core_types::ValidatedFree::block) }
}

const _: unsafe fn(crate::core_types::PointerLookup) -> Option<crate::core_types::BlockForFree> =
    mi_block_for_free_from_lookup;

// SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
#[cold]
#[inline(never)]
pub(crate) unsafe extern "C" fn mi_free_generic_local(page: *mut mi_page_t, p: *mut c_void) {
    unsafe {
        crate::internal::mi_assert_internal!(!p.is_null() && !page.is_null());
        let page = crate::core_types::PagePtr::new_unchecked(page);
        let user = crate::core_types::UserPtr::new_unchecked(p);
        let Some(validated) = mi_prepare_validated_free(mi_validate_block_from_ptr(page, user))
        else {
            return;
        };

        // SAFETY: The generic dispatch boundary supplies a non-null page/user pair;
        // validation reconstructs the typed free witness.
        let page_wrapper = crate::core_types::LocalPage::new_unchecked(validated.page());

        mi_free_block_local(
            page_wrapper,
            validated.block(),
            crate::core_types::LocalFreeMode::TrackAndCheckFull,
        );
    }
}

// SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
#[cold]
#[inline(never)]
pub(crate) unsafe extern "C" fn mi_free_generic_mt(page: *mut mi_page_t, p: *mut c_void) {
    unsafe {
        crate::internal::mi_assert_internal!(!p.is_null() && !page.is_null());
        let page = crate::core_types::PagePtr::new_unchecked(page);
        let user = crate::core_types::UserPtr::new_unchecked(p);
        let Some(validated) = mi_prepare_validated_free(mi_validate_block_from_ptr(page, user))
        else {
            return;
        };

        // SAFETY: The generic dispatch boundary supplies a non-null page/user pair;
        // validation reconstructs the typed free witness.
        let page_wrapper = crate::core_types::ForeignPage::new_unchecked(validated.page());

        mi_free_block_mt(page_wrapper, validated.block());
    }
}

// SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_free_generic(
    page: *mut mi_page_t,
    is_local: bool,
    p: *mut c_void,
) {
    // SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
    unsafe {
        if is_local {
            mi_free_generic_local(page, p);
        } else {
            mi_free_generic_mt(page, p);
        }
    }
}

#[used]
// SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
static MI_FREE_GENERIC_KEEP: unsafe extern "C" fn(*mut mi_page_t, bool, *mut c_void) =
    _mi_free_generic;

pub(crate) unsafe fn mi_validate_pointer(
    p: *mut c_void,
    msg: *const c_char,
) -> crate::core_types::PointerLookup {
    // SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
    unsafe {
        use crate::core_types::{PointerInvalidReason, PointerLookup, UserPtr};
        if p.is_null() {
            return PointerLookup::Null;
        }
        let user_ptr = UserPtr::new_unchecked(p);

        if (MI_DEBUG != 0 || cfg!(feature = "secure"))
            && ((p as usize) & (MI_INTPTR_SIZE - 1)) != 0
            && !mi_option_is_enabled(mi_option_t::mi_option_guarded_precise)
        {
            let msg_str = core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes());
            _mi_error_message_fmt(
                Errno::EINVAL,
                format_args!("{msg_str}: invalid (unaligned) pointer: {p:p}\n"),
            );
            return PointerLookup::Invalid {
                user: Some(user_ptr),
                reason: PointerInvalidReason::BadAlignment,
            };
        }

        let registered_page = mi_lookup_pointer_page(user_ptr);

        let Some(page_ptr) = registered_page else {
            if MI_DEBUG != 0 || cfg!(feature = "secure") {
                let msg_str = core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes());
                _mi_error_message_fmt(
                    Errno::EINVAL,
                    format_args!("{msg_str}: invalid pointer: {p:p}\n"),
                );
            }
            return PointerLookup::Invalid {
                user: Some(user_ptr),
                reason: PointerInvalidReason::NotOwnedByAllocator,
            };
        };

        let location = mi_pointer_location(page_ptr, user_ptr, false, false);

        PointerLookup::valid(user_ptr, page_ptr, location)
    }
}

#[inline(always)]
unsafe fn mi_prepare_validated_free(
    lookup: crate::core_types::PointerLookup,
) -> Option<crate::core_types::ValidatedFree> {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let validated = lookup.prepare_free()?;
        if validated.block().was_guarded() == crate::core_types::WasGuarded::Yes {
            mi_block_check_unguard(validated.page(), validated.block(), validated.user());
        }
        Some(validated)
    }
}

#[inline(always)]
unsafe fn mi_free_validated(validated: crate::core_types::ValidatedFree, usable: *mut usize) {
    // SAFETY[AUDIT:free-pointer-lookup-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let page = validated.page().as_ptr();
        crate::core_types::UsableOut::new(usable).write_if_present(mi_page_usable_block_size(page));

        let current_tid = crate::core_types::ThreadId::new_unchecked(_mi_prim_thread_id());
        let page_tid = validated.page().thread_id_cell().load_relaxed();
        let path = page_tid.classify_free_path(current_tid);
        let owner = validated.page().classify_for_free(current_tid);

        match (path, owner) {
            (
                crate::core_types::FreePathKind::LocalFast,
                crate::core_types::PageFreeOwner::Local(local_page),
            ) => {
                mi_free_block_local(
                    local_page,
                    validated.block(),
                    crate::core_types::LocalFreeMode::TrackOnly,
                );
            }
            (_, owner) => mi_free_ex_slow(validated, owner, path),
        }
    }
}

#[inline(always)]
fn mi_free_fast_path_enabled(usable: *mut usize) -> bool {
    usable.is_null()
        && !cfg!(debug_assertions)
        && MI_DEBUG == 0
        && MI_SECURE == 0
        && MI_GUARDED == 0
        && MI_PADDING == 0
}

#[inline(always)]
unsafe fn mi_free_fast_try(p: *mut c_void, usable: *mut usize) -> bool {
    // SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
    unsafe {
        if !mi_free_fast_path_enabled(usable) {
            return false;
        }

        let page = _mi_ptr_page(p);
        let Some(page_ptr) = crate::core_types::PagePtr::new(page) else {
            return false;
        };
        let current = crate::core_types::ThreadId::new_unchecked(_mi_prim_thread_id());
        let page_tid = page_ptr.thread_id_cell().load_relaxed();
        let block = crate::core_types::BlockForFree::from_block_start_unchecked(
            crate::core_types::BlockPtr::new_unchecked(p.cast()),
        );

        match page_tid.classify_free_path(current) {
            crate::core_types::FreePathKind::LocalFast => {
                mi_free_block_local(
                    crate::core_types::LocalPage::new_unchecked(page_ptr),
                    block,
                    crate::core_types::LocalFreeMode::TrackOnly,
                );
                true
            }
            crate::core_types::FreePathKind::RemotePlain
                if !page_tid.is_abandoned() && !page_tid.is_abandoned_mapped() =>
            {
                mi_free_block_mt(
                    crate::core_types::ForeignPage::new_unchecked(page_ptr),
                    block,
                );
                true
            }
            _ => false,
        }
    }
}

#[cold]
#[inline(never)]
pub(crate) unsafe fn mi_free_ex_slow(
    validated: crate::core_types::ValidatedFree,
    owner: crate::core_types::PageFreeOwner,
    path: crate::core_types::FreePathKind,
) {
    // SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
    unsafe {
        // `validated` was produced by `mi_validate_pointer`, which already derived the
        // interior-corrected block and resolved guard state. Route the typed witness
        // directly to the block free functions instead of re-deriving from the raw
        // pointer via `mi_free_generic_local`/`mi_free_generic_mt` (which would also
        // double-call `mi_block_check_unguard` on guarded blocks).
        match owner {
            crate::core_types::PageFreeOwner::Local(local_page) => {
                // Local page with flags set (e.g. in the full queue): check for unfull.
                mi_free_block_local(
                    local_page,
                    validated.block(),
                    crate::core_types::LocalFreeMode::for_free_path(path),
                );
            }
            crate::core_types::PageFreeOwner::Foreign(foreign_page) => {
                // Remote (plain or with flags): push onto the owning thread's free list.
                // The flag distinction collapses here because validation already resolved
                // the block; `mi_free_block_mt` does not branch on page flags.
                mi_free_block_mt(foreign_page, validated.block());
            }
            crate::core_types::PageFreeOwner::Abandoned(abandoned_page) => {
                mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());
            }
            crate::core_types::PageFreeOwner::AbandonedMapped(abandoned_page) => {
                mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());
            }
        }
    }
}

pub(crate) unsafe fn mi_route_free(p: *mut c_void, msg: *const c_char, usable: *mut usize) {
    // SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
    unsafe {
        if p.is_null() {
            return;
        }
        if mi_free_fast_try(p, usable) {
            return;
        }
        if let Some(validated) = mi_prepare_validated_free(mi_validate_pointer(p, msg)) {
            mi_free_validated(validated, usable);
        }
    }
}

pub(crate) unsafe fn mi_route_usable_size_typed(
    p: *const c_void,
    msg: *const c_char,
) -> crate::core_types::UsableSize {
    // SAFETY[AUDIT:free-route-dispatch-boundary]: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
    unsafe {
        if p.is_null() {
            return crate::core_types::UsableSize::new(0);
        }
        if MI_DEBUG == 0 && !cfg!(feature = "secure") && MI_GUARDED == 0 {
            let Some(user) = crate::core_types::UserPtr::new(p.cast_mut()) else {
                return crate::core_types::UsableSize::new(0);
            };
            let Some(page) = mi_lookup_pointer_page(user) else {
                return crate::core_types::UsableSize::new(0);
            };
            return _mi_usable_size_typed(p, page.as_ptr());
        }
        match mi_validate_pointer(p.cast_mut(), msg) {
            crate::core_types::PointerLookup::Valid(valid) => {
                let located = valid.location().located_block();
                mi_page_usable_size_of_typed(
                    valid.page().as_ptr(),
                    located.block().as_ptr(),
                    located.was_guarded() == crate::core_types::WasGuarded::Yes,
                )
                .checked_sub(located.offset().0)
                .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
            }
            _ => crate::core_types::UsableSize::new(0),
        }
    }
}

pub(crate) unsafe fn mi_route_usable_size(p: *const c_void, msg: *const c_char) -> usize {
    // SAFETY[AUDIT:free-usable-size-boundary]: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
    unsafe { mi_route_usable_size_typed(p, msg).get() }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator and not yet freed.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
// SAFETY[AUDIT:free-usable-size-boundary]: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
pub unsafe extern "C" fn mi_free(p: *mut c_void) {
    unsafe {
        mi_route_free(p, c"mi_free".as_ptr(), ptr::null_mut());
    }
}

// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[cold]
#[inline(never)]
pub(crate) unsafe extern "C" fn mi_free_try_collect_mt(
    page: *mut mi_page_t,
    mt_free: *mut mi_block_t,
) {
    // SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_internal!(!mt_free.is_null());
        mi_free_try_collect_mt_typed(
            crate::core_types::OwnedAbandonedPage::new_unchecked(
                crate::core_types::PagePtr::new_unchecked(page),
            ),
            crate::core_types::RemoteFreeList::from_detached_head(
                crate::core_types::BlockPtr::new_unchecked(mt_free),
            ),
        );
    }
}

// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
const _: unsafe extern "C" fn(*mut mi_page_t, *mut mi_block_t) = mi_free_try_collect_mt;

/// # Safety
/// `page` must be the page containing `p`.
// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[inline(never)]
pub unsafe extern "C" fn mi_page_usable_aligned_size_of(
    page: *const mi_page_t,
    p: *const c_void,
) -> usize {
    // SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe { mi_page_usable_aligned_size_of_typed(page, p).get() }
}

pub(crate) unsafe fn mi_page_usable_aligned_size_of_typed(
    page: *const mi_page_t,
    p: *const c_void,
) -> crate::core_types::UsableSize {
    // SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe {
        let block = _mi_page_ptr_unalign(page, p);
        let is_guarded = mi_block_ptr_is_guarded(block, p.cast_mut());
        let size = mi_page_usable_size_of_typed(page, block, is_guarded);
        let adjust = p.cast::<u8>().offset_from(block.cast::<u8>());
        crate::internal::mi_assert_internal!(adjust >= 0 && adjust as usize <= size.get());
        size.checked_sub(adjust as usize)
            .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
    }
}

/// # Safety
/// `page` and `location` must come from the same successful pointer validation.
#[inline(always)]
pub(crate) unsafe fn mi_usable_size_from_validated_location(
    page: crate::core_types::PagePtr,
    location: crate::core_types::PointerLocation,
) -> crate::core_types::UsableSize {
    // SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe {
        let located = location.located_block();
        let was_guarded = located.was_guarded() == crate::core_types::WasGuarded::Yes;
        let size =
            mi_page_usable_size_of_typed(page.as_ptr(), located.block().as_ptr(), was_guarded);
        let adjust = located.offset().0;
        if adjust == 0 {
            return size;
        }
        crate::internal::mi_assert_internal!(adjust <= size.get());
        size.checked_sub(adjust)
            .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
    }
}

pub(crate) unsafe fn _mi_usable_size_typed(
    p: *const c_void,
    page: *const mi_page_t,
) -> crate::core_types::UsableSize {
    // SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe {
        if page.is_null() {
            return crate::core_types::UsableSize::new(0);
        }
        if !mi_page_has_interior_pointers(page) {
            mi_page_usable_size_of_typed(page, p.cast(), false)
        } else {
            mi_page_usable_aligned_size_of_typed(page, p)
        }
    }
}

/// # Safety
/// `page` must be null or the page containing `p`.
pub unsafe fn _mi_usable_size(p: *const c_void, page: *const mi_page_t) -> usize {
    // SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe { _mi_usable_size_typed(p, page).get() }
}

#[used]
// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
static MI_PAGE_USABLE_ALIGNED_SIZE_OF_KEEP: unsafe extern "C" fn(
    *const mi_page_t,
    *const c_void,
) -> usize = mi_page_usable_aligned_size_of;

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY[AUDIT:free-raw-export-boundary]: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_usable_size(p: *const c_void) -> usize {
    unsafe { mi_route_usable_size(p, c"mi_usable_size".as_ptr()) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator and not yet freed.
// Retained as an exported C ABI free-size symbol; release cdylib builds may
// reference it only from external dynamic callers or assembly aliases.
#[allow(dead_code)]
// SAFETY[AUDIT:free-abi-validation-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_free_size(p: *mut c_void, size: usize) {
    unsafe {
        let _ = size;
        if MI_DEBUG != 0 {
            let available = mi_route_usable_size(p, c"mi_free_size".as_ptr());
            crate::internal::mi_assert!(p.is_null() || size <= available || available == 0);
        }
        mi_free(p);
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator with the given alignment.
// Retained as an exported C ABI free-size-aligned symbol; release cdylib builds
// may reference it only from external dynamic callers or assembly aliases.
#[allow(dead_code)]
// SAFETY[AUDIT:free-abi-validation-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_free_size_aligned(p: *mut c_void, size: usize, alignment: usize) {
    unsafe {
        crate::internal::mi_assert!(alignment != 0 && (p as usize).is_multiple_of(alignment));
        mi_free_size(p, size);
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator with the given alignment.
// Retained as an exported C ABI aligned-free symbol; release cdylib builds may
// reference it only from external dynamic callers or assembly aliases.
#[allow(dead_code)]
// SAFETY[AUDIT:free-abi-validation-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_free_aligned(p: *mut c_void, alignment: usize) {
    unsafe {
        crate::internal::mi_assert!(alignment != 0 && (p as usize).is_multiple_of(alignment));
        mi_free(p);
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator and not yet freed. If
/// `usable` is non-null it must be writable for one `usize`.
// SAFETY[AUDIT:free-abi-validation-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_ufree(p: *mut c_void, usable: *mut usize) {
    unsafe {
        mi_route_free(p, c"mi_ufree".as_ptr(), usable);
    }
}

/// # Safety
/// `p` must be null or a small allocation pointer previously returned by this allocator.
// SAFETY[AUDIT:free-abi-validation-boundary]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_free_small(p: *mut c_void) {
    unsafe {
        if MI_PAGE_META_ALIGNED_FREE_SMALL == 0 || MI_GUARDED != 0 {
            mi_free(p);
            return;
        }

        let page = _mi_align_down_ptr(p, MI_SMALL_PAGE_SIZE).cast::<mi_page_t>();
        crate::internal::mi_assert!(if let crate::core_types::PointerLookup::Valid(valid) =
            mi_validate_pointer(p, c"mi_free_small".as_ptr())
        {
            page == valid.page().as_ptr()
        } else {
            false
        });
        crate::internal::mi_assert!(
            page.cast::<c_void>()
                == _mi_align_down_ptr(mi_page_start(page).cast(), MI_SMALL_PAGE_SIZE)
        );
        crate::internal::mi_assert!((*page).block_size <= MI_SMALL_SIZE_MAX);
        mi_route_free(p, c"mi_free_small".as_ptr(), ptr::null_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_types::{
        BlockPtr, FreePathKind, PageFlags, PageFreeOwner, PageOwnerId, PagePtr, PageThreadId,
        PointerInvalidReason, PointerLocation, PointerLookup, ThreadFreeOwner, ThreadFreeWord,
        ThreadId, UserPtr,
    };
    use crate::init::_mi_thread_id;
    use crate::types::{mi_block_t, mi_page_t};
    use core::{mem::MaybeUninit, ptr};

    #[cfg(feature = "guarded")]
    use crate::core_types::WasGuarded;

    struct FreeFixture {
        page: MaybeUninit<mi_page_t>,
        blocks: [mi_block_t; 4],
    }

    impl FreeFixture {
        fn new(owner: PageOwnerId, flags: PageFlags) -> Self {
            let mut fixture = Self {
                page: MaybeUninit::zeroed(),
                blocks: [
                    mi_block_t::default(),
                    mi_block_t::default(),
                    mi_block_t::default(),
                    mi_block_t::default(),
                ],
            };
            // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
            unsafe {
                let page = fixture.page.as_mut_ptr();
                ptr::addr_of_mut!((*page).xthread_id)
                    .write(PageThreadId::from_parts(owner, flags).raw());
                ptr::addr_of_mut!((*page).free).write(ptr::null_mut());
                ptr::addr_of_mut!((*page).used).write(2);
                ptr::addr_of_mut!((*page).capacity).write(2);
                ptr::addr_of_mut!((*page).reserved).write(2);
                ptr::addr_of_mut!((*page).retire_expire).write(1);
                ptr::addr_of_mut!((*page).free_is_zero).write(false);
                ptr::addr_of_mut!((*page).local_free).write(ptr::null_mut());
                ptr::addr_of_mut!((*page).xthread_free)
                    .write(ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned).raw());
                ptr::addr_of_mut!((*page).block_size).write(2 * core::mem::size_of::<mi_block_t>());
                ptr::addr_of_mut!((*page).page_start)
                    .write(ptr::addr_of_mut!(fixture.blocks[0]).cast::<u8>());
                #[cfg(any(
                    feature = "secure",
                    debug_assertions,
                    feature = "track-valgrind",
                    feature = "track-asan"
                ))]
                ptr::addr_of_mut!((*page).keys).write([0, 0]);
            }
            fixture
        }

        fn page(&mut self) -> PagePtr {
            // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
            unsafe {
                ptr::addr_of_mut!((*self.page.as_mut_ptr()).page_start)
                    .write(ptr::addr_of_mut!(self.blocks[0]).cast::<u8>());
                PagePtr::new_unchecked(self.page.as_mut_ptr())
            }
        }

        fn user(&mut self, index: usize) -> UserPtr {
            // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
            unsafe { UserPtr::new_unchecked(ptr::addr_of_mut!(self.blocks[index * 2]).cast()) }
        }

        fn block(&mut self, index: usize) -> BlockPtr {
            unsafe { BlockPtr::new_unchecked(ptr::addr_of_mut!(self.blocks[index * 2])) }
        }
    }

    fn current_thread_owner() -> PageOwnerId {
        PageOwnerId::Thread(ThreadId::new(_mi_thread_id()).expect("current thread id is valid"))
    }

    struct ShowErrorsGuard {
        previous: core::ffi::c_long,
    }

    impl ShowErrorsGuard {
        fn disabled() -> Self {
            let option = crate::options::mi_option_t::mi_option_show_errors;
            let previous = crate::options::mi_option_get(option);
            crate::options::mi_option_set(option, 0);
            Self { previous }
        }
    }

    impl Drop for ShowErrorsGuard {
        fn drop(&mut self) {
            crate::options::mi_option_set(
                crate::options::mi_option_t::mi_option_show_errors,
                self.previous,
            );
        }
    }

    #[test]
    fn free_validation_covers_null_invalid_and_bad_alignment() {
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            assert!(matches!(
                mi_validate_pointer(ptr::null_mut(), c"free-test".as_ptr()),
                PointerLookup::Null
            ));

            let mut aligned_non_heap = 0usize;
            assert!(matches!(
                mi_validate_pointer(
                    ptr::addr_of_mut!(aligned_non_heap).cast(),
                    c"free-test".as_ptr()
                ),
                PointerLookup::Invalid {
                    reason: PointerInvalidReason::NotOwnedByAllocator,
                    ..
                }
            ));

            assert!(matches!(
                mi_validate_pointer(ptr::dangling_mut::<u8>().cast(), c"free-test".as_ptr()),
                PointerLookup::Invalid {
                    reason: PointerInvalidReason::BadAlignment,
                    ..
                }
            ));
        }
    }

    #[test]
    fn free_validation_covers_interior_pointer_lookup() {
        let mut fixture =
            FreeFixture::new(current_thread_owner(), PageFlags::HAS_INTERIOR_POINTERS);
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let page = fixture.page();
            let user = UserPtr::new_unchecked(fixture.user(0).as_u8_ptr().add(1).cast());
            let lookup = mi_validate_block_from_ptr(page, user);

            match lookup {
                PointerLookup::Valid(valid) => match valid.location() {
                    PointerLocation::Interior { block, offset } => {
                        assert_eq!(block.as_ptr(), fixture.block(0).as_ptr());
                        assert_eq!(offset.0, 1);
                    }
                    _ => panic!("interior pointer did not produce an interior location"),
                },
                _ => panic!("interior pointer lookup was not valid"),
            }
        }
    }

    #[cfg(feature = "guarded")]
    #[test]
    fn free_validation_covers_guarded_pointer_lookup() {
        let mut fixture =
            FreeFixture::new(current_thread_owner(), PageFlags::HAS_INTERIOR_POINTERS);
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            crate::core_types::BlockTag::write_guarded_to(fixture.block(0));
            let page = fixture.page();
            let user = UserPtr::new_unchecked(
                fixture
                    .user(0)
                    .as_u8_ptr()
                    .add(core::mem::size_of::<mi_block_t>())
                    .cast(),
            );

            let lookup = mi_validate_block_from_ptr(page, user);
            match lookup {
                PointerLookup::Valid(valid) => match valid.location() {
                    PointerLocation::Guarded { block, offset } => {
                        assert_eq!(block.block().as_ptr(), fixture.block(0).as_ptr());
                        assert_eq!(offset.0, core::mem::size_of::<mi_block_t>());
                    }
                    _ => panic!("guarded pointer did not produce a guarded location"),
                },
                _ => panic!("guarded pointer lookup was not valid"),
            }

            let validated = mi_validate_block_from_ptr(page, user)
                .prepare_free()
                .expect("guarded pointer prepares free witness");

            assert_eq!(
                validated.block().block().as_ptr(),
                fixture.block(0).as_ptr()
            );
            assert_eq!(validated.block().was_guarded(), WasGuarded::Yes);
            assert!(crate::core_types::BlockLink::is_guarded_raw(
                fixture.block(0).as_ptr()
            ));
        }
    }

    #[test]
    fn free_route_covers_local_remote_and_abandoned_owners() {
        let current = ThreadId::new(_mi_thread_id()).expect("current thread id is valid");

        let mut local = FreeFixture::new(current_thread_owner(), PageFlags::EMPTY);
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let validated =
                mi_prepare_validated_free(mi_validate_block_from_ptr(local.page(), local.user(0)))
                    .expect("local free validates");
            assert_eq!(validated.block().block().as_ptr(), local.block(0).as_ptr());
            assert!(matches!(
                PageThreadId::new((*local.page().as_ptr()).xthread_id).classify_free_path(current),
                FreePathKind::LocalFast
            ));
            assert!(matches!(
                validated.page().classify_for_free(current),
                PageFreeOwner::Local(_)
            ));
        }

        let remote_raw = if current.get() == 0x1000 {
            0x2000
        } else {
            0x1000
        };
        let remote_owner = PageOwnerId::Thread(ThreadId::new(remote_raw).expect("valid remote id"));
        let mut remote = FreeFixture::new(remote_owner, PageFlags::EMPTY);
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let validated = mi_prepare_validated_free(mi_validate_block_from_ptr(
                remote.page(),
                remote.user(0),
            ))
            .expect("remote free validates");
            assert_eq!(validated.block().block().as_ptr(), remote.block(0).as_ptr());
            assert!(matches!(
                PageThreadId::new((*remote.page().as_ptr()).xthread_id).classify_free_path(current),
                FreePathKind::RemotePlain
            ));
            assert!(matches!(
                validated.page().classify_for_free(current),
                PageFreeOwner::Foreign(_)
            ));
        }

        let mut abandoned = FreeFixture::new(PageOwnerId::Abandoned, PageFlags::EMPTY);
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let validated = mi_prepare_validated_free(mi_validate_block_from_ptr(
                abandoned.page(),
                abandoned.user(0),
            ))
            .expect("abandoned free validates");
            assert_eq!(
                validated.block().block().as_ptr(),
                abandoned.block(0).as_ptr()
            );
            assert!(matches!(
                validated.page().classify_for_free(current),
                PageFreeOwner::Abandoned(_)
            ));
        }
    }

    #[test]
    fn free_checks_cover_double_free_detection() {
        let mut fixture = FreeFixture::new(current_thread_owner(), PageFlags::EMPTY);
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        // SAFETY[AUDIT:free-validation-fixtures]: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let page = fixture.page().as_ptr();
            let block = fixture.block(0).as_ptr();
            (*page).free = block;
            assert!(mi_check_is_double_freex(page, block));
        }
    }
}
