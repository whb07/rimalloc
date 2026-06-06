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
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::{c_char, c_int, c_void};
use core::ptr::{addr_of_mut, null_mut};

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

#[repr(C)]
pub struct malloc_zone_t {
    pub zone_name: *const c_char,
}

pub static mut mi_malloc_zone: malloc_zone_t = malloc_zone_t {
    zone_name: MI_MALLOC_ZONE_NAME.as_ptr().cast(),
};

#[inline]
pub unsafe fn is_mimalloc_zone(zone: *mut malloc_zone_t) -> bool {
    zone.is_null() || core::ptr::eq(zone.cast_const(), addr_of_mut!(mi_malloc_zone).cast_const())
}

#[inline]
pub unsafe fn mi_get_default_zone() -> *mut malloc_zone_t {
    addr_of_mut!(mi_malloc_zone)
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

#[inline]
pub unsafe fn mi__malloc_fork_parent() {
    // nothing
}

#[inline]
pub unsafe fn mi__malloc_fork_child() {
    // nothing
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
