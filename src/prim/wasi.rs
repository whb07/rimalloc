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

use core::ffi::{c_int, c_void};
use core::ptr::null_mut;

use crate::internal::{_mi_align_up, _mi_align_up_ptr};
use crate::os::_mi_os_page_size;
use crate::prim::{mi_os_mem_config_t, mi_process_info_t};
use crate::types::mi_theap_t;

pub const MI_WASI_PRIM_C: () = ();
const ENOMEM: c_int = 12;

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
        if p as isize == -1 { null_mut() } else { p }
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
pub unsafe extern "C" fn _mi_prim_alloc(
    hint_addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
    addr: *mut *mut c_void,
) -> c_int {
    let _ = hint_addr;
    let _ = commit;
    let _ = allow_large;
    if !is_large.is_null() {
        *is_large = false;
    }
    if !is_zero.is_null() {
        *is_zero = false;
    }
    let p = mi_prim_mem_grow(size, try_alignment);
    if !addr.is_null() {
        *addr = p;
    }
    if p.is_null() { ENOMEM } else { 0 }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> c_int {
    let _ = addr;
    let _ = size;
    if !is_zero.is_null() {
        *is_zero = false;
    }
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
pub unsafe extern "C" fn _mi_prim_reuse(addr: *mut c_void, size: usize) -> c_int {
    let _ = addr;
    let _ = size;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_protect(addr: *mut c_void, size: usize, protect: bool) -> c_int {
    let _ = addr;
    let _ = size;
    let _ = protect;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node() -> usize {
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node_count() -> usize {
    1
}

#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    let _ = pinfo;
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
