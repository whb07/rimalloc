//! Bounded mechanical translation of `src/prim/emscripten/prim.c`.
//!
//! This file intentionally covers the early primitive-layer surface only.

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::CStr;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bits::MI_KiB;
use crate::prim::{mi_msecs_t, mi_os_mem_config_t, mi_process_info_t};
use crate::types::mi_theap_t;

pub const MI_EMSCRIPTEN_PRIM_C: () = ();
pub const MIN_EMMALLOC_ALIGN: usize = 8;

const ENOMEM: c_int = 12;
const ENOSYS: c_int = 38;
const PTHREAD_KEY_INVALID: usize = usize::MAX;

type pthread_key_t = usize;

#[cfg(target_os = "emscripten")]
// SAFETY[AUDIT:emscripten-import-boundary]: declares Emscripten primitive
// imports only; callers validate pointers, sizes, and fallback behavior locally.
unsafe extern "C" {
    fn emmalloc_free(p: *mut c_void);
    fn emmalloc_memalign(alignment: usize, size: usize) -> *mut c_void;
    fn emscripten_date_now() -> f64;
    fn emscripten_console_error(msg: *const c_char);
    fn getentropy(buf: *mut c_void, len: usize) -> c_int;
    fn pthread_key_create(
        key: *mut pthread_key_t,
        // SAFETY[AUDIT:emscripten-import-boundary]: the pthread destructor
        // callback uses the C ABI and receives the associated thread heap value.
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn pthread_key_delete(key: pthread_key_t) -> c_int;
    fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;
    fn sched_yield() -> c_int;
}

thread_local! {
    static MI_EM_PTHREAD_DEFAULT_THEAP: std::cell::Cell<*mut c_void> = const { std::cell::Cell::new(ptr::null_mut()) };
}

static mut _mi_heap_default_key: pthread_key_t = PTHREAD_KEY_INVALID;

/// # Safety
///
/// `config`, when non-null, must point to writable `mi_os_mem_config_t` storage
/// owned by the primitive initialization caller.
// SAFETY[AUDIT:emscripten-memory-boundary]: C callers may pass a nullable
// config out-pointer; non-null storage is initialized with primitive constants.
#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    // SAFETY[AUDIT:emscripten-memory-boundary]: the function contract
    // requires any non-null config pointer to be writable config storage.
    let Some(config) = (unsafe { config.as_mut() }) else {
        return;
    };
    config.page_size = 64 * MI_KiB;
    config.alloc_granularity = 16;
    config.has_overcommit = false;
    config.has_partial_free = false;
    config.has_virtual_reserve = false;
}

/// # Safety
///
/// `addr` must be either null/unused for `size == 0`, or a pointer previously
/// returned by the Emscripten primitive allocator for a compatible allocation.
// SAFETY[AUDIT:emscripten-memory-boundary]: this C ABI primitive free shim
// delegates to emmalloc only for non-empty Emscripten allocations.
#[inline]
pub unsafe extern "C" fn _mi_prim_free(addr: *mut c_void, size: usize) -> c_int {
    if size == 0 {
        return 0;
    }
    #[cfg(target_os = "emscripten")]
    {
        // SAFETY[AUDIT:emscripten-memory-boundary]: non-empty frees are for
        // pointers returned by the Emscripten primitive allocator.
        unsafe { emmalloc_free(addr) };
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        let _ = addr;
    }
    0
}

