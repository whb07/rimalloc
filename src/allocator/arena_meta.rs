//! Mechanical translation of `src/arena-meta.c`.

// Retains upstream arena metadata constants for C layout/parity even when the
// current Rust target does not reference every translated macro.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::mem::offset_of;
use core::num::NonZeroU32;
use core::ptr;

use crate::allocator::arena::{MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE, MI_META_PAGE_SIZE};
use crate::bitmap::{
    MI_BCHUNK_BITS, MI_BCHUNK_SIZE, MI_BFIELD_BITS_SHIFT, mi_bbitmap_init, mi_bbitmap_is_clearN,
    mi_bbitmap_setN, mi_bbitmap_size, mi_bbitmap_t, mi_bbitmap_try_find_and_clearN,
    mi_bbitmap_unsafe_setN,
};
use crate::core_types::{BlockIndex, MemId, MetaMem};
use crate::internal::{
    _mi_align_down_ptr, _mi_align_up, _mi_arenas_alloc_aligned, _mi_arenas_free, _mi_divide_up,
    _mi_is_aligned, _mi_memzero_aligned, _mi_os_alloc, MI_SECURE, mi_memid_t,
};
use crate::mimalloc::mi_heap_main;
use crate::os::{
    _mi_os_secure_guard_page_set_at, _mi_os_secure_guard_page_set_before,
    _mi_os_secure_guard_page_size,
};

mod pages;
use pages::{
    mi_meta_page_list_changed_since, mi_meta_page_list_head, mi_meta_page_next,
    mi_meta_page_publish,
};

pub const MI_META_BLOCK_SIZE: usize = 1 << (16 - MI_BFIELD_BITS_SHIFT);
pub const MI_META_BLOCK_ALIGN: usize = MI_META_BLOCK_SIZE;
pub const MI_META_BLOCKS_PER_PAGE: usize = MI_META_PAGE_SIZE / MI_META_BLOCK_SIZE;
pub const MI_META_MAX_SIZE: usize = MI_BCHUNK_SIZE * MI_META_BLOCK_SIZE;
pub const _Atomic: () = ();

#[repr(C)]
pub struct mi_meta_page_s {
    pub next: *mut mi_meta_page_t,
    pub memid: mi_memid_t,
    pub blocks_free: mi_bbitmap_t,
}

pub type mi_meta_page_t = mi_meta_page_s;

#[inline]
fn meta_block_index(raw: usize) -> BlockIndex {
    crate::internal::mi_assert_internal!(raw <= u32::MAX as usize);
    BlockIndex(raw as u32)
}

#[inline]
fn meta_block_count(block_count: usize) -> NonZeroU32 {
    crate::internal::mi_assert_internal!(block_count > 0 && block_count <= u32::MAX as usize);
    // SAFETY: the assertions above establish the
    // non-zero u32 block-count domain used by meta memids.
    unsafe { NonZeroU32::new_unchecked(block_count as u32) }
}

#[inline]
unsafe fn mi_meta_memid(
    mpage: *mut mi_meta_page_t,
    block_index: BlockIndex,
    block_count: NonZeroU32,
) -> mi_memid_t {
    // SAFETY: callers pass a live meta page plus
    // checked block index/count domains before constructing the typed memid.
    unsafe {
        crate::internal::mi_assert_internal!(!mpage.is_null());
        MemId::<MetaMem>::meta(
            core::ptr::NonNull::new_unchecked(mpage.cast()),
            block_index,
            block_count,
        )
        .into_raw()
    }
}

/// # Safety
///
/// `p` must point inside a meta allocation managed by the arena-meta allocator.
/// If `block_idx` is non-null, it must be valid for writing one `usize`.
#[inline]
pub unsafe fn mi_meta_page_of_ptr(p: *mut c_void, block_idx: *mut usize) -> *mut mi_meta_page_t {
    // SAFETY: `p` is inside a meta allocation, so
    // aligning to the slice base and adding the guard-page size reaches the
    // owning meta-page header.
    let mpage = unsafe {
        (_mi_align_down_ptr(p, MI_ARENA_SLICE_ALIGN).cast::<u8>())
            .add(_mi_os_secure_guard_page_size())
            .cast::<mi_meta_page_t>()
    };
    if !block_idx.is_null() {
        // SAFETY: `block_idx` was checked non-null
        // and this function's contract requires it to be writable.
        unsafe {
            *block_idx =
                (p.cast::<u8>().offset_from(mpage.cast::<u8>()) as usize) / MI_META_BLOCK_SIZE;
        }
    }
    mpage
}

