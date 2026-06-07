//! Primitive OS / platform layer

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc
)]

use core::ffi::{c_char, c_void};
use std::ffi::CStr;
use std::io::{self, Write};

use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::bits::{MI_GiB, MI_INTPTR_SIZE, MI_KiB, MI_MAX_VABITS, MI_MiB};
use crate::internal::{_mi_align_down_ptr, _mi_align_up, _mi_align_up_ptr, MI_SECURE};
use crate::options::_mi_warning_message;
use crate::prim::mi_os_mem_config_t;
use crate::prim::{_mi_prim_commit, _mi_prim_protect};
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

#[inline]
fn mi_os_page_align_areax(
    conservative: bool,
    addr: *mut c_void,
    size: usize,
    newsize: *mut usize,
) -> *mut c_void {
    crate::mi_assert!(!addr.is_null() && size > 0);
    if !newsize.is_null() {
        unsafe { *newsize = 0 };
    }
    if addr.is_null() || size == 0 {
        return core::ptr::null_mut();
    }

    let page_size = unsafe { _mi_os_page_size() };
    let start = if conservative {
        _mi_align_up_ptr(addr.cast_const(), page_size)
    } else {
        _mi_align_down_ptr(addr.cast_const(), page_size)
    };
    let end_addr = unsafe { addr.cast::<u8>().add(size) };
    let end = if conservative {
        _mi_align_down_ptr(end_addr.cast::<c_void>().cast_const(), page_size)
    } else {
        _mi_align_up_ptr(end_addr.cast::<c_void>().cast_const(), page_size)
    };

    let diff = (end as usize).wrapping_sub(start as usize);
    if end as usize <= start as usize {
        return core::ptr::null_mut();
    }

    crate::mi_assert_internal!((conservative && diff <= size) || (!conservative && diff >= size));
    if !newsize.is_null() {
        unsafe { *newsize = diff };
    }
    start
}

#[inline]
fn mi_os_page_align_area_conservative(
    addr: *mut c_void,
    size: usize,
    newsize: *mut usize,
) -> *mut c_void {
    mi_os_page_align_areax(true, addr, size, newsize)
}

#[inline]
#[allow(clippy::absurd_extreme_comparisons)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_size() -> usize {
    if MI_SECURE > 0 {
        unsafe { _mi_os_guard_page_size() }
    } else {
        0
    }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
pub unsafe extern "C" fn _mi_os_commit_ex(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
    stat_size: usize,
) -> bool {
    let _ = stat_size;
    if !is_zero.is_null() {
        unsafe { *is_zero = false };
    }

    let mut csize = 0;
    let start = mi_os_page_align_areax(false, addr, size, &mut csize);
    if csize == 0 {
        return true;
    }

    let mut os_is_zero = false;
    let err = unsafe { _mi_prim_commit(start, csize, &mut os_is_zero) };
    if err != 0 {
        _mi_warning_message(format_args!(
            "cannot commit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            err, err, start, csize
        ));
        return false;
    }
    if os_is_zero && !is_zero.is_null() {
        unsafe { *is_zero = true };
    }
    true
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
pub unsafe extern "C" fn _mi_os_commit(addr: *mut c_void, size: usize, is_zero: *mut bool) -> bool {
    unsafe { _mi_os_commit_ex(addr, size, is_zero, size) }
}

#[inline]
unsafe fn mi_os_protectx(addr: *mut c_void, size: usize, protect: bool) -> bool {
    let mut csize = 0;
    let start = mi_os_page_align_area_conservative(addr, size, &mut csize);
    if csize == 0 {
        return false;
    }
    let err = unsafe { _mi_prim_protect(start, csize, protect) };
    if err != 0 {
        _mi_warning_message(format_args!(
            "cannot {} OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            if protect { "protect" } else { "unprotect" },
            err,
            err,
            start,
            csize
        ));
    }
    err == 0
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
pub unsafe extern "C" fn _mi_os_protect(addr: *mut c_void, size: usize) -> bool {
    unsafe { mi_os_protectx(addr, size, true) }
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_os_commit(p: *mut c_void, size: usize) {
    let _ = unsafe { _mi_os_commit(p, size, core::ptr::null_mut()) };
}
