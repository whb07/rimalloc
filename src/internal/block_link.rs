//! Encoded block-link and free-list pointer helpers.
//!
//! This module owns the page/null/key context used to encode, decode, read, and
//! write free-list links stored in `mi_block_t::next`.

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;

use crate::allocator::options::_mi_error_message_fmt;
use crate::internal::bits::{mi_rotl, mi_rotr};
use crate::internal::page_metadata::mi_page_block_size;
use crate::types::{Errno, MI_ENCODE_FREELIST, mi_block_t, mi_page_t};

use super::{_mi_ptr_page, MI_BIG_ENDIAN};

/// # Safety
/// `p` must resolve to live initialized allocator page metadata with stable,
/// valid block-area geometry. `q` is compared by exposed address only and need
/// not be dereferenceable.
#[inline]
pub unsafe fn mi_is_in_same_page(p: *const c_void, q: *const c_void) -> bool {
    // SAFETY: the caller guarantees `_mi_ptr_page(p)` is non-null live page
    // metadata with stable initialized geometry; `area` only snapshots that
    // geometry before the exposed-address comparison with `q`.
    let page = unsafe { _mi_ptr_page(p) };
    // SAFETY: the resolved page is caller-proven non-null live metadata whose
    // initialized PageArea fields remain stable for this geometry snapshot.
    unsafe {
        crate::core_types::PagePtr::new_unchecked(page)
            .area()
            .is_some_and(|area| area.contains(q))
    }
}

#[inline]
pub(crate) fn mi_ptr_decode(
    null: *const c_void,
    x: crate::core_types::EncodedWord,
    keys: &[usize; 2],
) -> *mut c_void {
    let decoded_addr = mi_rotr(x.get().wrapping_sub(keys[0]), keys[0]) ^ keys[1];

    #[cfg(miri)]
    {
        if decoded_addr == crate::core_types::ptr_exposed_addr(null) {
            ptr::null_mut()
        } else {
            crate::internal::provenance::resolve_encoded(x.get(), null, keys).unwrap_or_else(|| {
                crate::internal::provenance::missing_encoded(x.get(), null, keys)
            })
        }
    }

    #[cfg(not(miri))]
    {
        let p = decoded_addr as *mut c_void;
        if ptr::eq(p.cast_const(), null) {
            ptr::null_mut()
        } else {
            p
        }
    }
}

#[inline]
pub(crate) fn mi_ptr_encode(
    null: *const c_void,
    p: *const c_void,
    keys: &[usize; 2],
) -> crate::core_types::EncodedWord {
    let x = if p.is_null() {
        crate::core_types::ptr_exposed_addr(null)
    } else {
        crate::core_types::ptr_exposed_addr(p)
    };
    let word =
        crate::core_types::EncodedWord::new(mi_rotl(x ^ keys[1], keys[0]).wrapping_add(keys[0]));
    crate::internal::provenance::remember_encoded(word.get(), null, keys, p);
    word
}

#[inline]
pub(crate) fn mi_ptr_encode_canary(
    null: *const c_void,
    p: *const c_void,
    keys: &[usize; 2],
) -> u32 {
    let x = mi_ptr_encode(null, p, keys).get() as u32;
    if MI_BIG_ENDIAN != 0 {
        x & 0x00FF_FFFF
    } else {
        x & 0xFFFF_FF00
    }
}

/// # Safety
/// When keys are enabled, `page` must be aligned live page metadata whose
/// initialized `keys` field remains readable and stable for the snapshot.
#[inline]
pub unsafe fn mi_page_keys(page: *const mi_page_t) -> [usize; 2] {
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    {
        // SAFETY: the caller keeps this exact live page's initialized key array
        // readable and stable for the two-word copy.
        unsafe { (*page).keys }
    }

    #[cfg(not(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    )))]
    {
        let _ = page;
        [0, 0]
    }
}

/// # Safety
/// When keys are enabled, `page` must refer to aligned live page backing with
/// a writable `keys` field. The caller must exclusively initialize or replace
/// that field before an encoded-list operation can observe it.
#[inline]
pub(crate) unsafe fn mi_page_set_keys(
    page: crate::core_types::PagePtr,
    keys: crate::core_types::PageKeys,
) {
    // SAFETY: the caller supplies exclusive access to this page's in-bounds
    // keys field and `keys` is the complete initialized replacement value.
    unsafe { page.set_keys(keys) }
}

