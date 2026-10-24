use core::sync::atomic::Ordering;

use crate::allocator::stats::{
    MI_CBIN_COUNT, MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_NONE, MI_CBIN_OTHER,
    MI_CBIN_SMALL, mi_chunkbin_t,
};
use crate::types::MI_ENABLE_LARGE_PAGES;

use super::{
    MI_BCHUNK_BITS, MI_BFIELD_BITS, MI_BIT_SET, mi_bbitmap_chunk, mi_bbitmap_chunk_count,
    mi_bbitmap_t, mi_bchunk_all_are_clear_relaxed, mi_bchunk_all_are_set_relaxed, mi_bchunk_clear,
    mi_bchunk_is_xsetN, mi_bchunk_set, mi_bchunk_t, mi_bitmap_chunk, mi_bitmap_chunk_count,
    mi_bitmap_t,
};

/// Mark the chunk in this initialized header's atomic map.
#[inline]
pub(super) fn mi_bitmap_chunkmap_set(bitmap: &mi_bitmap_t, chunk_idx: usize) {
    crate::internal::mi_debug_assert!(bitmap.chunk_count.load(Ordering::Relaxed) > chunk_idx);
    mi_bchunk_set(&bitmap.chunkmap, chunk_idx, None);
}

/// # Safety
/// The bitmap header and trailing chunks must remain initialized and live,
/// and `chunk_idx` must be below its initialized chunk count.
#[inline(never)]
pub(super) unsafe fn mi_bitmap_chunkmap_try_clear(
    bitmap: *const mi_bitmap_t,
    chunk_idx: usize,
) -> bool {
    unsafe {
        crate::internal::mi_debug_assert!(mi_bitmap_chunk_count(bitmap) > chunk_idx);
        if !mi_bchunk_all_are_clear_relaxed(&*mi_bitmap_chunk(bitmap, chunk_idx)) {
            return false;
        }
        mi_bchunk_clear(&(*bitmap).chunkmap, chunk_idx, None);
        if !mi_bchunk_all_are_clear_relaxed(&*mi_bitmap_chunk(bitmap, chunk_idx)) {
            mi_bchunk_set(&(*bitmap).chunkmap, chunk_idx, None);
            return false;
        }
        true
    }
}

/// Update this initialized header's atomic bin maps for one chunk.
#[inline(never)]
pub(super) fn mi_bbitmap_set_chunk_bin(
    bbitmap: &mi_bbitmap_t,
    chunk_idx: usize,
    bin: mi_chunkbin_t,
) {
    crate::internal::mi_debug_assert!(bbitmap.chunk_count.load(Ordering::Relaxed) > chunk_idx);
    let mut ibin = MI_CBIN_SMALL;
    while (ibin as i32) < (MI_CBIN_NONE as i32) {
        if ibin == bin {
            mi_bchunk_set(&bbitmap.chunkmap_bins[ibin as usize], chunk_idx, None);
        } else {
            mi_bchunk_clear(&bbitmap.chunkmap_bins[ibin as usize], chunk_idx, None);
        }
        ibin = mi_chunkbin_inc(ibin);
    }
}

#[inline]
fn mi_bbitmap_chunkmap_set_max(bbitmap: &mi_bbitmap_t, chunk_idx: usize) {
    let mut oldmax = bbitmap.chunk_max_accessed.load(Ordering::Relaxed);
    while chunk_idx > oldmax {
        match bbitmap.chunk_max_accessed.compare_exchange_weak(
            oldmax,
            chunk_idx,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(next) => oldmax = next,
        }
    }
}

/// # Safety
/// The bitmap header and trailing chunks must remain initialized and live,
/// and `chunk_idx` must be below its initialized chunk count.
#[inline]
pub(super) unsafe fn mi_bbitmap_chunkmap_set(
    bbitmap: *const mi_bbitmap_t,
    chunk_idx: usize,
    check_all_set: bool,
) {
    unsafe {
        crate::internal::mi_debug_assert!(mi_bbitmap_chunk_count(bbitmap) > chunk_idx);
        if check_all_set && mi_bchunk_all_are_set_relaxed(&*mi_bbitmap_chunk(bbitmap, chunk_idx)) {
            mi_bbitmap_set_chunk_bin(&*bbitmap, chunk_idx, MI_CBIN_NONE);
        }
        mi_bchunk_set(&(*bbitmap).chunkmap, chunk_idx, None);
        mi_bbitmap_chunkmap_set_max(&*bbitmap, chunk_idx);
    }
}

/// # Safety
/// The bitmap header and trailing chunks must remain initialized and live,
/// and `chunk_idx` must be below its initialized chunk count.
#[inline(never)]
pub(super) unsafe fn mi_bbitmap_chunkmap_try_clear(
    bbitmap: *const mi_bbitmap_t,
    chunk_idx: usize,
) -> bool {
    unsafe {
        crate::internal::mi_debug_assert!(mi_bbitmap_chunk_count(bbitmap) > chunk_idx);
        if !mi_bchunk_all_are_clear_relaxed(&*mi_bbitmap_chunk(bbitmap, chunk_idx)) {
            return false;
        }
        mi_bchunk_clear(&(*bbitmap).chunkmap, chunk_idx, None);
        if !mi_bchunk_all_are_clear_relaxed(&*mi_bbitmap_chunk(bbitmap, chunk_idx)) {
            mi_bchunk_set(&(*bbitmap).chunkmap, chunk_idx, None);
            return false;
        }
        mi_bbitmap_chunkmap_set_max(&*bbitmap, chunk_idx);
        true
    }
}

#[inline]
pub(super) fn mi_chunkbin_inc(bbin: mi_chunkbin_t) -> mi_chunkbin_t {
    crate::internal::mi_debug_assert!((bbin as i32) < (MI_CBIN_COUNT as i32));
    match bbin {
        MI_CBIN_SMALL => MI_CBIN_OTHER,
        MI_CBIN_OTHER => MI_CBIN_MEDIUM,
        MI_CBIN_MEDIUM => MI_CBIN_LARGE,
        MI_CBIN_LARGE => MI_CBIN_HUGE,
        MI_CBIN_HUGE => MI_CBIN_NONE,
        MI_CBIN_NONE | MI_CBIN_COUNT => unreachable!(),
    }
}

#[inline]
pub(super) fn mi_chunkbin_of(slice_count: usize) -> mi_chunkbin_t {
    if slice_count == 1 {
        MI_CBIN_SMALL
    } else if slice_count == 8 {
        MI_CBIN_MEDIUM
    } else if MI_ENABLE_LARGE_PAGES != 0 && slice_count == MI_BFIELD_BITS {
        MI_CBIN_LARGE
    } else if slice_count > MI_BCHUNK_BITS {
        MI_CBIN_HUGE
    } else {
        MI_CBIN_OTHER
    }
}

/// Returns the first matching chunk bin for a valid bit index.
#[inline(never)]
pub fn mi_bbitmap_debug_get_bin(
    chunkmap_bins: &[mi_bchunk_t; MI_CBIN_COUNT as usize - 1],
    chunk_idx: usize,
) -> mi_chunkbin_t {
    let mut ibin = MI_CBIN_SMALL;
    while (ibin as i32) < (MI_CBIN_NONE as i32) {
        if mi_bchunk_is_xsetN(MI_BIT_SET, &chunkmap_bins[ibin as usize], chunk_idx, 1) {
            return ibin;
        }
        ibin = mi_chunkbin_inc(ibin);
    }
    MI_CBIN_NONE
}
