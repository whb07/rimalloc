//! Primitive OS / platform layer declarations and inline helpers
//!
//! Mechanical translation of:
//! - `include/mimalloc/prim.h`
//! - `src/prim/osx/prim.c`

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

pub mod emscripten;
pub mod osx;
pub mod unix;
pub mod wasi;
pub mod windows;

use core::cell::Cell;
use core::ffi::{c_char, c_void};
use core::ptr;

use crate::bits::{MI_GiB, MI_INTPTR_SIZE};
use crate::init::_mi_thread_id;
use crate::types::{
    mi_heap_t, mi_msecs_t, mi_page_t, mi_theap_t, mi_thread_local_t, mi_threadid_t,
};

pub const MIMALLOC_PRIM_H: () = ();
pub const MI_USE_SBRK: usize = if cfg!(target_os = "wasi") { 1 } else { 0 };
pub const mi_attr_constructor: () = ();
pub const mi_attr_destructor: () = ();
pub const MI_PRIM_HAS_PROCESS_ATTACH: usize = 0;
pub const MI_PRIM_HAS_ALLOCATOR_INIT: usize = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_os_mem_config_t {
    pub page_size: usize,
    pub large_page_size: usize,
    pub alloc_granularity: usize,
    pub physical_memory_in_kib: usize,
    pub virtual_address_bits: usize,
    pub has_overcommit: bool,
    pub has_partial_free: bool,
    pub has_virtual_reserve: bool,
    pub has_transparent_huge_pages: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_process_info_t {
    pub elapsed: mi_msecs_t,
    pub utime: mi_msecs_t,
    pub stime: mi_msecs_t,
    pub current_rss: usize,
    pub peak_rss: usize,
    pub current_commit: usize,
    pub peak_commit: usize,
    pub page_faults: usize,
}

pub const MI_HAS_TLS_SLOT: bool = cfg!(any(windows, target_os = "macos", target_arch = "x86_64"));

pub const MI_TLS_MODEL_THREAD_LOCAL: bool = cfg!(not(any(
    windows,
    target_os = "macos",
    target_os = "openbsd",
    target_os = "android"
)));
pub const MI_TLS_MODEL_FIXED_SLOT: bool = cfg!(all(
    target_os = "macos",
    any(target_arch = "x86_64", target_arch = "aarch64")
));
pub const MI_TLS_MODEL_DYNAMIC_WIN32: bool = cfg!(windows);
pub const MI_TLS_MODEL_DYNAMIC_PTHREADS: bool =
    cfg!(any(target_os = "openbsd", target_os = "android"));
pub const MI_USE_BUILTIN_THREAD_POINTER: bool = cfg!(all(
    not(target_os = "macos"),
    not(target_env = "musl"),
    not(target_os = "cygwin"),
    any(target_arch = "aarch64", target_arch = "x86_64")
));

pub const MI_TLS_MODEL_FIXED_SLOT_DEFAULT: usize = 108;
pub const MI_TLS_MODEL_FIXED_SLOT_CACHED: usize = 109;
pub const MI_THEAP_INITASNULL: bool =
    MI_TLS_MODEL_FIXED_SLOT || MI_TLS_MODEL_DYNAMIC_WIN32 || MI_TLS_MODEL_DYNAMIC_PTHREADS;

pub const MI_TLS_EXPANSION_SLOT: usize = if MI_INTPTR_SIZE == 4 {
    0x0F94 / MI_INTPTR_SIZE
} else {
    0x1780 / MI_INTPTR_SIZE
};

pub const MI_PRIM_HUGE_PAGE_SIZE: usize = MI_GiB;
pub const MI_PRIM_OSX_IMPL_SOURCE: &str = "../unix/prim.c";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_init_done_t;

pub static mi_init_done: mi_init_done_t = mi_init_done_t;

thread_local! {
    static TLS_SLOT_DEFAULT: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static TLS_SLOT_CACHED: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static TLS_THEAP_DEFAULT: Cell<*mut mi_theap_t> = const { Cell::new(ptr::null_mut()) };
    static TLS_THEAP_CACHED: Cell<*mut mi_theap_t> = const { Cell::new(ptr::null_mut()) };
}

unsafe extern "C" {
    pub static _mi_theap_empty: mi_theap_t;

    pub fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t);
    pub fn _mi_prim_free(addr: *mut c_void, size: usize) -> i32;
    pub fn _mi_prim_alloc(
        hint_addr: *mut c_void,
        size: usize,
        try_alignment: usize,
        commit: bool,
        allow_large: bool,
        is_large: *mut bool,
        is_zero: *mut bool,
        addr: *mut *mut c_void,
    ) -> i32;
    pub fn _mi_prim_commit(addr: *mut c_void, size: usize, is_zero: *mut bool) -> i32;
    pub fn _mi_prim_decommit(addr: *mut c_void, size: usize, needs_recommit: *mut bool) -> i32;
    pub fn _mi_prim_reset(addr: *mut c_void, size: usize) -> i32;
    pub fn _mi_prim_reuse(addr: *mut c_void, size: usize) -> i32;
    pub fn _mi_prim_protect(addr: *mut c_void, size: usize, protect: bool) -> i32;
    pub fn _mi_prim_alloc_huge_os_pages(
        hint_addr: *mut c_void,
        size: usize,
        numa_node: i32,
        is_zero: *mut bool,
        addr: *mut *mut c_void,
    ) -> i32;
    pub fn _mi_prim_numa_node() -> usize;
    pub fn _mi_prim_numa_node_count() -> usize;
    pub fn _mi_prim_clock_now() -> mi_msecs_t;
    pub fn _mi_prim_process_info(pinfo: *mut mi_process_info_t);
    pub fn _mi_prim_out_stderr(msg: *const c_char);
    pub fn _mi_prim_getenv(name: *const c_char, result: *mut c_char, result_size: usize) -> bool;
    pub fn _mi_prim_random_buf(buf: *mut c_void, buf_len: usize) -> bool;
    pub fn _mi_prim_thread_init_auto_done();
    pub fn _mi_prim_thread_done_auto_done();
    pub fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t);
    pub fn _mi_prim_thread_is_in_threadpool() -> bool;
    pub fn _mi_prim_thread_yield();

    pub static mut _mi_process_is_initialized: bool;
    pub static mut _mi_theap_default_slot: usize;
    pub static mut _mi_theap_cached_slot: usize;
    pub static mut _mi_theap_default_expansion_slot: usize;
    pub static mut _mi_theap_cached_expansion_slot: usize;
    pub static mut _mi_theap_default_key: mi_thread_local_t;
    pub static mut _mi_theap_cached_key: mi_thread_local_t;

    pub fn _mi_theap_empty_get() -> *mut mi_theap_t;
    pub fn _mi_heap_theap_get_or_init(heap: *const mi_heap_t) -> *mut mi_theap_t;
    pub fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t;
    pub fn _mi_thread_local_get(key: mi_thread_local_t) -> *mut c_void;
}

