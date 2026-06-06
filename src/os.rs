//! Primitive OS / platform layer

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc
)]

use core::ffi::c_char;
use std::ffi::CStr;
use std::io::{self, Write};

use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::bits::{MI_GiB, MI_INTPTR_SIZE, MI_KiB, MI_MAX_VABITS, MI_MiB};
use crate::internal::_mi_align_up;
use crate::prim::mi_os_mem_config_t;
use crate::types::mi_process_info_t;

pub const MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_4_GIB: usize = 4 * MI_MiB;
pub const MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_32_GIB: usize = 32 * MI_MiB;
pub const MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB: usize = if MI_INTPTR_SIZE < 8 {
    MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_4_GIB
} else {
    MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_32_GIB
};

pub const MI_HINT_ALIGN: usize = 4usize << 20;
pub const MI_HINT_AREA: usize = 4usize << 40;
pub const MI_HUGE_OS_PAGE_SIZE: usize = MI_GiB;

static mut mi_os_mem_config: mi_os_mem_config_t = mi_os_mem_config_t {
    page_size: 4096,
    large_page_size: 0,
    alloc_granularity: 4096,
    physical_memory_in_kib: MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB,
    virtual_address_bits: MI_MAX_VABITS,
    has_overcommit: true,
    has_partial_free: false,
    has_virtual_reserve: true,
    has_transparent_huge_pages: false,
};

#[inline]
unsafe fn mi_os_mem_config_ptr() -> *const mi_os_mem_config_t {
    core::ptr::addr_of!(mi_os_mem_config)
}

#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let bytes = CStr::from_ptr(msg).to_bytes();
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(bytes);
    let _ = stderr.flush();
}

#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    if pinfo.is_null() {
        return;
    }
    // Leave any values already filled by higher layers intact; only provide the
    // primitive defaults that are currently available in the Rust port.
    unsafe {
        (*pinfo).utime = 0;
        (*pinfo).stime = 0;
        (*pinfo).page_faults = 0;
    }
}

#[inline]
pub unsafe extern "C" fn _mi_os_has_overcommit() -> bool {
    unsafe { (*mi_os_mem_config_ptr()).has_overcommit }
}

#[inline]
pub unsafe extern "C" fn _mi_os_has_virtual_reserve() -> bool {
    unsafe { (*mi_os_mem_config_ptr()).has_virtual_reserve }
}

#[inline]
pub unsafe extern "C" fn _mi_os_page_size() -> usize {
    unsafe { (*mi_os_mem_config_ptr()).page_size }
}

#[inline]
pub unsafe extern "C" fn _mi_os_large_page_size() -> usize {
    let large = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    if large != 0 {
        large
    } else {
        unsafe { _mi_os_page_size() }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_os_guard_page_size() -> usize {
    let gsize = unsafe { _mi_os_page_size() };
    debug_assert!(gsize <= (MI_ARENA_SLICE_SIZE / 4));
    gsize
}

#[inline]
pub unsafe extern "C" fn _mi_os_virtual_address_bits() -> usize {
    let vbits = unsafe { (*mi_os_mem_config_ptr()).virtual_address_bits };
    debug_assert!(vbits <= MI_MAX_VABITS);
    vbits
}

#[inline]
pub unsafe extern "C" fn _mi_os_good_alloc_size(size: usize) -> usize {
    let align_size = if size < 512 * MI_KiB {
        unsafe { _mi_os_page_size() }
    } else if size < 2 * MI_MiB {
        64 * MI_KiB
    } else if size < 8 * MI_MiB {
        256 * MI_KiB
    } else if size < 32 * MI_MiB {
        MI_MiB
    } else {
        4 * MI_MiB
    };
    if size >= usize::MAX - align_size {
        size
    } else {
        _mi_align_up(size, align_size)
    }
}
