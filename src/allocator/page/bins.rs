use core::mem::size_of;
use core::ptr;

use crate::allocator::init::_mi_theap_empty;
use crate::allocator::stats::MI_BIN_HUGE;
use crate::types::{MI_LARGE_MAX_OBJ_WSIZE, mi_theap_t};

const MI_ALIGN4W: bool = cfg!(target_pointer_width = "32");
const MI_ALIGN2W: bool = cfg!(target_pointer_width = "64");

#[inline(always)]
pub(crate) fn mi_bin_impl(size: usize) -> usize {
    let mut wsize = crate::internal::_mi_wsize_from_size(size);
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
    crate::internal::mi_debug_assert!(bin > 0 && bin < MI_BIN_HUGE);
    bin
}

#[inline(never)]
pub extern "C" fn _mi_bin(size: usize) -> usize {
    mi_bin_impl(size)
}

#[inline(always)]
fn mi_bin_from_raw_size(size: usize) -> crate::core_types::Bin {
    let bin = mi_bin_impl(size);
    crate::internal::mi_debug_assert!(u16::try_from(bin).is_ok());
    crate::internal::mi_debug_assert!(bin < crate::types::MI_BIN_COUNT);
    unsafe { crate::core_types::Bin::new_unchecked(bin as u16) }
}

#[inline(always)]
pub(crate) fn mi_bin_from_padded(padded: crate::core_types::PaddedSize) -> crate::core_types::Bin {
    mi_bin_from_raw_size(padded.get())
}

#[inline(always)]
pub(crate) fn mi_bin_from_block_size(
    block_size: crate::core_types::BlockSize,
) -> crate::core_types::Bin {
    mi_bin_from_raw_size(block_size.get())
}

#[inline(always)]
pub(crate) fn mi_bin_from_usable(usable: crate::core_types::UsableSize) -> crate::core_types::Bin {
    mi_bin_from_raw_size(usable.get())
}

#[inline(never)]
pub extern "C" fn _mi_bin_size(bin: usize) -> usize {
    crate::internal::mi_debug_assert!(bin <= MI_BIN_HUGE);
    unsafe { (*ptr::addr_of!(_mi_theap_empty).cast::<mi_theap_t>()).pages[bin].block_size }
}

#[inline(always)]
fn mi_good_size_small(padded: crate::core_types::PaddedSize) -> crate::core_types::GoodSize {
    crate::core_types::GoodSize::new(_mi_bin_size(mi_bin_from_padded(padded).as_usize()))
}

pub(crate) fn mi_good_size_typed(
    size: crate::core_types::RequestedSize,
) -> crate::core_types::GoodSize {
    if size.get() == 0 {
        return crate::core_types::GoodSize::new(size_of::<usize>());
    }
    let sz = size.effective_size().get();
    let size = crate::core_types::RequestedSize::new(sz);
    if sz <= super::MI_LARGE_MAX_OBJ_SIZE {
        // `sz <= MI_LARGE_MAX_OBJ_SIZE` is far below `usize::MAX`, so padding
        // cannot overflow; `checked_padded` makes that explicit rather than open-
        // coding `sz + MI_PADDING_SIZE`.
        let Ok(padded) = size.checked_padded() else {
            return crate::core_types::GoodSize::new(sz);
        };
        mi_good_size_small(padded)
    } else if sz <= super::MI_MAX_ALLOC_SIZE {
        let Ok(padded) = size.checked_padded() else {
            return crate::core_types::GoodSize::new(sz);
        };
        crate::core_types::GoodSize::new(crate::internal::_mi_align_up(
            padded.get(),
            crate::internal::MI_OS_PAGE_SIZE,
        ))
    } else {
        crate::core_types::GoodSize::new(sz)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_good_size(size: usize) -> usize {
    mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()
}
