//! Bit operation helpers and platform-size definitions

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unused_macros,
    clippy::missing_const_for_fn
)]

macro_rules! __has_builtin {
    ($x:expr) => {
        false
    };
}

macro_rules! mi_builtin {
    ($name:ident) => {
        $name
    };
}

macro_rules! mi_builtin32 {
    ($name:ident) => {
        mi_builtin!($name)
    };
}

macro_rules! mi_builtin64 {
    ($name:ident) => {
        mi_builtin!($name)
    };
}

macro_rules! mi_builtinz {
    ($name:ident) => {
        mi_builtin32!($name)
    };
}

macro_rules! mi_msc_builtinz {
    ($name:ident) => {
        $name
    };
}

macro_rules! MI_PU {
    ($x:expr) => {
        $x
    };
}

macro_rules! MI_ZU {
    ($x:expr) => {
        $x
    };
}

pub const MI_BITS_H: () = ();

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

#[cfg(target_arch = "aarch64")]
pub const MI_ARCH_ARM64: usize = 1;
#[cfg(not(target_arch = "aarch64"))]
pub const MI_ARCH_ARM64: usize = 0;

#[cfg(target_arch = "x86_64")]
pub const MI_ARCH_X64: usize = 1;
#[cfg(not(target_arch = "x86_64"))]
pub const MI_ARCH_X64: usize = 0;

#[cfg(target_arch = "x86")]
pub const MI_ARCH_X86: usize = 1;
#[cfg(not(target_arch = "x86"))]
pub const MI_ARCH_X86: usize = 0;

#[cfg(target_arch = "arm")]
pub const MI_ARCH_ARM32: usize = 1;
#[cfg(not(target_arch = "arm"))]
pub const MI_ARCH_ARM32: usize = 0;

#[cfg(target_arch = "riscv64")]
pub const MI_ARCH_RISCV: usize = 1;
#[cfg(target_arch = "riscv32")]
pub const MI_ARCH_RISCV: usize = 1;
#[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32")))]
pub const MI_ARCH_RISCV: usize = 0;

#[cfg(target_arch = "riscv32")]
pub const MI_ARCH_RISCV32: usize = 1;
#[cfg(not(target_arch = "riscv32"))]
pub const MI_ARCH_RISCV32: usize = 0;

#[cfg(target_arch = "riscv64")]
pub const MI_ARCH_RISCV64: usize = 1;
#[cfg(not(target_arch = "riscv64"))]
pub const MI_ARCH_RISCV64: usize = 0;

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

#[cfg(target_arch = "x86_64")]
pub const MI_HAS_FAST_POPCOUNT: usize = 1;
#[cfg(not(target_arch = "x86_64"))]
pub const MI_HAS_FAST_POPCOUNT: usize = 0;

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "arm"
))]
pub const MI_HAS_FAST_BITSCAN: usize = 1;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "arm"
)))]
pub const MI_HAS_FAST_BITSCAN: usize = 0;

pub type mi_ssize_t = isize;

#[inline]
pub(crate) fn _mi_popcount_generic(x: usize) -> usize {
    x.count_ones() as usize
}

#[inline]
pub(crate) fn _mi_ctz_generic(x: usize) -> usize {
    x.trailing_zeros() as usize
}

#[inline]
pub(crate) fn _mi_clz_generic(x: usize) -> usize {
    x.leading_zeros() as usize
}

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
