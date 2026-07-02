#![allow(non_snake_case)]

use super::bfield::{
    mi_bfield_atomic_is_xset_mask, mi_bfield_atomic_popcount_mask, mi_bfield_mask,
};
use super::{MI_BCHUNK_BITS, MI_BCHUNK_FIELDS, MI_BFIELD_BITS, mi_bchunk_t, mi_xset_t};

#[inline(never)]
pub(super) fn mi_bchunk_popcountNC(
    chunk: &mi_bchunk_t,
    mut field_idx: usize,
    mut idx: usize,
    mut n: usize,
) -> usize {
    crate::internal::mi_debug_assert!((field_idx * MI_BFIELD_BITS) + idx + n <= MI_BCHUNK_BITS);
    let mut count = 0usize;
    while n > 0 {
        let mut m = MI_BFIELD_BITS - idx;
        if m > n {
            m = n;
        }
        crate::internal::mi_debug_assert!(idx + m <= MI_BFIELD_BITS);
        crate::internal::mi_debug_assert!(field_idx < MI_BCHUNK_FIELDS);
        let mask = mi_bfield_mask(m, idx);
        count += mi_bfield_atomic_popcount_mask(&chunk.bfields[field_idx], mask);
        field_idx += 1;
        idx = 0;
        n -= m;
    }
    count
}

#[inline(never)]
pub(super) fn mi_bchunk_is_xsetNC(
    set: mi_xset_t,
    chunk: &mi_bchunk_t,
    mut field_idx: usize,
    mut idx: usize,
    mut n: usize,
) -> bool {
    crate::internal::mi_debug_assert!((field_idx * MI_BFIELD_BITS) + idx + n <= MI_BCHUNK_BITS);
    while n > 0 {
        let mut m = MI_BFIELD_BITS - idx;
        if m > n {
            m = n;
        }
        crate::internal::mi_debug_assert!(idx + m <= MI_BFIELD_BITS);
        crate::internal::mi_debug_assert!(field_idx < MI_BCHUNK_FIELDS);
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
