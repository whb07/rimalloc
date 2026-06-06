//! Bounded mechanical translation of `src/prim/emscripten/prim.c`.
//!
//! This file intentionally covers the early primitive-layer surface only.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

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
unsafe extern "C" {
    fn emmalloc_free(p: *mut c_void);
    fn emmalloc_memalign(alignment: usize, size: usize) -> *mut c_void;
    fn emscripten_date_now() -> f64;
    fn emscripten_console_error(msg: *const c_char);
    fn getentropy(buf: *mut c_void, len: usize) -> c_int;
    fn pthread_key_create(
        key: *mut pthread_key_t,
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

#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    if config.is_null() {
        return;
    }
    (*config).page_size = 64 * MI_KiB;
    (*config).alloc_granularity = 16;
    (*config).has_overcommit = false;
    (*config).has_partial_free = false;
    (*config).has_virtual_reserve = false;
}

#[inline]
pub unsafe extern "C" fn _mi_prim_free(addr: *mut c_void, size: usize) -> c_int {
    if size == 0 {
        return 0;
    }
    #[cfg(target_os = "emscripten")]
    {
        emmalloc_free(addr);
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        let _ = addr;
    }
    0
}

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
    if !is_large.is_null() {
        *is_large = false;
    }
    if !is_zero.is_null() {
        *is_zero = false;
    }
    if try_alignment < MIN_EMMALLOC_ALIGN {
        try_alignment = MIN_EMMALLOC_ALIGN;
    }
    #[cfg(target_os = "emscripten")]
    {
        let p = emmalloc_memalign(try_alignment, size);
        if !addr.is_null() {
            *addr = p;
        }
        if p.is_null() { ENOMEM } else { 0 }
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        let _ = size;
        let _ = try_alignment;
        if !addr.is_null() {
            *addr = core::ptr::null_mut();
        }
        ENOMEM
    }
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
) -> c_int {
    let _ = addr;
    let _ = size;
    if !needs_recommit.is_null() {
        *needs_recommit = false;
    }
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_reset(addr: *mut c_void, size: usize) -> c_int {
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
    if !is_zero.is_null() {
        *is_zero = true;
    }
    if !addr.is_null() {
        *addr = core::ptr::null_mut();
    }
    ENOSYS
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
pub unsafe extern "C" fn _mi_prim_clock_now() -> mi_msecs_t {
    #[cfg(target_os = "emscripten")]
    {
        emscripten_date_now() as mi_msecs_t
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as mi_msecs_t)
            .unwrap_or(0)
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    let _ = pinfo;
}

#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    #[cfg(target_os = "emscripten")]
    {
        emscripten_console_error(msg);
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        let bytes = CStr::from_ptr(msg).to_bytes();
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(bytes);
        let _ = stderr.flush();
    }
}

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

#[inline]
pub unsafe extern "C" fn mi_pthread_done(value: *mut c_void) {
    if !value.is_null() {
        crate::internal::_mi_thread_done(value.cast::<mi_theap_t>());
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {
    crate::mi_assert_internal!(_mi_heap_default_key == PTHREAD_KEY_INVALID);
    #[cfg(target_os = "emscripten")]
    {
        let mut key = PTHREAD_KEY_INVALID;
        if pthread_key_create(&mut key, Some(mi_pthread_done)) == 0 {
            _mi_heap_default_key = key;
        }
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        _mi_heap_default_key = 0;
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {
    if _mi_heap_default_key != PTHREAD_KEY_INVALID {
        #[cfg(target_os = "emscripten")]
        {
            let _ = pthread_key_delete(_mi_heap_default_key);
        }
        #[cfg(not(target_os = "emscripten"))]
        {
            MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(ptr::null_mut()));
        }
        _mi_heap_default_key = PTHREAD_KEY_INVALID;
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    if _mi_heap_default_key != PTHREAD_KEY_INVALID {
        #[cfg(target_os = "emscripten")]
        {
            let _ = pthread_setspecific(_mi_heap_default_key, theap.cast_const().cast());
        }
        #[cfg(not(target_os = "emscripten"))]
        {
            MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(theap.cast()));
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_is_in_threadpool() -> bool {
    false
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_yield() {
    #[cfg(target_os = "emscripten")]
    {
        let _ = sched_yield();
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        std::thread::yield_now();
    }
}
