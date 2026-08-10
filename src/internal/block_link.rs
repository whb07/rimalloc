//! Encoded block-link and free-list pointer helpers.
//!
//! This module owns the page/null/key context used to encode, decode, read, and
//! write free-list links stored in `mi_block_t::next`.

use super::*;

/// # Safety
/// `p` must be a pointer in the allocator's page-map domain, and `q` must be comparable against
/// that page's usable address range.
#[inline]
pub unsafe fn mi_is_in_same_page(p: *const c_void, q: *const c_void) -> bool {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    let page = unsafe { _mi_ptr_page(p) };
    unsafe { mi_page_contains_address(page, q) }
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
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_keys(page: *const mi_page_t) -> [usize; 2] {
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    {
        // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
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
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
pub(crate) unsafe fn mi_page_set_keys(
    page: crate::core_types::PagePtr,
    keys: crate::core_types::PageKeys,
) {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe { page.set_keys(keys) }
}

/// # Safety
/// `block` must point to a valid block link word encoded with `keys`; `null` must be the sentinel
/// used for that list.
#[inline]
pub unsafe fn mi_block_nextx(
    null: *const c_void,
    block: *const mi_block_t,
    keys: &[usize; 2],
) -> *mut mi_block_t {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe {
        crate::allocator::tracking::mi_track_mem_defined(
            block.cast_mut().cast(),
            size_of::<mi_block_t>(),
        )
    };
    let link = unsafe { crate::core_types::EncodedPtr::read_word_from_raw(block) };
    let next = if MI_ENCODE_FREELIST != 0 {
        unsafe { link.decode_block_raw(null, crate::core_types::PageKeys::from_raw(*keys)) }
    } else {
        link.as_unencoded_raw()
    };
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe {
        crate::allocator::tracking::mi_track_mem_noaccess(
            block.cast_mut().cast(),
            size_of::<mi_block_t>(),
        )
    };
    next
}

/// # Safety
/// `block` must point to a writable block link word encoded with `keys`; `null` must be the
/// sentinel used for that list.
#[inline]
pub(crate) unsafe fn mi_block_set_nextx_typed(
    page: crate::core_types::PagePtr,
    block: crate::core_types::BlockPtr,
    next: Option<crate::core_types::BlockPtr>,
    keys: crate::core_types::PageKeys,
) {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe {
        crate::allocator::tracking::mi_track_mem_undefined(
            block.as_ptr().cast(),
            size_of::<mi_block_t>(),
        )
    };
    let link = if MI_ENCODE_FREELIST != 0 {
        unsafe { crate::core_types::EncodedPtr::encode_block(next, page, keys) }
    } else {
        crate::core_types::EncodedPtr::from_unencoded_raw(
            next.map(crate::core_types::BlockPtr::as_ptr)
                .unwrap_or(ptr::null_mut())
                .cast_const(),
        )
    };
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe { block.write_link(crate::core_types::BlockLink::FreeList(link)) };
    unsafe {
        crate::allocator::tracking::mi_track_mem_noaccess(
            block.as_ptr().cast(),
            size_of::<mi_block_t>(),
        )
    };
}

/// # Safety
/// `block` must point to a writable block link word encoded with `keys`; `null` must be the
/// sentinel used for that list.
#[inline]
pub unsafe fn mi_block_set_nextx(
    null: *const c_void,
    block: *mut mi_block_t,
    next: *const mi_block_t,
    keys: &[usize; 2],
) {
    if !null.is_null() {
        // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
        let page = unsafe { crate::core_types::PagePtr::new_unchecked(null.cast_mut().cast()) };
        let block = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
        let next = crate::core_types::BlockPtr::new(next.cast_mut());
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

    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe {
        crate::allocator::tracking::mi_track_mem_undefined(block.cast(), size_of::<mi_block_t>())
    };
    let link = if MI_ENCODE_FREELIST != 0 {
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
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    let block_ptr = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
    unsafe { block_ptr.write_link(crate::core_types::BlockLink::FreeList(link)) };
    unsafe {
        crate::allocator::tracking::mi_track_mem_noaccess(block.cast(), size_of::<mi_block_t>())
    };
}

/// # Safety
/// `page` must be valid, and `block` must point to a block list node belonging to that page.
#[inline]
pub unsafe fn mi_block_next(page: *const mi_page_t, block: *const mi_block_t) -> *mut mi_block_t {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    unsafe {
        let keys = mi_page_keys(page);
        if MI_ENCODE_FREELIST == 0 {
            return mi_block_nextx(page.cast(), block, &keys);
        }
        let mut next = mi_block_nextx(page.cast(), block, &keys);
        if !next.is_null() && !mi_is_in_same_page(block.cast(), next.cast()) {
            _mi_error_message_fmt(
                Errno::EFAULT,
                format_args!(
                    "corrupted free list entry of size {}b at {block:p}: value 0x{:x}\n",
                    mi_page_block_size(page),
                    next as usize
                ),
            );
            next = ptr::null_mut();
        }
        next
    }
}

/// # Safety
/// `page` must be valid, and `block` must point to a writable block list node belonging to that
/// page.
#[inline]
pub(crate) unsafe fn mi_block_set_next_typed(
    page: crate::core_types::PagePtr,
    block: crate::core_types::BlockPtr,
    next: Option<crate::core_types::BlockPtr>,
) {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    let keys = unsafe { page.keys() };
    unsafe { mi_block_set_nextx_typed(page, block, next, keys) };
}

/// # Safety
/// `page` must be valid, and `block` must point to a writable block list node belonging to that
/// page.
#[inline]
pub unsafe fn mi_block_set_next(
    page: *const mi_page_t,
    block: *mut mi_block_t,
    next: *const mi_block_t,
) {
    // SAFETY: encoded block-link helpers preserve free-list provenance, tracking, and same-page checks.
    let page = unsafe { crate::core_types::PagePtr::new_unchecked(page.cast_mut()) };
    let block = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
    let next = crate::core_types::BlockPtr::new(next.cast_mut());
    unsafe { mi_block_set_next_typed(page, block, next) };
}