#[unsafe(no_mangle)]
pub static mut __mi_theap_default: *mut mi_theap_t = ptr::null_mut();

#[unsafe(no_mangle)]
pub static mut __mi_theap_cached: *mut mi_theap_t = ptr::null_mut();

#[inline]
pub fn mi_prim_tls_slot(slot: usize) -> *mut c_void {
    if MI_TLS_MODEL_FIXED_SLOT {
        match slot {
            MI_TLS_MODEL_FIXED_SLOT_DEFAULT => TLS_SLOT_DEFAULT.with(Cell::get),
            MI_TLS_MODEL_FIXED_SLOT_CACHED => TLS_SLOT_CACHED.with(Cell::get),
            _ => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

#[inline]
pub fn mi_prim_tls_slot_set(slot: usize, value: *mut c_void) {
    if MI_TLS_MODEL_FIXED_SLOT {
        match slot {
            MI_TLS_MODEL_FIXED_SLOT_DEFAULT => TLS_SLOT_DEFAULT.with(|cell| cell.set(value)),
            MI_TLS_MODEL_FIXED_SLOT_CACHED => TLS_SLOT_CACHED.with(|cell| cell.set(value)),
            _ => {}
        }
    }
}

#[inline]
pub fn __mi_prim_thread_id() -> mi_threadid_t {
    _mi_thread_id()
}

#[inline]
pub fn _mi_prim_thread_id() -> mi_threadid_t {
    let tid = __mi_prim_thread_id();
    debug_assert!(tid > 1);
    debug_assert_eq!(tid & 0x03, 0);
    tid
}

#[inline]
pub unsafe fn __mi_theap_empty() -> *mut mi_theap_t {
    core::ptr::addr_of!(_mi_theap_empty).cast_mut()
}

#[inline]
unsafe fn mi_prim_tls_slot_theap(slot: usize, expansion_slot: usize) -> *mut mi_theap_t {
    let theap = mi_prim_tls_slot(slot) as *mut mi_theap_t;
    if cfg!(windows) && slot == MI_TLS_EXPANSION_SLOT && !theap.is_null() {
        (theap as *mut *mut mi_theap_t).add(expansion_slot).read()
    } else {
        theap
    }
}

#[inline]
pub unsafe fn _mi_theap_default() -> *mut mi_theap_t {
    if MI_TLS_MODEL_THREAD_LOCAL {
        TLS_THEAP_DEFAULT.with(|cell| {
            let theap = cell.get();
            if theap.is_null() {
                __mi_theap_empty()
            } else {
                theap
            }
        })
    } else if MI_TLS_MODEL_FIXED_SLOT {
        mi_prim_tls_slot(MI_TLS_MODEL_FIXED_SLOT_DEFAULT) as *mut mi_theap_t
    } else if MI_TLS_MODEL_DYNAMIC_WIN32 {
        mi_prim_tls_slot_theap(_mi_theap_default_slot, _mi_theap_default_expansion_slot)
    } else if MI_TLS_MODEL_DYNAMIC_PTHREADS {
        if _mi_theap_default_key == 0 {
            ptr::null_mut()
        } else {
            _mi_thread_local_get(_mi_theap_default_key) as *mut mi_theap_t
        }
    } else {
        ptr::null_mut()
    }
}

#[inline]
pub unsafe fn _mi_theap_cached() -> *mut mi_theap_t {
    if MI_TLS_MODEL_THREAD_LOCAL {
        TLS_THEAP_CACHED.with(|cell| {
            let theap = cell.get();
            if theap.is_null() {
                __mi_theap_empty()
            } else {
                theap
            }
        })
    } else if MI_TLS_MODEL_FIXED_SLOT {
        mi_prim_tls_slot(MI_TLS_MODEL_FIXED_SLOT_CACHED) as *mut mi_theap_t
    } else if MI_TLS_MODEL_DYNAMIC_WIN32 {
        mi_prim_tls_slot_theap(_mi_theap_cached_slot, _mi_theap_cached_expansion_slot)
    } else if MI_TLS_MODEL_DYNAMIC_PTHREADS {
        if _mi_theap_cached_key == 0 {
            ptr::null_mut()
        } else {
            _mi_thread_local_get(_mi_theap_cached_key) as *mut mi_theap_t
        }
    } else {
        ptr::null_mut()
    }
}

#[inline]
pub fn mi_prim_thread_local_default_set(theap: *mut mi_theap_t) {
    if MI_TLS_MODEL_THREAD_LOCAL {
        TLS_THEAP_DEFAULT.with(|cell| cell.set(theap));
    }
}

#[inline]
pub fn mi_prim_thread_local_cached_set(theap: *mut mi_theap_t) {
    if MI_TLS_MODEL_THREAD_LOCAL {
        TLS_THEAP_CACHED.with(|cell| cell.set(theap));
    }
}

#[inline]
pub unsafe fn _mi_thread_is_initialized() -> bool {
    let theap = _mi_theap_default();
    !theap.is_null() && crate::internal::mi_theap_is_initialized(theap)
}

#[inline]
pub unsafe fn _mi_heap_theap(heap: *const mi_heap_t) -> *mut mi_theap_t {
    let theap = _mi_theap_cached();
    if !theap.is_null() && crate::internal::_mi_theap_heap(theap) == heap.cast_mut() {
        return theap;
    }
    _mi_heap_theap_get_or_init(heap)
}

#[inline]
pub unsafe fn _mi_heap_theap_peek(heap: *const mi_heap_t) -> *mut mi_theap_t {
    let theap = _mi_theap_cached();
    if !theap.is_null() && crate::internal::_mi_theap_heap(theap) == heap.cast_mut() {
        return theap;
    }
    _mi_heap_theap_get_peek(heap)
}

#[inline]
pub unsafe fn _mi_page_associated_theap_peek(page: *mut mi_page_t) -> *mut mi_theap_t {
    let heap = (*page).heap;
    let theap = if heap.is_null() {
        crate::init::mi_theap_main_tls_peek()
    } else {
        _mi_heap_theap_peek(heap)
    };
    crate::mi_assert_internal!(
        theap.is_null() || crate::init::_mi_thread_id() == (*(*theap).tld).thread_id
    );
    theap
}

#[inline]
pub unsafe fn mi_process_attach() {
    unsafe { crate::internal::_mi_auto_process_init() }
}

#[inline]
pub unsafe fn mi_process_detach() {
    unsafe { crate::internal::_mi_auto_process_done() }
}

#[inline]
pub unsafe extern "C" fn _mi_is_redirected() -> bool {
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_allocator_init(message: *mut *const c_char) -> bool {
    if !message.is_null() {
        unsafe { *message = ptr::null() };
    }
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_allocator_done() {}
