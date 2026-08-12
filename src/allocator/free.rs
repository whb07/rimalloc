//! Mechanical translation of the early `src/free.c` free-path helpers.

mod abandoned;

use core::ffi::{CStr, c_char, c_void};
use core::{cmp, ptr};

use crate::allocator::arena::{
    _mi_arenas_page_free_internal, _mi_arenas_page_try_reabandon_to_mapped_arena,
    _mi_arenas_page_unabandon_internal,
};
use crate::allocator::options::{_mi_error_message_fmt, mi_option_is_enabled, mi_option_t};
use crate::allocator::page::{
    _mi_page_retire_internal, _mi_page_unfull_internal, _mi_theap_page_reclaim,
    MI_MEDIUM_MAX_OBJ_SIZE, mi_bin_from_usable, mi_page_is_abandoned, mi_page_is_huge,
    mi_page_queue_count_typed,
};
use crate::allocator::tracking::{mi_track_free_size, mi_track_mem_defined, mi_track_mem_noaccess};
use crate::bits::MI_INTPTR_SIZE;
use crate::core_types::Errno;
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_decrease, _mi_align_down_ptr,
    _mi_arena_memid_is_suitable, _mi_is_power_of_two, _mi_memset_aligned, _mi_os_page_size_typed,
    _mi_ptr_page, _mi_safe_ptr_page_registered, _mi_theap_heap, MI_DEBUG, MI_GUARDED,
    MI_LARGE_MAX_OBJ_SIZE, MI_MiB, MI_SECURE, MI_TRACK_ENABLED, mi_block_next, mi_block_nextx,
    mi_block_ptr_is_guarded, mi_block_set_next_typed, mi_is_in_same_page, mi_page_all_free,
    mi_page_block_size, mi_page_has_interior_pointers, mi_page_is_abandoned_mapped,
    mi_page_is_full, mi_page_is_mostly_used, mi_page_is_owned, mi_page_is_singleton, mi_page_keys,
    mi_page_queue, mi_page_start, mi_page_thread_free, mi_page_usable_block_size,
    mi_theap_is_initialized,
};
use crate::prim::{
    _mi_page_associated_theap_peek, _mi_prim_thread_id, _mi_theap_default,
    _mi_thread_is_initialized,
};
use crate::types::{
    MI_DEBUG_FREED, MI_DEBUG_PADDING, MI_ENCODE_FREELIST, MI_MAX_ALIGN_SIZE, MI_PADDING,
    MI_PADDING_CHECK, MI_PAGE_META_ALIGNED_FREE_SMALL, MI_SMALL_PAGE_SIZE, MI_SMALL_SIZE_MAX,
    mi_block_t, mi_page_t, mi_theap_t,
};

use abandoned::collect_after_remote_free;

// SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[cfg(mi_c_malloc_fast)]
core::arch::global_asm!(
    ".hidden _mi_padding_shrink",
    ".type _mi_padding_shrink,@function",
    "_mi_padding_shrink:",
    "ret",
    ".size _mi_padding_shrink, .-_mi_padding_shrink",
);

// SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
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