/// # Safety
///
/// `is_large`, `is_zero`, and `addr`, when non-null, must be valid writable
/// out-pointers for the duration of the call.
// SAFETY[AUDIT:emscripten-memory-boundary]: C callers provide nullable
// out-pointers and allocation parameters; the backend publishes only emmalloc or
// fallback allocation results.
#[inline]
pub unsafe extern "C" fn _mi_prim_alloc(
    hint_addr: *mut c_void,
    size: usize,
    mut try_alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
    addr: *mut *mut c_void,
) -> c_int {
    let _ = hint_addr;
    let _ = commit;
    let _ = allow_large;
    // SAFETY[AUDIT:emscripten-memory-boundary]: non-null out-pointers are
    // caller-provided writable slots for primitive allocation result flags.
    if let Some(is_large) = unsafe { is_large.as_mut() } {
        *is_large = false;
    }
    // SAFETY[AUDIT:emscripten-memory-boundary]: non-null out-pointers are
    // caller-provided writable slots for primitive allocation result flags.
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = false;
    }
    if try_alignment < MIN_EMMALLOC_ALIGN {
        try_alignment = MIN_EMMALLOC_ALIGN;
    }
    #[cfg(target_os = "emscripten")]
    {
        // SAFETY[AUDIT:emscripten-memory-boundary]: alignment is clamped to
        // emmalloc's minimum and the returned pointer is checked for null.
        let p = unsafe { emmalloc_memalign(try_alignment, size) };
        // SAFETY[AUDIT:emscripten-memory-boundary]: non-null `addr` is
        // caller-provided writable storage for the allocation result pointer.
        if let Some(addr) = unsafe { addr.as_mut() } {
            *addr = p;
        }
        if p.is_null() { ENOMEM } else { 0 }
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        let _ = size;
        let _ = try_alignment;
        // SAFETY[AUDIT:emscripten-memory-boundary]: non-null `addr` is
        // caller-provided writable storage for the fallback null pointer.
        if let Some(addr) = unsafe { addr.as_mut() } {
            *addr = core::ptr::null_mut();
        }
        ENOMEM
    }
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer. `is_zero`, when non-null, must be a valid writable
/// out-pointer.
// SAFETY[AUDIT:emscripten-memory-boundary]: commit is a compatibility no-op
// and writes only the nullable zero-fill out-pointer when supplied.
#[inline]
pub unsafe extern "C" fn _mi_prim_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> c_int {
    let _ = addr;
    let _ = size;
    // SAFETY[AUDIT:emscripten-memory-boundary]: non-null `is_zero` is a
    // caller-provided writable zero-fill result slot.
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
// SAFETY[AUDIT:emscripten-memory-boundary]: decommit is a compatibility
// no-op and writes only the nullable recommit out-pointer when supplied.
#[inline]
pub unsafe extern "C" fn _mi_prim_decommit(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
) -> c_int {
    let _ = addr;
    let _ = size;
    // SAFETY[AUDIT:emscripten-memory-boundary]: non-null `needs_recommit` is
    // a caller-provided writable recommit result slot.
    if let Some(needs_recommit) = unsafe { needs_recommit.as_mut() } {
        *needs_recommit = false;
    }
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer.
// SAFETY[AUDIT:emscripten-memory-boundary]: reset is a C ABI compatibility
// no-op over a caller-supplied primitive allocation range.
#[inline]
pub unsafe extern "C" fn _mi_prim_reset(addr: *mut c_void, size: usize) -> c_int {
    let _ = addr;
    let _ = size;
    0
}

/// # Safety
///
/// `addr..addr + size` must describe an allocation range managed by the
/// primitive layer.
// SAFETY[AUDIT:emscripten-memory-boundary]: reuse is a C ABI compatibility
// no-op over a caller-supplied primitive allocation range.
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
// SAFETY[AUDIT:emscripten-memory-boundary]: protect is a C ABI compatibility
// no-op over a caller-supplied primitive allocation range and protection flag.
#[inline]
pub unsafe extern "C" fn _mi_prim_protect(addr: *mut c_void, size: usize, protect: bool) -> c_int {
    let _ = addr;
    let _ = size;
    let _ = protect;
    0
}

/// # Safety
///
/// `is_zero` and `addr`, when non-null, must be valid writable out-pointers for
/// the duration of the call.
// SAFETY[AUDIT:emscripten-memory-boundary]: huge OS pages are unsupported
// here, so nullable result out-pointers are initialized to failure values.
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
    // SAFETY[AUDIT:emscripten-memory-boundary]: non-null `is_zero` is a
    // caller-provided writable huge-page result slot.
    if let Some(is_zero) = unsafe { is_zero.as_mut() } {
        *is_zero = true;
    }
    // SAFETY[AUDIT:emscripten-memory-boundary]: non-null `addr` is
    // caller-provided writable storage for the unsupported result pointer.
    if let Some(addr) = unsafe { addr.as_mut() } {
        *addr = core::ptr::null_mut();
    }
    ENOSYS
}

/// # Safety
///
/// Must be called only in primitive-layer contexts where reading the current
/// NUMA node is permitted.
// SAFETY[AUDIT:emscripten-clock-io-boundary]: Emscripten exposes a single
// primitive NUMA domain in this backend.
#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node() -> usize {
    0
}

