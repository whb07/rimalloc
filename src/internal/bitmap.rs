//! Bitmap, page-map, page-queue, arena metadata

// Retains upstream bitmap sizing constants and optional SIMD scaffolding used
// by selected arena configurations.
#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::allocator::arena::MI_ARENA_BCHUNK_BITS;
use crate::allocator::stats::{MI_CBIN_COUNT, MI_CBIN_HUGE, MI_CBIN_NONE, MI_CBIN_SMALL};
use crate::bits::{MI_SIZE_BITS, MI_SIZE_SHIFT, MI_SIZE_SIZE, mi_ctz, mi_popcount};
use crate::core_types::{
    ArenaPtr, SliceCount, SliceIndex, SliceRange, ThreadLocalClaimCallback, ThreadSeq,
};
use crate::types::{mi_arena_t, mi_forall_set_fun_t};

pub const MI_BITMAP_H: usize = 1;
pub const MI_OPT_SIMD: usize = 0;

mod bfield;
mod chunk_bits;
mod chunkmap;
mod views;
pub use bfield::{
    mi_bfield_all_set, mi_bfield_clear_least_bit, mi_bfield_clz, mi_bfield_ctz, mi_bfield_one,
    mi_bfield_popcount, mi_bfield_zero,
};
use bfield::{
    mi_bfield_atomic_clear, mi_bfield_atomic_clear_mask, mi_bfield_atomic_clear_once_set,
    mi_bfield_atomic_is_set, mi_bfield_atomic_is_xset, mi_bfield_atomic_is_xset_mask,
    mi_bfield_atomic_popcount_mask, mi_bfield_atomic_set, mi_bfield_atomic_set_mask,
    mi_bfield_atomic_setX, mi_bfield_atomic_try_clear_mask, mi_bfield_atomic_try_clear_mask_of,
    mi_bfield_atomic_try_clearX, mi_bfield_find_highest_bit, mi_bfield_find_least_bit,
    mi_bfield_foreach_bit, mi_bfield_mask,
};
use chunk_bits::{mi_bchunk_is_xsetNC, mi_bchunk_popcountNC};
pub use chunkmap::mi_bbitmap_debug_get_bin;
use chunkmap::{
    mi_bbitmap_chunkmap_set, mi_bbitmap_chunkmap_try_clear, mi_bbitmap_set_chunk_bin,
    mi_bitmap_chunkmap_set, mi_bitmap_chunkmap_try_clear, mi_chunkbin_inc, mi_chunkbin_of,
};
pub(crate) use views::{BBitmapView, BitmapView};

/// A bounded bit range, advanced by complete or partial chunks. While
/// `remaining` is nonzero, `chunk_idx` identifies a live chunk and the remaining
/// bits fit within the slice from `cidx` onward. Only `new` and `next` can
/// change those fields.
struct BitmapChunks<'a> {
    chunks: &'a [mi_bchunk_t],
    chunk_idx: usize,
    cidx: usize,
    remaining: usize,
}

impl<'a> BitmapChunks<'a> {
    #[inline(always)]
    fn new(chunks: &'a [mi_bchunk_t], idx: usize, n: usize) -> Option<Self> {
        let maxbits = chunks.len() * MI_BCHUNK_BITS;
        crate::internal::mi_debug_assert!(n > 0);
        crate::internal::mi_debug_assert!(idx + n <= maxbits);
        if idx >= maxbits {
            return (idx == maxbits && n == 0).then_some(Self {
                chunks,
                chunk_idx: chunks.len(),
                cidx: 0,
                remaining: 0,
            });
        }
        Some(Self {
            chunks,
            chunk_idx: idx / MI_BCHUNK_BITS,
            cidx: idx % MI_BCHUNK_BITS,
            remaining: n.min(maxbits - idx),
        })
    }
}

impl<'a> Iterator for BitmapChunks<'a> {
    type Item = (usize, &'a mi_bchunk_t, usize, core::num::NonZeroUsize);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let chunk_idx = self.chunk_idx;
        let cidx = self.cidx;
        let n = self.remaining.min(MI_BCHUNK_BITS - cidx);
        // The checked constructor bounds the complete range; each advance
        // consumes a nonempty portion of this live chunk before moving onward.
        let (chunk, count) = unsafe {
            (
                self.chunks.get_unchecked(chunk_idx),
                core::num::NonZeroUsize::new_unchecked(n),
            )
        };
        self.remaining -= n;
        self.chunk_idx += 1;
        self.cidx = 0;
        Some((chunk_idx, chunk, cidx, count))
    }
}

impl<'a> BitmapView<'a> {
    /// # Safety
    /// The initialized compact header and its published trailing chunks must
    /// remain aligned, live and atomically accessible for `'a`. The count must
    /// fit `MI_BITMAP_MAX_CHUNK_COUNT`, with stable backing throughout the loan.
    #[inline(always)]
    pub(crate) unsafe fn from_raw(raw: *const mi_bitmap_t) -> Self {
        unsafe {
            let count = (*raw).chunk_count.load(Ordering::Relaxed);
            let chunks = ptr::addr_of!((*raw).chunks).cast::<mi_bchunk_t>();
            Self::new(&*raw, core::slice::from_raw_parts(chunks, count))
        }
    }
}

impl<'a> BBitmapView<'a> {
    /// # Safety
    /// The initialized compact header and its published trailing chunks must
    /// remain aligned, live and atomically accessible for `'a`. The count must
    /// fit `MI_BITMAP_MAX_CHUNK_COUNT`, with stable backing throughout the loan.
    #[inline(always)]
    pub(crate) unsafe fn from_raw(raw: *const mi_bbitmap_t) -> Self {
        unsafe {
            let count = (*raw).chunk_count.load(Ordering::Relaxed);
            let chunks = ptr::addr_of!((*raw).chunks).cast::<mi_bchunk_t>();
            Self::new(&*raw, core::slice::from_raw_parts(chunks, count))
        }
    }
}

pub const MI_BFIELD_BITS_SHIFT: usize = MI_SIZE_SHIFT + 3;
pub const MI_BFIELD_BITS: usize = 1 << MI_BFIELD_BITS_SHIFT;
pub const MI_BFIELD_SIZE: usize = MI_BFIELD_BITS / 8;
pub const MI_BFIELD_LO_BIT8: usize = (!0usize) / 0xFF;
pub const MI_BFIELD_HI_BIT8: usize = MI_BFIELD_LO_BIT8 << 7;

pub const MI_BCHUNK_BITS: usize = MI_ARENA_BCHUNK_BITS;
pub const MI_BCHUNK_SIZE: usize = MI_BCHUNK_BITS / 8;
pub const MI_BCHUNK_FIELDS: usize = MI_BCHUNK_BITS / MI_BFIELD_BITS;
pub const MI_BCHUNKMAP_BITS: usize = MI_BCHUNK_BITS;

pub const MI_BITMAP_MAX_CHUNK_COUNT: usize = MI_BCHUNKMAP_BITS;
pub const MI_BITMAP_MIN_CHUNK_COUNT: usize = 1;
pub const MI_BITMAP_DEFAULT_CHUNK_COUNT: usize = if MI_SIZE_BITS > 32 { 64 } else { 1 };
pub const MI_BITMAP_MAX_BIT_COUNT: usize = MI_BITMAP_MAX_CHUNK_COUNT * MI_BCHUNK_BITS;
pub const MI_BITMAP_MIN_BIT_COUNT: usize = MI_BITMAP_MIN_CHUNK_COUNT * MI_BCHUNK_BITS;
pub const MI_BITMAP_DEFAULT_BIT_COUNT: usize = MI_BITMAP_DEFAULT_CHUNK_COUNT * MI_BCHUNK_BITS;

pub type mi_bfield_t = usize;
pub type mi_xset_t = bool;
pub const MI_BIT_SET: mi_xset_t = true;
pub const MI_BIT_CLEAR: mi_xset_t = false;

