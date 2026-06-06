//! Page management

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::mem::size_of;

use crate::stats::{MI_BIN_HUGE, MI_LARGE_MAX_OBJ_WSIZE, bin_block_size};

pub const MI_PADDING_SIZE: usize = 0;
pub const MI_OS_PAGE_SIZE: usize = 4096;
pub const MI_LARGE_MAX_OBJ_SIZE: usize = MI_LARGE_MAX_OBJ_WSIZE * size_of::<usize>();
pub const MI_MAX_ALLOC_SIZE: usize = isize::MAX as usize;

const MI_ALIGN4W: bool = cfg!(target_pointer_width = "32");
const MI_ALIGN2W: bool = cfg!(target_pointer_width = "64");

fn align_up(size: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (size + alignment - 1) & !(alignment - 1)
}

pub extern "C" fn mi_bin(size: usize) -> usize {
    let mut wsize = size.div_ceil(size_of::<usize>());
    if MI_ALIGN4W {
        if wsize <= 4 {
            return if wsize <= 1 { 1 } else { (wsize + 1) & !1 };
        }
    } else if MI_ALIGN2W {
        if wsize <= 8 {
            return if wsize <= 1 { 1 } else { (wsize + 1) & !1 };
        }
    } else if wsize <= 8 {
        return if wsize == 0 { 1 } else { wsize };
    }

    if wsize > MI_LARGE_MAX_OBJ_WSIZE {
        return MI_BIN_HUGE;
    }

    if MI_ALIGN4W && wsize <= 16 {
        wsize = (wsize + 3) & !3;
    }

    wsize -= 1;
    let b = (usize::BITS as usize - 1) - wsize.leading_zeros() as usize;
    let bin = ((b << 2) + ((wsize >> (b - 2)) & 0x03)) - 3;
    debug_assert!(bin > 0 && bin < MI_BIN_HUGE);
    bin
}

pub extern "C" fn _mi_bin(size: usize) -> usize {
    mi_bin(size)
}

pub extern "C" fn _mi_bin_size(bin: usize) -> usize {
    debug_assert!(bin <= MI_BIN_HUGE);
    bin_block_size(bin)
}

pub extern "C" fn mi_good_size(size: usize) -> usize {
    if size <= MI_LARGE_MAX_OBJ_SIZE {
        _mi_bin_size(mi_bin(size + MI_PADDING_SIZE))
    } else if size <= MI_MAX_ALLOC_SIZE {
        align_up(size + MI_PADDING_SIZE, MI_OS_PAGE_SIZE)
    } else {
        size
    }
}
