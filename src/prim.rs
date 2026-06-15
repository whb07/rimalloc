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

#[cfg(target_os = "emscripten")]
pub mod emscripten;
#[cfg(target_os = "macos")]
pub mod osx;
#[cfg(all(not(windows), not(target_os = "emscripten"), not(target_os = "wasi")))]
pub mod unix;
#[cfg(target_os = "wasi")]
pub mod wasi;
#[cfg(windows)]
pub mod windows;

#[cfg(not(target_os = "linux"))]
use core::cell::Cell;
use core::ffi::{c_char, c_void};
use core::ptr;

use crate::bits::{MI_GiB, MI_INTPTR_SIZE};
use crate::heap::{_mi_heap_theap_get_or_init, _mi_heap_theap_get_peek};
use crate::init::{
    _mi_theap_cached_expansion_slot, _mi_theap_cached_key, _mi_theap_cached_slot,
    _mi_theap_default_expansion_slot, _mi_theap_default_key, _mi_theap_default_slot,
    _mi_theap_empty,
};
#[cfg(target_os = "emscripten")]
pub(crate) use crate::types::mi_msecs_t;
pub(crate) use crate::types::mi_process_info_t;
use crate::types::{mi_heap_t, mi_page_t, mi_theap_t, mi_thread_local_t, mi_threadid_t};

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

#[cfg(not(target_os = "linux"))]
thread_local! {
    static TLS_SLOT_DEFAULT: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static TLS_SLOT_CACHED: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static TLS_THEAP_DEFAULT: Cell<*mut mi_theap_t> = const { Cell::new(ptr::null_mut()) };
    static TLS_THEAP_CACHED: Cell<*mut mi_theap_t> = const { Cell::new(ptr::null_mut()) };
}

#[cfg(target_os = "emscripten")]
pub(crate) use emscripten::{
    _mi_prim_alloc, _mi_prim_alloc_huge_os_pages, _mi_prim_commit, _mi_prim_decommit,
    _mi_prim_free, _mi_prim_getenv, _mi_prim_mem_init, _mi_prim_numa_node,
    _mi_prim_numa_node_count, _mi_prim_out_stderr, _mi_prim_process_info, _mi_prim_protect,
    _mi_prim_reset, _mi_prim_reuse, _mi_prim_thread_associate_default_theap,
    _mi_prim_thread_done_auto_done, _mi_prim_thread_init_auto_done,
    _mi_prim_thread_is_in_threadpool, _mi_prim_thread_yield,
};
#[cfg(all(not(windows), not(target_os = "emscripten"), not(target_os = "wasi")))]
pub(crate) use unix::{
    _mi_prim_alloc, _mi_prim_alloc_huge_os_pages, _mi_prim_clock_now, _mi_prim_commit,
    _mi_prim_decommit, _mi_prim_free, _mi_prim_getenv, _mi_prim_mem_init, _mi_prim_numa_node,
    _mi_prim_numa_node_count, _mi_prim_out_stderr, _mi_prim_process_info, _mi_prim_protect,
    _mi_prim_reset, _mi_prim_reuse, _mi_prim_thread_associate_default_theap,
    _mi_prim_thread_done_auto_done, _mi_prim_thread_init_auto_done,
    _mi_prim_thread_is_in_threadpool, _mi_prim_thread_yield,
};
#[cfg(target_os = "wasi")]
pub(crate) use wasi::{
    _mi_prim_alloc, _mi_prim_alloc_huge_os_pages, _mi_prim_commit, _mi_prim_decommit,
    _mi_prim_free, _mi_prim_getenv, _mi_prim_mem_init, _mi_prim_numa_node,
    _mi_prim_numa_node_count, _mi_prim_out_stderr, _mi_prim_process_info, _mi_prim_protect,
    _mi_prim_reset, _mi_prim_reuse, _mi_prim_thread_associate_default_theap,
    _mi_prim_thread_done_auto_done, _mi_prim_thread_init_auto_done,
    _mi_prim_thread_is_in_threadpool, _mi_prim_thread_yield,
};
#[cfg(windows)]
pub(crate) use windows::{
    _mi_prim_alloc, _mi_prim_alloc_huge_os_pages, _mi_prim_commit, _mi_prim_decommit,
    _mi_prim_free, _mi_prim_getenv, _mi_prim_mem_init, _mi_prim_numa_node,
    _mi_prim_numa_node_count, _mi_prim_out_stderr, _mi_prim_process_info, _mi_prim_protect,
    _mi_prim_reset, _mi_prim_reuse, _mi_prim_thread_associate_default_theap,
    _mi_prim_thread_done_auto_done, _mi_prim_thread_init_auto_done,
    _mi_prim_thread_is_in_threadpool, _mi_prim_thread_yield,
};