macro_rules! define_bitmap_types {
    ($align:literal) => {
        #[repr(C, align($align))]
        #[derive(Debug)]
        pub struct mi_bchunk_s {
            pub bfields: [AtomicUsize; MI_BCHUNK_FIELDS],
        }

        impl Default for mi_bchunk_s {
            fn default() -> Self {
                Self {
                    bfields: core::array::from_fn(|_| AtomicUsize::new(0)),
                }
            }
        }

        pub type mi_bchunk_t = mi_bchunk_s;
        pub type mi_bchunkmap_t = mi_bchunk_t;

        #[repr(C, align($align))]
        #[derive(Debug)]
        pub struct mi_bitmap_s {
            pub chunk_count: AtomicUsize,
            pub _padding: [usize; MI_BCHUNK_SIZE / MI_SIZE_SIZE - 1],
            pub chunkmap: mi_bchunkmap_t,
            /// Flexible trailing storage; only `chunk_count` chunks are allocated.
            pub chunks: [mi_bchunk_t; 0],
        }

        pub type mi_bitmap_t = mi_bitmap_s;

        #[repr(C, align($align))]
        #[derive(Debug)]
        pub struct mi_bbitmap_s {
            pub chunk_count: AtomicUsize,
            pub chunk_max_accessed: AtomicUsize,
            pub _padding: [usize; MI_BCHUNK_SIZE / MI_SIZE_SIZE - 2],
            pub chunkmap: mi_bchunkmap_t,
            pub chunkmap_bins: [mi_bchunkmap_t; MI_CBIN_COUNT as usize - 1],
            /// Flexible trailing storage; only `chunk_count` chunks are allocated.
            pub chunks: [mi_bchunk_t; 0],
        }

        pub type mi_bbitmap_t = mi_bbitmap_s;
    };
}

#[cfg(target_pointer_width = "64")]
define_bitmap_types!(64);

#[cfg(target_pointer_width = "32")]
define_bitmap_types!(32);

/// # Safety
/// `bitmap` must have initialized trailing storage covering `index`.
#[inline(always)]
unsafe fn mi_bitmap_chunk(bitmap: *const mi_bitmap_t, index: usize) -> *const mi_bchunk_t {
    // Project from the original allocation pointer, not from a header reference:
    // a header borrow does not cover the flexible trailing allocation.
    unsafe {
        ptr::addr_of!((*bitmap).chunks)
            .cast::<mi_bchunk_t>()
            .add(index)
    }
}

/// # Safety
/// `bbitmap` must have initialized trailing storage covering `index`.
#[inline(always)]
unsafe fn mi_bbitmap_chunk(bbitmap: *const mi_bbitmap_t, index: usize) -> *const mi_bchunk_t {
    unsafe {
        ptr::addr_of!((*bbitmap).chunks)
            .cast::<mi_bchunk_t>()
            .add(index)
    }
}

/// # Safety
/// `bitmap` must point to an initialized `mi_bitmap_t`.
#[inline]
pub unsafe fn mi_bitmap_chunk_count(bitmap: *const mi_bitmap_t) -> usize {
    unsafe { (*bitmap).chunk_count.load(Ordering::Relaxed) }
}

/// # Safety
/// `bitmap` must point to an initialized `mi_bitmap_t`.
#[inline]
pub unsafe fn mi_bitmap_max_bits(bitmap: *const mi_bitmap_t) -> usize {
    unsafe { mi_bitmap_chunk_count(bitmap) * MI_BCHUNK_BITS }
}

/// # Safety
/// `bbitmap` must point to an initialized `mi_bbitmap_t`.
#[inline]
pub unsafe fn mi_bbitmap_chunk_count(bbitmap: *const mi_bbitmap_t) -> usize {
    unsafe { (*bbitmap).chunk_count.load(Ordering::Relaxed) }
}

/// # Safety
/// `bbitmap` must point to an initialized `mi_bbitmap_t`.
#[inline]
pub unsafe fn mi_bbitmap_max_bits(bbitmap: *const mi_bbitmap_t) -> usize {
    unsafe { mi_bbitmap_chunk_count(bbitmap) * MI_BCHUNK_BITS }
}

#[inline]
fn mi_bchunk_set(chunk: &mi_bchunk_t, cidx: usize, already_set: Option<&mut usize>) -> bool {
    crate::internal::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    let was_clear = mi_bfield_atomic_set(&chunk.bfields[i], idx);
    if let Some(already_set) = already_set {
        *already_set = if was_clear { 0 } else { 1 };
    }
    was_clear
}

