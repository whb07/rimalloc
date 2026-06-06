//! Bounded mechanical translation of `src/prim/wasi/prim.c`.
//!
//! This file intentionally covers an early standalone helper slice only.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;
use core::ptr::null_mut;

use crate::internal::{_mi_align_up, _mi_align_up_ptr};
use crate::os::_mi_os_page_size;
use crate::prim::mi_os_mem_config_t;
use crate::types::mi_theap_t;

pub const MI_WASI_PRIM_C: () = ();

#[cfg(target_os = "wasi")]
unsafe extern "C" {
    fn sbrk(increment: isize) -> *mut c_void;
}

#[inline]
pub unsafe fn mi_memory_grow(size: usize) -> *mut c_void {
    #[cfg(target_os = "wasi")]
    {
        let increment = match isize::try_from(size) {
            Ok(n) => n,
            Err(_) => return null_mut(),
        };
        let p = sbrk(increment);
        if p as isize == -1 {
            null_mut()
        } else {
            p
        }
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let _ = size;
        null_mut()
    }
}

#[inline]
unsafe fn mi_prim_mem_grow(size: usize, try_alignment: usize) -> *mut c_void {
    if try_alignment <= 1 {
        return mi_memory_grow(size);
    }
    let current = mi_memory_grow(0);
    if current.is_null() {
        return null_mut();
    }
    let aligned_current = _mi_align_up_ptr(current, try_alignment);
    let alloc_size = _mi_align_up(
        ((aligned_current as usize) - (current as usize)).saturating_add(size),
        _mi_os_page_size(),
    );
    let base = mi_memory_grow(alloc_size);
    if base.is_null() {
        return null_mut();
    }
    let p = _mi_align_up_ptr(base, try_alignment);
    if (p as usize).saturating_add(size) > (base as usize).saturating_add(alloc_size) {
        null_mut()
    } else {
        p
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    if config.is_null() {
        return;
    }
    (*config).page_size = 64 * crate::bits::MI_KiB;
    (*config).alloc_granularity = 16;
    (*config).has_overcommit = false;
    (*config).has_partial_free = false;
    (*config).has_virtual_reserve = false;
}

#[inline]
pub unsafe extern "C" fn _mi_prim_free(addr: *mut c_void, size: usize) -> i32 {
    let _ = addr;
    let _ = size;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_decommit(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
) -> i32 {
    let _ = addr;
    let _ = size;
    if !needs_recommit.is_null() {
        *needs_recommit = false;
    }
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_reset(addr: *mut c_void, size: usize) -> i32 {
    let _ = addr;
    let _ = size;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node() -> usize {
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    let _ = theap;
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_is_in_threadpool() -> bool {
    false
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_yield() {
    std::thread::yield_now();
}

#[inline]
pub unsafe fn mi_wasi_try_alloc(size: usize, try_alignment: usize) -> *mut c_void {
    mi_prim_mem_grow(size, try_alignment)
}
