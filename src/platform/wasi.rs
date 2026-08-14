//! Bounded mechanical translation of `src/prim/wasi/prim.c`.
//!
//! This file intentionally covers an early standalone helper slice only.

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use core::ffi::{c_char, c_int, c_void};
use core::ptr::null_mut;
use std::ffi::CStr;
use std::io::{self, Write};

use crate::internal::{
    _mi_align_up, _mi_align_up_ptr, _mi_preloading, _mi_strlcpy, _mi_strnlen, _mi_toupper,
};
use crate::os::_mi_os_page_size_typed;
use crate::prim::{mi_os_mem_config_t, mi_process_info_t};
use crate::types::mi_theap_t;

pub const MI_WASI_PRIM_C: () = ();
const ENOMEM: c_int = 12;
const ENOSYS: c_int = 38;

unsafe extern "C" {
    fn getenv(name: *const c_char) -> *const c_char;
}

#[cfg(target_os = "wasi")]
unsafe extern "C" {
    fn sbrk(increment: isize) -> *mut c_void;
}

/// # Safety
///
/// Must be called only while the WASI linear-memory/sbrk interface is valid for
/// the process. The returned memory, when non-null, is uninitialized and owned
/// by the primitive allocator.
#[inline]
pub unsafe fn mi_memory_grow(size: usize) -> *mut c_void {
    #[cfg(target_os = "wasi")]
    {
        let increment = match isize::try_from(size) {
            Ok(n) => n,
            Err(_) => return null_mut(),
        };
        let p = unsafe { sbrk(increment) };
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
        return unsafe { mi_memory_grow(size) };
    }
    let current = unsafe { mi_memory_grow(0) };
    if current.is_null() {
        return null_mut();
    }
    let aligned_current = _mi_align_up_ptr(current, try_alignment);
    let alloc_size = _mi_align_up(
        ((aligned_current as usize) - (current as usize)).saturating_add(size),
        _mi_os_page_size_typed().get(),
    );
    let base = unsafe { mi_memory_grow(alloc_size) };
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

/// # Safety
///
/// `config`, when non-null, must point to writable `mi_os_mem_config_t` storage
/// owned by the primitive initialization caller.
#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    let Some(config) = (unsafe { config.as_mut() }) else {
        return;
    };
    config.page_size = 64 * crate::bits::MI_KiB;
    config.alloc_granularity = 16;
    config.has_overcommit = false;
    config.has_partial_free = false;
    config.has_virtual_reserve = false;
}

/// # Safety
///
/// `addr..addr + size` must describe a range previously returned by the WASI
/// primitive allocator, though this backend cannot release individual ranges.
#[inline]
pub unsafe extern "C" fn _mi_prim_free(addr: *mut c_void, size: usize) -> i32 {
    let _ = addr;
    let _ = size;
    0
}

