//! Core internal layouts, structs, typedefs, metadata

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::{c_char, c_void};

pub const MI_MALLOC_VERSION: i32 = 30302;
pub const MI_SMALL_WSIZE_MAX: usize = 128;
pub const MI_SMALL_SIZE_MAX: usize = MI_SMALL_WSIZE_MAX * core::mem::size_of::<*const c_void>();

pub type mi_arena_id_t = *mut c_void;

pub enum mi_heap_s {}
pub type mi_heap_t = mi_heap_s;

pub enum mi_theap_s {}
pub type mi_theap_t = mi_theap_s;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_heap_area_t {
    pub blocks: *mut c_void,
    pub reserved: usize,
    pub committed: usize,
    pub used: usize,
    pub block_size: usize,
    pub full_block_size: usize,
    pub reserved1: *mut c_void,
}

pub type mi_deferred_free_fun = unsafe extern "C" fn(force: bool, heartbeat: u64, arg: *mut c_void);
pub type mi_output_fun = unsafe extern "C" fn(msg: *const c_char, arg: *mut c_void);
pub type mi_error_fun = unsafe extern "C" fn(err: i32, arg: *mut c_void);
pub type mi_heap_visit_fun = unsafe extern "C" fn(heap: *mut mi_heap_t, arg: *mut c_void) -> bool;
pub type mi_block_visit_fun = unsafe extern "C" fn(
    heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    block: *mut c_void,
    block_size: usize,
    arg: *mut c_void,
) -> bool;
pub type mi_commit_fun_t = unsafe extern "C" fn(
    commit: bool,
    start: *mut c_void,
    size: usize,
    is_zero: *mut bool,
    user_arg: *mut c_void,
) -> bool;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_subproc_id_t {
    pub _mi_subproc_id: *mut c_void,
}
