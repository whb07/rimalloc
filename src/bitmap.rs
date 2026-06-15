//! Bitmap, page-map, page-queue, arena metadata

#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    clippy::collapsible_if,
    clippy::if_same_then_else,
    clippy::implicit_saturating_sub,
    clippy::manual_is_multiple_of,
    clippy::manual_range_contains,
    clippy::missing_const_for_fn,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;
use core::hint::spin_loop;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arena::MI_ARENA_BCHUNK_BITS;
use crate::bits::{
    MI_SIZE_BITS, MI_SIZE_SHIFT, MI_SIZE_SIZE, mi_bsf, mi_bsr, mi_clz, mi_ctz, mi_popcount,
};
use crate::stats::{
    MI_CBIN_COUNT, MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_NONE, MI_CBIN_OTHER,
    MI_CBIN_SMALL, mi_chunkbin_t,
};
use crate::types::{MI_ENABLE_LARGE_PAGES, mi_arena_t, mi_claim_fun_t, mi_forall_set_fun_t};

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
#[cfg(target_arch = "x86")]
use core::arch::x86::{
    __m256i, _mm256_cmpeq_epi32, _mm256_set1_epi64x, _mm256_setzero_si256, _mm256_testc_si256,
    _mm256_testz_si256,
};
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m256i, _mm256_cmpeq_epi32, _mm256_set1_epi64x, _mm256_setzero_si256, _mm256_testc_si256,
    _mm256_testz_si256,
};

pub const MI_BITMAP_H: usize = 1;
pub const MI_OPT_SIMD: usize = 0;

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
#[inline]
fn mi_mm256_zero() -> __m256i {
    unsafe { _mm256_setzero_si256() }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
#[inline]
fn mi_mm256_ones() -> __m256i {
    unsafe { _mm256_set1_epi64x(!0) }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
#[inline]
fn mi_mm256_is_ones(vec: __m256i) -> bool {
    unsafe { _mm256_testc_si256(vec, _mm256_cmpeq_epi32(vec, vec)) != 0 }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
#[inline]
fn mi_mm256_is_zero(vec: __m256i) -> bool {
    unsafe { _mm256_testz_si256(vec, vec) != 0 }
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
            pub chunks: [mi_bchunk_t; MI_BITMAP_MAX_CHUNK_COUNT],
        }

        impl Default for mi_bitmap_s {
            fn default() -> Self {
                Self {
                    chunk_count: AtomicUsize::new(0),
                    _padding: [0; MI_BCHUNK_SIZE / MI_SIZE_SIZE - 1],
                    chunkmap: Default::default(),
                    chunks: core::array::from_fn(|_| Default::default()),
                }
            }
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
            pub chunks: [mi_bchunk_t; MI_BITMAP_MAX_CHUNK_COUNT],
        }

        impl Default for mi_bbitmap_s {
            fn default() -> Self {
                Self {
                    chunk_count: AtomicUsize::new(0),
                    chunk_max_accessed: AtomicUsize::new(0),
                    _padding: [0; MI_BCHUNK_SIZE / MI_SIZE_SIZE - 2],
                    chunkmap: Default::default(),
                    chunkmap_bins: core::array::from_fn(|_| Default::default()),
                    chunks: core::array::from_fn(|_| Default::default()),
                }
            }
        }

        pub type mi_bbitmap_t = mi_bbitmap_s;
    };
}

#[cfg(target_pointer_width = "64")]
define_bitmap_types!(64);

#[cfg(target_pointer_width = "32")]
define_bitmap_types!(32);

#[inline]
pub unsafe fn mi_bitmap_chunk_count(bitmap: *const mi_bitmap_t) -> usize {
    (*bitmap).chunk_count.load(Ordering::Relaxed)
}

#[inline]
pub unsafe fn mi_bitmap_max_bits(bitmap: *const mi_bitmap_t) -> usize {
    mi_bitmap_chunk_count(bitmap) * MI_BCHUNK_BITS
}

#[inline]
pub unsafe fn mi_bbitmap_chunk_count(bbitmap: *const mi_bbitmap_t) -> usize {
    (*bbitmap).chunk_count.load(Ordering::Relaxed)
}

#[inline]
pub unsafe fn mi_bbitmap_max_bits(bbitmap: *const mi_bbitmap_t) -> usize {
    mi_bbitmap_chunk_count(bbitmap) * MI_BCHUNK_BITS
}

#[inline]
pub const fn mi_bfield_zero() -> mi_bfield_t {
    0
}

#[inline]
pub const fn mi_bfield_one() -> mi_bfield_t {
    1
}

#[inline]
pub const fn mi_bfield_all_set() -> mi_bfield_t {
    !0usize
}

#[inline]
pub const fn mi_bfield_clear_least_bit(x: mi_bfield_t) -> mi_bfield_t {
    x & x.wrapping_sub(1)
}

#[inline]
pub fn mi_bfield_ctz(x: mi_bfield_t) -> usize {
    mi_ctz(x)
}

#[inline]
pub fn mi_bfield_clz(x: mi_bfield_t) -> usize {
    mi_clz(x)
}

#[inline]
pub fn mi_bfield_popcount(x: mi_bfield_t) -> usize {
    mi_popcount(x)
}

#[inline]
fn mi_bfield_find_least_bit(x: mi_bfield_t, idx: &mut usize) -> bool {
    mi_bsf(x, idx)
}

#[inline]
fn mi_bfield_find_highest_bit(x: mi_bfield_t, idx: &mut usize) -> bool {
    mi_bsr(x, idx)
}

#[inline]
fn mi_bfield_foreach_bit(x: &mut mi_bfield_t, idx: &mut usize) -> bool {
    let found = mi_bfield_find_least_bit(*x, idx);
    *x = mi_bfield_clear_least_bit(*x);
    found
}

#[inline]
fn mi_bfield_mask(bit_count: usize, shiftl: usize) -> mi_bfield_t {
    crate::mi_debug_assert!(bit_count > 0);
    crate::mi_debug_assert!(shiftl < MI_BFIELD_BITS);
    crate::mi_debug_assert!(bit_count + shiftl <= MI_BFIELD_BITS);
    let mask0 = if bit_count < MI_BFIELD_BITS {
        (mi_bfield_one() << bit_count) - 1
    } else {
        mi_bfield_all_set()
    };
    mask0 << shiftl
}

#[inline]
fn mi_bfield_atomic_set(b: &AtomicUsize, idx: usize) -> bool {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let mask = mi_bfield_mask(1, idx);
    let old = b.fetch_or(mask, Ordering::AcqRel);
    (old & mask) == 0
}

#[inline]
fn mi_bfield_atomic_clear(b: &AtomicUsize, idx: usize, all_clear: Option<&mut bool>) -> bool {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let mask = mi_bfield_mask(1, idx);
    let old = b.fetch_and(!mask, Ordering::AcqRel);
    if let Some(all_clear) = all_clear {
        *all_clear = (old & !mask) == 0;
    }
    (old & mask) == mask
}

#[inline]
fn mi_bfield_atomic_clear_once_set(b: &AtomicUsize, idx: usize) {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let mask = mi_bfield_mask(1, idx);
    let mut old = b.load(Ordering::Relaxed);
    loop {
        if (old & mask) == 0 {
            old = b.load(Ordering::Acquire);
            while (old & mask) == 0 {
                spin_loop();
                old = b.load(Ordering::Acquire);
            }
        }
        match b.compare_exchange_weak(old, old & !mask, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(next) => old = next,
        }
    }
}

#[inline]
fn mi_bfield_atomic_set_mask(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    already_set: Option<&mut usize>,
) -> bool {
    crate::mi_debug_assert!(mask != 0);
    let mut old = b.load(Ordering::Relaxed);
    while b
        .compare_exchange_weak(old, old | mask, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        old = b.load(Ordering::Relaxed);
    }
    if let Some(already_set) = already_set {
        *already_set = mi_bfield_popcount(old & mask);
    }
    (old & mask) == 0
}

#[inline]
fn mi_bfield_atomic_clear_mask(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(mask != 0);
    let mut old = b.load(Ordering::Relaxed);
    while b
        .compare_exchange_weak(old, old & !mask, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        old = b.load(Ordering::Relaxed);
    }
    if let Some(all_clear) = all_clear {
        *all_clear = (old & !mask) == 0;
    }
    (old & mask) == mask
}

#[inline]
fn mi_bfield_atomic_setX(b: &AtomicUsize, already_set: Option<&mut usize>) -> bool {
    let old = b.swap(mi_bfield_all_set(), Ordering::Release);
    if let Some(already_set) = already_set {
        *already_set = mi_bfield_popcount(old);
    }
    old == 0
}

#[inline]
fn mi_bfield_atomic_try_clear_mask_of(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    mut expect: mi_bfield_t,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(mask != 0);
    loop {
        if (expect & mask) != mask {
            if let Some(all_clear) = all_clear {
                *all_clear = expect == 0;
            }
            return false;
        }
        match b.compare_exchange_weak(expect, expect & !mask, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                if let Some(all_clear) = all_clear {
                    *all_clear = (expect & !mask) == 0;
                }
                return true;
            }
            Err(actual) => expect = actual,
        }
    }
}

