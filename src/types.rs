//! Core internal layouts, structs, typedefs, metadata

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::{c_char, c_void};
use std::os::raw::c_long;

pub const MI_MALLOC_VERSION: i32 = 30302;
pub const MI_ENABLE_LARGE_PAGES: usize = 1;
pub const MI_SMALL_WSIZE_MAX: usize = 128;
pub const MI_SMALL_SIZE_MAX: usize = MI_SMALL_WSIZE_MAX * core::mem::size_of::<*const c_void>();

pub type mi_arena_id_t = *mut c_void;
pub type mi_threadid_t = usize;
pub type mi_msecs_t = i64;

pub enum mi_arena_s {}
pub type mi_arena_t = mi_arena_s;

pub enum mi_heap_s {}
pub type mi_heap_t = mi_heap_s;

pub enum mi_theap_s {}
pub type mi_theap_t = mi_theap_s;

pub enum mi_page_s {}
pub type mi_page_t = mi_page_s;

pub enum mi_subproc_s {}
pub type mi_subproc_t = mi_subproc_s;

pub type mi_bitmap_t = crate::bitmap::mi_bitmap_t;
pub type mi_bbitmap_t = crate::bitmap::mi_bbitmap_t;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_random_ctx_t {
    pub input: [u32; 16],
    pub output: [u32; 16],
    pub output_available: i32,
    pub weak: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_page_queue_t {
    pub first: *mut mi_page_t,
    pub last: *mut mi_page_t,
    pub count: usize,
    pub block_size: usize,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_page_kind_e {
    MI_PAGE_SMALL = 0,
    MI_PAGE_MEDIUM = 1,
    MI_PAGE_LARGE = 2,
    MI_PAGE_SINGLETON = 3,
}

pub type mi_page_kind_t = mi_page_kind_e;

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
pub type mi_claim_fun_t =
    unsafe extern "C" fn(slice_index: usize, arena: *mut mi_arena_t, keep_set: *mut bool) -> bool;
pub type mi_forall_set_fun_t = unsafe extern "C" fn(
    slice_index: usize,
    slice_count: usize,
    arena: *mut mi_arena_t,
    arg2: *mut c_void,
) -> bool;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum mi_option_init_e {
    #[default]
    MI_OPTION_UNINIT = 0,
    MI_OPTION_DEFAULTED = 1,
    MI_OPTION_INITIALIZED = 2,
}

pub type mi_option_init_t = mi_option_init_e;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_option_desc_t {
    pub value: c_long,
    pub init: mi_option_init_t,
    pub option: crate::options::mi_option_t,
    pub name: *const c_char,
    pub legacy_name: *const c_char,
}

unsafe impl Send for mi_option_desc_t {}
unsafe impl Sync for mi_option_desc_t {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_subproc_id_t {
    pub _mi_subproc_id: *mut c_void,
}
