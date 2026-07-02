//! Raw memory-origin identifiers from mimalloc's internal header.
//!
//! This module owns the C-compatible `mi_memid_t` layout and the narrow helper
//! functions that classify or construct memory-origin identifiers. The typed
//! Rust view lives in `crate::core_types::MemId`; translated allocator code
//! reaches these definitions through the stable `crate::internal::*` re-export.

#![allow(non_camel_case_types)]

use core::ffi::c_void;
use core::ptr;

use crate::types::mi_arena_t;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum mi_memkind_t {
    #[default]
    MI_MEM_NONE = 0,
    MI_MEM_EXTERNAL = 1,
    MI_MEM_STATIC = 2,
    MI_MEM_META = 3,
    MI_MEM_OS = 4,
    MI_MEM_OS_HUGE = 5,
    MI_MEM_OS_REMAP = 6,
    MI_MEM_ARENA = 7,
    MI_MEM_HEAP_MAIN = 8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_memid_os_info_t {
    pub base: *mut c_void,
    pub size: usize,
}
pub type mi_memid_os_info = mi_memid_os_info_t;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_memid_arena_info_t {
    pub arena: *mut mi_arena_t,
    pub slice_index: u32,
    pub slice_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_memid_meta_info_t {
    pub meta_page: *mut c_void,
    pub block_index: u32,
    pub block_count: u32,
}
pub type mi_memid_meta_info = mi_memid_meta_info_t;

#[repr(C)]
#[derive(Clone, Copy)]
pub union mi_memid_mem_t {
    pub os: mi_memid_os_info_t,
    pub arena: mi_memid_arena_info_t,
    pub meta: mi_memid_meta_info_t,
}

impl Default for mi_memid_mem_t {
    fn default() -> Self {
        Self {
            os: mi_memid_os_info_t::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct mi_memid_t {
    pub mem: mi_memid_mem_t,
    pub memkind: mi_memkind_t,
    pub is_pinned: bool,
    pub initially_committed: bool,
    pub initially_zero: bool,
}

#[inline]
pub fn mi_memid_is_os(memid: mi_memid_t) -> bool {
    matches!(
        crate::core_types::MemId::view(memid),
        crate::core_types::MemIdView::Os(_)
    )
}

#[inline]
pub fn mi_memid_needs_no_free(memid: mi_memid_t) -> bool {
    matches!(
        crate::core_types::MemId::view(memid),
        crate::core_types::MemIdView::None(_)
            | crate::core_types::MemIdView::External(_)
            | crate::core_types::MemIdView::Static(_)
    )
}

#[inline]
pub fn mi_memid_arena(memid: mi_memid_t) -> *mut mi_arena_t {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(a) => a.arena.as_ptr(),
        _ => ptr::null_mut(),
    }
}

/// # Safety
/// Returns the inert memid sentinel and has no caller preconditions.
#[inline]
pub unsafe fn _mi_memid_none() -> mi_memid_t {
    crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw()
}

/// # Safety
/// `base..base + size` must describe the reserved OS memory region represented by the returned id.
#[inline]
pub unsafe fn _mi_memid_create_os(
    base: *mut c_void,
    size: crate::core_types::ReservedBytes,
    committed: bool,
    is_zero: bool,
    is_large: bool,
) -> mi_memid_t {
    crate::core_types::MemId::<crate::core_types::OsMem>::os(
        crate::core_types::OsMemPtr::new(base),
        size,
        crate::core_types::InitiallyCommitted(committed),
        crate::core_types::InitiallyZero(is_zero),
        crate::core_types::Pinned(is_large),
    )
    .into_raw()
}