/// # Safety
///
/// `mpage` must point to a live meta page, and `block_idx` must be within the
/// page's meta block range.
#[inline]
pub unsafe fn mi_meta_block_start(
    mpage: *mut mi_meta_page_t,
    block_index: BlockIndex,
) -> *mut c_void {
    let block_idx = block_index.0 as usize;
    // SAFETY: secure guard-page size is a platform
    // primitive used only for meta-page address derivation.
    let guard_size = unsafe { _mi_os_secure_guard_page_size() };
    crate::internal::mi_assert_internal!(_mi_is_aligned(
        // SAFETY: `mpage` is a live meta page, and
        // subtracting the guard size returns the aligned slice base.
        unsafe { mpage.cast::<u8>().sub(guard_size).cast() },
        MI_ARENA_SLICE_ALIGN
    ));
    crate::internal::mi_assert_internal!(block_idx < MI_META_BLOCKS_PER_PAGE);
    // SAFETY: `block_idx` is within the meta page
    // and the computed block start stays inside the owning slice.
    let p = unsafe {
        mpage
            .cast::<u8>()
            .sub(guard_size)
            .add(block_idx * MI_META_BLOCK_SIZE)
            .cast::<c_void>()
    };
    crate::internal::mi_assert_internal!(
        // SAFETY: the computed block pointer is
        // checked by round-tripping to the same meta-page header.
        mpage == unsafe { mi_meta_page_of_ptr(p, ptr::null_mut()) }
    );
    p
}

unsafe fn mi_meta_page_zalloc() -> *mut mi_meta_page_t {
    // SAFETY: this helper owns creation of a new
    // meta page, initializes its memid/bitmap header, and publishes it with the
    // existing acquire/release stack discipline.
    unsafe {
        let mut memid: mi_memid_t = core::mem::zeroed();
        let base = _mi_arenas_alloc_aligned(
            mi_heap_main(),
            MI_META_PAGE_SIZE,
            MI_ARENA_SLICE_ALIGN,
            0,
            true,
            MI_SECURE == 0,
            ptr::null_mut(),
            0,
            -1,
            ptr::addr_of_mut!(memid),
        )
        .cast::<u8>();
        if base.is_null() {
            return ptr::null_mut();
        }
        crate::internal::mi_assert_internal!(_mi_is_aligned(base.cast(), MI_ARENA_SLICE_ALIGN));
        if !memid.initially_zero {
            _mi_memzero_aligned(base.cast(), MI_ARENA_SLICE_SIZE);
        }

        if MI_SECURE != 0 {
            _mi_os_secure_guard_page_set_at(base.cast(), memid);
            _mi_os_secure_guard_page_set_before(base.add(MI_META_PAGE_SIZE).cast(), memid);
        }

        let mpage = base
            .add(_mi_os_secure_guard_page_size())
            .cast::<mi_meta_page_t>();
        (*mpage).memid = memid;
        mi_bbitmap_init(
            ptr::addr_of_mut!((*mpage).blocks_free),
            MI_META_BLOCKS_PER_PAGE,
            true,
        );
        let mpage_size = offset_of!(mi_meta_page_t, blocks_free)
            + mi_bbitmap_size(MI_META_BLOCKS_PER_PAGE, ptr::null_mut());
        let info_blocks = _mi_divide_up(mpage_size, MI_META_BLOCK_SIZE);
        let guard_blocks = _mi_divide_up(_mi_os_secure_guard_page_size(), MI_META_BLOCK_SIZE);
        crate::internal::mi_assert_internal!(
            info_blocks + 2 * guard_blocks < MI_META_BLOCKS_PER_PAGE
        );
        mi_bbitmap_unsafe_setN(
            ptr::addr_of_mut!((*mpage).blocks_free),
            info_blocks + guard_blocks,
            MI_META_BLOCKS_PER_PAGE - info_blocks - 2 * guard_blocks,
        );

        mi_meta_page_publish(mpage)
    }
}

