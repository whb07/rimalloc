//! Bounded mechanical translation of `src/prim/osx/alloc-override-zone.c`.
//!
//! This file is intentionally standalone for now. The crate does not yet wire
//! a `prim::osx` submodule, but these helpers are translated here so the
//! remaining macOS zone-override surface exists in Rust in one place.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unused_imports,
    unused_macros,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::{c_char, c_int, c_void};
use core::ptr::{addr_of_mut, null_mut};

use crate::alloc_override::mi_interpose_s;
use crate::mimalloc::mi_any_heap_contains;

pub type boolean_t = c_int;
pub type kern_return_t = c_int;
pub type task_t = u32;
pub type uintptr_t = usize;
pub type vm_address_t = usize;
pub type vm_size_t = usize;

pub const KERN_SUCCESS: kern_return_t = 0;
pub const MI_MALLOC_OVERRIDE_ZONE_C: () = ();

static MI_MALLOC_ZONE_NAME: &[u8; 9] = b"mimalloc\0";

macro_rules! MI_INTERPOSE_FUN {
    ($oldfun:path, $newfun:path) => {
        mi_interpose_s {
            replacement: ($newfun as *const ()) as *const c_void,
            target: ($oldfun as *const ()) as *const c_void,
        }
    };
}

macro_rules! MI_INTERPOSE_ZONE {
    (zone_batch_free) => {
        MI_INTERPOSE_FUN!(malloc_zone_batch_free, crate::prim::osx::zone_batch_free)
    };
    (zone_batch_malloc) => {
        MI_INTERPOSE_FUN!(
            malloc_zone_batch_malloc,
            crate::prim::osx::zone_batch_malloc
        )
    };
    (zone_calloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_calloc, crate::prim::osx::zone_calloc)
    };
    (zone_check) => {
        MI_INTERPOSE_FUN!(malloc_zone_check, crate::prim::osx::zone_check)
    };
    (zone_free) => {
        MI_INTERPOSE_FUN!(malloc_zone_free, crate::prim::osx::zone_free)
    };
    (zone_from_ptr) => {
        MI_INTERPOSE_FUN!(malloc_zone_from_ptr, crate::prim::osx::zone_from_ptr)
    };
    (zone_log) => {
        MI_INTERPOSE_FUN!(malloc_zone_log, crate::prim::osx::zone_log)
    };
    (zone_malloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_malloc, crate::prim::osx::zone_malloc)
    };
    (zone_memalign) => {
        MI_INTERPOSE_FUN!(malloc_zone_memalign, crate::prim::osx::zone_memalign)
    };
    (zone_print) => {
        MI_INTERPOSE_FUN!(malloc_zone_print, crate::prim::osx::zone_print)
    };
    (zone_print_ptr_info) => {
        MI_INTERPOSE_FUN!(
            malloc_zone_print_ptr_info,
            crate::prim::osx::zone_print_ptr_info
        )
    };
    (zone_realloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_realloc, crate::prim::osx::zone_realloc)
    };
    (zone_register) => {
        MI_INTERPOSE_FUN!(malloc_zone_register, crate::prim::osx::zone_register)
    };
    (zone_unregister) => {
        MI_INTERPOSE_FUN!(malloc_zone_unregister, crate::prim::osx::zone_unregister)
    };
    (zone_valloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_valloc, crate::prim::osx::zone_valloc)
    };
}

#[repr(C)]
pub struct malloc_zone_t {
    pub zone_name: *const c_char,
}