unsafe extern "C" {
    pub fn _mi_thread_local_get(key: mi_thread_local_t) -> *mut c_void;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn _mi_tls_theap_default_get() -> *mut mi_theap_t;
    fn _mi_tls_theap_default_set(theap: *mut mi_theap_t);
    fn _mi_tls_theap_cached_get() -> *mut mi_theap_t;
    fn _mi_tls_theap_cached_set(theap: *mut mi_theap_t);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
#[inline(always)]
unsafe fn mi_linux_x64_theap_default_get() -> *mut mi_theap_t {
    let theap: *mut mi_theap_t;
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + __mi_theap_default@GOTTPOFF]",
            "mov {theap}, qword ptr fs:[{tls_offset}]",
            tls_offset = lateout(reg) _,
            theap = lateout(reg) theap,
            options(nostack, preserves_flags, readonly),
        );
    }
    theap
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
#[inline(always)]
unsafe fn mi_linux_x64_theap_default_set(theap: *mut mi_theap_t) {
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + __mi_theap_default@GOTTPOFF]",
            "mov qword ptr fs:[{tls_offset}], {theap}",
            tls_offset = lateout(reg) _,
            theap = in(reg) theap,
            options(nostack, preserves_flags),
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
#[inline(always)]
unsafe fn mi_linux_x64_theap_cached_get() -> *mut mi_theap_t {
    let theap: *mut mi_theap_t;
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + __mi_theap_cached@GOTTPOFF]",
            "mov {theap}, qword ptr fs:[{tls_offset}]",
            tls_offset = lateout(reg) _,
            theap = lateout(reg) theap,
            options(nostack, preserves_flags, readonly),
        );
    }
    theap
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
#[inline(always)]
unsafe fn mi_linux_x64_theap_cached_set(theap: *mut mi_theap_t) {
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + __mi_theap_cached@GOTTPOFF]",
            "mov qword ptr fs:[{tls_offset}], {theap}",
            tls_offset = lateout(reg) _,
            theap = in(reg) theap,
            options(nostack, preserves_flags),
        );
    }
}

