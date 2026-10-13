//! Bit operation helpers and platform-size definitions

// Retains upstream architecture and bit-operation macros for parity across
// targets even when the current target resolves them to unused constants.
#![allow(non_camel_case_types, non_upper_case_globals)]

macro_rules! MI_ZU {
    ($x:expr) => {
        $x
    };
}

#[cfg(target_pointer_width = "64")]
pub const MI_INTPTR_SHIFT: usize = 3;
#[cfg(target_pointer_width = "32")]
pub const MI_INTPTR_SHIFT: usize = 2;
#[cfg(not(any(target_pointer_width = "64", target_pointer_width = "32")))]
pub const MI_INTPTR_SHIFT: usize = 4;

#[cfg(target_pointer_width = "64")]
pub const MI_SIZE_SHIFT: usize = 3;
#[cfg(target_pointer_width = "32")]
pub const MI_SIZE_SHIFT: usize = 2;
#[cfg(not(any(target_pointer_width = "64", target_pointer_width = "32")))]
pub const MI_SIZE_SHIFT: usize = 4;

pub const MI_INTPTR_SIZE: usize = 1 << MI_INTPTR_SHIFT;
pub const MI_SIZE_SIZE: usize = 1 << MI_SIZE_SHIFT;
pub const MI_INTPTR_BITS: usize = usize::BITS as usize;
pub const MI_SIZE_BITS: usize = usize::BITS as usize;

pub const MI_KiB: usize = MI_ZU!(1024);
pub const MI_MiB: usize = MI_KiB * MI_KiB;
pub const MI_GiB: usize = MI_MiB * MI_KiB;

#[cfg(target_arch = "x86_64")]
pub const __BMI2__: usize = 1;
#[cfg(not(target_arch = "x86_64"))]
pub const __BMI2__: usize = 0;

#[cfg(target_arch = "x86_64")]
pub const __BMI1__: usize = 1;
#[cfg(not(target_arch = "x86_64"))]
pub const __BMI1__: usize = 0;

#[cfg(target_arch = "x86_64")]
pub const __LZCNT__: usize = 1;
#[cfg(not(target_arch = "x86_64"))]
pub const __LZCNT__: usize = 0;

#[cfg(target_pointer_width = "64")]
pub const MI_MAX_VABITS: usize = 47;
#[cfg(target_pointer_width = "32")]
pub const MI_MAX_VABITS: usize = 32;
#[cfg(not(any(target_pointer_width = "64", target_pointer_width = "32")))]
pub const MI_MAX_VABITS: usize = 48;

#[cfg(target_pointer_width = "32")]
pub const MI_PAGE_MAP_FLAT: usize = 1;
#[cfg(not(target_pointer_width = "32"))]
pub const MI_PAGE_MAP_FLAT: usize = 0;

#[inline]
pub fn mi_popcount(x: usize) -> usize {
    x.count_ones() as usize
}

#[inline]
pub fn mi_ctz(x: usize) -> usize {
    if x == 0 {
        MI_SIZE_BITS
    } else {
        x.trailing_zeros() as usize
    }
}

#[inline]
pub fn mi_clz(x: usize) -> usize {
    if x == 0 {
        MI_SIZE_BITS
    } else {
        x.leading_zeros() as usize
    }
}

#[inline]
pub fn mi_bsf(x: usize, idx: &mut usize) -> bool {
    if x != 0 {
        *idx = mi_ctz(x);
        true
    } else {
        false
    }
}

#[inline]
pub fn mi_bsr(x: usize, idx: &mut usize) -> bool {
    if x != 0 {
        *idx = MI_SIZE_BITS - 1 - mi_clz(x);
        true
    } else {
        false
    }
}

#[inline]
pub fn mi_rotr(x: usize, r: usize) -> usize {
    x.rotate_right((r as u32) & ((MI_SIZE_BITS as u32) - 1))
}

#[inline]
pub fn mi_rotl(x: usize, r: usize) -> usize {
    x.rotate_left((r as u32) & ((MI_SIZE_BITS as u32) - 1))
}

#[inline]
pub fn mi_rotl32(x: u32, r: u32) -> u32 {
    x.rotate_left(r & 31)
}
