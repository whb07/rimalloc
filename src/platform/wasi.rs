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

// SAFETY: declares the C environment lookup used
// by the primitive backend without taking ownership of returned storage.
unsafe extern "C" {
    fn getenv(name: *const c_char) -> *const c_char;
}

#[cfg(target_os = "wasi")]
// SAFETY: declares WASI linear-memory growth;
// callers convert sizes and handle the C `-1` failure sentinel before use.
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
        // SAFETY: the increment fits `isize`,
        // and the returned WASI `sbrk` sentinel is checked before publication.
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
        // SAFETY: forwards primitive allocation
        // growth to the bounded WASI/non-WASI memory-growth shim.
        return unsafe { mi_memory_grow(size) };
    }
    // SAFETY: querying the current break uses a
    // zero-sized growth request and still checks for unsupported/null results.
    let current = unsafe { mi_memory_grow(0) };
    if current.is_null() {
        return null_mut();
    }
    let aligned_current = _mi_align_up_ptr(current, try_alignment);
    let alloc_size = _mi_align_up(
        ((aligned_current as usize) - (current as usize)).saturating_add(size),
        _mi_os_page_size_typed().get(),
    );
    // SAFETY: `alloc_size` includes page-aligned
    // padding needed to return an aligned pointer inside the grown range.
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
// SAFETY: C callers may pass a nullable config
// out-pointer; non-null storage is initialized with WASI primitive constants.
#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    // SAFETY: the function contract requires any
    // non-null config pointer to be valid writable storage for one config value.
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
// SAFETY: this C ABI primitive free shim accepts
// raw ranges but intentionally performs no per-range release for WASI.
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
// SAFETY: C callers provide nullable out-pointers
// and allocation parameters; the backend publishes only the primitive grow result.
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
    // SAFETY: non-null out-pointers are caller
    // provided writable slots for primitive allocation result flags.
    if let Some(is_large) = unsafe { is_large.as_mut() } {
        *is_large = false;
    }
    // SAFETY: non-null out-pointers are caller
    // provided writable slots for primitive allocation result flags.
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = false;
    }
    // SAFETY: this shim owns the primitive growth
    // attempt and returns C-compatible `ENOMEM` on null.
    let p = unsafe { mi_prim_mem_grow(size, try_alignment) };
    // SAFETY: non-null `addr` is caller-provided
    // writable storage for the allocated primitive pointer.
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
// SAFETY: commit is a compatibility no-op and
// writes only the nullable zero-fill out-pointer when supplied.
#[inline]
pub unsafe extern "C" fn _mi_prim_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> c_int {
    let _ = addr;
    let _ = size;
    // SAFETY: non-null `is_zero` is a caller
    // provided writable zero-fill result slot.
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
// SAFETY: decommit is a compatibility no-op and
// writes only the nullable recommit out-pointer when supplied.
#[inline]
pub unsafe extern "C" fn _mi_prim_decommit(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
) -> i32 {
    let _ = addr;
    let _ = size;
    // SAFETY: non-null `needs_recommit` is a
    // caller-provided writable recommit result slot.
    if let Some(needs_recommit) = unsafe { needs_recommit.as_mut() } {
        *needs_recommit = false;
    }
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer.
// SAFETY: reset is a C ABI compatibility no-op
// over a caller-supplied primitive allocation range.
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
// SAFETY: reuse is a C ABI compatibility no-op
// over a caller-supplied primitive allocation range.
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
// SAFETY: protect is a C ABI compatibility no-op
// over a caller-supplied primitive allocation range and protection flag.
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
// SAFETY: WASI exposes a single primitive NUMA
// domain in this backend.
#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node() -> usize {
    0
}

/// # Safety
///
/// Must be called only in primitive-layer contexts where reading the process
/// NUMA topology is permitted.
// SAFETY: WASI exposes a single primitive NUMA
// domain in this backend.
#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node_count() -> usize {
    1
}