/// # Safety
///
/// Must be called only in primitive-layer contexts where reading the process
/// NUMA topology is permitted.
// SAFETY[AUDIT:emscripten-clock-io-boundary]: Emscripten exposes a single
// primitive NUMA domain in this backend.
#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node_count() -> usize {
    1
}

/// # Safety
///
/// Must be called only in primitive-layer contexts where querying the platform
/// clock is permitted.
// SAFETY[AUDIT:emscripten-clock-io-boundary]: clock queries use the
// Emscripten date import when available and a non-Emscripten host fallback.
#[inline]
pub unsafe extern "C" fn _mi_prim_clock_now() -> mi_msecs_t {
    #[cfg(target_os = "emscripten")]
    {
        // SAFETY[AUDIT:emscripten-clock-io-boundary]: Emscripten date import
        // returns process time in milliseconds and does not retain pointers.
        unsafe { emscripten_date_now() as mi_msecs_t }
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as mi_msecs_t)
            .unwrap_or(0)
    }
}

/// # Safety
///
/// `pinfo`, when non-null, must point to writable process-info storage owned by
/// the caller.
// SAFETY[AUDIT:emscripten-clock-io-boundary]: process-info collection is
// unsupported in this bounded backend and does not dereference the output pointer.
#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    let _ = pinfo;
}

/// # Safety
///
/// `msg` must be either null or point to a valid NUL-terminated C string for
/// the duration of the call.
// SAFETY[AUDIT:emscripten-clock-io-boundary]: console output accepts nullable C
// strings and writes either through Emscripten console or host stderr fallback.
#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    #[cfg(target_os = "emscripten")]
    {
        // SAFETY[AUDIT:emscripten-clock-io-boundary]: `msg` was checked
        // non-null and the contract requires a NUL-terminated C string.
        unsafe { emscripten_console_error(msg) };
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        // SAFETY[AUDIT:emscripten-clock-io-boundary]: `msg` was checked
        // non-null and the contract requires a NUL-terminated C string.
        let bytes = unsafe { CStr::from_ptr(msg) }.to_bytes();
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(bytes);
        let _ = stderr.flush();
    }
}

/// # Safety
///
/// `name` must be a valid NUL-terminated C string when non-null. `result`, when
/// non-null, must point to a writable buffer of `result_size` bytes.
// SAFETY[AUDIT:emscripten-clock-io-boundary]: environment lookup is unsupported
// in this bounded backend and never dereferences its raw C buffers.
#[inline]
pub unsafe extern "C" fn _mi_prim_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    let _ = name;
    let _ = result;
    let _ = result_size;
    false
}

/// # Safety
///
/// `value`, when non-null, must be a thread heap pointer previously associated
/// with the Emscripten pthread destructor key.
// SAFETY[AUDIT:emscripten-pthread-key-boundary]: pthread destructors receive the
// raw thread heap pointer previously associated with the default-heap key.
#[inline]
pub unsafe extern "C" fn mi_pthread_done(value: *mut c_void) {
    if value.is_null() {
        return;
    }
    // SAFETY[AUDIT:emscripten-pthread-key-boundary]: non-null `value` is the
    // thread heap pointer installed by `_mi_prim_thread_associate_default_theap`.
    unsafe { crate::internal::_mi_thread_done(value.cast::<mi_theap_t>()) };
}