#[inline]
pub fn mi_prim_tls_slot(slot: usize) -> *mut c_void {
    #[cfg(not(target_os = "linux"))]
    {
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
    #[cfg(target_os = "linux")]
    {
        let _ = slot;
        ptr::null_mut()
    }
}

#[inline]
pub fn mi_prim_tls_slot_set(slot: usize, value: *mut c_void) {
    #[cfg(not(target_os = "linux"))]
    {
        if MI_TLS_MODEL_FIXED_SLOT {
            match slot {
                MI_TLS_MODEL_FIXED_SLOT_DEFAULT => TLS_SLOT_DEFAULT.with(|cell| cell.set(value)),
                MI_TLS_MODEL_FIXED_SLOT_CACHED => TLS_SLOT_CACHED.with(|cell| cell.set(value)),
                _ => {}
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = slot;
        let _ = value;
    }
}

#[inline]
pub fn __mi_prim_thread_id() -> mi_threadid_t {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
    {
        let tid: usize;
        unsafe {
            core::arch::asm!(
                "mov {}, fs:0",
                out(reg) tid,
                options(nostack, preserves_flags, readonly)
            );
        }
        tid
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl"))))]
    {
        crate::init::fallback_thread_id()
    }
}

#[inline]
pub fn _mi_prim_thread_id() -> mi_threadid_t {
    let tid = __mi_prim_thread_id();
    crate::mi_debug_assert!(tid > 1);
    crate::mi_debug_assert_eq!(tid & 0x03, 0);
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
        #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
        {
            let theap = unsafe { mi_linux_x64_theap_default_get() };
            if theap.is_null() {
                unsafe { __mi_theap_empty() }
            } else {
                theap
            }
        }
        #[cfg(all(
            target_os = "linux",
            not(all(target_arch = "x86_64", not(target_env = "musl")))
        ))]
        {
            let theap = unsafe { _mi_tls_theap_default_get() };
            if theap.is_null() {
                unsafe { __mi_theap_empty() }
            } else {
                theap
            }
        }
        #[cfg(not(target_os = "linux"))]
        TLS_THEAP_DEFAULT.with(|cell| {
            let theap = cell.get();
            if theap.is_null() {
                unsafe { __mi_theap_empty() }
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
        #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
        {
            let theap = unsafe { mi_linux_x64_theap_cached_get() };
            if theap.is_null() {
                unsafe { __mi_theap_empty() }
            } else {
                theap
            }
        }
        #[cfg(all(
            target_os = "linux",
            not(all(target_arch = "x86_64", not(target_env = "musl")))
        ))]
        {
            let theap = unsafe { _mi_tls_theap_cached_get() };
            if theap.is_null() {
                unsafe { __mi_theap_empty() }
            } else {
                theap
            }
        }
        #[cfg(not(target_os = "linux"))]
        TLS_THEAP_CACHED.with(|cell| {
            let theap = cell.get();
            if theap.is_null() {
                unsafe { __mi_theap_empty() }
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
pub(crate) fn mi_prim_thread_local_default_set(theap: *mut mi_theap_t) {
    if MI_TLS_MODEL_THREAD_LOCAL {
        #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
        unsafe {
            mi_linux_x64_theap_default_set(theap);
        }
        #[cfg(all(
            target_os = "linux",
            not(all(target_arch = "x86_64", not(target_env = "musl")))
        ))]
        unsafe {
            _mi_tls_theap_default_set(theap);
        }
        #[cfg(not(target_os = "linux"))]
        TLS_THEAP_DEFAULT.with(|cell| cell.set(theap));
    }
}

#[inline]
pub(crate) fn mi_prim_thread_local_cached_set(theap: *mut mi_theap_t) {
    if MI_TLS_MODEL_THREAD_LOCAL {
        #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
        unsafe {
            mi_linux_x64_theap_cached_set(theap);
        }
        #[cfg(all(
            target_os = "linux",
            not(all(target_arch = "x86_64", not(target_env = "musl")))
        ))]
        unsafe {
            _mi_tls_theap_cached_set(theap);
        }
        #[cfg(not(target_os = "linux"))]
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

#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
unsafe extern "C" fn mi_process_attach_ctor() {
    unsafe { mi_process_attach() };
}

#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
unsafe extern "C" fn mi_process_detach_dtor() {
    unsafe { mi_process_detach() };
}

#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
#[used]
#[unsafe(link_section = ".init_array")]
pub(crate) static _MI_PROCESS_ATTACH_CTOR: unsafe extern "C" fn() = mi_process_attach_ctor;

#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
#[used]
#[unsafe(link_section = ".fini_array")]
pub(crate) static _MI_PROCESS_DETACH_DTOR: unsafe extern "C" fn() = mi_process_detach_dtor;

#[inline]
pub unsafe extern "C" fn _mi_is_redirected() -> bool {
    false
}

pub(crate) unsafe extern "C" fn _mi_allocator_init(message: *mut *const c_char) -> bool {
    if !message.is_null() {
        unsafe { *message = ptr::null() };
    }
    true
}

pub(crate) unsafe extern "C" fn _mi_allocator_done() {}

#[cfg(test)]
mod tests {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
    #[test]
    fn linux_x86_64_prim_thread_id_uses_thread_pointer() {
        let tid = super::__mi_prim_thread_id();
        assert_eq!(tid, super::_mi_prim_thread_id());
        assert_eq!(tid, crate::init::_mi_thread_id());
        assert!(tid > 1);
        assert_eq!(tid & crate::internal::MI_PAGE_FLAG_MASK, 0);

        let child = std::thread::spawn(super::__mi_prim_thread_id)
            .join()
            .unwrap_or(0);
        assert!(child > 1);
        assert_eq!(child & crate::internal::MI_PAGE_FLAG_MASK, 0);
        assert_ne!(tid, child);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_theap_tls_shim_stores_empty_sentinel() {
        let marker = core::ptr::addr_of!(super::_mi_theap_empty).cast_mut();
        unsafe {
            super::_mi_tls_theap_default_set(marker);
            super::_mi_tls_theap_cached_set(marker);
            assert_eq!(super::_mi_tls_theap_default_get(), marker);
            assert_eq!(super::_mi_tls_theap_cached_get(), marker);
            super::_mi_tls_theap_default_set(core::ptr::null_mut());
            super::_mi_tls_theap_cached_set(core::ptr::null_mut());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_theap_tls_accessors_map_null_to_empty() {
        unsafe {
            super::_mi_tls_theap_default_set(core::ptr::null_mut());
            super::_mi_tls_theap_cached_set(core::ptr::null_mut());
            assert_eq!(super::_mi_theap_default(), super::__mi_theap_empty());
            assert_eq!(super::_mi_theap_cached(), super::__mi_theap_empty());

            super::_mi_tls_theap_default_set(super::__mi_theap_empty());
            super::_mi_tls_theap_cached_set(super::__mi_theap_empty());
            assert_eq!(super::_mi_theap_default(), super::__mi_theap_empty());
            assert_eq!(super::_mi_theap_cached(), super::__mi_theap_empty());

            super::_mi_tls_theap_default_set(core::ptr::null_mut());
            super::_mi_tls_theap_cached_set(core::ptr::null_mut());
        }
    }

    #[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
    #[test]
    fn elf_process_lifecycle_hooks_are_registered() {
        assert_eq!(
            super::_MI_PROCESS_ATTACH_CTOR as usize,
            super::mi_process_attach_ctor as usize
        );
        assert_eq!(
            super::_MI_PROCESS_DETACH_DTOR as usize,
            super::mi_process_detach_dtor as usize
        );
    }
}