/// # Safety
///
/// `is_zero` and `addr`, when non-null, must be valid writable out-pointers for
/// the duration of the call.
// SAFETY: huge OS pages are unsupported here, so
// nullable result out-pointers are initialized to C-compatible failure values.
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
    // SAFETY: non-null `is_zero` is a caller
    // provided writable huge-page result slot.
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = true;
    }
    // SAFETY: non-null `addr` is caller-provided
    // writable storage for the unsupported huge-page result pointer.
    if let Some(addr) = unsafe { addr.as_mut() } {
        *addr = null_mut();
    }
    ENOSYS
}

/// # Safety
///
/// `pinfo`, when non-null, must point to writable process-info storage owned by
/// the caller.
// SAFETY: process-info collection is unsupported
// in this bounded backend and does not dereference the nullable output pointer.
#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    let _ = pinfo;
}

/// # Safety
///
/// `msg` must be either null or point to a valid NUL-terminated C string for
/// the duration of the call.
// SAFETY: stderr output accepts nullable C
// strings and writes bytes directly to the process stderr stream.
#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    // SAFETY: `msg` was checked non-null and the
    // function contract requires a valid NUL-terminated C string.
    let bytes = unsafe { CStr::from_ptr(msg) }.to_bytes();
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(bytes);
    let _ = stderr.flush();
}

/// # Safety
///
/// `name` must be a valid NUL-terminated C string when non-null. `result`, when
/// non-null, must point to a writable buffer of `result_size` bytes.
// SAFETY: environment lookup accepts nullable C
// buffers, avoids allocator use, and copies only bounded C strings to `result`.
#[inline]
pub unsafe extern "C" fn _mi_prim_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    if name.is_null() || result.is_null() || result_size == 0 || _mi_preloading() {
        return false;
    }

    // SAFETY: `name` is non-null and required to
    // be a valid NUL-terminated C string for the environment lookup.
    let mut s = unsafe { getenv(name) };
    if s.is_null() {
        let mut buf = [0 as c_char; 65];
        // SAFETY: the local buffer reserves one
        // byte for NUL termination, bounding the read from `name`.
        let len = unsafe { _mi_strnlen(name, buf.len() - 1) };
        for (i, slot) in buf.iter_mut().take(len).enumerate() {
            // SAFETY: `i < len`, and `len` was
            // obtained by scanning at most `buf.len() - 1` bytes from `name`.
            let ch = unsafe { *name.add(i) };
            *slot = _mi_toupper(ch);
        }
        buf[len] = 0;
        // SAFETY: `buf` is stack-local and
        // NUL-terminated before the fallback environment lookup.
        s = unsafe { getenv(buf.as_ptr()) };
    }
    // SAFETY: `s` is either null or a C
    // environment string; the length scan is bounded by the result buffer size.
    if s.is_null() || unsafe { _mi_strnlen(s, result_size) } >= result_size {
        return false;
    }
    // SAFETY: `result` is non-null writable
    // storage of `result_size` bytes, and the source length was checked to fit.
    unsafe { _mi_strlcpy(result, s, result_size) };
    true
}

/// # Safety
///
/// Must be called during primitive thread initialization.
// SAFETY: WASI thread auto-init is a compatibility
// shim with no runtime state to mutate in this backend.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {
    // nothing
}

/// # Safety
///
/// Must be called during primitive thread teardown or process teardown.
// SAFETY: WASI thread auto-teardown is a
// compatibility shim with no runtime state to mutate in this backend.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {}

/// # Safety
///
/// `theap` must be null or a valid thread heap pointer for the current thread.
// SAFETY: the default thread-heap association is
// unsupported in this bounded backend and never dereferences the raw pointer.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    let _ = theap;
}

/// # Safety
///
/// Must be called only from primitive thread-state queries.
// SAFETY: WASI reports no internal threadpool
// membership through this compatibility query.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_is_in_threadpool() -> bool {
    false
}

/// # Safety
///
/// Must be called only in contexts where yielding the current thread is
/// permitted by the embedding runtime.
// SAFETY: yielding delegates to Rust's thread
// yield primitive and does not inspect allocator state.
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
    // SAFETY: test/compatibility entry point for
    // the same primitive growth helper used by `_mi_prim_alloc`.
    unsafe { mi_prim_mem_grow(size, try_alignment) }
}