/// # Safety
/// `block` must be aligned and point to a live, initialized block link word
/// that remains stable for the read. `null` and `keys` must be the exact
/// sentinel context and stable keys used to encode that list. The caller must
/// permit the balanced tracking defined/no-access transition and prevent
/// concurrent mutation or destruction of the link and its page metadata.
#[inline]
pub unsafe fn mi_block_nextx(
    null: *const c_void,
    block: *const mi_block_t,
    keys: &[usize; 2],
) -> *mut mi_block_t {
    // SAFETY: the caller keeps the complete block-link word live for tracking;
    // marking that exact range defined makes its initialized value readable.
    unsafe {
        crate::allocator::tracking::mi_track_mem_defined(
            block.cast_mut().cast(),
            size_of::<mi_block_t>(),
        )
    };
    // SAFETY: the tracking transition above exposes this aligned initialized
    // block link, which remains stable for the raw word snapshot.
    let link = unsafe { crate::core_types::EncodedPtr::read_word_from_raw(block) };
    let next = if MI_ENCODE_FREELIST != 0 {
        // SAFETY: the caller supplied the exact stable sentinel and keys used
        // to encode this link; decoding only reconstructs the stored address.
        unsafe { link.decode_block_raw(null, crate::core_types::PageKeys::from_raw(*keys)) }
    } else {
        link.as_unencoded_raw()
    };
    // SAFETY: the link snapshot is complete, and the caller's tracking
    // protocol requires restoring this exact block-link range to no-access.
    unsafe {
        crate::allocator::tracking::mi_track_mem_noaccess(
            block.cast_mut().cast(),
            size_of::<mi_block_t>(),
        )
    };
    next
}

/// # Safety
/// `page` must be the live owning page of `block` and every non-null `next`.
/// `block` must be aligned with an exclusively writable link word, `keys` must
/// be that page's initialized stable keys, and the caller must permit the
/// balanced tracking undefined/no-access transition. No participating page or
/// block may be destroyed or relinked concurrently.
#[inline]
pub(crate) unsafe fn mi_block_set_nextx_typed(
    page: crate::core_types::PagePtr,
    block: crate::core_types::BlockPtr,
    next: Option<crate::core_types::BlockPtr>,
    keys: crate::core_types::PageKeys,
) {
    // SAFETY: the caller keeps this block-link range live and exclusively
    // writable; marking it undefined permits initialization of the new word.
    unsafe {
        crate::allocator::tracking::mi_track_mem_undefined(
            block.as_ptr().cast(),
            size_of::<mi_block_t>(),
        )
    };
    let link = if MI_ENCODE_FREELIST != 0 {
        // SAFETY: `page`, `keys`, and `next` belong to the same stable encoding
        // domain established by the caller.
        unsafe { crate::core_types::EncodedPtr::encode_block(next, page, keys) }
    } else {
        crate::core_types::EncodedPtr::from_unencoded_raw(
            next.map(crate::core_types::BlockPtr::as_ptr)
                .unwrap_or(ptr::null_mut())
                .cast_const(),
        )
    };
    // SAFETY: the caller exclusively owns the block's in-bounds link word and
    // `link` is its fully initialized representation for the owning page.
    unsafe { block.write_link(crate::core_types::BlockLink::FreeList(link)) };
    // SAFETY: the initialized write is complete; restore this exact live link
    // range to the caller-required no-access tracking state.
    unsafe {
        crate::allocator::tracking::mi_track_mem_noaccess(
            block.as_ptr().cast(),
            size_of::<mi_block_t>(),
        )
    };
}

/// # Safety
/// `block` must be non-null, aligned, live, and exclusively writable for one
/// link word. `null` and `keys` must be the stable encoding context for its
/// list; when `null` is non-null it must be the owning live page metadata.
/// `next` must be null or an aligned block in that same list domain. The caller
/// must permit the balanced tracking transition and exclude concurrent link,
/// key, or backing mutation/destruction.
#[inline]
pub unsafe fn mi_block_set_nextx(
    null: *const c_void,
    block: *mut mi_block_t,
    next: *const mi_block_t,
    keys: &[usize; 2],
) {
    if !null.is_null() {
        // SAFETY: non-null `null` is caller-proven live owning page metadata;
        // `block` is caller-proven non-null and aligned, while `next` is
        // intentionally checked for null before becoming a block witness.
        let page = unsafe { crate::core_types::PagePtr::new_unchecked(null.cast_mut().cast()) };
        // SAFETY: the caller guarantees a non-null aligned writable block link.
        let block = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
        let next = crate::core_types::BlockPtr::new(next.cast_mut());
        // SAFETY: the raw arguments establish one live owning page/list domain,
        // exact stable keys, exclusive link access, and balanced tracking.
        unsafe {
            mi_block_set_nextx_typed(
                page,
                block,
                next,
                crate::core_types::PageKeys::from_raw(*keys),
            );
        }
        return;
    }

    // SAFETY: in the null-context form the caller still provides a live,
    // exclusively writable block-link range for the tracking transition.
    unsafe {
        crate::allocator::tracking::mi_track_mem_undefined(block.cast(), size_of::<mi_block_t>())
    };
    let link = if MI_ENCODE_FREELIST != 0 {
        // SAFETY: the caller supplied the exact null sentinel and stable keys
        // for this raw list domain, and `next` is null or a member address.
        unsafe {
            crate::core_types::EncodedPtr::encode_block_raw(
                next,
                null,
                crate::core_types::PageKeys::from_raw(*keys),
            )
        }
    } else {
        crate::core_types::EncodedPtr::from_unencoded_raw(next)
    };
    // SAFETY: the caller guarantees `block` is non-null and aligned for its
    // exclusively writable link word.
    let block_ptr = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
    // SAFETY: `link` is fully initialized for the caller-proven raw list domain
    // and the caller exclusively owns this destination word.
    unsafe { block_ptr.write_link(crate::core_types::BlockLink::FreeList(link)) };
    // SAFETY: the write is complete; restore this exact live link range to the
    // caller-required no-access tracking state.
    unsafe {
        crate::allocator::tracking::mi_track_mem_noaccess(block.cast(), size_of::<mi_block_t>())
    };
}

