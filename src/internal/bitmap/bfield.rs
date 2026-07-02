use core::hint::spin_loop;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::bits::{mi_bsf, mi_bsr, mi_clz, mi_ctz, mi_popcount};

use super::{MI_BFIELD_BITS, mi_bfield_t, mi_xset_t};

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
pub(super) fn mi_bfield_find_least_bit(x: mi_bfield_t, idx: &mut usize) -> bool {
    mi_bsf(x, idx)
}

#[inline]
pub(super) fn mi_bfield_find_highest_bit(x: mi_bfield_t, idx: &mut usize) -> bool {
    mi_bsr(x, idx)
}

#[inline]
pub(super) fn mi_bfield_foreach_bit(x: &mut mi_bfield_t, idx: &mut usize) -> bool {
    let found = mi_bfield_find_least_bit(*x, idx);
    *x = mi_bfield_clear_least_bit(*x);
    found
}

#[inline]
pub(super) fn mi_bfield_mask(bit_count: usize, shiftl: usize) -> mi_bfield_t {
    crate::internal::mi_debug_assert!(bit_count > 0);
    crate::internal::mi_debug_assert!(shiftl < MI_BFIELD_BITS);
    crate::internal::mi_debug_assert!(bit_count + shiftl <= MI_BFIELD_BITS);
    let mask0 = if bit_count < MI_BFIELD_BITS {
        (mi_bfield_one() << bit_count) - 1
    } else {
        mi_bfield_all_set()
    };
    mask0 << shiftl
}

#[inline]
pub(super) fn mi_bfield_atomic_set(b: &AtomicUsize, idx: usize) -> bool {
    crate::internal::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let mask = mi_bfield_mask(1, idx);
    let old = b.fetch_or(mask, Ordering::AcqRel);
    (old & mask) == 0
}

#[inline]
pub(super) fn mi_bfield_atomic_clear(
    b: &AtomicUsize,
    idx: usize,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let mask = mi_bfield_mask(1, idx);
    let old = b.fetch_and(!mask, Ordering::AcqRel);
    if let Some(all_clear) = all_clear {
        *all_clear = (old & !mask) == 0;
    }
    (old & mask) == mask
}

#[inline]
pub(super) fn mi_bfield_atomic_clear_once_set(b: &AtomicUsize, idx: usize) {
    crate::internal::mi_debug_assert!(idx < MI_BFIELD_BITS);
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
pub(super) fn mi_bfield_atomic_set_mask(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    already_set: Option<&mut usize>,
) -> bool {
    crate::internal::mi_debug_assert!(mask != 0);
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
pub(super) fn mi_bfield_atomic_clear_mask(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(mask != 0);
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
pub(super) fn mi_bfield_atomic_setX(b: &AtomicUsize, already_set: Option<&mut usize>) -> bool {
    let old = b.swap(mi_bfield_all_set(), Ordering::Release);
    if let Some(already_set) = already_set {
        *already_set = mi_bfield_popcount(old);
    }
    old == 0
}

#[inline]
pub(super) fn mi_bfield_atomic_try_clear_mask_of(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    mut expect: mi_bfield_t,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(mask != 0);
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
pub(super) fn mi_bfield_atomic_try_clear_mask(
    b: &AtomicUsize,
    mask: mi_bfield_t,
    all_clear: Option<&mut bool>,
) -> bool {
    crate::internal::mi_debug_assert!(mask != 0);
    let expect = b.load(Ordering::Relaxed);
    mi_bfield_atomic_try_clear_mask_of(b, mask, expect, all_clear)
}

#[inline]
pub(super) fn mi_bfield_atomic_try_clearX(b: &AtomicUsize, all_clear: Option<&mut bool>) -> bool {
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
pub(super) fn mi_bfield_atomic_is_set(b: &AtomicUsize, idx: usize) -> bool {
    crate::internal::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let x = b.load(Ordering::Acquire);
    (x & mi_bfield_mask(1, idx)) != 0
}

#[inline]
pub(super) fn mi_bfield_atomic_is_clear(b: &AtomicUsize, idx: usize) -> bool {
    crate::internal::mi_debug_assert!(idx < MI_BFIELD_BITS);
    let x = b.load(Ordering::Acquire);
    (x & mi_bfield_mask(1, idx)) == 0
}

#[inline]
pub(super) fn mi_bfield_atomic_is_xset(set: mi_xset_t, b: &AtomicUsize, idx: usize) -> bool {
    if set {
        mi_bfield_atomic_is_set(b, idx)
    } else {
        mi_bfield_atomic_is_clear(b, idx)
    }
}

#[inline]
pub(super) fn mi_bfield_atomic_is_set_mask(b: &AtomicUsize, mask: mi_bfield_t) -> bool {
    crate::internal::mi_debug_assert!(mask != 0);
    let x = b.load(Ordering::Acquire);
    (x & mask) == mask
}

#[inline]
pub(super) fn mi_bfield_atomic_is_clear_mask(b: &AtomicUsize, mask: mi_bfield_t) -> bool {
    crate::internal::mi_debug_assert!(mask != 0);
    let x = b.load(Ordering::Acquire);
    (x & mask) == 0
}

#[inline]
pub(super) fn mi_bfield_atomic_is_xset_mask(
    set: mi_xset_t,
    b: &AtomicUsize,
    mask: mi_bfield_t,
) -> bool {
    if set {
        mi_bfield_atomic_is_set_mask(b, mask)
    } else {
        mi_bfield_atomic_is_clear_mask(b, mask)
    }
}

#[inline]
pub(super) fn mi_bfield_atomic_popcount_mask(b: &AtomicUsize, mask: mi_bfield_t) -> usize {
    let x = b.load(Ordering::Acquire);
    mi_bfield_popcount(x & mask)
}