#[inline]
fn mi_bchunk_setNX(
    chunk: &mi_bchunk_t,
    cidx: usize,
    n: usize,
    already_set: Option<&mut usize>,
) -> bool {
    crate::internal::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    crate::internal::mi_debug_assert!(n > 0 && n <= MI_BFIELD_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    if idx + n <= MI_BFIELD_BITS {
        mi_bfield_atomic_set_mask(&chunk.bfields[i], mi_bfield_mask(n, idx), already_set)
    } else {
        let m = MI_BFIELD_BITS - idx;
        crate::internal::mi_debug_assert!(m < n);
        crate::internal::mi_debug_assert!(i < MI_BCHUNK_FIELDS - 1);
        let mut already_set1 = 0usize;
        let all_set1 = mi_bfield_atomic_set_mask(
            &chunk.bfields[i],
            mi_bfield_mask(m, idx),
            Some(&mut already_set1),
        );
        let mut already_set2 = 0usize;
        let all_set2 = mi_bfield_atomic_set_mask(
            &chunk.bfields[i + 1],
            mi_bfield_mask(n - m, 0),
            Some(&mut already_set2),
        );
        if let Some(already_set) = already_set {
            *already_set = already_set1 + already_set2;
        }
        all_set1 && all_set2
    }
}

#[inline(never)]
fn mi_bchunk_xsetNC(
    set: mi_xset_t,
    chunk: &mi_bchunk_t,
    cidx: usize,
    mut n: usize,
    palready_set: Option<&mut usize>,
    mut pmaybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::internal::mi_debug_assert!(n > 0);
    let mut all_transition = true;
    let mut maybe_all_clear = true;
    let mut total_already_set = 0usize;
    let mut idx = cidx % MI_BFIELD_BITS;
    let mut field = cidx / MI_BFIELD_BITS;
    while n > 0 {
        let mut m = MI_BFIELD_BITS - idx;
        if m > n {
            m = n;
        }
        crate::internal::mi_debug_assert!(idx + m <= MI_BFIELD_BITS);
        crate::internal::mi_debug_assert!(field < MI_BCHUNK_FIELDS);
        let mask = mi_bfield_mask(m, idx);
        let mut already_set = 0usize;
        let mut all_clear = false;
        let transition = if set {
            mi_bfield_atomic_set_mask(&chunk.bfields[field], mask, Some(&mut already_set))
        } else {
            mi_bfield_atomic_clear_mask(&chunk.bfields[field], mask, Some(&mut all_clear))
        };
        crate::internal::mi_debug_assert!(
            (transition && already_set == 0) || (!transition && already_set > 0) || set
        );
        all_transition &= transition;
        total_already_set += already_set;
        maybe_all_clear &= all_clear;
        field += 1;
        idx = 0;
        n -= m;
    }
    if let Some(palready_set) = palready_set {
        *palready_set = total_already_set;
    }
    if let Some(pmaybe_all_clear) = &mut pmaybe_all_clear {
        **pmaybe_all_clear = maybe_all_clear;
    }
    all_transition
}

#[inline]
fn mi_bchunk_setN(
    chunk: &mi_bchunk_t,
    cidx: usize,
    n: usize,
    already_set: Option<&mut usize>,
) -> bool {
    crate::internal::mi_debug_assert!(n > 0 && n <= MI_BCHUNK_BITS);
    if n == 1 {
        mi_bchunk_set(chunk, cidx, already_set)
    } else if n <= MI_BFIELD_BITS {
        mi_bchunk_setNX(chunk, cidx, n, already_set)
    } else {
        mi_bchunk_xsetNC(MI_BIT_SET, chunk, cidx, n, already_set, None)
    }
}

#[inline]
fn mi_bchunk_clear(chunk: &mi_bchunk_t, cidx: usize, all_clear: Option<&mut bool>) -> bool {
    crate::internal::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    mi_bfield_atomic_clear(&chunk.bfields[i], idx, all_clear)
}

#[inline]
fn mi_bchunk_clearN(
    chunk: &mi_bchunk_t,
    cidx: usize,
    n: usize,
    maybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(n > 0 && n <= MI_BCHUNK_BITS);
    if n == 1 {
        mi_bchunk_clear(chunk, cidx, maybe_all_clear)
    } else {
        mi_bchunk_xsetNC(MI_BIT_CLEAR, chunk, cidx, n, None, maybe_all_clear)
    }
}

#[inline]
fn mi_bchunk_popcountN(chunk: &mi_bchunk_t, cidx: usize, n: usize) -> usize {
    crate::internal::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::internal::mi_debug_assert!(n > 0);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    if n == 1 {
        if mi_bfield_atomic_is_set(&chunk.bfields[i], idx) {
            1
        } else {
            0
        }
    } else if idx + n <= MI_BFIELD_BITS {
        mi_bfield_atomic_popcount_mask(&chunk.bfields[i], mi_bfield_mask(n, idx))
    } else {
        mi_bchunk_popcountNC(chunk, i, idx, n)
    }
}

#[inline]
fn mi_bchunk_is_xsetN(set: mi_xset_t, chunk: &mi_bchunk_t, cidx: usize, n: usize) -> bool {
    crate::internal::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::internal::mi_debug_assert!(n > 0);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    if n == 1 {
        mi_bfield_atomic_is_xset(set, &chunk.bfields[i], idx)
    } else if idx + n <= MI_BFIELD_BITS {
        mi_bfield_atomic_is_xset_mask(set, &chunk.bfields[i], mi_bfield_mask(n, idx))
    } else {
        mi_bchunk_is_xsetNC(set, chunk, i, idx, n)
    }
}

#[inline]
fn mi_bchunk_try_clearNX(
    chunk: &mi_bchunk_t,
    cidx: usize,
    n: usize,
    pmaybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    crate::internal::mi_debug_assert!(n <= MI_BFIELD_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    if idx + n <= MI_BFIELD_BITS {
        mi_bfield_atomic_try_clear_mask(&chunk.bfields[i], mi_bfield_mask(n, idx), pmaybe_all_clear)
    } else {
        let m = MI_BFIELD_BITS - idx;
        crate::internal::mi_debug_assert!(m < n);
        crate::internal::mi_debug_assert!(i < MI_BCHUNK_FIELDS - 1);
        let mut field1_is_clear = false;
        if !mi_bfield_atomic_try_clear_mask(
            &chunk.bfields[i],
            mi_bfield_mask(m, idx),
            Some(&mut field1_is_clear),
        ) {
            return false;
        }
        let mut field2_is_clear = false;
        if !mi_bfield_atomic_try_clear_mask(
            &chunk.bfields[i + 1],
            mi_bfield_mask(n - m, 0),
            Some(&mut field2_is_clear),
        ) {
            mi_bfield_atomic_set_mask(&chunk.bfields[i], mi_bfield_mask(m, idx), None);
            return false;
        }
        if let Some(pmaybe_all_clear) = pmaybe_all_clear {
            *pmaybe_all_clear = field1_is_clear && field2_is_clear;
        }
        true
    }
}

#[inline(never)]
fn mi_bchunk_try_clearNC(
    chunk: &mi_bchunk_t,
    cidx: usize,
    mut n: usize,
    mut pmaybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::internal::mi_debug_assert!(n > 0);
    if let Some(pmaybe_all_clear) = &mut pmaybe_all_clear {
        **pmaybe_all_clear = true;
    }
    let start_idx = cidx % MI_BFIELD_BITS;
    let start_field = cidx / MI_BFIELD_BITS;
    let mut field = start_field;
    let mut m = MI_BFIELD_BITS - start_idx;
    if m > n {
        m = n;
    }
    crate::internal::mi_debug_assert!(start_idx + m <= MI_BFIELD_BITS);
    crate::internal::mi_debug_assert!(start_field < MI_BCHUNK_FIELDS);
    let mask_start = mi_bfield_mask(m, start_idx);
    let mut maybe_all_clear = false;
    if !mi_bfield_atomic_try_clear_mask(
        &chunk.bfields[field],
        mask_start,
        Some(&mut maybe_all_clear),
    ) {
        return false;
    }
    n -= m;
    while n >= MI_BFIELD_BITS {
        field += 1;
        crate::internal::mi_debug_assert!(field < MI_BCHUNK_FIELDS);
        let mut field_is_clear = false;
        if !mi_bfield_atomic_try_clearX(&chunk.bfields[field], Some(&mut field_is_clear)) {
            let mut restore = field;
            while restore > start_field {
                restore -= 1;
                if restore == start_field {
                    mi_bfield_atomic_set_mask(&chunk.bfields[restore], mask_start, None);
                } else {
                    mi_bfield_atomic_setX(&chunk.bfields[restore], None);
                }
            }
            return false;
        }
        maybe_all_clear &= field_is_clear;
        n -= MI_BFIELD_BITS;
    }
    if n > 0 {
        field += 1;
        crate::internal::mi_debug_assert!(field < MI_BCHUNK_FIELDS);
        let mut field_is_clear = false;
        if !mi_bfield_atomic_try_clear_mask(
            &chunk.bfields[field],
            mi_bfield_mask(n, 0),
            Some(&mut field_is_clear),
        ) {
            let mut restore = field;
            while restore > start_field {
                restore -= 1;
                if restore == start_field {
                    mi_bfield_atomic_set_mask(&chunk.bfields[restore], mask_start, None);
                } else {
                    mi_bfield_atomic_setX(&chunk.bfields[restore], None);
                }
            }
            return false;
        }
        maybe_all_clear &= field_is_clear;
    }
    if let Some(pmaybe_all_clear) = &mut pmaybe_all_clear {
        **pmaybe_all_clear = maybe_all_clear;
    }
    true
}

#[inline]
fn mi_bchunk_try_clearN(
    chunk: &mi_bchunk_t,
    cidx: usize,
    n: usize,
    maybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(n > 0);
    if n <= MI_BFIELD_BITS {
        mi_bchunk_try_clearNX(chunk, cidx, n, maybe_all_clear)
    } else {
        mi_bchunk_try_clearNC(chunk, cidx, n, maybe_all_clear)
    }
}

#[inline]
fn mi_bchunk_try_find_and_clear_at(
    chunk: &mi_bchunk_t,
    chunk_idx: usize,
    pidx: &mut usize,
) -> bool {
    crate::internal::mi_debug_assert!(chunk_idx < MI_BCHUNK_FIELDS);
    let b = chunk.bfields[chunk_idx].load(Ordering::Acquire);
    let mut idx = 0usize;
    if mi_bfield_find_least_bit(b, &mut idx)
        && mi_bfield_atomic_try_clear_mask_of(
            &chunk.bfields[chunk_idx],
            mi_bfield_mask(1, idx),
            b,
            None,
        )
    {
        *pidx = (chunk_idx * MI_BFIELD_BITS) + idx;
        crate::internal::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
        return true;
    }
    false
}

#[inline]
fn mi_bchunk_try_find_and_clear(chunk: &mi_bchunk_t, pidx: &mut usize) -> bool {
    for i in 0..MI_BCHUNK_FIELDS {
        if mi_bchunk_try_find_and_clear_at(chunk, i, pidx) {
            return true;
        }
    }
    false
}

#[inline]
fn mi_bchunk_try_find_and_clear8_at(
    chunk: &mi_bchunk_t,
    chunk_idx: usize,
    pidx: &mut usize,
) -> bool {
    let b = chunk.bfields[chunk_idx].load(Ordering::Relaxed);
    let has_set8 = (((!b).wrapping_sub(MI_BFIELD_LO_BIT8)) & (b & MI_BFIELD_HI_BIT8)) >> 7;
    let mut idx = 0usize;
    if mi_bfield_find_least_bit(has_set8, &mut idx) {
        crate::internal::mi_debug_assert!(idx <= MI_BFIELD_BITS - 8);
        crate::internal::mi_debug_assert!(idx.is_multiple_of(8));
        if mi_bfield_atomic_try_clear_mask_of(&chunk.bfields[chunk_idx], 0xFFusize << idx, b, None)
        {
            *pidx = (chunk_idx * MI_BFIELD_BITS) + idx;
            crate::internal::mi_debug_assert!(*pidx + 8 <= MI_BCHUNK_BITS);
            return true;
        }
    }
    false
}

#[inline(never)]
fn mi_bchunk_try_find_and_clear8(chunk: &mi_bchunk_t, pidx: &mut usize) -> bool {
    for i in 0..MI_BCHUNK_FIELDS {
        if mi_bchunk_try_find_and_clear8_at(chunk, i, pidx) {
            return true;
        }
    }
    false
}

#[inline(never)]
fn mi_bchunk_try_find_and_clearNX(chunk: &mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
    if n == 0 || n > MI_BFIELD_BITS {
        return false;
    }
    let mask = mi_bfield_mask(n, 0);
    for i in 0..MI_BCHUNK_FIELDS {
        let mut b0 = chunk.bfields[i].load(Ordering::Relaxed);
        let mut b = b0;
        let mut idx = 0usize;
        while mi_bfield_find_least_bit(b, &mut idx) {
            if idx + n > MI_BFIELD_BITS {
                break;
            }
            let bmask = mask << idx;
            crate::internal::mi_debug_assert!(bmask >> idx == mask);
            if (b & bmask) == bmask {
                if mi_bfield_atomic_try_clear_mask_of(&chunk.bfields[i], bmask, b0, None) {
                    *pidx = (i * MI_BFIELD_BITS) + idx;
                    crate::internal::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
                    crate::internal::mi_debug_assert!(*pidx + n <= MI_BCHUNK_BITS);
                    return true;
                }
                b0 = chunk.bfields[i].load(Ordering::Acquire);
                b = b0;
            } else {
                b = b & b.wrapping_add(mi_bfield_one() << idx);
            }
        }

        if b != 0 && i < MI_BCHUNK_FIELDS - 1 {
            let post = mi_bfield_clz(!b);
            if post > 0 {
                let pre = mi_bfield_ctz(!chunk.bfields[i + 1].load(Ordering::Relaxed));
                if post + pre >= n {
                    let cidx = (i * MI_BFIELD_BITS) + (MI_BFIELD_BITS - post);
                    if mi_bchunk_try_clearNX(chunk, cidx, n, None) {
                        *pidx = cidx;
                        crate::internal::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
                        crate::internal::mi_debug_assert!(*pidx + n <= MI_BCHUNK_BITS);
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[inline(never)]
#[allow(clippy::many_single_char_names)]
fn mi_bchunk_try_find_and_clearNC(chunk: &mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
    if n == 0 || n > MI_BCHUNK_BITS {
        return false;
    }
    let skip_count = (n - 1) / MI_BFIELD_BITS;
    let limit = MI_BCHUNK_FIELDS - skip_count;
    let mut i = 0usize;
    while i < limit {
        let mut m = n;
        let mut b = chunk.bfields[i].load(Ordering::Relaxed);
        let mut ones = mi_bfield_clz(!b);
        let cidx = (i * MI_BFIELD_BITS) + (MI_BFIELD_BITS - ones);
        if ones >= m {
            m = 0;
        } else if ones > 0 {
            m -= ones;
            let mut j = 1usize;
            while i + j < MI_BCHUNK_FIELDS {
                b = chunk.bfields[i + j].load(Ordering::Relaxed);
                ones = mi_bfield_ctz(!b);
                if ones >= m {
                    m = 0;
                    break;
                } else if ones == MI_BFIELD_BITS {
                    j += 1;
                    m -= MI_BFIELD_BITS;
                } else {
                    i = i + j - 1;
                    break;
                }
            }
        }
        if m == 0 && mi_bchunk_try_clearN(chunk, cidx, n, None) {
            *pidx = cidx;
            crate::internal::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
            crate::internal::mi_debug_assert!(*pidx + n <= MI_BCHUNK_BITS);
            return true;
        }
        i += 1;
    }
    false
}

#[inline]
fn mi_bchunk_clear_once_set(chunk: &mi_bchunk_t, cidx: usize) {
    crate::internal::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    mi_bfield_atomic_clear_once_set(&chunk.bfields[i], idx);
}

#[inline]
fn mi_bchunk_all_are_clear_relaxed(chunk: &mi_bchunk_t) -> bool {
    for i in 0..MI_BCHUNK_FIELDS {
        if chunk.bfields[i].load(Ordering::Relaxed) != 0 {
            return false;
        }
    }
    true
}

#[inline]
fn mi_bchunk_all_are_set_relaxed(chunk: &mi_bchunk_t) -> bool {
    for i in 0..MI_BCHUNK_FIELDS {
        if chunk.bfields[i].load(Ordering::Relaxed) != mi_bfield_all_set() {
            return false;
        }
    }
    true
}

#[inline]
fn mi_bchunk_bsr(chunk: &mi_bchunk_t, pidx: &mut usize) -> bool {
    for i in (0..MI_BCHUNK_FIELDS).rev() {
        let b = chunk.bfields[i].load(Ordering::Relaxed);
        let mut idx = 0usize;
        if mi_bfield_find_highest_bit(b, &mut idx) {
            *pidx = (i * MI_BFIELD_BITS) + idx;
            return true;
        }
    }
    false
}

#[inline]
fn mi_bchunk_bsr_inv(chunk: &mi_bchunk_t, pidx: &mut usize) -> bool {
    for i in (0..MI_BCHUNK_FIELDS).rev() {
        let b = chunk.bfields[i].load(Ordering::Relaxed);
        let mut idx = 0usize;
        if mi_bfield_find_highest_bit(!b, &mut idx) {
            *pidx = (i * MI_BFIELD_BITS) + idx;
            return true;
        }
    }
    false
}

#[inline]
fn mi_bchunk_popcount(chunk: &mi_bchunk_t) -> usize {
    let mut popcount = 0usize;
    for i in 0..MI_BCHUNK_FIELDS {
        popcount += mi_bfield_popcount(chunk.bfields[i].load(Ordering::Relaxed));
    }
    popcount
}

#[inline]
fn mi_bfield_cycle_iterate<F>(bfield: mi_bfield_t, tseq: usize, cycle: usize, mut visit: F) -> bool
where
    F: FnMut(usize) -> bool,
{
    crate::internal::mi_debug_assert!(cycle > 0);
    crate::internal::mi_debug_assert!(cycle <= MI_BFIELD_BITS);
    let start = (tseq as u32 as usize) % cycle;
    crate::internal::mi_debug_assert!(start <= cycle);
    let cycle_mask = mi_bfield_mask(cycle - start, start);
    let mut bcount = mi_bfield_popcount(bfield);
    let mut b = bfield & cycle_mask;
    while bcount > 0 {
        if b == 0 {
            b = bfield & !cycle_mask;
        }
        let mut name_idx = 0usize;
        let found = mi_bfield_find_least_bit(b, &mut name_idx);
        b = mi_bfield_clear_least_bit(b);
        crate::internal::mi_debug_assert!(found);
        if !visit(name_idx) {
            return false;
        }
        bcount -= 1;
    }
    true
}

/// # Safety
/// `bitmap` must remain live and initialized throughout the search. `claim` must
/// be callable for each selected slice, and `arena`, when present, must remain
/// valid for the claim callback.
#[inline(never)]
pub(crate) unsafe fn mi_bitmap_try_find_and_claim(
    bitmap: *mut mi_bitmap_t,
    tseq: ThreadSeq,
    pidx: &mut SliceIndex,
    claim: ThreadLocalClaimCallback,
    arena: Option<ArenaPtr>,
) -> bool {
    unsafe {
        let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
        for i in 0..chunkmap_max {
            let cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            let mut hi = 0usize;
            if mi_bfield_find_highest_bit(cmap_entry, &mut hi) {
                let mut found = false;
                let _ = mi_bfield_cycle_iterate(cmap_entry, tseq.get() % 8, hi + 1, |eidx| {
                    let chunk_idx = (i * MI_BFIELD_BITS) + eidx;
                    crate::internal::mi_debug_assert!(chunk_idx < mi_bitmap_chunk_count(bitmap));
                    if mi_bitmap_try_find_and_claim_visit(bitmap, chunk_idx, pidx, claim, arena) {
                        found = true;
                        return false;
                    }
                    true
                });
                if found {
                    return true;
                }
            }
        }
        false
    }
}

#[inline]
unsafe fn mi_bitmap_try_find_and_claim_visit(
    bitmap: *mut mi_bitmap_t,
    chunk_idx: usize,
    pidx: &mut SliceIndex,
    claim: ThreadLocalClaimCallback,
    arena: Option<ArenaPtr>,
) -> bool {
    unsafe {
        let chunk = &*mi_bitmap_chunk(bitmap, chunk_idx);
        let mut cidx = 0usize;
        if mi_bchunk_try_find_and_clear(chunk, &mut cidx) {
            let slice_index = (chunk_idx * MI_BCHUNK_BITS) + cidx;
            crate::internal::mi_debug_assert!(slice_index < mi_bitmap_max_bits(bitmap));
            let mut keep_set = true;
            if claim.invoke(slice_index, arena, &mut keep_set) {
                crate::internal::mi_debug_assert!(!keep_set);
                *pidx = SliceIndex::new(slice_index);
                return true;
            }
            if keep_set {
                let wasclear = mi_bchunk_set(chunk, cidx, None);
                crate::internal::mi_debug_assert!(wasclear);
            }
        } else {
            mi_bitmap_chunkmap_try_clear(&*bitmap, chunk_idx, chunk);
        }
        false
    }
}

/// # Safety
/// If non-null, `pchunk_count` must be valid for a `usize` write.
#[inline(never)]
pub unsafe fn mi_bitmap_size(bit_count: usize, pchunk_count: *mut usize) -> usize {
    unsafe {
        if bit_count == 0 {
            if !pchunk_count.is_null() {
                *pchunk_count = 0;
            }
            return 0;
        }
        let bit_count = crate::internal::_mi_align_up(bit_count, MI_BCHUNK_BITS);
        crate::internal::mi_debug_assert!(bit_count <= MI_BITMAP_MAX_BIT_COUNT);
        let chunk_count = bit_count / MI_BCHUNK_BITS;
        if !pchunk_count.is_null() {
            *pchunk_count = chunk_count;
        }
        (chunk_count + 2) * MI_BCHUNK_SIZE
    }
}

/// # Safety
/// `bitmap` must point to enough writable storage for `bit_count` bits rounded up to bitmap
/// chunk granularity. If `already_zero` is true, that storage must already be zeroed.
#[inline(never)]
pub unsafe fn mi_bitmap_init(
    bitmap: *mut mi_bitmap_t,
    bit_count: usize,
    already_zero: bool,
) -> usize {
    unsafe {
        let mut chunk_count = 0usize;
        let size = mi_bitmap_size(bit_count, &mut chunk_count as *mut usize);
        if !already_zero {
            ptr::write_bytes(bitmap.cast::<u8>(), 0, size);
        }
        (*bitmap).chunk_count.store(chunk_count, Ordering::Release);
        crate::internal::mi_debug_assert!(
            mi_bitmap_chunk_count(bitmap) <= MI_BITMAP_MAX_CHUNK_COUNT
        );
        size
    }
}

#[inline]
fn mi_bchunks_unsafe_setN(
    chunks: *mut mi_bchunk_t,
    cmap: *mut mi_bchunk_t,
    idx: usize,
    mut n: usize,
) {
    crate::internal::mi_debug_assert!(n > 0);
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let cidx = idx % MI_BCHUNK_BITS;
    let ccount = n.div_ceil(MI_BCHUNK_BITS);
    mi_bchunk_setN(unsafe { &*cmap.add(0) }, chunk_idx, ccount, None);
    let mut m = MI_BCHUNK_BITS - cidx;
    if m > n {
        m = n;
    }
    mi_bchunk_setN(unsafe { &*chunks.add(chunk_idx) }, cidx, m, None);
    chunk_idx += 1;
    n -= m;
    let mid_chunks = n / MI_BCHUNK_BITS;
    if mid_chunks > 0 {
        unsafe {
            ptr::write_bytes(
                chunks.add(chunk_idx).cast::<u8>(),
                0xFF,
                mid_chunks * MI_BCHUNK_SIZE,
            );
        }
        chunk_idx += mid_chunks;
        n -= mid_chunks * MI_BCHUNK_BITS;
    }
    if n > 0 {
        crate::internal::mi_debug_assert!(n < MI_BCHUNK_BITS);
        mi_bchunk_setN(unsafe { &*chunks.add(chunk_idx) }, 0, n, None);
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `idx..idx + n` must be within its bit range.
#[inline(never)]
pub unsafe fn mi_bitmap_unsafe_setN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) {
    unsafe {
        crate::internal::mi_debug_assert!(n > 0);
        crate::internal::mi_debug_assert!(idx + n <= mi_bitmap_max_bits(bitmap));
        mi_bchunks_unsafe_setN(
            mi_bitmap_chunk(bitmap, 0).cast_mut(),
            &mut (*bitmap).chunkmap,
            idx,
            n,
        );
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap. If non-null, `palready_set` must be valid for a
/// `usize` write and disjoint from the bitmap allocation.
#[inline]
pub unsafe fn mi_bitmap_setN(
    bitmap: *mut mi_bitmap_t,
    idx: usize,
    n: usize,
    palready_set: *mut usize,
) -> bool {
    // The raw output may be uninitialized. Only the local counter is borrowed
    // as `usize`; the compatibility boundary writes the final value once.
    unsafe {
        let mut already_set = usize::MAX;
        let output = if palready_set.is_null() {
            None
        } else {
            Some(&mut already_set)
        };
        let result = BitmapView::from_raw(bitmap).set_n(idx, n, output);
        if !palready_set.is_null() && already_set != usize::MAX {
            palready_set.write(already_set);
        }
        result
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline]
pub unsafe fn mi_bitmap_clearN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) -> bool {
    unsafe { BitmapView::from_raw(bitmap).clear_n(idx, n) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `idx` must be a valid bit index.
#[inline(never)]
pub unsafe fn mi_bitmap_set(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    unsafe { mi_bitmap_setN(bitmap, idx, 1, core::ptr::null_mut()) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `idx` must be a valid bit index.
#[inline(never)]
pub unsafe fn mi_bitmap_clear(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    unsafe { mi_bitmap_clearN(bitmap, idx, 1) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline]
pub unsafe fn mi_bitmap_popcountN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) -> usize {
    unsafe { BitmapView::from_raw(bitmap).popcount_n(idx, n) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline]
pub unsafe fn mi_bitmap_is_xsetN(
    set: mi_xset_t,
    bitmap: *mut mi_bitmap_t,
    idx: usize,
    n: usize,
) -> bool {
    unsafe { BitmapView::from_raw(bitmap).is_xset_n(set, idx, n) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline(never)]
pub unsafe fn mi_bitmap_is_setN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) -> bool {
    unsafe { mi_bitmap_is_xsetN(MI_BIT_SET, bitmap, idx, n) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline(never)]
pub unsafe fn mi_bitmap_is_clearN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) -> bool {
    unsafe { mi_bitmap_is_xsetN(MI_BIT_CLEAR, bitmap, idx, n) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `idx` must be a valid bit index.
#[inline(never)]
pub unsafe fn mi_bitmap_is_set(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    unsafe { mi_bitmap_is_setN(bitmap, idx, 1) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `idx` must be a valid bit index.
#[inline(never)]
pub unsafe fn mi_bitmap_is_clear(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    unsafe { mi_bitmap_is_clearN(bitmap, idx, 1) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline(never)]
pub unsafe fn mi_bitmap_is_all_clear(bitmap: *mut mi_bitmap_t) -> bool {
    unsafe { mi_bitmap_is_xsetN(MI_BIT_CLEAR, bitmap, 0, mi_bitmap_max_bits(bitmap)) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `idx` must be a valid bit index that will
/// eventually become set by another thread.
#[inline(never)]
pub unsafe fn mi_bitmap_clear_once_set(bitmap: *mut mi_bitmap_t, idx: usize) {
    unsafe {
        crate::internal::mi_debug_assert!(idx < mi_bitmap_max_bits(bitmap));
        let chunk_idx = idx / MI_BCHUNK_BITS;
        let cidx = idx % MI_BCHUNK_BITS;
        crate::internal::mi_debug_assert!(chunk_idx < mi_bitmap_chunk_count(bitmap));
        mi_bchunk_clear_once_set(&*mi_bitmap_chunk(bitmap, chunk_idx), cidx);
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap and `idx` must be valid for a `usize` write.
#[inline(never)]
pub unsafe fn mi_bitmap_bsr(bitmap: *mut mi_bitmap_t, idx: *mut usize) -> bool {
    unsafe {
        let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
        for i in (0..chunkmap_max).rev() {
            let cmap = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            let mut cmap_idx = 0usize;
            if mi_bfield_find_highest_bit(cmap, &mut cmap_idx) {
                let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
                let mut cidx = 0usize;
                if mi_bchunk_bsr(&*mi_bitmap_chunk(bitmap, chunk_idx), &mut cidx) {
                    *idx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                    return true;
                }
            }
        }
        false
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap.
#[inline(never)]
pub unsafe fn mi_bitmap_popcount(bitmap: *mut mi_bitmap_t) -> usize {
    unsafe {
        let mut popcount = 0usize;
        let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
        for i in 0..chunkmap_max {
            let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            let mut cmap_idx = 0usize;
            while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
                let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
                popcount += mi_bchunk_popcount(&*mi_bitmap_chunk(bitmap, chunk_idx));
            }
        }
        popcount
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `visit` must be callable for each set bit.
#[inline(never)]
pub unsafe extern "C" fn _mi_bitmap_forall_set(
    bitmap: *mut mi_bitmap_t,
    visit: mi_forall_set_fun_t,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    unsafe { mi_bitmap_forall_set_typed(bitmap, visit, ArenaPtr::new(arena), arg) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `visit` must be callable for each set bit.
#[inline(never)]
pub(crate) unsafe fn mi_bitmap_forall_set_typed(
    bitmap: *mut mi_bitmap_t,
    visit: mi_forall_set_fun_t,
    arena: Option<ArenaPtr>,
    arg: *mut c_void,
) -> bool {
    unsafe {
        let arena = arena.map(ArenaPtr::as_ptr).unwrap_or(ptr::null_mut());
        let one = SliceCount::ONE;
        let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
        for i in 0..chunkmap_max {
            let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            let mut cmap_idx = 0usize;
            while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
                let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
                let chunk = &*mi_bitmap_chunk(bitmap, chunk_idx);
                for j in 0..MI_BCHUNK_FIELDS {
                    let base_idx = (chunk_idx * MI_BCHUNK_BITS) + (j * MI_BFIELD_BITS);
                    let mut b = chunk.bfields[j].load(Ordering::Relaxed);
                    let mut bidx = 0usize;
                    while mi_bfield_foreach_bit(&mut b, &mut bidx) {
                        let index = SliceIndex::new(base_idx + bidx);
                        if !visit(index.get(), one.get() as usize, arena, arg) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `visit` must be callable for each set range.
#[inline(never)]
pub unsafe extern "C" fn _mi_bitmap_forall_setc_ranges(
    bitmap: *mut mi_bitmap_t,
    visit: mi_forall_set_fun_t,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    unsafe { mi_bitmap_forall_setc_ranges_typed(bitmap, visit, ArenaPtr::new(arena), arg) }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `visit` must be callable for each set range.
#[inline(never)]
pub(crate) unsafe fn mi_bitmap_forall_setc_ranges_typed(
    bitmap: *mut mi_bitmap_t,
    visit: mi_forall_set_fun_t,
    arena: Option<ArenaPtr>,
    arg: *mut c_void,
) -> bool {
    unsafe {
        let arena = arena.map(ArenaPtr::as_ptr).unwrap_or(ptr::null_mut());
        let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
        for i in 0..chunkmap_max {
            let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            let mut cmap_idx = 0usize;
            while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
                let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
                let chunk = &*mi_bitmap_chunk(bitmap, chunk_idx);
                for j in 0..MI_BCHUNK_FIELDS {
                    let base_idx = (chunk_idx * MI_BCHUNK_BITS) + (j * MI_BFIELD_BITS);
                    let mut b = chunk.bfields[j].swap(0, Ordering::Relaxed);
                    let mut rngcount = 0usize;
                    let bpopcount = mi_popcount(b);
                    let mut bidx = 0usize;
                    while mi_bfield_find_least_bit(b, &mut bidx) {
                        let rng = mi_ctz(!(b >> bidx));
                        rngcount += rng;
                        let idx = base_idx + bidx;
                        crate::internal::mi_assert_internal!(u32::try_from(idx).is_ok());
                        crate::internal::mi_assert_internal!(u32::try_from(rng).is_ok());
                        let range =
                            SliceRange::from_raw(idx as u32, rng as u32).unwrap_or_else(|_| {
                                crate::internal::mi_assert_internal!(false);
                                SliceRange::FIRST_SLICE
                            });
                        crate::internal::mi_debug_assert!((1..=MI_BFIELD_BITS).contains(&rng));
                        crate::internal::mi_debug_assert!(
                            (idx % MI_BFIELD_BITS) + rng <= MI_BFIELD_BITS
                        );
                        crate::internal::mi_debug_assert!(
                            (idx / MI_BCHUNK_BITS) < mi_bitmap_chunk_count(bitmap)
                        );
                        if !visit(range.start_usize(), range.count_usize(), arena, arg) {
                            if b != 0 {
                                chunk.bfields[j].fetch_or(b, Ordering::Relaxed);
                            }
                            return false;
                        }
                        b &= !mi_bfield_mask(rng, bidx);
                    }
                    crate::internal::mi_debug_assert!(rngcount == bpopcount);
                }
            }
        }
        true
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `visit` must be callable for each set range.
#[inline(never)]
pub unsafe extern "C" fn _mi_bitmap_forall_setc_rangesn(
    bitmap: *mut mi_bitmap_t,
    rngslices: usize,
    visit: mi_forall_set_fun_t,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    unsafe {
        if rngslices <= 1 {
            return mi_bitmap_forall_setc_ranges_typed(bitmap, visit, ArenaPtr::new(arena), arg);
        }
        let rngslices_raw = if rngslices > MI_BFIELD_BITS {
            MI_BFIELD_BITS
        } else {
            rngslices
        };
        crate::internal::mi_assert_internal!(rngslices_raw > 0);
        let rngslices = SliceCount::new_unchecked(rngslices_raw as u32);
        mi_bitmap_forall_setc_rangesn_typed(bitmap, rngslices, visit, ArenaPtr::new(arena), arg)
    }
}

/// # Safety
/// `bitmap` must point to an initialized bitmap, and `visit` must be callable for each set range.
#[inline(never)]
pub(crate) unsafe fn mi_bitmap_forall_setc_rangesn_typed(
    bitmap: *mut mi_bitmap_t,
    rngslices: SliceCount,
    visit: mi_forall_set_fun_t,
    arena: Option<ArenaPtr>,
    arg: *mut c_void,
) -> bool {
    unsafe {
        let arena = arena.map(ArenaPtr::as_ptr).unwrap_or(ptr::null_mut());
        let rngslices_count = rngslices;
        let rngslices = rngslices_count.get() as usize;
        let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
        for i in 0..chunkmap_max {
            let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            let mut cmap_idx = 0usize;
            while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
                let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
                let chunk = &*mi_bitmap_chunk(bitmap, chunk_idx);
                for j in 0..MI_BCHUNK_FIELDS {
                    let base_idx = (chunk_idx * MI_BCHUNK_BITS) + (j * MI_BFIELD_BITS);
                    let b = chunk.bfields[j].swap(0, Ordering::Relaxed);
                    let mut skipped = 0usize;
                    for shift in (0..=MI_BFIELD_BITS - rngslices).step_by(rngslices) {
                        let rngmask = mi_bfield_mask(rngslices, shift);
                        if (b & rngmask) == rngmask {
                            let idx = base_idx + shift;
                            crate::internal::mi_assert_internal!(u32::try_from(idx).is_ok());
                            let range = SliceRange::from_raw(idx as u32, rngslices_count.get())
                                .unwrap_or_else(|_| {
                                    crate::internal::mi_assert_internal!(false);
                                    SliceRange::FIRST_SLICE
                                });
                            if !visit(range.start_usize(), range.count_usize(), arena, arg) {
                                let mut notyet_visited = 0usize;
                                if shift + rngslices < MI_BFIELD_BITS {
                                    notyet_visited = b & (!0usize << (shift + rngslices));
                                }
                                crate::internal::mi_debug_assert!((notyet_visited & skipped) == 0);
                                if (notyet_visited | skipped) != 0 {
                                    chunk.bfields[j]
                                        .fetch_or(notyet_visited | skipped, Ordering::Relaxed);
                                }
                                return false;
                            }
                        } else {
                            skipped |= b & rngmask;
                        }
                    }
                    if skipped != 0 {
                        chunk.bfields[j].fetch_or(skipped, Ordering::Relaxed);
                    }
                }
            }
        }
        true
    }
}

/// # Safety
/// If non-null, `pchunk_count` must be valid for a `usize` write.
#[inline(never)]
pub unsafe fn mi_bbitmap_size(bit_count: usize, pchunk_count: *mut usize) -> usize {
    unsafe {
        if bit_count == 0 {
            if !pchunk_count.is_null() {
                *pchunk_count = 0;
            }
            return 0;
        }
        let bit_count = crate::internal::_mi_align_up(bit_count, MI_BCHUNK_BITS);
        crate::internal::mi_debug_assert!(bit_count <= MI_BITMAP_MAX_BIT_COUNT);
        let chunk_count = bit_count / MI_BCHUNK_BITS;
        if !pchunk_count.is_null() {
            *pchunk_count = chunk_count;
        }
        (chunk_count + MI_CBIN_COUNT as usize + 1) * MI_BCHUNK_SIZE
    }
}

/// # Safety
/// `bbitmap` must point to enough writable storage for `bit_count` bits rounded up to bitmap
/// chunk granularity. If `already_zero` is true, that storage must already be zeroed.
#[inline(never)]
pub unsafe fn mi_bbitmap_init(
    bbitmap: *mut mi_bbitmap_t,
    bit_count: usize,
    already_zero: bool,
) -> usize {
    unsafe {
        let mut chunk_count = 0usize;
        let size = mi_bbitmap_size(bit_count, &mut chunk_count as *mut usize);
        if !already_zero {
            ptr::write_bytes(bbitmap.cast::<u8>(), 0, size);
        }
        (*bbitmap).chunk_count.store(chunk_count, Ordering::Release);
        crate::internal::mi_debug_assert!(
            mi_bbitmap_chunk_count(bbitmap) <= MI_BITMAP_MAX_CHUNK_COUNT
        );
        size
    }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap, and `idx..idx + n` must be within its bit
/// range.
#[inline(never)]
pub unsafe fn mi_bbitmap_unsafe_setN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) {
    unsafe {
        crate::internal::mi_debug_assert!(n > 0);
        crate::internal::mi_debug_assert!(idx + n <= mi_bbitmap_max_bits(bbitmap));
        mi_bchunks_unsafe_setN(
            mi_bbitmap_chunk(bbitmap, 0).cast_mut(),
            &mut (*bbitmap).chunkmap,
            idx,
            n,
        );
    }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap and `idx` must be valid for a `usize` write.
#[inline(never)]
pub unsafe fn mi_bbitmap_bsr_inv(bbitmap: *mut mi_bbitmap_t, idx: *mut usize) -> bool {
    unsafe {
        let chunk_count = mi_bbitmap_chunk_count(bbitmap);
        let chunkmap_max = chunk_count.div_ceil(MI_BFIELD_BITS);
        let mut skip_at_top = chunk_count % MI_BFIELD_BITS;
        for i in (0..chunkmap_max).rev() {
            let mut cmap = (*bbitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
            if skip_at_top > 0 {
                let mask_top = (!mi_bfield_zero()) << (MI_BFIELD_BITS - skip_at_top);
                skip_at_top = 0;
                cmap |= mask_top;
            }
            let mut cmap_idx = 0usize;
            if mi_bfield_find_highest_bit(!cmap, &mut cmap_idx) {
                let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
                let mut cidx = 0usize;
                if mi_bchunk_bsr_inv(&*mi_bbitmap_chunk(bbitmap, chunk_idx), &mut cidx) {
                    *idx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                    return true;
                }
            }
        }
        false
    }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap.
#[inline]
pub unsafe fn mi_bbitmap_setN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    unsafe { BBitmapView::from_raw(bbitmap).set_n(idx, n) }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap, and `idx..idx + n` must fit within one chunk.
#[inline(never)]
pub unsafe fn mi_bbitmap_try_clearNC(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    unsafe {
        crate::internal::mi_debug_assert!(n > 0);
        crate::internal::mi_debug_assert!(n <= MI_BCHUNK_BITS);
        crate::internal::mi_debug_assert!(idx + n <= mi_bbitmap_max_bits(bbitmap));
        let chunk_idx = idx / MI_BCHUNK_BITS;
        let cidx = idx % MI_BCHUNK_BITS;
        crate::internal::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
        crate::internal::mi_debug_assert!(chunk_idx < mi_bbitmap_chunk_count(bbitmap));
        if cidx + n > MI_BCHUNK_BITS {
            return false;
        }
        let mut maybe_all_clear = false;
        let chunk = &*mi_bbitmap_chunk(bbitmap, chunk_idx);
        let cleared = mi_bchunk_try_clearN(chunk, cidx, n, Some(&mut maybe_all_clear));
        if cleared && maybe_all_clear {
            mi_bbitmap_chunkmap_try_clear(&*bbitmap, chunk_idx, chunk);
        }
        cleared
    }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap.
#[inline]
pub unsafe fn mi_bbitmap_is_xsetN(
    set: mi_xset_t,
    bbitmap: *mut mi_bbitmap_t,
    idx: usize,
    n: usize,
) -> bool {
    unsafe { BBitmapView::from_raw(bbitmap).is_xset_n(set, idx, n) }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap.
#[inline]
pub unsafe fn mi_bbitmap_is_setN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    unsafe { mi_bbitmap_is_xsetN(MI_BIT_SET, bbitmap, idx, n) }
}

/// # Safety
/// `bbitmap` must point to an initialized bbitmap.
#[inline]
pub unsafe fn mi_bbitmap_is_clearN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    unsafe { mi_bbitmap_is_xsetN(MI_BIT_CLEAR, bbitmap, idx, n) }
}

#[inline]
fn mi_bchunk_try_find_and_clear_1(chunk: &mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
    crate::internal::mi_debug_assert!(n == 1);
    mi_bchunk_try_find_and_clear(chunk, pidx)
}

#[inline]
fn mi_bchunk_try_find_and_clear_8(chunk: &mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
    crate::internal::mi_debug_assert!(n == 8);
    mi_bchunk_try_find_and_clear8(chunk, pidx)
}

/// # Safety
/// `bbitmap` must identify initialized stable compact bitmap backing, and
/// `pidx` must be writable for one `usize`, disjoint from that backing.
#[inline]
pub unsafe fn mi_bbitmap_try_find_and_clearN(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
) -> bool {
    unsafe {
        let mut index = 0;
        let found = BBitmapView::from_raw(bbitmap).try_find_and_clear_n(tseq, n, &mut index);
        if found {
            pidx.write(index);
        }
        found
    }
}

type mi_bchunk_xsetNC_fun_t =
    fn(mi_xset_t, &mi_bchunk_t, usize, usize, Option<&mut usize>, Option<&mut bool>) -> bool;

#[cfg(test)]
pub(crate) mod storage_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct VisitLog {
        len: usize,
        entries: [(usize, usize); 8],
    }

    impl VisitLog {
        fn push(&mut self, slice_index: usize, slice_count: usize) {
            assert!(self.len < self.entries.len());
            self.entries[self.len] = (slice_index, slice_count);
            self.len += 1;
        }

        fn as_slice(&self) -> &[(usize, usize)] {
            &self.entries[..self.len]
        }
    }

    unsafe extern "C" fn record_visit(
        slice_index: usize,
        slice_count: usize,
        _arena: *mut mi_arena_t,
        arg: *mut core::ffi::c_void,
    ) -> bool {
        let log = unsafe { &mut *(arg.cast::<VisitLog>()) };
        log.push(slice_index, slice_count);
        true
    }

    #[test]
    fn bitmap_set_clear_tracks_existing_bits() {
        let storage =
            storage_tests::BitmapStorage::new(unsafe { mi_bitmap_size(1024, ptr::null_mut()) });
        let bitmap = storage.ptr::<mi_bitmap_t>();
        unsafe {
            mi_bitmap_init(bitmap, 1024, true);
            assert!(mi_bitmap_is_all_clear(bitmap));

            let mut already_set = usize::MAX;
            assert!(mi_bitmap_setN(bitmap, 10, 3, &mut already_set));
            assert_eq!(already_set, 0);
            assert!(mi_bitmap_is_setN(bitmap, 10, 3));
            assert_eq!(mi_bitmap_popcount(bitmap), 3);

            already_set = 0;
            assert!(!mi_bitmap_setN(bitmap, 11, 2, &mut already_set));
            assert_eq!(already_set, 2);
            assert_eq!(mi_bitmap_popcount(bitmap), 3);

            assert!(mi_bitmap_clearN(bitmap, 11, 2));
            assert!(mi_bitmap_is_set(bitmap, 10));
            assert!(mi_bitmap_is_clearN(bitmap, 11, 2));
            assert_eq!(mi_bitmap_popcount(bitmap), 1);
        }
    }

    #[test]
    fn bitmap_claim_preserves_result_and_bit_disposition() {
        unsafe extern "C" fn accept(
            index: usize,
            arena: *mut mi_arena_t,
            keep_set: *mut bool,
        ) -> bool {
            assert_eq!(index, 7);
            assert!(arena.is_null());
            unsafe { *keep_set = false };
            true
        }

        unsafe extern "C" fn retry(
            _index: usize,
            _arena: *mut mi_arena_t,
            keep_set: *mut bool,
        ) -> bool {
            unsafe { *keep_set = true };
            false
        }

        unsafe extern "C" fn discard(
            _index: usize,
            _arena: *mut mi_arena_t,
            keep_set: *mut bool,
        ) -> bool {
            unsafe { *keep_set = false };
            false
        }

        for (callback, found, keep_set) in [
            (ThreadLocalClaimCallback::new(accept), true, false),
            (ThreadLocalClaimCallback::new(retry), false, true),
            (ThreadLocalClaimCallback::new(discard), false, false),
        ] {
            let storage =
                storage_tests::BitmapStorage::new(unsafe { mi_bitmap_size(1024, ptr::null_mut()) });
            let bitmap = storage.ptr::<mi_bitmap_t>();
            let mut index = SliceIndex::new(99);
            unsafe {
                mi_bitmap_init(bitmap, 1024, true);
                assert!(mi_bitmap_set(bitmap, 7));
                assert_eq!(
                    mi_bitmap_try_find_and_claim(
                        bitmap,
                        ThreadSeq::new(0),
                        &mut index,
                        callback,
                        None,
                    ),
                    found,
                );
                assert_eq!(mi_bitmap_is_set(bitmap, 7), keep_set);
            }
            assert_eq!(index.get(), if found { 7 } else { 99 });
        }
    }

    #[test]
    fn bitmap_traversal_visits_bits_and_ranges() {
        let storage =
            storage_tests::BitmapStorage::new(unsafe { mi_bitmap_size(1024, ptr::null_mut()) });
        let bitmap = storage.ptr::<mi_bitmap_t>();
        let mut visits = VisitLog::default();
        unsafe {
            mi_bitmap_init(bitmap, 1024, true);
            assert!(mi_bitmap_setN(bitmap, 2, 3, core::ptr::null_mut()));
            assert!(_mi_bitmap_forall_set(
                bitmap,
                record_visit,
                core::ptr::null_mut(),
                (&mut visits as *mut VisitLog).cast::<core::ffi::c_void>(),
            ));
        }
        assert_eq!(visits.as_slice(), &[(2, 1), (3, 1), (4, 1)]);

        let range_storage =
            storage_tests::BitmapStorage::new(unsafe { mi_bitmap_size(1024, ptr::null_mut()) });
        let range_bitmap = range_storage.ptr::<mi_bitmap_t>();
        let mut ranges = VisitLog::default();
        unsafe {
            mi_bitmap_init(range_bitmap, 1024, true);
            assert!(mi_bitmap_setN(range_bitmap, 16, 4, core::ptr::null_mut()));
            assert!(_mi_bitmap_forall_setc_ranges(
                range_bitmap,
                record_visit,
                core::ptr::null_mut(),
                (&mut ranges as *mut VisitLog).cast::<core::ffi::c_void>(),
            ));
            assert!(mi_bitmap_is_all_clear(range_bitmap));
        }
        assert_eq!(ranges.as_slice(), &[(16, 4)]);
    }

    #[test]
    fn bbitmap_find_and_clear_claims_one_range() {
        let storage =
            storage_tests::BitmapStorage::new(unsafe { mi_bbitmap_size(1024, ptr::null_mut()) });
        let bitmap = storage.ptr::<mi_bbitmap_t>();
        unsafe {
            mi_bbitmap_init(bitmap, 1024, true);
            assert!(mi_bbitmap_setN(bitmap, 0, 4));

            let mut idx = usize::MAX;
            assert!(mi_bbitmap_try_find_and_clearN(bitmap, 0, 1, &mut idx));
            assert_eq!(idx, 0);

            let remaining = (0..4).filter(|i| mi_bbitmap_is_setN(bitmap, *i, 1)).count();
            assert_eq!(remaining, 3);
        }
    }

    #[test]
    fn bbitmap_find_and_clear_rejects_empty_request() {
        let storage =
            storage_tests::BitmapStorage::new(unsafe { mi_bbitmap_size(1024, ptr::null_mut()) });
        let bitmap = storage.ptr::<mi_bbitmap_t>();
        unsafe {
            mi_bbitmap_init(bitmap, 1024, true);
            assert!(mi_bbitmap_setN(bitmap, 0, 4));

            let mut idx = usize::MAX;
            assert!(!mi_bbitmap_try_find_and_clearN(bitmap, 0, 0, &mut idx));
            assert_eq!(idx, usize::MAX);
            assert!(mi_bbitmap_is_setN(bitmap, 0, 4));
        }
    }
}