/// # Safety
///
/// `pmemid` must be valid for writing one `mi_memid_t`. The returned pointer,
/// when non-null, must be released with `_mi_meta_free` using the returned memid.
// SAFETY: this C-shaped shim validates size and
// output memid storage before allocating from the meta bitmap or arena/OS
// fallback.
pub(crate) unsafe extern "C" fn _mi_meta_zalloc(
    size: usize,
    pmemid: *mut mi_memid_t,
) -> *mut c_void {
    // SAFETY: the function contract provides valid
    // memid output storage and the body keeps all meta-page mutations local to
    // this allocation attempt.
    unsafe {
        crate::internal::mi_assert_internal!(!pmemid.is_null());
        let size = _mi_align_up(size, MI_META_BLOCK_SIZE);
        if size == 0 || size > MI_META_MAX_SIZE {
            return ptr::null_mut();
        }
        let block_count = _mi_divide_up(size, MI_META_BLOCK_SIZE);
        crate::internal::mi_assert_internal!(block_count > 0 && block_count < MI_BCHUNK_BITS);
        let meta_block_count = meta_block_count(block_count);

        let mpage0 = mi_meta_page_list_head();
        let mut mpage = mpage0;
        while !mpage.is_null() {
            let mut block_idx = 0usize;
            if mi_bbitmap_try_find_and_clearN(
                ptr::addr_of_mut!((*mpage).blocks_free),
                0,
                block_count,
                ptr::addr_of_mut!(block_idx),
            ) {
                let block_index = meta_block_index(block_idx);
                *pmemid = mi_meta_memid(mpage, block_index, meta_block_count);
                return mi_meta_block_start(mpage, block_index);
            }
            mpage = mi_meta_page_next(mpage);
        }

        if mi_meta_page_list_changed_since(mpage0) {
            return _mi_meta_zalloc(size, pmemid);
        }

        mpage = mi_meta_page_zalloc();
        if !mpage.is_null() {
            let mut block_idx = 0usize;
            if mi_bbitmap_try_find_and_clearN(
                ptr::addr_of_mut!((*mpage).blocks_free),
                0,
                block_count,
                ptr::addr_of_mut!(block_idx),
            ) {
                let block_index = meta_block_index(block_idx);
                *pmemid = mi_meta_memid(mpage, block_index, meta_block_count);
                return mi_meta_block_start(mpage, block_index);
            }
        }

        _mi_os_alloc(size, pmemid)
    }
}

/// # Safety
///
/// `p` may be null. Otherwise `p`, `size`, and `memid` must describe a live
/// allocation returned by `_mi_meta_zalloc` or its arena fallback.
// SAFETY: this C-shaped shim accepts the memid
// returned by allocation, reclaims meta bitmap ranges, and delegates non-meta
// memory to the arena free path.
pub(crate) unsafe extern "C" fn _mi_meta_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    unsafe {
        if p.is_null() {
            return;
        }
        if let crate::core_types::MemIdView::Meta(meta) = crate::core_types::MemId::view(memid) {
            let block_count = meta.block_count.get() as usize;
            let block_idx = meta.block_index.0 as usize;
            let block_index = meta.block_index;
            let mpage = meta.meta_page.as_ptr().cast::<mi_meta_page_t>();
            crate::internal::mi_assert_internal!(
                _mi_divide_up(size, MI_META_BLOCK_SIZE) == block_count
            );
            crate::internal::mi_assert_internal!(mi_meta_page_of_ptr(p, ptr::null_mut()) == mpage);
            crate::internal::mi_assert_internal!(
                block_idx + block_count <= MI_META_BLOCKS_PER_PAGE
            );
            crate::internal::mi_assert_internal!(mi_bbitmap_is_clearN(
                ptr::addr_of_mut!((*mpage).blocks_free),
                block_idx,
                block_count
            ));
            _mi_memzero_aligned(
                mi_meta_block_start(mpage, block_index),
                block_count * MI_META_BLOCK_SIZE,
            );
            mi_bbitmap_setN(
                ptr::addr_of_mut!((*mpage).blocks_free),
                block_idx,
                block_count,
            );
        } else {
            _mi_arenas_free(p, size, memid);
        }
    }
}

/// # Safety
///
/// `p` may be any pointer-sized value. The meta-page list must not be
/// concurrently destroyed while the lookup runs.
// SAFETY: debug classification only traverses the
// acquire-loaded meta-page stack and compares header identity.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_meta_is_meta_page(p: *mut c_void) -> bool {
    let mut mpage = mi_meta_page_list_head();
    while !mpage.is_null() {
        if mpage.cast::<c_void>() == p {
            return true;
        }
        // SAFETY: `mpage` came from the published
        // meta-page stack and remains valid for this debug traversal.
        mpage = unsafe { mi_meta_page_next(mpage) };
    }
    false
}

#[used]
// SAFETY: keep the debug classification shim linked
// with its exact C-shaped function pointer type.
static MI_META_IS_META_PAGE_KEEP: unsafe extern "C" fn(*mut c_void) -> bool = _mi_meta_is_meta_page;