pub static mut mi_malloc_zone: malloc_zone_t = malloc_zone_t {
    zone_name: MI_MALLOC_ZONE_NAME.as_ptr().cast(),
};

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn malloc_default_zone() -> *mut malloc_zone_t;
    fn malloc_default_purgeable_zone() -> *mut malloc_zone_t;
    fn malloc_get_all_zones(
        task: task_t,
        mr: *mut c_void,
        addresses: *mut *mut *mut malloc_zone_t,
        count: *mut u32,
    ) -> kern_return_t;
    fn malloc_zone_register(zone: *mut malloc_zone_t);
    fn malloc_zone_unregister(zone: *mut malloc_zone_t);

    fn malloc_create_zone(size: vm_size_t, flags: u32) -> *mut malloc_zone_t;
    fn malloc_get_zone_name(zone: *mut malloc_zone_t) -> *const c_char;
    fn malloc_set_zone_name(zone: *mut malloc_zone_t, name: *const c_char);
    fn malloc_destroy_zone(zone: *mut malloc_zone_t);
    fn malloc_jumpstart(cookie: uintptr_t) -> c_int;
    fn malloc_printf(fmt: *const c_char, ...);
    fn malloc_zone_batch_free(zone: *mut malloc_zone_t, ps: *mut *mut c_void, count: u32);
    fn malloc_zone_batch_malloc(
        zone: *mut malloc_zone_t,
        size: usize,
        ps: *mut *mut c_void,
        count: u32,
    ) -> u32;
    fn malloc_zone_calloc(zone: *mut malloc_zone_t, count: usize, size: usize) -> *mut c_void;
    fn malloc_zone_check(zone: *mut malloc_zone_t) -> boolean_t;
    fn malloc_zone_free(zone: *mut malloc_zone_t, p: *mut c_void);
    fn malloc_zone_from_ptr(p: *const c_void) -> *mut malloc_zone_t;
    fn malloc_zone_log(zone: *mut malloc_zone_t, p: *mut c_void);
    fn malloc_zone_malloc(zone: *mut malloc_zone_t, size: usize) -> *mut c_void;
    fn malloc_zone_memalign(zone: *mut malloc_zone_t, alignment: usize, size: usize)
    -> *mut c_void;
    fn malloc_zone_print(zone: *mut malloc_zone_t, verbose: boolean_t);
    fn malloc_zone_print_ptr_info(p: *mut c_void);
    fn malloc_zone_realloc(zone: *mut malloc_zone_t, p: *mut c_void, newsize: usize)
    -> *mut c_void;
    fn malloc_zone_valloc(zone: *mut malloc_zone_t, size: usize) -> *mut c_void;
}

#[inline]
pub unsafe fn is_mimalloc_zone(zone: *mut malloc_zone_t) -> bool {
    zone.is_null() || core::ptr::eq(zone.cast_const(), addr_of_mut!(mi_malloc_zone).cast_const())
}