/// # Safety
///
/// Must be called during primitive thread initialization while no Emscripten
/// default-heap key is active for this process.
// SAFETY[AUDIT:emscripten-pthread-key-boundary]: initializes the process-global
// pthread default-heap key once and records the fallback key state.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {
    // SAFETY[AUDIT:emscripten-pthread-key-boundary]: reading the process-global
    // key is serialized by primitive thread initialization.
    let key_is_invalid = unsafe { _mi_heap_default_key == PTHREAD_KEY_INVALID };
    crate::internal::mi_assert_internal!(key_is_invalid);
    #[cfg(target_os = "emscripten")]
    {
        let mut key = PTHREAD_KEY_INVALID;
        // SAFETY[AUDIT:emscripten-pthread-key-boundary]: `key` is local writable
        // storage and the destructor has the expected pthread C ABI.
        if unsafe { pthread_key_create(&mut key, Some(mi_pthread_done)) } == 0 {
            // SAFETY[AUDIT:emscripten-pthread-key-boundary]: publishing the key is
            // part of the serialized primitive thread initialization path.
            unsafe { _mi_heap_default_key = key };
        }
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        // SAFETY[AUDIT:emscripten-pthread-key-boundary]: non-Emscripten tests use
        // `0` as the initialized fallback key sentinel.
        unsafe { _mi_heap_default_key = 0 };
    }
}

/// # Safety
///
/// Must be called during primitive thread teardown or process teardown after
/// `_mi_prim_thread_init_auto_done` has initialized the key.
// SAFETY[AUDIT:emscripten-pthread-key-boundary]: tears down the process-global
// pthread default-heap key and clears fallback TLS state.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {
    // SAFETY[AUDIT:emscripten-pthread-key-boundary]: key teardown owns the
    // process-global key state and snapshots it before platform deletion.
    let key = unsafe { _mi_heap_default_key };
    if key != PTHREAD_KEY_INVALID {
        #[cfg(target_os = "emscripten")]
        {
            // SAFETY[AUDIT:emscripten-pthread-key-boundary]: `key` was the
            // initialized pthread key snapshot for this teardown.
            let _ = unsafe { pthread_key_delete(key) };
        }
        #[cfg(not(target_os = "emscripten"))]
        {
            MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(ptr::null_mut()));
        }
        // SAFETY[AUDIT:emscripten-pthread-key-boundary]: teardown publishes the
        // invalid sentinel after platform/fallback state is cleared.
        unsafe { _mi_heap_default_key = PTHREAD_KEY_INVALID };
    }
}

/// # Safety
///
/// `theap` must be null or a valid thread heap pointer whose lifetime covers
/// subsequent primitive thread-done callbacks.
// SAFETY[AUDIT:emscripten-pthread-key-boundary]: associates a nullable default
// thread heap pointer with the initialized pthread key or fallback TLS cell.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    // SAFETY[AUDIT:emscripten-pthread-key-boundary]: association snapshots the
    // process-global key before publishing the thread-local heap pointer.
    let key = unsafe { _mi_heap_default_key };
    if key != PTHREAD_KEY_INVALID {
        #[cfg(target_os = "emscripten")]
        {
            // SAFETY[AUDIT:emscripten-pthread-key-boundary]: `key` is initialized
            // and `theap` is the caller-provided thread heap association value.
            let _ = unsafe { pthread_setspecific(key, theap.cast_const().cast()) };
        }
        #[cfg(not(target_os = "emscripten"))]
        {
            MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(theap.cast()));
        }
    }
}

/// # Safety
///
/// Must be called only from primitive thread-state queries.
// SAFETY[AUDIT:emscripten-thread-shim-boundary]: Emscripten reports no internal
// threadpool membership through this compatibility query.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_is_in_threadpool() -> bool {
    false
}

/// # Safety
///
/// Must be called only in contexts where yielding the current thread is
/// permitted by the embedding runtime.
// SAFETY[AUDIT:emscripten-thread-shim-boundary]: yielding delegates to the
// Emscripten scheduler when available or Rust's host thread yield fallback.
#[inline]
pub unsafe extern "C" fn _mi_prim_thread_yield() {
    #[cfg(target_os = "emscripten")]
    {
        // SAFETY[AUDIT:emscripten-thread-shim-boundary]: scheduler yield has no
        // pointer arguments and does not inspect allocator state.
        let _ = unsafe { sched_yield() };
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        std::thread::yield_now();
    }
}