#[inline]
fn mi_bfield_atomic_try_clear_mask(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(mask != 0);
    let expect = b.load(Ordering::Relaxed);
    mi_bfield_atomic_try_clear_mask_of(b, mask, expect, all_clear)
}

#[inline]
fn mi_bfield_atomic_try_clear(b: &AtomicUsize, idx: usize, all_clear: Option<&mut bool>) -> bool {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let mask = mi_bfield_one() << idx;
    mi_bfield_atomic_try_clear_mask(b, mask, all_clear)
}

#[inline]
fn mi_bfield_atomic_try_clear8(b: &AtomicUsize, idx: usize, all_clear: Option<&mut bool>) -> bool {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    crate::mi_debug_assert!((idx % 8) == 0);
    let mask = (0xFFusize) << idx;
    mi_bfield_atomic_try_clear_mask(b, mask, all_clear)
}

#[inline]
fn mi_bfield_atomic_try_clearX(b: &AtomicUsize, all_clear: Option<&mut bool>) -> bool {
    let old = mi_bfield_all_set();
    match b.compare_exchange(old, mi_bfield_zero(), Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            if let Some(all_clear) = all_clear {
                *all_clear = true;
            }
            true
        }
        Err(_) => false,
    }
}

#[inline]
fn mi_bfield_atomic_is_set(b: &AtomicUsize, idx: usize) -> bool {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let x = b.load(Ordering::Acquire);
    (x & mi_bfield_mask(1, idx)) != 0
}

#[inline]
fn mi_bfield_atomic_is_clear(b: &AtomicUsize, idx: usize) -> bool {
    crate::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let x = b.load(Ordering::Acquire);
    (x & mi_bfield_mask(1, idx)) == 0
}

#[inline]
fn mi_bfield_atomic_is_xset(set: mi_xset_t, b: &AtomicUsize, idx: usize) -> bool {
    if set {
        mi_bfield_atomic_is_set(b, idx)
    } else {
        mi_bfield_atomic_is_clear(b, idx)
    }
}

#[inline]
fn mi_bfield_atomic_is_set_mask(b: &AtomicUsize, mask: mi_bfield_t) -> bool {
    crate::mi_debug_assert!(mask != 0);
    let x = b.load(Ordering::Acquire);
    (x & mask) == mask
}

#[inline]
fn mi_bfield_atomic_is_clear_mask(b: &AtomicUsize, mask: mi_bfield_t) -> bool {
    crate::mi_debug_assert!(mask != 0);
    let x = b.load(Ordering::Acquire);
    (x & mask) == 0
}

#[inline]
fn mi_bfield_atomic_is_xset_mask(set: mi_xset_t, b: &AtomicUsize, mask: mi_bfield_t) -> bool {
    if set {
        mi_bfield_atomic_is_set_mask(b, mask)
    } else {
        mi_bfield_atomic_is_clear_mask(b, mask)
    }
}

#[inline]
fn mi_bfield_atomic_popcount_mask(b: &AtomicUsize, mask: mi_bfield_t) -> usize {
    let x = b.load(Ordering::Acquire);
    mi_bfield_popcount(x & mask)
}