/// # Safety
///
/// `page` must identify live initialized page metadata with stable encoded-list
/// keys. `list` must be null or a readable, well-formed, terminating encoded
/// list of aligned blocks from that page; its head and links must remain stable
/// for the traversal, or be a protocol-safe remote snapshot whose published
/// links remain readable and are not relinked. `elem` is used only for pointer
/// comparison and need not be dereferenceable.
unsafe fn mi_list_contains(
    page: *const mi_page_t,
    mut list: *const mi_block_t,
    elem: *const mi_block_t,
) -> bool {
    // SAFETY: the caller keeps the page keys and this well-formed same-page
    // encoded list readable and stable for the traversal.
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

// SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_ptr_unalign(
    page: *const mi_page_t,
    p: *const c_void,
) -> *mut mi_block_t {
    // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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

/// # Safety
///
/// `page` must identify live initialized page metadata and `block` must be a
/// readable, aligned block location in that page; the block may already be
/// free. The caller must locally own the page or hold its abandoned-page claim.
/// Its encoded keys and local free-list heads and links must remain stable, and
/// a remote-list snapshot must contain fully initialized readable links that
/// are not relinked after publication. Any registered diagnostic callback must
/// not unwind.
unsafe fn mi_check_is_double_freex(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    // SAFETY: the caller provides stable page keys and local lists plus a
    // protocol-safe remote snapshot, all as readable same-page encoded lists.
    // The diagnostic path may invoke only a non-unwinding registered callback.
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());
        if mi_list_contains(page, page_ptr.free().as_raw(), block)
            || mi_list_contains(page, page_ptr.local_free().as_raw(), block)
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

/// Decode one allocated block's padding trailer into its stored delta and page
/// usable block size.
///
/// # Safety
/// `page` must identify live initialized page metadata and `block` must be the
/// aligned start of a live allocated block owned by that page. The page's usable
/// size, padding keys, block backing, and initialized padding trailer must remain
/// readable and stable with no concurrent free, realloc, or trailer mutation.
unsafe fn mi_page_decode_padding(
    page: *const mi_page_t,
    block: *const mi_block_t,
    delta: &mut usize,
    bsize: &mut usize,
) -> bool {
    // SAFETY: the caller couples this live block start to `page` and keeps its
    // usable size, complete backing, padding trailer, and page keys initialized
    // and stable. Tracking visibility is restored after the trailer read and
    // canary check.
    let canary_ok = unsafe {
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
        let keys = crate::core_types::PageKeys::from_raw(mi_page_keys(page));
        let canary_ok =
            crate::core_types::PaddingCanary::from_page_block_keys(page_ptr, block_ptr, keys).get()
                == canary;
        mi_track_mem_noaccess(
            padding_ptr.cast::<c_void>(),
            crate::core_types::PaddingPtr::SIZE,
        );
        canary_ok
    };
    canary_ok && *delta <= *bsize
}

#[cfg(not(mi_c_malloc_fast))]
#[inline(never)]
/// Shrink the logical padding delta so at least `min_size` bytes are reported
/// usable when the page's physical block size permits it.
///
/// # Safety
/// When padding is enabled, `page` must identify live initialized page metadata
/// and `block` must be the aligned start of a live allocated block owned by that
/// page. The caller must exclusively own mutation of its initialized padding
/// trailer while page usable-size metadata, keys, and backing remain live and
/// stable with no concurrent free or realloc. If shrinking is required,
/// `bsize - min_size` must fit the padding trailer's `u32` delta field, as it
/// does for allocator-produced aligned allocation layouts.
pub(crate) unsafe extern "C" fn _mi_padding_shrink(
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
    // SAFETY: the caller provides this matching live page/block pair and keeps
    // its usable size, keys, backing, and initialized padding trailer stable.
    let ok = unsafe { mi_page_decode_padding(page, block, &mut delta, &mut bsize) };
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
    // SAFETY: the caller exclusively owns this live block's padding mutation;
    // `new_delta < bsize` and the caller's allocator-layout bound make the delta
    // representable for this trailer. Tracking visibility is balanced around
    // the write.
    unsafe {
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

/// # Safety
///
/// `page` must be a live initialized page and `block` must be a live allocated
/// block owned by that page. The page's usable-size metadata, padding-canary
/// keys, block backing, and padding metadata must remain initialized, readable,
/// and stable; the trailing bytes covered by the padding check must likewise
/// remain stable for the call, with no concurrent mutation or free of the block.
unsafe fn mi_verify_padding(
    page: *const mi_page_t,
    block: *const mi_block_t,
    size: &mut usize,
    wrong: &mut usize,
) -> bool {
    let mut bsize = 0usize;
    let mut delta = 0usize;
    // SAFETY: the caller provides this matching live page/block pair and keeps
    // its usable size, keys, backing, and initialized padding trailer stable.
    let mut ok = unsafe { mi_page_decode_padding(page, block, &mut delta, &mut bsize) };
    *size = bsize;
    *wrong = bsize;
    if !ok {
        return false;
    }
    crate::internal::mi_assert_internal!(bsize >= delta);
    *size = bsize - delta;
    // SAFETY: the caller keeps the live page's initialized huge-page flag
    // readable and stable. For a non-huge page, the validated delta bounds the
    // initialized trailing fill inside this live block; concurrent mutation is
    // excluded and tracking visibility is balanced around the bytewise check.
    unsafe {
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
    }
    ok
}

/// # Safety
///
/// When padding checks are enabled, `page` must be a live initialized page and
/// `block` must be a live allocated block owned by that page. The page's usable-
/// size metadata, padding-canary keys, block backing, padding metadata, and the
/// trailing bytes covered by the check must be initialized, readable, and
/// stable for the call, with no concurrent mutation or free of the block.
/// Registered diagnostic callbacks and opaque arguments must remain valid for
/// synchronous error reporting, must not incompatibly reenter this check, and
/// must not unwind. When padding checks are disabled, the function returns
/// without dereferencing `page` or `block`.
pub unsafe fn mi_check_padding(page: *const mi_page_t, block: *const mi_block_t) {
    if MI_PADDING == 0 || MI_PADDING_CHECK == 0 {
        return;
    }

    let mut size = 0usize;
    let mut wrong = 0usize;
    // SAFETY: the caller supplies the matching live page and allocated block,
    // stable usable-size/keys/backing/padding metadata, and valid diagnostics.
    let valid = unsafe { mi_verify_padding(page, block, &mut size, &mut wrong) };
    if !valid {
        _mi_error_message_fmt(
            Errno::EFAULT,
            format_args!(
                "buffer overflow in theap block {block:p} of size {size}: write after {wrong} bytes\n"
            ),
        );
    }
}

/// # Safety
/// `page` must identify live initialized page metadata and `block` must identify
/// a readable, aligned block location in that page; the block may already be
/// free. The page backing, encoded-list keys, and local free-list heads and
/// links must remain stable and readable for the call. The caller must locally
/// own the page or hold its abandoned-page claim. Concurrent protocol
/// remote-free publication may update the remote head only after initializing
/// each readable link and must not relink an already-published node. Any
/// registered diagnostic callback must not unwind.
pub unsafe fn mi_check_is_double_free(page: *const mi_page_t, block: *const mi_block_t) -> bool {
    if MI_ENCODE_FREELIST == 0 || (crate::internal::MI_SECURE < 4 && MI_DEBUG == 0) {
        return false;
    }

    // SAFETY: the caller supplies a live page and a matching readable aligned
    // block location, possibly already free, owns or has claimed the page,
    // keeps its keys and local lists stable, and permits only protocol
    // publication of initialized remote links during this coherent traversal.
    unsafe {
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

/// Return the usable size of one validated block, excluding padding or its
/// trailing guard page as applicable.
///
/// # Safety
/// `page` must identify live initialized page metadata with stable block-size
/// and usable-size fields. When padding is enabled and `is_guarded` is false,
/// `block` must be the aligned start of a live block in `page`; its initialized
/// padding trailer and the page keys used by its canary must remain readable
/// and stable, with no concurrent free, realloc, or user write to that trailer.
/// When `is_guarded` is true, it must truthfully classify the block and the
/// page's block size must cover its trailing OS guard page; allocator OS-page
/// configuration must be initialized and stable. The page registration,
/// metadata, and backing must not be concurrently torn down or released.
pub(crate) unsafe fn mi_page_usable_size_of_typed(
    page: *const mi_page_t,
    block: *const mi_block_t,
    is_guarded: bool,
) -> crate::core_types::UsableSize {
    if MI_PADDING == 0 {
        let _ = block;
        let _ = is_guarded;
        // SAFETY: the caller keeps the live page's initialized usable block
        // size readable and stable for this query.
        return unsafe { mi_page_usable_block_size(page) };
    }

    if is_guarded {
        // SAFETY: the caller keeps this guarded page's initialized block size
        // and the process OS-page configuration stable; the block covers the
        // trailing guard page removed by the checked subtraction below.
        let (bsize, os_page_size) =
            unsafe { (mi_page_block_size(page), _mi_os_page_size_typed().get()) };
        crate::core_types::UsableSize::new(bsize)
            .checked_sub(os_page_size)
            .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
    } else {
        let mut bsize = 0usize;
        let mut delta = 0usize;
        // SAFETY: `block` is the matching live block start in `page`; its
        // initialized padding trailer and page canary keys remain readable and
        // stable while the helper balances its tracking visibility changes.
        let ok = unsafe { mi_page_decode_padding(page, block, &mut delta, &mut bsize) };
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

/// # Safety
/// `page` and `block` must refer to a valid block owned by that page.
pub unsafe fn mi_page_usable_size_of(
    page: *const mi_page_t,
    block: *const mi_block_t,
    is_guarded: bool,
) -> usize {
    // SAFETY: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
    unsafe { mi_page_usable_size_of_typed(page, block, is_guarded).get() }
}

/// # Safety
/// `page` and `block` must refer to a valid block being freed.
pub unsafe fn mi_stat_free(page: *const mi_page_t, block: *const mi_block_t) {
    // SAFETY: free stat updates mutate current thread-heap counters for caller-validated blocks.
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
    // SAFETY: local frees mutate the owning page free list after validation and optional padding checks.
    unsafe {
        let page = page_wrapper.page().as_ptr();
        let block = block_wrapper.block().as_ptr();
        let was_guarded = block_wrapper.is_guarded();
        let check_full = mode.check_full();
        // SAFETY: the typed witnesses identify a live locally owned page and a
        // matching readable aligned block location, possibly already free;
        // page keys and local lists are stable, remote publication follows the
        // allocator protocol, and registered diagnostics cannot unwind.
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

        // SAFETY: `page_wrapper` proves local ownership and `block_wrapper`
        // carries the freed block witness for the same page.
        let used = page_wrapper.push_free_block(block_wrapper.into_free_block());
        if used.get() == 0 {
            if crate::allocator::page::mi_page_retire_expire(page).is_zero() {
                let active = crate::core_types::assume_active_page(
                    crate::core_types::PagePtr::new_unchecked(page),
                );
                let Ok(all_free) = active.try_mark_all_free() else {
                    crate::internal::mi_assert_internal!(false);
                    return;
                };
                _mi_page_retire_internal(all_free);
            }
        } else if check_full && crate::allocator::page::mi_page_is_in_full(page) {
            let _ = _mi_page_unfull_internal(crate::core_types::assume_full_page(
                crate::core_types::PagePtr::new_unchecked(page),
            ));
        }
    }
}

pub(crate) unsafe fn mi_free_block_mt(
    page_wrapper: crate::core_types::ForeignPage,
    block_wrapper: crate::core_types::BlockForFree,
) {
    // SAFETY: remote frees publish blocks through the typed atomic thread-free word.
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

        let atomic_tf = page_ptr.thread_free_cell();

        let mut expected = atomic_tf.load_relaxed();
        let (previous, published) = loop {
            mi_block_set_next_typed(page_ptr, bp, expected.head());
            match atomic_tf.try_push(expected, bp) {
                crate::core_types::RemotePushResult::Published { previous, head } => {
                    break (previous, head);
                }
                crate::core_types::RemotePushResult::Retry { observed } => {
                    expected = observed;
                }
            }
        };

        if !previous.is_owned() {
            crate::internal::mi_assert_internal!(mi_page_is_abandoned(page));
            collect_after_remote_free(
                crate::core_types::assume_owned_abandoned_page(
                    crate::core_types::PagePtr::new_unchecked(page),
                ),
                published,
            )
            .discard();
        } else {
            published.leave_published();
        }
    }
}

unsafe fn mi_block_unguard(
    page: crate::core_types::PagePtr,
    block: crate::core_types::GuardedBlock,
    p: crate::core_types::UserPtr,
) {
    // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
    block: &crate::core_types::BlockForFree,
    p: crate::core_types::UserPtr,
) -> bool {
    // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
// The fast-malloc assembly reaches this through the generic free shims below.
#[cfg_attr(not(mi_c_malloc_fast), allow(dead_code))]
pub(crate) unsafe fn mi_validate_block_from_ptr(
    page: crate::core_types::PagePtr,
    user: crate::core_types::UserPtr,
) -> crate::core_types::PointerLookup {
    // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let location = mi_pointer_location(page, user, MI_SECURE > 0, true);

        crate::core_types::PointerLookup::valid(user, page, location)
    }
}

unsafe fn mi_lookup_pointer_page(
    user: crate::core_types::UserPtr,
) -> Option<crate::core_types::PagePtr> {
    // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
    unsafe {
        let p = user.as_ptr();
        if MI_DEBUG != 0 || cfg!(feature = "secure") {
            let registered = _mi_safe_ptr_page_registered(p);
            match registered {
                Some(registered)
                    if registered
                        .page()
                        .area()
                        .is_some_and(|area| area.contains(user.as_ptr())) =>
                {
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
    // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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

// SAFETY: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
#[cold]
#[inline(never)]
// `raw::macros` names this function as an assembly operand in fast-malloc builds.
#[cfg_attr(not(mi_c_malloc_fast), allow(dead_code))]
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
        let parts = validated.into_parts();
        let page_wrapper = crate::core_types::assume_local_page(parts.page);

        mi_free_block_local(
            page_wrapper,
            parts.block,
            crate::core_types::LocalFreeMode::TrackAndCheckFull,
        );
    }
}

// SAFETY: free routing dispatches validated blocks to local, remote, or abandoned owners without allocation.
#[cold]
#[inline(never)]
// `raw::macros` names this function as an assembly operand in fast-malloc builds.
#[cfg_attr(not(mi_c_malloc_fast), allow(dead_code))]
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
        let parts = validated.into_parts();
        let page_wrapper = crate::core_types::assume_foreign_page(parts.page);

        mi_free_block_mt(page_wrapper, parts.block);
    }
}

/// # Safety
///
/// `p` may be null. In configurations without registered pointer validation, a
/// nonnull `p` must point into a live allocation returned by this allocator. If
/// lookup resolves `p` to a page, that page's registration, backing, initialized
/// block geometry, and guard metadata must remain live and stable while it is
/// classified; `p` must not be concurrently freed or reallocated. Page-map
/// teardown or backing release must not race this operation;
/// protocol atomic remote-free publication remains permitted. `msg` must point
/// to a live NUL-terminated UTF-8 diagnostic string whenever an error path can use it.
/// Registered diagnostic callbacks and their opaque arguments must remain valid
/// for synchronous invocation, must not incompatibly reenter pointer validation,
/// and must not unwind.
pub(crate) unsafe fn mi_validate_pointer(
    p: *mut c_void,
    msg: *const c_char,
) -> crate::core_types::PointerLookup {
    use crate::core_types::{PointerLookup, UserPtr};
    if p.is_null() {
        return PointerLookup::Null;
    }
    // SAFETY: the null case returned above; `UserPtr` adds no dereference or
    // allocation-liveness claim beyond recording this nonnull address.
    let user_ptr = unsafe { UserPtr::new_unchecked(p) };

    if (MI_DEBUG != 0 || cfg!(feature = "secure"))
        && ((p as usize) & (MI_INTPTR_SIZE - 1)) != 0
        && !mi_option_is_enabled(mi_option_t::mi_option_guarded_precise)
    {
        // SAFETY: this diagnostic branch is the first access to `msg`; the
        // caller keeps its NUL-terminated bytes live, and the synchronous error
        // callback satisfies the validity, reentry, and non-unwind contract.
        let msg_str = unsafe { core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes()) };
        _mi_error_message_fmt(
            Errno::EINVAL,
            format_args!("{msg_str}: invalid (unaligned) pointer: {p:p}\n"),
        );
        return PointerLookup::Invalid;
    }

    // SAFETY: the allocator page map is initialized and stable against
    // teardown. In unchecked configurations the caller supplies a live
    // allocator pointer; registered configurations validate arbitrary input.
    let registered_page = unsafe { mi_lookup_pointer_page(user_ptr) };

    let Some(page_ptr) = registered_page else {
        if MI_DEBUG != 0 || cfg!(feature = "secure") {
            // SAFETY: this diagnostic branch is the first access to `msg`; the
            // caller keeps its NUL-terminated bytes live, and the synchronous
            // error callback satisfies the function contract.
            let msg_str = unsafe { core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes()) };
            _mi_error_message_fmt(
                Errno::EINVAL,
                format_args!("{msg_str}: invalid pointer: {p:p}\n"),
            );
        }
        return PointerLookup::Invalid;
    };

    // SAFETY: successful lookup couples `user_ptr` to this live registered
    // page. Its initialized block geometry and guard metadata remain readable
    // and stable while the exact block and interior offset are classified.
    let location = unsafe { mi_pointer_location(page_ptr, user_ptr, false, false) };

    PointerLookup::valid(user_ptr, page_ptr, location)
}

#[inline(always)]
/// # Safety
/// A valid `lookup` must still identify the same live registered allocation, page backing, block,
/// and guard metadata produced by pointer validation. They must remain stable against concurrent
/// free, realloc, page-map teardown, or backing release through the returned witness's use. For a
/// guarded block, its protection geometry must remain valid for synchronous unprotection.
unsafe fn mi_prepare_validated_free(
    lookup: crate::core_types::PointerLookup,
) -> Option<crate::core_types::ValidatedFree> {
    let validated = lookup.prepare_free()?;
    if validated.block().was_guarded() == crate::core_types::WasGuarded::Yes {
        // SAFETY: the caller keeps the validated page, block, user pointer, backing, and guard
        // geometry live and stable for this synchronous unprotection transition.
        unsafe { mi_block_check_unguard(validated.page(), validated.block(), validated.user()) };
    }
    Some(validated)
}

#[inline(always)]
/// # Safety
/// `validated` must identify one live allocator allocation whose page registration, backing, block
/// geometry, owner metadata, keys, free lists, and accounting remain valid through this terminal
/// free. It must not race another free/realloc, page-map teardown, backing release, or non-protocol
/// owner mutation. Local state must be exclusively mutable by the current thread; remote publication
/// may occur only through the allocator atomics. If nonnull, `usable` must be aligned and writable
/// for one `usize` when accessed; it may alias the live user allocation but not allocator metadata,
/// and is written before route classification and release. Registered callbacks and opaque arguments
/// must be valid for synchronous invocation, exclude incompatible reentry, and must not unwind.
unsafe fn mi_free_validated(validated: crate::core_types::ValidatedFree, usable: *mut usize) {
    let parts = validated.into_parts();
    let _ = parts.user;
    let page = parts.page.as_ptr();
    // SAFETY: the validated page geometry remains live for the size read, and the caller keeps a
    // nonnull `usable` aligned and writable at this access before any terminal free occurs.
    unsafe {
        crate::core_types::UsableOut::new(usable).write_if_present(mi_page_usable_block_size(page));
    }

    // SAFETY: the primitive returns the executing thread id, and the validated page's owner word
    // remains live and protocol-stable while it is loaded and classified exactly once.
    let route = unsafe {
        let current_tid = crate::core_types::ThreadId::new_unchecked(_mi_prim_thread_id());
        parts.page.snapshot_free_route(current_tid)
    };

    match route {
        crate::core_types::FreeRouteSnapshot::LocalFast(local_page) => {
            // SAFETY: the route snapshot proves current-thread ownership; the caller supplies
            // exclusive local free-list/accounting mutation and stable allocation metadata.
            unsafe {
                mi_free_block_local(
                    local_page,
                    parts.block,
                    crate::core_types::LocalFreeMode::TrackOnly,
                )
            };
        }
        // SAFETY: `route` was sampled from `parts.page` in the same stable owner epoch, and the
        // caller supplies the matching local-exclusive or remote-atomic mutation obligations.
        route => unsafe { mi_free_ex_slow(parts, route) },
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
/// # Safety
/// When the fast-path configuration is enabled, nonnull `p` must be the block start of a live,
/// unguarded, unpadded allocator allocation. Its registered page, backing, initialized geometry,
/// owner word, keys, free lists, and accounting must remain valid through classification and free,
/// without concurrent free/realloc, teardown, backing release, or non-protocol owner mutation.
/// Current-thread local state must be exclusively mutable; foreign publication may occur only via
/// allocator atomics. Registered callbacks and opaque arguments must be synchronously valid, exclude
/// incompatible reentry, and must not unwind. A nonnull `usable` disables this path without access.
unsafe fn mi_free_fast_try(p: *mut c_void, usable: *mut usize) -> bool {
    if !mi_free_fast_path_enabled(usable) {
        return false;
    }

    // SAFETY: the caller supplies one live block-start allocation and stable page-map/backing/owner
    // epoch. The release configuration excludes guarded, padded, and diagnostic pointer adjustment;
    // local mutation is exclusive and foreign publication uses the page's atomic protocol.
    unsafe {
        let page = _mi_ptr_page(p);
        let Some(page_ptr) = crate::core_types::PagePtr::new(page) else {
            return false;
        };
        let current = crate::core_types::ThreadId::new_unchecked(_mi_prim_thread_id());
        let block = crate::core_types::BlockForFree::from_block_start_unchecked(
            crate::core_types::BlockPtr::new_unchecked(p.cast()),
        );

        match page_ptr.snapshot_free_route(current) {
            crate::core_types::FreeRouteSnapshot::LocalFast(local_page) => {
                mi_free_block_local(
                    local_page,
                    block,
                    crate::core_types::LocalFreeMode::TrackOnly,
                );
                true
            }
            crate::core_types::FreeRouteSnapshot::ForeignPlain(foreign_page) => {
                mi_free_block_mt(foreign_page, block);
                true
            }
            _ => false,
        }
    }
}

#[cold]
#[inline(never)]
/// # Safety
/// `parts` must identify one live validated allocation, and `route` must be the still-current owner
/// snapshot taken from that same page. Page registration, backing, geometry, keys, free lists, and
/// accounting must remain valid until the selected terminal mutation. Local routes require exclusive
/// current-thread mutation; foreign and abandoned routes permit only protocol atomic publication.
/// The operation must not race another free/realloc, teardown, backing release, or owner relink.
/// Registered callbacks and opaque arguments must be synchronously valid, exclude incompatible
/// reentry, and must not unwind.
pub(crate) unsafe fn mi_free_ex_slow(
    parts: crate::core_types::ValidatedFreeParts,
    route: crate::core_types::FreeRouteSnapshot,
) {
    let block = parts.block;
    // `validated` was produced by `mi_validate_pointer`, which already derived the
    // interior-corrected block and resolved guard state. Route the typed witness
    // directly to the block free functions instead of re-deriving from the raw
    // pointer via `mi_free_generic_local`/`mi_free_generic_mt` (which would also
    // double-call `mi_block_check_unguard` on guarded blocks).
    match route {
        crate::core_types::FreeRouteSnapshot::LocalFast(local_page) => {
            // SAFETY: this matching route proves current-thread ownership, and the caller supplies
            // exclusive local free-list/accounting mutation for the validated block.
            unsafe {
                mi_free_block_local(
                    local_page,
                    block,
                    crate::core_types::LocalFreeMode::TrackOnly,
                )
            };
        }
        crate::core_types::FreeRouteSnapshot::LocalSlow(local_page) => {
            // Local page with flags set (e.g. in the full queue): check for unfull.
            // SAFETY: this matching route proves current-thread ownership, and the caller supplies
            // exclusive local free-list, full-queue, and accounting mutation.
            unsafe {
                mi_free_block_local(
                    local_page,
                    block,
                    crate::core_types::LocalFreeMode::TrackAndCheckFull,
                )
            };
        }
        crate::core_types::FreeRouteSnapshot::ForeignPlain(foreign_page)
        | crate::core_types::FreeRouteSnapshot::ForeignSlow(foreign_page) => {
            // Remote (plain or with flags): push onto the owning thread's free list.
            // The flag distinction collapses here because validation already resolved
            // the block; `mi_free_block_mt` does not branch on page flags.
            // SAFETY: the matching foreign route and caller contract keep the page live while the
            // block is published through its atomic remote-free protocol.
            unsafe { mi_free_block_mt(foreign_page, block) };
        }
        crate::core_types::FreeRouteSnapshot::Abandoned(abandoned_page) => {
            // SAFETY: the matching abandoned route keeps the page/arena ownership epoch live while
            // the block is published and any protocol reclamation is attempted.
            unsafe { mi_free_block_mt(abandoned_page.remote_free_page(), block) };
        }
        crate::core_types::FreeRouteSnapshot::AbandonedMapped(abandoned_page) => {
            // SAFETY: the matching mapped-abandoned route keeps page-map/backing ownership live
            // while the block is published and any protocol reclamation is attempted.
            unsafe { mi_free_block_mt(abandoned_page.remote_free_page(), block) };
        }
    }
}

#[inline(never)]
/// # Safety
/// `p` may be null. Otherwise it must identify one live allocator allocation whose page-map entry,
/// backing, geometry, guard metadata, owner word, keys, free lists, and accounting remain valid until
/// terminal release, without concurrent free/realloc, teardown, backing release, or non-protocol
/// owner mutation. Local state must be exclusively mutable by the current thread; remote publication
/// may occur only through allocator atomics. `msg` must be live NUL-terminated diagnostic text when
/// validation can use it. If nonnull, `usable` must be aligned and writable for one `usize` when
/// accessed; it may alias live user storage but not allocator metadata, and is written before free.
/// Registered callbacks and opaque arguments must be synchronously valid, exclude incompatible
/// reentry, and must not unwind.
pub(crate) unsafe fn mi_route_free(p: *mut c_void, msg: *const c_char, usable: *mut usize) {
    if p.is_null() {
        return;
    }
    // SAFETY: the caller supplies the live allocation/page/owner epoch and optional usable-output
    // validity required by the coherent fast classification-and-free attempt.
    if unsafe { mi_free_fast_try(p, usable) } {
        return;
    }
    // SAFETY: the caller supplies the live pointer, page-map/backing stability, diagnostic text, and
    // callback validity required to classify `p` without racing free/realloc or teardown.
    let lookup = unsafe { mi_validate_pointer(p, msg) };
    // SAFETY: successful validation remains live and stable through guarded unprotection and the
    // returned terminal-free witness; invalid/null lookup variants are not dereferenced.
    let validated = unsafe { mi_prepare_validated_free(lookup) };
    if let Some(validated) = validated {
        // SAFETY: the caller supplies the allocation/owner epoch and usable-output validity through
        // the terminal local or remote mutation; output publication precedes route/free operations.
        unsafe { mi_free_validated(validated, usable) };
    }
}

/// Resolve a user pointer and return its remaining usable size.
///
/// # Safety
/// `p` may be null. In configurations without registered pointer validation, a
/// nonnull `p` must identify a live allocation returned by this allocator. If
/// lookup resolves `p` to a page, that page's registration, backing, initialized
/// geometry, padding, and guard metadata must remain live and stable, with no
/// concurrent free or realloc of `p`, page-map teardown, or backing release.
/// Protocol atomic remote-free publication remains permitted. `msg` must point
/// to a live NUL-terminated UTF-8 diagnostic string whenever validation can use
/// it. Registered diagnostic callbacks and their opaque arguments must remain
/// valid for synchronous invocation, must not incompatibly reenter pointer
/// validation, and must not unwind.
pub(crate) unsafe fn mi_route_usable_size_typed(
    p: *const c_void,
    msg: *const c_char,
) -> crate::core_types::UsableSize {
    if p.is_null() {
        return crate::core_types::UsableSize::new(0);
    }
    if MI_DEBUG == 0 && !cfg!(feature = "secure") && MI_GUARDED == 0 {
        let Some(user) = crate::core_types::UserPtr::new(p.cast_mut()) else {
            return crate::core_types::UsableSize::new(0);
        };
        // SAFETY: this unchecked release configuration requires `p` to name a
        // live allocator allocation; the page map and its registration remain
        // initialized and stable against teardown for lookup.
        let Some(page) = (unsafe { mi_lookup_pointer_page(user) }) else {
            return crate::core_types::UsableSize::new(0);
        };
        // SAFETY: lookup couples `p` to this live page; its allocation geometry,
        // padding, guard metadata, and backing remain stable for the size query.
        return unsafe { _mi_usable_size_typed(p, page.as_ptr()) };
    }
    // SAFETY: the caller provides the conditional live-allocation and
    // diagnostic-string obligations required by pointer validation; page-map
    // and allocation metadata remain stable throughout this query.
    let lookup = unsafe { mi_validate_pointer(p.cast_mut(), msg) };
    match lookup {
        crate::core_types::PointerLookup::Valid(valid) => {
            let (_, page, location) = valid.into_parts();
            let located = location.located_block();
            // SAFETY: successful validation couples the page to this located
            // block and guard classification; their usable-size and padding
            // metadata remain live, readable, and stable for this access.
            let size = unsafe {
                mi_page_usable_size_of_typed(
                    page.as_ptr(),
                    located.block().as_ptr(),
                    located.was_guarded() == crate::core_types::WasGuarded::Yes,
                )
            };
            size.checked_sub(located.offset().0)
                .unwrap_or_else(|_| crate::core_types::UsableSize::new(0))
        }
        _ => crate::core_types::UsableSize::new(0),
    }
}

pub(crate) unsafe fn mi_route_usable_size(p: *const c_void, msg: *const c_char) -> usize {
    // SAFETY: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
    unsafe { mi_route_usable_size_typed(p, msg).get() }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator and not yet freed.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
// SAFETY: usable-size queries derive block sizes from validated page/block or user-pointer witnesses.
pub unsafe extern "C" fn mi_free(p: *mut c_void) {
    unsafe {
        mi_route_free(p, c"mi_free".as_ptr(), ptr::null_mut());
    }
}

// SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[cold]
#[inline(never)]
pub(crate) unsafe extern "C" fn mi_free_try_collect_mt(
    page: *mut mi_page_t,
    mt_free: *mut mi_block_t,
) {
    // SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        crate::internal::mi_assert_internal!(!mt_free.is_null());
        collect_after_remote_free(
            crate::core_types::assume_owned_abandoned_page(
                crate::core_types::PagePtr::new_unchecked(page),
            ),
            crate::core_types::PublishedRemoteHead::from_raw_compat(
                crate::core_types::BlockPtr::new_unchecked(mt_free),
            ),
        )
        .discard();
    }
}

// SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
const _: unsafe extern "C" fn(*mut mi_page_t, *mut mi_block_t) = mi_free_try_collect_mt;

pub(crate) unsafe fn mi_page_usable_aligned_size_of_typed(
    page: *const mi_page_t,
    p: *const c_void,
) -> crate::core_types::UsableSize {
    // SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
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
    // SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
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

/// # Safety
///
/// `page` may be null, in which case this returns zero without accessing `p`.
/// Otherwise, `page` must be the live page containing the allocation identified
/// by `p`, which must have been returned by this allocator, not yet freed, and
/// remain valid for this operation. Its geometry, padding, and allocation
/// metadata must remain stable for the call, with no concurrent free or
/// reallocation of `p`.
pub(crate) unsafe fn _mi_usable_size_typed(
    p: *const c_void,
    page: *const mi_page_t,
) -> crate::core_types::UsableSize {
    // SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
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
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY: C ABI free/export shims preserve upstream symbols and delegate to validated free routing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_usable_size(p: *const c_void) -> usize {
    unsafe { mi_route_usable_size(p, c"mi_usable_size".as_ptr()) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator and not yet freed.
// Retained as an exported C ABI free-size symbol; release cdylib builds may
// reference it only from external dynamic callers or assembly aliases.
#[allow(dead_code)]
// SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
// SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
// SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
// SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_ufree(p: *mut c_void, usable: *mut usize) {
    unsafe {
        mi_route_free(p, c"mi_ufree".as_ptr(), usable);
    }
}

/// # Safety
/// `p` must be null or a small allocation pointer previously returned by this allocator.
// SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
        crate::internal::mi_assert!(
            crate::core_types::PagePtr::new_unchecked(page).block_size_raw() <= MI_SMALL_SIZE_MAX
        );
        mi_route_free(p, c"mi_free_small".as_ptr(), ptr::null_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::init::_mi_thread_id;
    use crate::core_types::{
        AvailableFreeList, BlockPtr, FreePathKind, FreeRouteSnapshot, LocalFreeList, PageFlags,
        PageOwnerId, PagePtr, PageThreadId, PointerLocation, PointerLookup, ThreadFreeOwner,
        ThreadFreeWord, ThreadId, UserPtr,
    };
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
            // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
            unsafe {
                let page = fixture.page.as_mut_ptr();
                let page_ptr = PagePtr::new_unchecked(page);
                ptr::addr_of_mut!((*page).xthread_id)
                    .write(PageThreadId::from_parts(owner, flags).raw());
                page_ptr.set_free(AvailableFreeList::from_raw(ptr::null_mut()));
                ptr::addr_of_mut!((*page).used).write(2);
                ptr::addr_of_mut!((*page).capacity).write(2);
                ptr::addr_of_mut!((*page).reserved).write(2);
                ptr::addr_of_mut!((*page).retire_expire).write(1);
                ptr::addr_of_mut!((*page).free_is_zero).write(false);
                page_ptr.set_local_free(LocalFreeList::empty());
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
            // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
            unsafe {
                ptr::addr_of_mut!((*self.page.as_mut_ptr()).page_start)
                    .write(ptr::addr_of_mut!(self.blocks[0]).cast::<u8>());
                PagePtr::new_unchecked(self.page.as_mut_ptr())
            }
        }

        fn user(&mut self, index: usize) -> UserPtr {
            // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
            let option = crate::allocator::options::mi_option_t::mi_option_show_errors;
            let previous = crate::allocator::options::mi_option_get(option);
            crate::allocator::options::mi_option_set(option, 0);
            Self { previous }
        }
    }

    impl Drop for ShowErrorsGuard {
        fn drop(&mut self) {
            crate::allocator::options::mi_option_set(
                crate::allocator::options::mi_option_t::mi_option_show_errors,
                self.previous,
            );
        }
    }

    #[test]
    fn free_validation_covers_null_invalid_and_bad_alignment() {
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
                PointerLookup::Invalid
            ));

            assert!(matches!(
                mi_validate_pointer(ptr::dangling_mut::<u8>().cast(), c"free-test".as_ptr()),
                PointerLookup::Invalid
            ));
        }
    }

    #[test]
    fn free_validation_covers_interior_pointer_lookup() {
        let mut fixture =
            FreeFixture::new(current_thread_owner(), PageFlags::HAS_INTERIOR_POINTERS);
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let page = fixture.page();
            let user = UserPtr::new_unchecked(fixture.user(0).as_ptr().cast::<u8>().add(1).cast());
            let lookup = mi_validate_block_from_ptr(page, user);

            match lookup {
                PointerLookup::Valid(valid) => match valid.into_parts().2 {
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
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            crate::core_types::BlockTag::write_guarded_to(fixture.block(0));
            let page = fixture.page();
            let user = UserPtr::new_unchecked(
                fixture
                    .user(0)
                    .as_ptr()
                    .cast::<u8>()
                    .add(core::mem::size_of::<mi_block_t>())
                    .cast(),
            );

            let lookup = mi_validate_block_from_ptr(page, user);
            match lookup {
                PointerLookup::Valid(valid) => match valid.into_parts().2 {
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

        let mut local = FreeFixture::new(current_thread_owner(), PageFlags::new(0).unwrap());
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
                validated.page().snapshot_free_route(current),
                FreeRouteSnapshot::LocalFast(_)
            ));
        }

        let remote_raw = if current.get() == 0x1000 {
            0x2000
        } else {
            0x1000
        };
        let remote_owner = PageOwnerId::Thread(ThreadId::new(remote_raw).expect("valid remote id"));
        let mut remote = FreeFixture::new(remote_owner, PageFlags::new(0).unwrap());
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
                validated.page().snapshot_free_route(current),
                FreeRouteSnapshot::ForeignPlain(_)
            ));
        }

        let mut abandoned = FreeFixture::new(PageOwnerId::Abandoned, PageFlags::new(0).unwrap());
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
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
                validated.page().snapshot_free_route(current),
                FreeRouteSnapshot::Abandoned(_)
            ));
        }
    }

    #[test]
    fn free_checks_cover_double_free_detection() {
        let mut fixture = FreeFixture::new(current_thread_owner(), PageFlags::new(0).unwrap());
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        // SAFETY: pointer validation and double-free checks only inspect allocator-owned page/block witnesses.
        unsafe {
            let page = fixture.page().as_ptr();
            let block = fixture.block(0).as_ptr();
            PagePtr::new_unchecked(page).set_free(AvailableFreeList::from_head(fixture.block(0)));
            assert!(mi_check_is_double_freex(page, block));
        }
    }
}