/// # Safety
/// `page` must be aligned live initialized page metadata with stable geometry,
/// block size, and encoding keys. `block` must be an aligned initialized link
/// node belonging to that page and remain readable under the balanced tracking
/// protocol. The caller must prevent concurrent relinking or destruction.
#[inline]
pub unsafe fn mi_block_next(page: *const mi_page_t, block: *const mi_block_t) -> *mut mi_block_t {
    // SAFETY: the caller keeps this exact page's initialized encoding keys
    // readable and stable throughout link decoding.
    let keys = unsafe { mi_page_keys(page) };
    // SAFETY: `block` is the caller-proven aligned readable node in `page`'s
    // stable list, and `page` plus `keys` are its exact encoding context.
    let mut next = unsafe { mi_block_nextx(page.cast(), block, &keys) };
    if MI_ENCODE_FREELIST == 0 {
        return next;
    }
    // SAFETY: the caller supplies this block's live page with stable geometry;
    // the decoded `next` is compared by address and is not dereferenced.
    if !next.is_null()
        && !unsafe {
            crate::core_types::PagePtr::new_unchecked(page.cast_mut())
                .area()
                .is_some_and(|area| area.contains(next))
        }
    {
        // SAFETY: the caller keeps `page` live with initialized stable block
        // geometry while the diagnostic snapshots its block size.
        let block_size = unsafe { mi_page_block_size(page) };
        _mi_error_message_fmt(
            Errno::EFAULT,
            format_args!(
                "corrupted free list entry of size {block_size}b at {block:p}: value 0x{:x}\n",
                next as usize
            ),
        );
        next = ptr::null_mut();
    }
    next
}

/// # Safety
/// `page` must be the live initialized owner of `block` and every non-null
/// `next`, with stable initialized encoding keys. `block` must be aligned and
/// exclusively writable for its link word. The caller must permit the balanced
/// tracking transition and exclude concurrent relinking or destruction.
#[inline]
pub(crate) unsafe fn mi_block_set_next_typed(
    page: crate::core_types::PagePtr,
    block: crate::core_types::BlockPtr,
    next: Option<crate::core_types::BlockPtr>,
) {
    // SAFETY: the caller keeps the owning page's initialized key field stable
    // and readable for this list-link update.
    let keys = unsafe { page.keys() };
    // SAFETY: the caller establishes matching page/block/next membership,
    // exclusive link access, stable keys, and the tracking transition.
    unsafe { mi_block_set_nextx_typed(page, block, next, keys) };
}

/// # Safety
/// `page` must be non-null, aligned, live initialized page metadata and own
/// `block` plus every non-null `next`. `block` must be non-null, aligned, and
/// exclusively writable for its link word; page keys must remain stable. The
/// caller must permit the balanced tracking transition and exclude concurrent
/// relinking or destruction.
#[inline]
pub unsafe fn mi_block_set_next(
    page: *const mi_page_t,
    block: *mut mi_block_t,
    next: *const mi_block_t,
) {
    // SAFETY: the caller proves non-null aligned owning page metadata and an
    // aligned non-null writable block; `next` is checked before wrapping.
    let page = unsafe { crate::core_types::PagePtr::new_unchecked(page.cast_mut()) };
    // SAFETY: the caller proves `block` is a non-null aligned block-link word.
    let block = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
    let next = crate::core_types::BlockPtr::new(next.cast_mut());
    // SAFETY: the raw contract establishes matching live list membership,
    // exclusive link access, stable keys, and balanced tracking.
    unsafe { mi_block_set_next_typed(page, block, next) };
}