#[inline]
fn mi_bchunk_set(chunk: &mut mi_bchunk_t, cidx: usize, already_set: Option<&mut usize>) -> bool {
    crate::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
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
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    n: usize,
    already_set: Option<&mut usize>,
) -> bool {
    crate::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    crate::mi_debug_assert!(n > 0 && n <= MI_BFIELD_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    if idx + n <= MI_BFIELD_BITS {
        mi_bfield_atomic_set_mask(&chunk.bfields[i], mi_bfield_mask(n, idx), already_set)
    } else {
        let m = MI_BFIELD_BITS - idx;
        crate::mi_debug_assert!(m < n);
        crate::mi_debug_assert!(i < MI_BCHUNK_FIELDS - 1);
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
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    mut n: usize,
    palready_set: Option<&mut usize>,
    mut pmaybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::mi_debug_assert!(n > 0);
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
        crate::mi_debug_assert!(idx + m <= MI_BFIELD_BITS);
        crate::mi_debug_assert!(field < MI_BCHUNK_FIELDS);
        let mask = mi_bfield_mask(m, idx);
        let mut already_set = 0usize;
        let mut all_clear = false;
        let transition = if set {
            mi_bfield_atomic_set_mask(&chunk.bfields[field], mask, Some(&mut already_set))
        } else {
            mi_bfield_atomic_clear_mask(&chunk.bfields[field], mask, Some(&mut all_clear))
        };
        crate::mi_debug_assert!(
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
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    n: usize,
    already_set: Option<&mut usize>,
) -> bool {
    crate::mi_debug_assert!(n > 0 && n <= MI_BCHUNK_BITS);
    if n == 1 {
        mi_bchunk_set(chunk, cidx, already_set)
    } else if n <= MI_BFIELD_BITS {
        mi_bchunk_setNX(chunk, cidx, n, already_set)
    } else {
        mi_bchunk_xsetNC(MI_BIT_SET, chunk, cidx, n, already_set, None)
    }
}

#[inline]
fn mi_bchunk_clear(chunk: &mut mi_bchunk_t, cidx: usize, all_clear: Option<&mut bool>) -> bool {
    crate::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    mi_bfield_atomic_clear(&chunk.bfields[i], idx, all_clear)
}

#[inline]
fn mi_bchunk_clearN(
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    n: usize,
    maybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(n > 0 && n <= MI_BCHUNK_BITS);
    if n == 1 {
        mi_bchunk_clear(chunk, cidx, maybe_all_clear)
    } else if n <= MI_BFIELD_BITS {
        mi_bchunk_xsetNC(MI_BIT_CLEAR, chunk, cidx, n, None, maybe_all_clear)
    } else {
        mi_bchunk_xsetNC(MI_BIT_CLEAR, chunk, cidx, n, None, maybe_all_clear)
    }
}

#[inline(never)]
fn mi_bchunk_popcountNC(
    chunk: &mi_bchunk_t,
    mut field_idx: usize,
    mut idx: usize,
    mut n: usize,
) -> usize {
    crate::mi_debug_assert!((field_idx * MI_BFIELD_BITS) + idx + n <= MI_BCHUNK_BITS);
    let mut count = 0usize;
    while n > 0 {
        let mut m = MI_BFIELD_BITS - idx;
        if m > n {
            m = n;
        }
        crate::mi_debug_assert!(idx + m <= MI_BFIELD_BITS);
        crate::mi_debug_assert!(field_idx < MI_BCHUNK_FIELDS);
        let mask = mi_bfield_mask(m, idx);
        count += mi_bfield_atomic_popcount_mask(&chunk.bfields[field_idx], mask);
        field_idx += 1;
        idx = 0;
        n -= m;
    }
    count
}

#[inline]
fn mi_bchunk_popcountN(chunk: &mi_bchunk_t, cidx: usize, n: usize) -> usize {
    crate::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::mi_debug_assert!(n > 0);
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

#[inline(never)]
fn mi_bchunk_is_xsetNC(
    set: mi_xset_t,
    chunk: &mi_bchunk_t,
    mut field_idx: usize,
    mut idx: usize,
    mut n: usize,
) -> bool {
    crate::mi_debug_assert!((field_idx * MI_BFIELD_BITS) + idx + n <= MI_BCHUNK_BITS);
    while n > 0 {
        let mut m = MI_BFIELD_BITS - idx;
        if m > n {
            m = n;
        }
        crate::mi_debug_assert!(idx + m <= MI_BFIELD_BITS);
        crate::mi_debug_assert!(field_idx < MI_BCHUNK_FIELDS);
        let mask = mi_bfield_mask(m, idx);
        if !mi_bfield_atomic_is_xset_mask(set, &chunk.bfields[field_idx], mask) {
            return false;
        }
        field_idx += 1;
        idx = 0;
        n -= m;
    }
    true
}

#[inline]
fn mi_bchunk_is_xsetN(set: mi_xset_t, chunk: &mi_bchunk_t, cidx: usize, n: usize) -> bool {
    crate::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::mi_debug_assert!(n > 0);
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
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    n: usize,
    pmaybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
    crate::mi_debug_assert!(n <= MI_BFIELD_BITS);
    let i = cidx / MI_BFIELD_BITS;
    let idx = cidx % MI_BFIELD_BITS;
    if idx + n <= MI_BFIELD_BITS {
        mi_bfield_atomic_try_clear_mask(&chunk.bfields[i], mi_bfield_mask(n, idx), pmaybe_all_clear)
    } else {
        let m = MI_BFIELD_BITS - idx;
        crate::mi_debug_assert!(m < n);
        crate::mi_debug_assert!(i < MI_BCHUNK_FIELDS - 1);
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
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    mut n: usize,
    mut pmaybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::mi_debug_assert!(n > 0);
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
    crate::mi_debug_assert!(start_idx + m <= MI_BFIELD_BITS);
    crate::mi_debug_assert!(start_field < MI_BCHUNK_FIELDS);
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
        crate::mi_debug_assert!(field < MI_BCHUNK_FIELDS);
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
        crate::mi_debug_assert!(field < MI_BCHUNK_FIELDS);
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
    chunk: &mut mi_bchunk_t,
    cidx: usize,
    n: usize,
    maybe_all_clear: Option<&mut bool>,
) -> bool {
    crate::mi_debug_assert!(n > 0);
    if n <= MI_BFIELD_BITS {
        mi_bchunk_try_clearNX(chunk, cidx, n, maybe_all_clear)
    } else {
        mi_bchunk_try_clearNC(chunk, cidx, n, maybe_all_clear)
    }
}

#[inline]
fn mi_bchunk_try_find_and_clear_at(
    chunk: &mut mi_bchunk_t,
    chunk_idx: usize,
    pidx: &mut usize,
) -> bool {
    crate::mi_debug_assert!(chunk_idx < MI_BCHUNK_FIELDS);
    let b = chunk.bfields[chunk_idx].load(Ordering::Acquire);
    let mut idx = 0usize;
    if mi_bfield_find_least_bit(b, &mut idx) {
        if mi_bfield_atomic_try_clear_mask_of(
            &chunk.bfields[chunk_idx],
            mi_bfield_mask(1, idx),
            b,
            None,
        ) {
            *pidx = (chunk_idx * MI_BFIELD_BITS) + idx;
            crate::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
            return true;
        }
    }
    false
}

#[inline]
fn mi_bchunk_try_find_and_clear(chunk: &mut mi_bchunk_t, pidx: &mut usize) -> bool {
    for i in 0..MI_BCHUNK_FIELDS {
        if mi_bchunk_try_find_and_clear_at(chunk, i, pidx) {
            return true;
        }
    }
    false
}

#[inline]
fn mi_bchunk_try_find_and_clear8_at(
    chunk: &mut mi_bchunk_t,
    chunk_idx: usize,
    pidx: &mut usize,
) -> bool {
    let b = chunk.bfields[chunk_idx].load(Ordering::Relaxed);
    let has_set8 = (((!b).wrapping_sub(MI_BFIELD_LO_BIT8)) & (b & MI_BFIELD_HI_BIT8)) >> 7;
    let mut idx = 0usize;
    if mi_bfield_find_least_bit(has_set8, &mut idx) {
        crate::mi_debug_assert!(idx <= MI_BFIELD_BITS - 8);
        crate::mi_debug_assert!((idx % 8) == 0);
        if mi_bfield_atomic_try_clear_mask_of(&chunk.bfields[chunk_idx], 0xFFusize << idx, b, None)
        {
            *pidx = (chunk_idx * MI_BFIELD_BITS) + idx;
            crate::mi_debug_assert!(*pidx + 8 <= MI_BCHUNK_BITS);
            return true;
        }
    }
    false
}

#[inline(never)]
fn mi_bchunk_try_find_and_clear8(chunk: &mut mi_bchunk_t, pidx: &mut usize) -> bool {
    for i in 0..MI_BCHUNK_FIELDS {
        if mi_bchunk_try_find_and_clear8_at(chunk, i, pidx) {
            return true;
        }
    }
    false
}

#[inline(never)]
fn mi_bchunk_try_find_and_clearNX(chunk: &mut mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
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
            crate::mi_debug_assert!(bmask >> idx == mask);
            if (b & bmask) == bmask {
                if mi_bfield_atomic_try_clear_mask_of(&chunk.bfields[i], bmask, b0, None) {
                    *pidx = (i * MI_BFIELD_BITS) + idx;
                    crate::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
                    crate::mi_debug_assert!(*pidx + n <= MI_BCHUNK_BITS);
                    return true;
                } else {
                    b0 = chunk.bfields[i].load(Ordering::Acquire);
                    b = b0;
                }
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
                        crate::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
                        crate::mi_debug_assert!(*pidx + n <= MI_BCHUNK_BITS);
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[inline(never)]
fn mi_bchunk_try_find_and_clearNC(chunk: &mut mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
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
            crate::mi_debug_assert!(*pidx < MI_BCHUNK_BITS);
            crate::mi_debug_assert!(*pidx + n <= MI_BCHUNK_BITS);
            return true;
        }
        i += 1;
    }
    false
}

#[inline]
fn mi_bchunk_clear_once_set(chunk: &mut mi_bchunk_t, cidx: usize) {
    crate::mi_debug_assert!(cidx < MI_BCHUNK_BITS);
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
fn mi_bitmap_chunkmap_set(bitmap: &mut mi_bitmap_t, chunk_idx: usize) {
    crate::mi_debug_assert!(unsafe { mi_bitmap_chunk_count(bitmap) } > chunk_idx);
    mi_bchunk_set(&mut bitmap.chunkmap, chunk_idx, None);
}

#[inline(never)]
fn mi_bitmap_chunkmap_try_clear(bitmap: &mut mi_bitmap_t, chunk_idx: usize) -> bool {
    crate::mi_debug_assert!(unsafe { mi_bitmap_chunk_count(bitmap) } > chunk_idx);
    if !mi_bchunk_all_are_clear_relaxed(&bitmap.chunks[chunk_idx]) {
        return false;
    }
    mi_bchunk_clear(&mut bitmap.chunkmap, chunk_idx, None);
    if !mi_bchunk_all_are_clear_relaxed(&bitmap.chunks[chunk_idx]) {
        mi_bchunk_set(&mut bitmap.chunkmap, chunk_idx, None);
        return false;
    }
    true
}

#[inline(never)]
fn mi_bbitmap_set_chunk_bin(bbitmap: &mut mi_bbitmap_t, chunk_idx: usize, bin: mi_chunkbin_t) {
    crate::mi_debug_assert!(unsafe { mi_bbitmap_chunk_count(bbitmap) } > chunk_idx);
    let mut ibin = MI_CBIN_SMALL;
    while (ibin as i32) < (MI_CBIN_NONE as i32) {
        if ibin == bin {
            mi_bchunk_set(&mut bbitmap.chunkmap_bins[ibin as usize], chunk_idx, None);
        } else {
            mi_bchunk_clear(&mut bbitmap.chunkmap_bins[ibin as usize], chunk_idx, None);
        }
        ibin = mi_chunkbin_inc(ibin);
    }
}

#[inline]
fn mi_bbitmap_chunkmap_set_max(bbitmap: &mut mi_bbitmap_t, chunk_idx: usize) {
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

#[inline]
fn mi_bbitmap_chunkmap_set(bbitmap: &mut mi_bbitmap_t, chunk_idx: usize, check_all_set: bool) {
    crate::mi_debug_assert!(unsafe { mi_bbitmap_chunk_count(bbitmap) } > chunk_idx);
    if check_all_set && mi_bchunk_all_are_set_relaxed(&bbitmap.chunks[chunk_idx]) {
        mi_bbitmap_set_chunk_bin(bbitmap, chunk_idx, MI_CBIN_NONE);
    }
    mi_bchunk_set(&mut bbitmap.chunkmap, chunk_idx, None);
    mi_bbitmap_chunkmap_set_max(bbitmap, chunk_idx);
}

#[inline(never)]
fn mi_bbitmap_chunkmap_try_clear(bbitmap: &mut mi_bbitmap_t, chunk_idx: usize) -> bool {
    crate::mi_debug_assert!(unsafe { mi_bbitmap_chunk_count(bbitmap) } > chunk_idx);
    if !mi_bchunk_all_are_clear_relaxed(&bbitmap.chunks[chunk_idx]) {
        return false;
    }
    mi_bchunk_clear(&mut bbitmap.chunkmap, chunk_idx, None);
    if !mi_bchunk_all_are_clear_relaxed(&bbitmap.chunks[chunk_idx]) {
        mi_bchunk_set(&mut bbitmap.chunkmap, chunk_idx, None);
        return false;
    }
    mi_bbitmap_chunkmap_set_max(bbitmap, chunk_idx);
    true
}

#[inline]
fn mi_bfield_cycle_iterate<F>(bfield: mi_bfield_t, tseq: usize, cycle: usize, mut visit: F) -> bool
where
    F: FnMut(usize) -> bool,
{
    crate::mi_debug_assert!(cycle > 0);
    crate::mi_debug_assert!(cycle <= MI_BFIELD_BITS);
    let start = (tseq as u32 as usize) % cycle;
    crate::mi_debug_assert!(start <= cycle);
    let cycle_mask = mi_bfield_mask(cycle - start, start);
    let mut bcount = mi_bfield_popcount(bfield);
    let mut b = bfield & cycle_mask;
    while bcount > 0 {
        bcount -= 1;
        if b == 0 {
            b = bfield & !cycle_mask;
        }
        let mut name_idx = 0usize;
        let found = mi_bfield_find_least_bit(b, &mut name_idx);
        b = mi_bfield_clear_least_bit(b);
        crate::mi_debug_assert!(found);
        if !visit(name_idx) {
            return false;
        }
    }
    true
}

type mi_bitmap_visit_fun_t =
    unsafe fn(*mut mi_bitmap_t, usize, usize, *mut usize, *mut c_void, *mut c_void) -> bool;

#[inline]
unsafe fn mi_bitmap_find(
    bitmap: *mut mi_bitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
    on_find: mi_bitmap_visit_fun_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
    for i in 0..chunkmap_max {
        let cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
        let mut hi = 0usize;
        if mi_bfield_find_highest_bit(cmap_entry, &mut hi) {
            let mut found = false;
            let _ = mi_bfield_cycle_iterate(cmap_entry, tseq % 8, hi + 1, |eidx| {
                let chunk_idx = (i * MI_BFIELD_BITS) + eidx;
                crate::mi_debug_assert!(chunk_idx < unsafe { mi_bitmap_chunk_count(bitmap) });
                if on_find(bitmap, chunk_idx, n, pidx, arg1, arg2) {
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

#[inline]
unsafe fn mi_bitmap_try_find_and_claim_visit(
    bitmap: *mut mi_bitmap_t,
    chunk_idx: usize,
    n: usize,
    pidx: *mut usize,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    crate::mi_debug_assert!(n == 1);
    let claim_fun = arg1 as *mut mi_claim_fun_t;
    let claim_data = &mut *(arg2 as *mut mi_claim_fun_data_t);
    let chunk = &mut (*bitmap).chunks[chunk_idx];
    let mut cidx = 0usize;
    if mi_bchunk_try_find_and_clear(chunk, &mut cidx) {
        let slice_index = (chunk_idx * MI_BCHUNK_BITS) + cidx;
        crate::mi_debug_assert!(slice_index < unsafe { mi_bitmap_max_bits(bitmap) });
        let mut keep_set = true;
        if (*claim_fun)(slice_index, claim_data.arena, &mut keep_set as *mut bool) {
            crate::mi_debug_assert!(!keep_set);
            *pidx = slice_index;
            return true;
        }
        if keep_set {
            let wasclear = mi_bchunk_set(chunk, cidx, None);
            crate::mi_debug_assert!(wasclear);
        }
    } else {
        mi_bitmap_chunkmap_try_clear(&mut *bitmap, chunk_idx);
    }
    false
}

#[inline(never)]
pub unsafe fn mi_bitmap_try_find_and_claim(
    bitmap: *mut mi_bitmap_t,
    tseq: usize,
    pidx: *mut usize,
    claim: *mut mi_claim_fun_t,
    arena: *mut mi_arena_t,
) -> bool {
    let claim_data = mi_claim_fun_data_t { arena };
    mi_bitmap_find(
        bitmap,
        tseq,
        1,
        pidx,
        mi_bitmap_try_find_and_claim_visit,
        claim.cast::<c_void>(),
        &claim_data as *const mi_claim_fun_data_t as *mut c_void,
    )
}

#[derive(Debug)]
struct mi_claim_fun_data_t {
    arena: *mut mi_arena_t,
}

#[inline]
fn mi_chunkbin_inc(bbin: mi_chunkbin_t) -> mi_chunkbin_t {
    crate::mi_debug_assert!((bbin as i32) < (MI_CBIN_COUNT as i32));
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
fn mi_chunkbin_dec(bbin: mi_chunkbin_t) -> mi_chunkbin_t {
    crate::mi_debug_assert!((bbin as i32) > (MI_CBIN_NONE as i32));
    match bbin {
        MI_CBIN_NONE => MI_CBIN_HUGE,
        MI_CBIN_HUGE => MI_CBIN_LARGE,
        MI_CBIN_LARGE => MI_CBIN_MEDIUM,
        MI_CBIN_MEDIUM => MI_CBIN_OTHER,
        MI_CBIN_OTHER => MI_CBIN_SMALL,
        MI_CBIN_SMALL | MI_CBIN_COUNT => unreachable!(),
    }
}

#[inline]
fn mi_chunkbin_of(slice_count: usize) -> mi_chunkbin_t {
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

#[inline(never)]
pub unsafe fn mi_bbitmap_debug_get_bin(
    chunkmap_bins: *const mi_bchunk_t,
    chunk_idx: usize,
) -> mi_chunkbin_t {
    let chunkmap_bins = core::slice::from_raw_parts(chunkmap_bins, MI_CBIN_COUNT as usize - 1);
    let mut ibin = MI_CBIN_SMALL;
    while (ibin as i32) < (MI_CBIN_NONE as i32) {
        if mi_bchunk_is_xsetN(MI_BIT_SET, &chunkmap_bins[ibin as usize], chunk_idx, 1) {
            return ibin;
        }
        ibin = mi_chunkbin_inc(ibin);
    }
    MI_CBIN_NONE
}

#[inline(never)]
pub unsafe fn mi_bitmap_size(bit_count: usize, pchunk_count: *mut usize) -> usize {
    if bit_count == 0 {
        if !pchunk_count.is_null() {
            *pchunk_count = 0;
        }
        return 0;
    }
    let bit_count = bit_count.div_ceil(MI_BCHUNK_BITS) * MI_BCHUNK_BITS;
    crate::mi_debug_assert!(bit_count <= MI_BITMAP_MAX_BIT_COUNT);
    let chunk_count = bit_count / MI_BCHUNK_BITS;
    if !pchunk_count.is_null() {
        *pchunk_count = chunk_count;
    }
    (chunk_count + 2) * MI_BCHUNK_SIZE
}

#[inline(never)]
pub unsafe fn mi_bitmap_init(
    bitmap: *mut mi_bitmap_t,
    bit_count: usize,
    already_zero: bool,
) -> usize {
    let mut chunk_count = 0usize;
    let size = mi_bitmap_size(bit_count, &mut chunk_count as *mut usize);
    if !already_zero {
        ptr::write_bytes(bitmap.cast::<u8>(), 0, size);
    }
    (*bitmap).chunk_count.store(chunk_count, Ordering::Release);
    crate::mi_debug_assert!(unsafe { mi_bitmap_chunk_count(bitmap) } <= MI_BITMAP_MAX_CHUNK_COUNT);
    size
}

#[inline]
fn mi_bchunks_unsafe_setN(
    chunks: *mut mi_bchunk_t,
    cmap: *mut mi_bchunk_t,
    idx: usize,
    mut n: usize,
) {
    crate::mi_debug_assert!(n > 0);
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let cidx = idx % MI_BCHUNK_BITS;
    let ccount = n.div_ceil(MI_BCHUNK_BITS);
    mi_bchunk_setN(unsafe { &mut *cmap.add(0) }, chunk_idx, ccount, None);
    let mut m = MI_BCHUNK_BITS - cidx;
    if m > n {
        m = n;
    }
    mi_bchunk_setN(unsafe { &mut *chunks.add(chunk_idx) }, cidx, m, None);
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
        crate::mi_debug_assert!(n < MI_BCHUNK_BITS);
        mi_bchunk_setN(unsafe { &mut *chunks.add(chunk_idx) }, 0, n, None);
    }
}

#[inline(never)]
pub unsafe fn mi_bitmap_unsafe_setN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) {
    crate::mi_debug_assert!(n > 0);
    crate::mi_debug_assert!(idx + n <= unsafe { mi_bitmap_max_bits(bitmap) });
    mi_bchunks_unsafe_setN(
        (*bitmap).chunks.as_mut_ptr(),
        &mut (*bitmap).chunkmap,
        idx,
        n,
    );
}

#[inline(never)]
pub unsafe fn mi_bitmap_setN(
    bitmap: *mut mi_bitmap_t,
    idx: usize,
    mut n: usize,
    palready_set: *mut usize,
) -> bool {
    crate::mi_debug_assert!(n > 0);
    let maxbits = mi_bitmap_max_bits(bitmap);
    crate::mi_debug_assert!(idx + n <= maxbits);
    if idx + n > maxbits {
        if idx >= maxbits {
            return false;
        }
        n = maxbits - idx;
    }
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let mut cidx = idx % MI_BCHUNK_BITS;
    let mut were_allclear = true;
    let mut already_set = 0usize;
    while n > 0 {
        let m = if cidx + n > MI_BCHUNK_BITS {
            MI_BCHUNK_BITS - cidx
        } else {
            n
        };
        let mut _already_set = 0usize;
        were_allclear &= mi_bchunk_setN(
            &mut (*bitmap).chunks[chunk_idx],
            cidx,
            m,
            Some(&mut _already_set),
        );
        already_set += _already_set;
        mi_bitmap_chunkmap_set(&mut *bitmap, chunk_idx);
        n -= m;
        cidx = 0;
        chunk_idx += 1;
    }
    if !palready_set.is_null() {
        *palready_set = already_set;
    }
    were_allclear
}

#[inline(never)]
pub unsafe fn mi_bitmap_clearN(bitmap: *mut mi_bitmap_t, idx: usize, mut n: usize) -> bool {
    crate::mi_debug_assert!(n > 0);
    let maxbits = mi_bitmap_max_bits(bitmap);
    crate::mi_debug_assert!(idx + n <= maxbits);
    if idx + n > maxbits {
        if idx >= maxbits {
            return false;
        }
        n = maxbits - idx;
    }
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let mut cidx = idx % MI_BCHUNK_BITS;
    let mut were_allset = true;
    while n > 0 {
        let m = if cidx + n > MI_BCHUNK_BITS {
            MI_BCHUNK_BITS - cidx
        } else {
            n
        };
        let mut maybe_all_clear = false;
        were_allset &= mi_bchunk_clearN(
            &mut (*bitmap).chunks[chunk_idx],
            cidx,
            m,
            Some(&mut maybe_all_clear),
        );
        if maybe_all_clear {
            mi_bitmap_chunkmap_try_clear(&mut *bitmap, chunk_idx);
        }
        n -= m;
        cidx = 0;
        chunk_idx += 1;
    }
    were_allset
}

#[inline(never)]
pub unsafe fn mi_bitmap_set(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    mi_bitmap_setN(bitmap, idx, 1, core::ptr::null_mut())
}

#[inline(never)]
pub unsafe fn mi_bitmap_clear(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    mi_bitmap_clearN(bitmap, idx, 1)
}

#[inline(never)]
pub unsafe fn mi_bitmap_popcountN(bitmap: *mut mi_bitmap_t, idx: usize, mut n: usize) -> usize {
    crate::mi_debug_assert!(n > 0);
    let maxbits = mi_bitmap_max_bits(bitmap);
    crate::mi_debug_assert!(idx + n <= maxbits);
    if idx + n > maxbits {
        if idx >= maxbits {
            return 0;
        }
        n = maxbits - idx;
    }
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let mut cidx = idx % MI_BCHUNK_BITS;
    let mut popcount = 0usize;
    while n > 0 {
        let m = if cidx + n > MI_BCHUNK_BITS {
            MI_BCHUNK_BITS - cidx
        } else {
            n
        };
        popcount += mi_bchunk_popcountN(&(*bitmap).chunks[chunk_idx], cidx, m);
        n -= m;
        cidx = 0;
        chunk_idx += 1;
    }
    popcount
}

#[inline(never)]
pub unsafe fn mi_bitmap_is_xsetN(
    set: mi_xset_t,
    bitmap: *mut mi_bitmap_t,
    idx: usize,
    mut n: usize,
) -> bool {
    crate::mi_debug_assert!(n > 0);
    let maxbits = mi_bitmap_max_bits(bitmap);
    crate::mi_debug_assert!(idx + n <= maxbits);
    if idx + n > maxbits {
        if idx >= maxbits {
            return false;
        }
        n = maxbits - idx;
    }
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let mut cidx = idx % MI_BCHUNK_BITS;
    let mut xset = true;
    while n > 0 && xset {
        let m = if cidx + n > MI_BCHUNK_BITS {
            MI_BCHUNK_BITS - cidx
        } else {
            n
        };
        xset &= mi_bchunk_is_xsetN(set, &(*bitmap).chunks[chunk_idx], cidx, m);
        n -= m;
        cidx = 0;
        chunk_idx += 1;
    }
    xset
}

#[inline(never)]
pub unsafe fn mi_bitmap_is_setN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) -> bool {
    mi_bitmap_is_xsetN(MI_BIT_SET, bitmap, idx, n)
}

#[inline(never)]
pub unsafe fn mi_bitmap_is_clearN(bitmap: *mut mi_bitmap_t, idx: usize, n: usize) -> bool {
    mi_bitmap_is_xsetN(MI_BIT_CLEAR, bitmap, idx, n)
}

#[inline(never)]
pub unsafe fn mi_bitmap_is_set(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    mi_bitmap_is_setN(bitmap, idx, 1)
}

#[inline(never)]
pub unsafe fn mi_bitmap_is_clear(bitmap: *mut mi_bitmap_t, idx: usize) -> bool {
    mi_bitmap_is_clearN(bitmap, idx, 1)
}

#[inline(never)]
pub unsafe fn mi_bitmap_is_all_clear(bitmap: *mut mi_bitmap_t) -> bool {
    mi_bitmap_is_xsetN(MI_BIT_CLEAR, bitmap, 0, mi_bitmap_max_bits(bitmap))
}

#[inline(never)]
pub unsafe fn mi_bitmap_clear_once_set(bitmap: *mut mi_bitmap_t, idx: usize) {
    crate::mi_debug_assert!(idx < unsafe { mi_bitmap_max_bits(bitmap) });
    let chunk_idx = idx / MI_BCHUNK_BITS;
    let cidx = idx % MI_BCHUNK_BITS;
    crate::mi_debug_assert!(chunk_idx < unsafe { mi_bitmap_chunk_count(bitmap) });
    mi_bchunk_clear_once_set(&mut (*bitmap).chunks[chunk_idx], cidx);
}

#[inline(never)]
pub unsafe fn mi_bitmap_bsr(bitmap: *mut mi_bitmap_t, idx: *mut usize) -> bool {
    let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
    for i in (0..chunkmap_max).rev() {
        let cmap = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
        let mut cmap_idx = 0usize;
        if mi_bfield_find_highest_bit(cmap, &mut cmap_idx) {
            let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
            let mut cidx = 0usize;
            if mi_bchunk_bsr(&(*bitmap).chunks[chunk_idx], &mut cidx) {
                *idx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                return true;
            }
        }
    }
    false
}

#[inline(never)]
pub unsafe fn mi_bitmap_popcount(bitmap: *mut mi_bitmap_t) -> usize {
    let mut popcount = 0usize;
    let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
    for i in 0..chunkmap_max {
        let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
        let mut cmap_idx = 0usize;
        while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
            let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
            popcount += mi_bchunk_popcount(&(*bitmap).chunks[chunk_idx]);
        }
    }
    popcount
}

#[used]
static MI_BITMAP_TRY_FIND_AND_CLAIM_KEEP: unsafe fn(
    *mut mi_bitmap_t,
    usize,
    *mut usize,
    *mut mi_claim_fun_t,
    *mut mi_arena_t,
) -> bool = mi_bitmap_try_find_and_claim;
#[used]
static MI_BITMAP_SIZE_KEEP: unsafe fn(usize, *mut usize) -> usize = mi_bitmap_size;
#[used]
static MI_BITMAP_INIT_KEEP: unsafe fn(*mut mi_bitmap_t, usize, bool) -> usize = mi_bitmap_init;
#[used]
static MI_BITMAP_UNSAFE_SETN_KEEP: unsafe fn(*mut mi_bitmap_t, usize, usize) =
    mi_bitmap_unsafe_setN;
#[used]
static MI_BITMAP_SETN_KEEP: unsafe fn(*mut mi_bitmap_t, usize, usize, *mut usize) -> bool =
    mi_bitmap_setN;
#[used]
static MI_BITMAP_CLEARN_KEEP: unsafe fn(*mut mi_bitmap_t, usize, usize) -> bool = mi_bitmap_clearN;
#[used]
static MI_BITMAP_SET_KEEP: unsafe fn(*mut mi_bitmap_t, usize) -> bool = mi_bitmap_set;
#[used]
static MI_BITMAP_CLEAR_KEEP: unsafe fn(*mut mi_bitmap_t, usize) -> bool = mi_bitmap_clear;
#[used]
static MI_BITMAP_POPCOUNTN_KEEP: unsafe fn(*mut mi_bitmap_t, usize, usize) -> usize =
    mi_bitmap_popcountN;
#[used]
static MI_BITMAP_IS_XSETN_KEEP: unsafe fn(mi_xset_t, *mut mi_bitmap_t, usize, usize) -> bool =
    mi_bitmap_is_xsetN;
#[used]
static MI_BITMAP_IS_SETN_KEEP: unsafe fn(*mut mi_bitmap_t, usize, usize) -> bool =
    mi_bitmap_is_setN;
#[used]
static MI_BITMAP_IS_CLEARN_KEEP: unsafe fn(*mut mi_bitmap_t, usize, usize) -> bool =
    mi_bitmap_is_clearN;
#[used]
static MI_BITMAP_IS_SET_KEEP: unsafe fn(*mut mi_bitmap_t, usize) -> bool = mi_bitmap_is_set;
#[used]
static MI_BITMAP_IS_CLEAR_KEEP: unsafe fn(*mut mi_bitmap_t, usize) -> bool = mi_bitmap_is_clear;
#[used]
static MI_BITMAP_IS_ALL_CLEAR_KEEP: unsafe fn(*mut mi_bitmap_t) -> bool = mi_bitmap_is_all_clear;
#[used]
static MI_BITMAP_CLEAR_ONCE_SET_KEEP: unsafe fn(*mut mi_bitmap_t, usize) = mi_bitmap_clear_once_set;
#[used]
static MI_BITMAP_BSR_KEEP: unsafe fn(*mut mi_bitmap_t, *mut usize) -> bool = mi_bitmap_bsr;
#[used]
static MI_BITMAP_POPCOUNT_KEEP: unsafe fn(*mut mi_bitmap_t) -> usize = mi_bitmap_popcount;

#[inline(never)]
pub unsafe extern "C" fn _mi_bitmap_forall_set(
    bitmap: *mut mi_bitmap_t,
    visit: mi_forall_set_fun_t,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
    for i in 0..chunkmap_max {
        let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
        let mut cmap_idx = 0usize;
        while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
            let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
            let chunk = &(*bitmap).chunks[chunk_idx];
            for j in 0..MI_BCHUNK_FIELDS {
                let base_idx = (chunk_idx * MI_BCHUNK_BITS) + (j * MI_BFIELD_BITS);
                let mut b = chunk.bfields[j].load(Ordering::Relaxed);
                let mut bidx = 0usize;
                while mi_bfield_foreach_bit(&mut b, &mut bidx) {
                    if !visit(base_idx + bidx, 1, arena, arg) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

#[inline(never)]
pub unsafe extern "C" fn _mi_bitmap_forall_setc_ranges(
    bitmap: *mut mi_bitmap_t,
    visit: mi_forall_set_fun_t,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
    for i in 0..chunkmap_max {
        let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
        let mut cmap_idx = 0usize;
        while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
            let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
            let chunk = &(*bitmap).chunks[chunk_idx];
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
                    crate::mi_debug_assert!(rng >= 1 && rng <= MI_BFIELD_BITS);
                    crate::mi_debug_assert!((idx % MI_BFIELD_BITS) + rng <= MI_BFIELD_BITS);
                    crate::mi_debug_assert!(
                        (idx / MI_BCHUNK_BITS) < unsafe { mi_bitmap_chunk_count(bitmap) }
                    );
                    if !visit(idx, rng, arena, arg) {
                        if b != 0 {
                            chunk.bfields[j].fetch_or(b, Ordering::Relaxed);
                        }
                        return false;
                    }
                    b &= !mi_bfield_mask(rng, bidx);
                }
                crate::mi_debug_assert!(rngcount == bpopcount);
            }
        }
    }
    true
}

#[inline(never)]
pub unsafe extern "C" fn _mi_bitmap_forall_setc_rangesn(
    bitmap: *mut mi_bitmap_t,
    rngslices: usize,
    visit: mi_forall_set_fun_t,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    if rngslices <= 1 {
        return _mi_bitmap_forall_setc_ranges(bitmap, visit, arena, arg);
    }
    let rngslices = if rngslices > MI_BFIELD_BITS {
        MI_BFIELD_BITS
    } else {
        rngslices
    };
    let chunkmap_max = mi_bitmap_chunk_count(bitmap).div_ceil(MI_BFIELD_BITS);
    for i in 0..chunkmap_max {
        let mut cmap_entry = (*bitmap).chunkmap.bfields[i].load(Ordering::Relaxed);
        let mut cmap_idx = 0usize;
        while mi_bfield_foreach_bit(&mut cmap_entry, &mut cmap_idx) {
            let chunk_idx = (i * MI_BFIELD_BITS) + cmap_idx;
            let chunk = &(*bitmap).chunks[chunk_idx];
            for j in 0..MI_BCHUNK_FIELDS {
                let base_idx = (chunk_idx * MI_BCHUNK_BITS) + (j * MI_BFIELD_BITS);
                let b = chunk.bfields[j].swap(0, Ordering::Relaxed);
                let mut skipped = 0usize;
                for shift in (0..=MI_BFIELD_BITS - rngslices).step_by(rngslices) {
                    let rngmask = mi_bfield_mask(rngslices, shift);
                    if (b & rngmask) == rngmask {
                        let idx = base_idx + shift;
                        if !visit(idx, rngslices, arena, arg) {
                            let mut notyet_visited = 0usize;
                            if shift + rngslices < MI_BFIELD_BITS {
                                notyet_visited = b & (!0usize << (shift + rngslices));
                            }
                            crate::mi_debug_assert!((notyet_visited & skipped) == 0);
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

#[inline(never)]
pub unsafe fn mi_bbitmap_size(bit_count: usize, pchunk_count: *mut usize) -> usize {
    if bit_count == 0 {
        if !pchunk_count.is_null() {
            *pchunk_count = 0;
        }
        return 0;
    }
    let bit_count = bit_count.div_ceil(MI_BCHUNK_BITS) * MI_BCHUNK_BITS;
    crate::mi_debug_assert!(bit_count <= MI_BITMAP_MAX_BIT_COUNT);
    let chunk_count = bit_count / MI_BCHUNK_BITS;
    if !pchunk_count.is_null() {
        *pchunk_count = chunk_count;
    }
    (chunk_count + MI_CBIN_COUNT as usize + 1) * MI_BCHUNK_SIZE
}

#[inline(never)]
pub unsafe fn mi_bbitmap_init(
    bbitmap: *mut mi_bbitmap_t,
    bit_count: usize,
    already_zero: bool,
) -> usize {
    let mut chunk_count = 0usize;
    let size = mi_bbitmap_size(bit_count, &mut chunk_count as *mut usize);
    if !already_zero {
        ptr::write_bytes(bbitmap.cast::<u8>(), 0, size);
    }
    (*bbitmap).chunk_count.store(chunk_count, Ordering::Release);
    crate::mi_debug_assert!(
        unsafe { mi_bbitmap_chunk_count(bbitmap) } <= MI_BITMAP_MAX_CHUNK_COUNT
    );
    size
}

#[inline(never)]
pub unsafe fn mi_bbitmap_unsafe_setN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) {
    crate::mi_debug_assert!(n > 0);
    crate::mi_debug_assert!(idx + n <= unsafe { mi_bbitmap_max_bits(bbitmap) });
    mi_bchunks_unsafe_setN(
        (*bbitmap).chunks.as_mut_ptr(),
        &mut (*bbitmap).chunkmap,
        idx,
        n,
    );
}

#[inline(never)]
pub unsafe fn mi_bbitmap_bsr_inv(bbitmap: *mut mi_bbitmap_t, idx: *mut usize) -> bool {
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
            if mi_bchunk_bsr_inv(&(*bbitmap).chunks[chunk_idx], &mut cidx) {
                *idx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                return true;
            }
        }
    }
    false
}

#[inline(never)]
pub unsafe fn mi_bbitmap_setN(bbitmap: *mut mi_bbitmap_t, idx: usize, mut n: usize) -> bool {
    crate::mi_debug_assert!(n > 0);
    let maxbits = mi_bbitmap_max_bits(bbitmap);
    crate::mi_debug_assert!(idx + n <= maxbits);
    if idx + n > maxbits {
        if idx >= maxbits {
            return false;
        }
        n = maxbits - idx;
    }
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let mut cidx = idx % MI_BCHUNK_BITS;
    let mut were_allclear = true;
    while n > 0 {
        let m = if cidx + n > MI_BCHUNK_BITS {
            MI_BCHUNK_BITS - cidx
        } else {
            n
        };
        were_allclear &= mi_bchunk_setN(&mut (*bbitmap).chunks[chunk_idx], cidx, m, None);
        mi_bbitmap_chunkmap_set(&mut *bbitmap, chunk_idx, true);
        n -= m;
        cidx = 0;
        chunk_idx += 1;
    }
    were_allclear
}

#[inline(never)]
pub unsafe fn mi_bbitmap_try_clearNC(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    crate::mi_debug_assert!(n > 0);
    crate::mi_debug_assert!(n <= MI_BCHUNK_BITS);
    crate::mi_debug_assert!(idx + n <= unsafe { mi_bbitmap_max_bits(bbitmap) });
    let chunk_idx = idx / MI_BCHUNK_BITS;
    let cidx = idx % MI_BCHUNK_BITS;
    crate::mi_debug_assert!(cidx + n <= MI_BCHUNK_BITS);
    crate::mi_debug_assert!(chunk_idx < unsafe { mi_bbitmap_chunk_count(bbitmap) });
    if cidx + n > MI_BCHUNK_BITS {
        return false;
    }
    let mut maybe_all_clear = false;
    let cleared = mi_bchunk_try_clearN(
        &mut (*bbitmap).chunks[chunk_idx],
        cidx,
        n,
        Some(&mut maybe_all_clear),
    );
    if cleared && maybe_all_clear {
        mi_bbitmap_chunkmap_try_clear(&mut *bbitmap, chunk_idx);
    }
    cleared
}

#[inline(never)]
pub unsafe fn mi_bbitmap_is_xsetN(
    set: mi_xset_t,
    bbitmap: *mut mi_bbitmap_t,
    idx: usize,
    mut n: usize,
) -> bool {
    crate::mi_debug_assert!(n > 0);
    let maxbits = mi_bbitmap_max_bits(bbitmap);
    crate::mi_debug_assert!(idx + n <= maxbits);
    if idx + n > maxbits {
        if idx >= maxbits {
            return false;
        }
        n = maxbits - idx;
    }
    let mut chunk_idx = idx / MI_BCHUNK_BITS;
    let mut cidx = idx % MI_BCHUNK_BITS;
    let mut xset = true;
    while n > 0 && xset {
        let m = if cidx + n > MI_BCHUNK_BITS {
            MI_BCHUNK_BITS - cidx
        } else {
            n
        };
        xset &= mi_bchunk_is_xsetN(set, &(*bbitmap).chunks[chunk_idx], cidx, m);
        n -= m;
        cidx = 0;
        chunk_idx += 1;
    }
    xset
}

#[inline]
pub unsafe fn mi_bbitmap_is_setN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    mi_bbitmap_is_xsetN(MI_BIT_SET, bbitmap, idx, n)
}

#[inline]
pub unsafe fn mi_bbitmap_is_clearN(bbitmap: *mut mi_bbitmap_t, idx: usize, n: usize) -> bool {
    mi_bbitmap_is_xsetN(MI_BIT_CLEAR, bbitmap, idx, n)
}

type mi_bchunk_try_find_and_clear_fun_t = fn(&mut mi_bchunk_t, usize, &mut usize) -> bool;

#[inline]
fn mi_bchunk_try_find_and_clear_1(chunk: &mut mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
    crate::mi_debug_assert!(n == 1);
    mi_bchunk_try_find_and_clear(chunk, pidx)
}

#[inline]
fn mi_bchunk_try_find_and_clear_8(chunk: &mut mi_bchunk_t, n: usize, pidx: &mut usize) -> bool {
    crate::mi_debug_assert!(n == 8);
    mi_bchunk_try_find_and_clear8(chunk, pidx)
}

#[inline]
unsafe fn mi_bbitmap_try_find_and_clear_generic(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
    on_find: mi_bchunk_try_find_and_clear_fun_t,
) -> bool {
    let chunk_count = mi_bbitmap_chunk_count(bbitmap);
    let cmap_max_count = mi_bbitmap_chunk_count(bbitmap).div_ceil(MI_BFIELD_BITS);
    let chunk_acc = (*bbitmap).chunk_max_accessed.load(Ordering::Relaxed);
    let cmap_acc = chunk_acc / MI_BFIELD_BITS;
    let cmap_acc_bits = 1 + (chunk_acc % MI_BFIELD_BITS);
    let cmap_mask = mi_bfield_mask(cmap_max_count, 0);
    let cmap_cycle = cmap_acc + 1;
    let bbin = mi_chunkbin_of(n);
    let mut cmap_idx = 0usize;
    let mut found = false;
    let _ = mi_bfield_cycle_iterate(cmap_mask, tseq, cmap_cycle, |cmap_idx_in_cycle| {
        cmap_idx = cmap_idx_in_cycle;
        let cmap_entry = (*bbitmap).chunkmap.bfields[cmap_idx].load(Ordering::Relaxed);
        let cmap_entry_cycle = if cmap_idx != cmap_acc {
            MI_BFIELD_BITS
        } else {
            cmap_acc_bits
        };
        if cmap_entry == 0 {
            return true;
        }
        let mut cmap_bins = [0usize; MI_CBIN_COUNT as usize];
        cmap_bins[MI_CBIN_NONE as usize] = cmap_entry;
        let mut ibin = MI_CBIN_SMALL;
        while (ibin as i32) < (MI_CBIN_NONE as i32) {
            let cmap_bin =
                (*bbitmap).chunkmap_bins[ibin as usize].bfields[cmap_idx].load(Ordering::Relaxed);
            cmap_bins[ibin as usize] = cmap_bin & cmap_entry;
            cmap_bins[MI_CBIN_NONE as usize] &= !cmap_bin;
            ibin = mi_chunkbin_inc(ibin);
        }
        let mut ibin = MI_CBIN_SMALL;
        loop {
            let cmap_bin = cmap_bins[ibin as usize];
            let mut eidx = 0usize;
            let _ = mi_bfield_cycle_iterate(cmap_bin, tseq, cmap_entry_cycle, |eidx_in_cycle| {
                eidx = eidx_in_cycle;
                let chunk_idx = (cmap_idx * MI_BFIELD_BITS) + eidx;
                if chunk_idx >= chunk_count {
                    return true;
                }
                let chunk = &mut (*bbitmap).chunks[chunk_idx];
                let mut cidx = 0usize;
                if on_find(chunk, n, &mut cidx) {
                    if cidx == 0 && ibin == MI_CBIN_NONE {
                        mi_bbitmap_set_chunk_bin(&mut *bbitmap, chunk_idx, bbin);
                    }
                    *pidx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                    crate::mi_debug_assert!(*pidx + n <= unsafe { mi_bbitmap_max_bits(bbitmap) });
                    found = true;
                    return false;
                } else {
                    mi_bbitmap_chunkmap_try_clear(&mut *bbitmap, chunk_idx);
                }
                true
            });
            if found {
                break;
            }
            if ibin == MI_CBIN_NONE {
                break;
            }
            ibin = if ibin == bbin {
                MI_CBIN_NONE
            } else {
                mi_chunkbin_inc(ibin)
            };
        }
        !found
    });
    found
}

#[inline(never)]
pub unsafe fn mi_bbitmap_try_find_and_clear(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    pidx: *mut usize,
) -> bool {
    mi_bbitmap_try_find_and_clear_generic(bbitmap, tseq, 1, pidx, mi_bchunk_try_find_and_clear_1)
}

#[inline(never)]
pub unsafe fn mi_bbitmap_try_find_and_clear8(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    pidx: *mut usize,
) -> bool {
    mi_bbitmap_try_find_and_clear_generic(bbitmap, tseq, 8, pidx, mi_bchunk_try_find_and_clear_8)
}

#[inline(never)]
pub unsafe fn mi_bbitmap_try_find_and_clearNX(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
) -> bool {
    crate::mi_debug_assert!(n <= MI_BFIELD_BITS);
    mi_bbitmap_try_find_and_clear_generic(bbitmap, tseq, n, pidx, mi_bchunk_try_find_and_clearNX)
}

#[inline(never)]
pub unsafe fn mi_bbitmap_try_find_and_clearNC(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
) -> bool {
    crate::mi_debug_assert!(n <= MI_BCHUNK_BITS);
    mi_bbitmap_try_find_and_clear_generic(bbitmap, tseq, n, pidx, mi_bchunk_try_find_and_clearNC)
}

#[inline]
fn mi_bchunk_try_clearN_(bbitmap: &mut mi_bbitmap_t, chunk_idx: usize, n: usize) -> bool {
    crate::mi_debug_assert!(
        (chunk_idx * MI_BCHUNK_BITS) + n <= unsafe { mi_bbitmap_max_bits(bbitmap) }
    );
    let mut m = n;
    let mut count = 0usize;
    while m > 0 {
        let chunk = &mut bbitmap.chunks[chunk_idx + count];
        if !mi_bchunk_try_clearN(
            chunk,
            0,
            if m > MI_BCHUNK_BITS {
                MI_BCHUNK_BITS
            } else {
                m
            },
            None,
        ) {
            while count > 0 {
                count -= 1;
                let chunk = &mut bbitmap.chunks[chunk_idx + count];
                mi_bchunk_setN(chunk, 0, MI_BCHUNK_BITS, None);
            }
            return false;
        }
        m = if m <= MI_BCHUNK_BITS {
            0
        } else {
            m - MI_BCHUNK_BITS
        };
        count += 1;
    }
    true
}

#[inline(never)]
pub unsafe fn mi_bbitmap_try_find_and_clearN_(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
) -> bool {
    let _ = tseq;
    crate::mi_debug_assert!(n > 0);
    let chunk_max = mi_bbitmap_chunk_count(bbitmap);
    let chunk_req = n.div_ceil(MI_BCHUNK_BITS);
    if chunk_max < chunk_req {
        return false;
    }
    let mut chunk_idx = 0usize;
    while chunk_idx <= chunk_max - chunk_req {
        let mut count = 0usize;
        while chunk_idx + count < chunk_max {
            let chunk = &mut (*bbitmap).chunks[chunk_idx + count];
            if !mi_bchunk_all_are_set_relaxed(chunk) {
                break;
            }
            count += 1;
            if count == chunk_req {
                break;
            }
        }
        if count == chunk_req {
            if mi_bchunk_try_clearN_(&mut *bbitmap, chunk_idx, n) {
                *pidx = chunk_idx * MI_BCHUNK_BITS;
                for i in 0..count {
                    mi_bbitmap_set_chunk_bin(&mut *bbitmap, chunk_idx + i, MI_CBIN_HUGE);
                }
                crate::mi_debug_assert!(*pidx + n <= unsafe { mi_bbitmap_max_bits(bbitmap) });
                return true;
            }
        }
        chunk_idx += count + 1;
    }
    false
}

#[inline]
pub unsafe fn mi_bbitmap_try_find_and_clearN(
    bbitmap: *mut mi_bbitmap_t,
    tseq: usize,
    n: usize,
    pidx: *mut usize,
) -> bool {
    if n == 1 {
        return mi_bbitmap_try_find_and_clear(bbitmap, tseq, pidx);
    }
    if n == 8 {
        return mi_bbitmap_try_find_and_clear8(bbitmap, tseq, pidx);
    }
    if n == 0 {
        return false;
    }
    if n <= MI_BFIELD_BITS {
        return mi_bbitmap_try_find_and_clearNX(bbitmap, tseq, n, pidx);
    }
    if n <= MI_BCHUNK_BITS {
        return mi_bbitmap_try_find_and_clearNC(bbitmap, tseq, n, pidx);
    }
    mi_bbitmap_try_find_and_clearN_(bbitmap, tseq, n, pidx)
}

#[used]
static MI_BBITMAP_DEBUG_GET_BIN_KEEP: unsafe fn(*const mi_bchunk_t, usize) -> mi_chunkbin_t =
    mi_bbitmap_debug_get_bin;
#[used]
static MI_BBITMAP_SIZE_KEEP: unsafe fn(usize, *mut usize) -> usize = mi_bbitmap_size;
#[used]
static MI_BBITMAP_INIT_KEEP: unsafe fn(*mut mi_bbitmap_t, usize, bool) -> usize = mi_bbitmap_init;
#[used]
static MI_BBITMAP_UNSAFE_SETN_KEEP: unsafe fn(*mut mi_bbitmap_t, usize, usize) =
    mi_bbitmap_unsafe_setN;
#[used]
static MI_BBITMAP_BSR_INV_KEEP: unsafe fn(*mut mi_bbitmap_t, *mut usize) -> bool =
    mi_bbitmap_bsr_inv;
#[used]
static MI_BBITMAP_SETN_KEEP: unsafe fn(*mut mi_bbitmap_t, usize, usize) -> bool = mi_bbitmap_setN;
#[used]
static MI_BBITMAP_TRY_CLEARNC_KEEP: unsafe fn(*mut mi_bbitmap_t, usize, usize) -> bool =
    mi_bbitmap_try_clearNC;
#[used]
static MI_BBITMAP_IS_XSETN_KEEP: unsafe fn(mi_xset_t, *mut mi_bbitmap_t, usize, usize) -> bool =
    mi_bbitmap_is_xsetN;
#[used]
static MI_BBITMAP_TRY_FIND_AND_CLEAR_KEEP: unsafe fn(*mut mi_bbitmap_t, usize, *mut usize) -> bool =
    mi_bbitmap_try_find_and_clear;
#[used]
static MI_BBITMAP_TRY_FIND_AND_CLEAR8_KEEP: unsafe fn(
    *mut mi_bbitmap_t,
    usize,
    *mut usize,
) -> bool = mi_bbitmap_try_find_and_clear8;
#[used]
static MI_BBITMAP_TRY_FIND_AND_CLEARNX_KEEP: unsafe fn(
    *mut mi_bbitmap_t,
    usize,
    usize,
    *mut usize,
) -> bool = mi_bbitmap_try_find_and_clearNX;
#[used]
static MI_BBITMAP_TRY_FIND_AND_CLEARNC_KEEP: unsafe fn(
    *mut mi_bbitmap_t,
    usize,
    usize,
    *mut usize,
) -> bool = mi_bbitmap_try_find_and_clearNC;
#[used]
static MI_BBITMAP_TRY_FIND_AND_CLEARN_KEEP: unsafe fn(
    *mut mi_bbitmap_t,
    usize,
    usize,
    *mut usize,
) -> bool = mi_bbitmap_try_find_and_clearN_;

type mi_bchunk_xsetNC_fun_t =
    fn(mi_xset_t, &mut mi_bchunk_t, usize, usize, Option<&mut usize>, Option<&mut bool>) -> bool;

#[used]
static MI_BCHUNK_XSETNC_KEEP: mi_bchunk_xsetNC_fun_t = mi_bchunk_xsetNC;
#[used]
static MI_BCHUNK_POPCOUNTNC_KEEP: fn(&mi_bchunk_t, usize, usize, usize) -> usize =
    mi_bchunk_popcountNC;
#[used]
static MI_BCHUNK_IS_XSETNC_KEEP: fn(mi_xset_t, &mi_bchunk_t, usize, usize, usize) -> bool =
    mi_bchunk_is_xsetNC;
#[used]
static MI_BCHUNK_TRY_CLEARNC_KEEP: fn(&mut mi_bchunk_t, usize, usize, Option<&mut bool>) -> bool =
    mi_bchunk_try_clearNC;
#[used]
static MI_BCHUNK_TRY_FIND_AND_CLEAR8_KEEP: fn(&mut mi_bchunk_t, &mut usize) -> bool =
    mi_bchunk_try_find_and_clear8;
#[used]
static MI_BCHUNK_TRY_FIND_AND_CLEARNX_KEEP: fn(&mut mi_bchunk_t, usize, &mut usize) -> bool =
    mi_bchunk_try_find_and_clearNX;
#[used]
static MI_BCHUNK_TRY_FIND_AND_CLEARNC_KEEP: fn(&mut mi_bchunk_t, usize, &mut usize) -> bool =
    mi_bchunk_try_find_and_clearNC;
#[used]
static MI_BITMAP_CHUNKMAP_TRY_CLEAR_KEEP: fn(&mut mi_bitmap_t, usize) -> bool =
    mi_bitmap_chunkmap_try_clear;
#[used]
static MI_BBITMAP_SET_CHUNK_BIN_KEEP: fn(&mut mi_bbitmap_t, usize, mi_chunkbin_t) =
    mi_bbitmap_set_chunk_bin;
#[used]
static MI_BBITMAP_CHUNKMAP_TRY_CLEAR_KEEP: fn(&mut mi_bbitmap_t, usize) -> bool =
    mi_bbitmap_chunkmap_try_clear;