#[inline]
pub unsafe fn mi_get_default_zone() -> *mut malloc_zone_t {
    #[cfg(all(target_os = "macos", target_feature = "crt-static"))]
    {
        crate::mi_atomic_do_once!({
            unsafe { malloc_zone_register(addr_of_mut!(mi_malloc_zone)) };
        });
        addr_of_mut!(mi_malloc_zone)
    }
    #[cfg(all(target_os = "macos", not(target_feature = "crt-static")))]
    {
        let mut zones: *mut *mut malloc_zone_t = null_mut();
        let mut count: u32 = 0;
        let ret = unsafe { malloc_get_all_zones(0, null_mut(), &mut zones, &mut count) };
        if ret == KERN_SUCCESS && count > 0 && !zones.is_null() {
            unsafe { *zones }
        } else {
            unsafe { malloc_default_zone() }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        addr_of_mut!(mi_malloc_zone)
    }
}

#[inline]
pub unsafe fn mi_malloc_create_zone(_size: vm_size_t, _flags: u32) -> *mut malloc_zone_t {
    mi_get_default_zone()
}

#[inline]
pub unsafe fn mi_malloc_default_zone() -> *mut malloc_zone_t {
    mi_get_default_zone()
}

#[inline]
pub unsafe fn mi_malloc_default_purgeable_zone() -> *mut malloc_zone_t {
    mi_get_default_zone()
}

#[inline]
pub unsafe fn mi_malloc_destroy_zone(_zone: *mut malloc_zone_t) {
    // nothing
}

#[inline]
pub unsafe fn mi_malloc_get_all_zones(
    _task: task_t,
    _mr: *mut c_void,
    addresses: *mut *mut vm_address_t,
    count: *mut u32,
) -> kern_return_t {
    if !addresses.is_null() {
        *addresses = null_mut();
    }
    if !count.is_null() {
        *count = 0;
    }
    KERN_SUCCESS
}

#[inline]
pub unsafe fn mi_malloc_get_zone_name(zone: *mut malloc_zone_t) -> *const c_char {
    let zone = if zone.is_null() {
        mi_get_default_zone()
    } else {
        zone
    };
    (*zone).zone_name
}

#[inline]
pub unsafe fn mi_malloc_set_zone_name(_zone: *mut malloc_zone_t, _name: *const c_char) {
    // nothing
}

#[inline]
pub unsafe fn mi_malloc_jumpstart(_cookie: uintptr_t) -> c_int {
    1
}

#[inline]
pub unsafe fn mi__malloc_fork_prepare() {
    // nothing
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_fork_prepare() {
    unsafe { mi__malloc_fork_prepare() }
}

#[inline]
pub unsafe fn mi__malloc_fork_parent() {
    // nothing
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_fork_parent() {
    unsafe { mi__malloc_fork_parent() }
}

#[inline]
pub unsafe fn mi__malloc_fork_child() {
    // nothing
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_fork_child() {
    unsafe { mi__malloc_fork_child() }
}

#[inline]
pub unsafe fn mi_malloc_printf(_fmt: *const c_char) {
    // nothing
}

#[inline]
pub unsafe fn zone_check(_zone: *mut malloc_zone_t) -> bool {
    true
}

#[inline]
pub unsafe fn zone_from_ptr(p: *const c_void) -> *mut malloc_zone_t {
    if mi_any_heap_contains(p) {
        mi_get_default_zone()
    } else {
        null_mut()
    }
}

#[inline]
pub unsafe fn zone_log(_zone: *mut malloc_zone_t, _p: *mut c_void) {
    // nothing
}

#[inline]
pub unsafe fn zone_print(_zone: *mut malloc_zone_t, _verbose: bool) {
    // nothing
}

#[inline]
pub unsafe fn zone_print_ptr_info(_p: *const c_void) {
    // nothing
}

#[inline]
pub unsafe fn zone_register(_zone: *mut malloc_zone_t) {
    // nothing
}

#[inline]
pub unsafe fn zone_unregister(_zone: *mut malloc_zone_t) {
    // nothing
}

#[cfg(target_os = "macos")]
#[used]
#[unsafe(link_section = "__DATA,__interpose")]
pub static _mi_zone_interposes: [mi_interpose_s; 27] = [
    MI_INTERPOSE_FUN!(malloc_create_zone, crate::prim::osx::mi_malloc_create_zone),
    MI_INTERPOSE_FUN!(
        malloc_default_purgeable_zone,
        crate::prim::osx::mi_malloc_default_purgeable_zone
    ),
    MI_INTERPOSE_FUN!(
        malloc_default_zone,
        crate::prim::osx::mi_malloc_default_zone
    ),
    MI_INTERPOSE_FUN!(
        malloc_destroy_zone,
        crate::prim::osx::mi_malloc_destroy_zone
    ),
    MI_INTERPOSE_FUN!(
        malloc_get_all_zones,
        crate::prim::osx::mi_malloc_get_all_zones
    ),
    MI_INTERPOSE_FUN!(
        malloc_get_zone_name,
        crate::prim::osx::mi_malloc_get_zone_name
    ),
    MI_INTERPOSE_FUN!(malloc_jumpstart, crate::prim::osx::mi_malloc_jumpstart),
    MI_INTERPOSE_FUN!(malloc_printf, crate::prim::osx::mi_malloc_printf),
    MI_INTERPOSE_FUN!(
        malloc_set_zone_name,
        crate::prim::osx::mi_malloc_set_zone_name
    ),
    MI_INTERPOSE_FUN!(_malloc_fork_child, crate::prim::osx::_malloc_fork_child),
    MI_INTERPOSE_FUN!(_malloc_fork_parent, crate::prim::osx::_malloc_fork_parent),
    MI_INTERPOSE_FUN!(_malloc_fork_prepare, crate::prim::osx::_malloc_fork_prepare),
    MI_INTERPOSE_ZONE!(zone_batch_free),
    MI_INTERPOSE_ZONE!(zone_batch_malloc),
    MI_INTERPOSE_ZONE!(zone_calloc),
    MI_INTERPOSE_ZONE!(zone_check),
    MI_INTERPOSE_ZONE!(zone_free),
    MI_INTERPOSE_ZONE!(zone_from_ptr),
    MI_INTERPOSE_ZONE!(zone_log),
    MI_INTERPOSE_ZONE!(zone_malloc),
    MI_INTERPOSE_ZONE!(zone_memalign),
    MI_INTERPOSE_ZONE!(zone_print),
    MI_INTERPOSE_ZONE!(zone_print_ptr_info),
    MI_INTERPOSE_ZONE!(zone_realloc),
    MI_INTERPOSE_ZONE!(zone_register),
    MI_INTERPOSE_ZONE!(zone_unregister),
    MI_INTERPOSE_ZONE!(zone_valloc),
];

#[cfg(target_os = "macos")]
pub unsafe extern "C" fn _mi_macos_override_malloc() {
    let mut purgeable_zone: *mut malloc_zone_t = null_mut();
    if unsafe { malloc_default_purgeable_zone as usize } != 0 {
        purgeable_zone = unsafe { malloc_default_purgeable_zone() };
    }

    unsafe { malloc_zone_register(addr_of_mut!(mi_malloc_zone)) };
    let default_zone = unsafe { mi_get_default_zone() };
    if !core::ptr::eq(
        default_zone.cast_const(),
        addr_of_mut!(mi_malloc_zone).cast_const(),
    ) {
        unsafe { malloc_zone_unregister(default_zone) };
        unsafe { malloc_zone_register(default_zone) };
    }

    if !purgeable_zone.is_null() {
        unsafe { malloc_zone_unregister(purgeable_zone) };
        unsafe { malloc_zone_register(purgeable_zone) };
    }
}

#[cfg(target_os = "macos")]
#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
pub static _mi_macos_override_malloc_ctor: unsafe extern "C" fn() = _mi_macos_override_malloc;