/// # Safety
///
/// `is_large`, `is_zero`, and `addr`, when non-null, must be valid writable
/// out-pointers for the duration of the call.
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
    if let Some(is_large) = unsafe { is_large.as_mut() } {
        *is_large = false;
    }
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = false;
    }
    let p = unsafe { mi_prim_mem_grow(size, try_alignment) };
    if let Some(addr) = unsafe { addr.as_mut() } {
        *addr = p;
    }
    if p.is_null() { ENOMEM } else { 0 }
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer. `is_zero`, when non-null, must be a valid writable
/// out-pointer.
#[inline]
pub unsafe extern "C" fn _mi_prim_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> c_int {
    let _ = addr;
    let _ = size;
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = false;
    }
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer. `needs_recommit`, when non-null, must be a valid writable
/// out-pointer.
#[inline]
pub unsafe extern "C" fn _mi_prim_decommit(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
) -> i32 {
    let _ = addr;
    let _ = size;
    if let Some(needs_recommit) = unsafe { needs_recommit.as_mut() } {
        *needs_recommit = false;
    }
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer.
#[inline]
pub unsafe extern "C" fn _mi_prim_reset(addr: *mut c_void, size: usize) -> i32 {
    let _ = addr;
    let _ = size;
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer.
#[inline]
pub unsafe extern "C" fn _mi_prim_reuse(addr: *mut c_void, size: usize) -> c_int {
    let _ = addr;
    let _ = size;
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer.
#[inline]
pub unsafe extern "C" fn _mi_prim_protect(addr: *mut c_void, size: usize, protect: bool) -> c_int {
    let _ = addr;
    let _ = size;
    let _ = protect;
    0
}

/// # Safety
///
/// Must be called only in primitive-layer contexts where reading the current
/// NUMA node is permitted.
#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node() -> usize {
    0
}

/// # Safety
///
/// Must be called only in primitive-layer contexts where reading the process
/// NUMA topology is permitted.
#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node_count() -> usize {
    1
}

/// # Safety
///
/// `is_zero` and `addr`, when non-null, must be valid writable out-pointers for
/// the duration of the call.
#[inline]
pub unsafe extern "C" fn _mi_prim_alloc_huge_os_pages(
    hint_addr: *mut c_void,
    size: usize,
    numa_node: c_int,
    is_zero: *mut bool,
    addr: *mut *mut c_void,
) -> c_int {
    let _ = hint_addr;
    let _ = size;
    let _ = numa_node;
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = true;
    }
    if let Some(addr) = unsafe { addr.as_mut() } {
        *addr = null_mut();
    }
    ENOSYS
}

/// # Safety
///
/// `pinfo`, when non-null, must point to writable process-info storage owned by
/// the caller.
#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    let _ = pinfo;
}

/// # Safety
///
/// `msg` must be either null or point to a valid NUL-terminated C string for
/// the duration of the call.
#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let bytes = unsafe { CStr::from_ptr(msg) }.to_bytes();
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(bytes);
    let _ = stderr.flush();
}

/// # Safety
///
/// `name` must be a valid NUL-terminated C string when non-null. `result`, when
/// non-null, must point to a writable buffer of `result_size` bytes.
#[inline]
pub unsafe extern "C" fn _mi_prim_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    if name.is_null() || result.is_null() || result_size == 0 || _mi_preloading() {
        return false;
    }

    let mut s = unsafe { getenv(name) };
    if s.is_null() {
        let mut buf = [0 as c_char; 65];
        let len = unsafe { _mi_strnlen(name, buf.len() - 1) };
        for (i, slot) in buf.iter_mut().take(len).enumerate() {
            let ch = unsafe { *name.add(i) };
            *slot = _mi_toupper(ch);
        }
        buf[len] = 0;
        s = unsafe { getenv(buf.as_ptr()) };
    }
    if s.is_null() || unsafe { _mi_strnlen(s, result_size) } >= result_size {
        return false;
    }
    unsafe { _mi_strlcpy(result, s, result_size) };
    true
}

/// # Safety
///
/// Must be called during primitive thread initialization.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {
    // nothing
}

/// # Safety
///
/// Must be called during primitive thread teardown or process teardown.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {}

/// # Safety
///
/// `theap` must be null or a valid thread heap pointer for the current thread.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    let _ = theap;
}

/// # Safety
///
/// Must be called only from primitive thread-state queries.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_is_in_threadpool() -> bool {
    false
}

/// # Safety
///
/// Must be called only in contexts where yielding the current thread is
/// permitted by the embedding runtime.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_yield() {
    std::thread::yield_now();
}

/// # Safety
///
/// Must be called only while the WASI primitive memory-growth interface is
/// available. The returned memory, when non-null, is owned by the primitive
/// allocator.
#[inline]
pub unsafe fn mi_wasi_try_alloc(size: usize, try_alignment: usize) -> *mut c_void {
    unsafe { mi_prim_mem_grow(size, try_alignment) }
}
