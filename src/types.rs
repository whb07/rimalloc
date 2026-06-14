//! Core internal layouts, structs, typedefs, metadata

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize};
use std::os::raw::c_long;

use crate::atomic::mi_lock_t;
use crate::stats::mi_stats_t;

pub const MI_TYPES_H: () = ();
pub const MI_MAX_ALIGN_SIZE: usize = 16;
pub const MI_ARENA_SLICE_SHIFT: usize = crate::bits::MI_SIZE_SHIFT + 13;
pub const MI_BCHUNK_BITS_SHIFT: usize = crate::bits::MI_SIZE_SHIFT + 6;

pub const MI_SECURE: usize = crate::internal::MI_SECURE;
const MI_DEBUG: usize = if cfg!(debug_assertions) { 2 } else { 0 };
pub const MI_STAT: usize = if MI_DEBUG > 0 { 2 } else { 0 };
pub const MI_GUARDED: usize = crate::internal::MI_GUARDED;
pub const MI_PADDING: usize = if cfg!(feature = "secure")
    || MI_DEBUG >= 1
    || crate::track::MI_TRACK_VALGRIND != 0
    || crate::track::MI_TRACK_ASAN != 0
    || crate::track::MI_TRACK_ETW != 0
{
    1
} else {
    0
};
pub const MI_PADDING_CHECK: usize =
    if MI_PADDING != 0 && (cfg!(feature = "secure") || MI_DEBUG >= 1) {
        1
    } else {
        0
    };
pub const MI_ENCODE_FREELIST: usize = if cfg!(feature = "secure") || MI_DEBUG >= 1 {
    1
} else {
    0
};
const MI_PAGE_META_IS_SEPARATED: usize = if crate::bits::MI_PAGE_MAP_FLAT != 0 {
    0
} else {
    1
};
pub const MI_PAGE_META_ALIGNED_FREE_SMALL: usize = 0;
const MI_ARENA_SLICE_ALIGN: usize = crate::arena::MI_ARENA_SLICE_SIZE;
pub const MI_ARENA_MIN_OBJ_SLICES: usize = 1;
pub const MI_ARENA_MIN_OBJ_SIZE: usize =
    MI_ARENA_MIN_OBJ_SLICES * crate::arena::MI_ARENA_SLICE_SIZE;
const MI_MAX_ALLOC_SIZE: usize = isize::MAX as usize;
pub const MI_PAGE_MIN_COMMIT_SIZE: usize = crate::arena::MI_ARENA_SLICE_SIZE;
pub const MI_PAGE_MAX_OVERALLOC_ALIGN: usize = crate::arena::MI_ARENA_SLICE_SIZE;
pub const MI_PAGE_ALIGN: usize = MI_ARENA_SLICE_ALIGN;
pub const MI_PAGE_MAX_START_BLOCK_ALIGN2: usize = 4 * crate::bits::MI_KiB;
pub const MI_MAX_ARENAS: usize = 160;
pub const MI_ARENA_MAX_SIZE: usize =
    crate::bitmap::MI_BITMAP_MAX_BIT_COUNT * crate::arena::MI_ARENA_SLICE_SIZE;
pub const MI_ARENA_BIN_COUNT: usize = MI_BIN_COUNT;
pub const EAGAIN: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EFAULT: c_int = 14;
pub const EINVAL: c_int = 22;
pub const EOVERFLOW: c_int = 75;
pub const MI_DEBUG_UNINIT: u8 = 0xD0;
pub const MI_DEBUG_FREED: u8 = 0xDF;
pub const MI_DEBUG_PADDING: u8 = 0xDE;

const _: (usize, usize, usize, usize, usize) = (
    MI_SECURE,
    MI_GUARDED,
    MI_PAGE_META_IS_SEPARATED,
    MI_ARENA_SLICE_ALIGN,
    MI_MAX_ALLOC_SIZE,
);

pub const MI_MALLOC_VERSION: i32 = 30302;
pub const MI_ENABLE_LARGE_PAGES: usize = 1;
pub const MI_SMALL_WSIZE_MAX: usize = 128;
pub const MI_SMALL_SIZE_MAX: usize = MI_SMALL_WSIZE_MAX * core::mem::size_of::<*const c_void>();
pub const MI_BIN_HUGE: usize = crate::stats::MI_BIN_HUGE;
pub const MI_BIN_FULL: usize = crate::stats::MI_BIN_HUGE + 1;
pub const MI_BIN_COUNT: usize = MI_BIN_FULL + 1;
pub const MI_PAGE_IN_FULL_QUEUE: usize = 0x01;
pub const MI_PAGE_HAS_INTERIOR_POINTERS: usize = 0x02;
pub const MI_PAGE_FLAG_MASK: usize = 0x03;
pub const MI_THREADID_ABANDONED: usize = 0;
pub const MI_THREADID_ABANDONED_MAPPED: usize = MI_PAGE_FLAG_MASK + 1;
pub const MI_PAGES_DIRECT: usize = MI_SMALL_WSIZE_MAX + MI_PADDING_WSIZE + 1;
pub const MI_SMALL_PAGE_SIZE: usize = crate::arena::MI_ARENA_SLICE_SIZE;
pub const MI_MEDIUM_PAGE_SIZE: usize = 8 * MI_SMALL_PAGE_SIZE;
pub const MI_LARGE_PAGE_SIZE: usize = core::mem::size_of::<usize>() * MI_MEDIUM_PAGE_SIZE;
pub const MI_LARGE_MAX_OBJ_SIZE: usize = if MI_ENABLE_LARGE_PAGES != 0 {
    MI_LARGE_PAGE_SIZE / 8
} else {
    MI_MEDIUM_PAGE_SIZE / 8
};
pub const MI_LARGE_MAX_OBJ_WSIZE: usize = MI_LARGE_MAX_OBJ_SIZE / core::mem::size_of::<usize>();

pub type mi_arena_id_t = *mut c_void;
pub use crate::internal::mi_memkind_t;
pub type mi_memkind_e = crate::internal::mi_memkind_t;
pub type mi_threadid_t = usize;
pub type mi_msecs_t = i64;
pub type mi_thread_local_t = usize;

pub type mi_theap_t = mi_theap_s;
pub type mi_page_t = mi_page_s;
pub type mi_page_flags_t = usize;
pub type mi_thread_free_t = usize;

pub type mi_bitmap_t = crate::bitmap::mi_bitmap_t;
pub type mi_bbitmap_t = crate::bitmap::mi_bbitmap_t;
pub type mi_encoded_t = usize;

#[inline]
pub const fn mi_memkind_needs_no_free(memkind: mi_memkind_t) -> bool {
    matches!(
        memkind,
        mi_memkind_t::MI_MEM_NONE | mi_memkind_t::MI_MEM_EXTERNAL | mi_memkind_t::MI_MEM_STATIC
    )
}

#[inline]
pub const fn mi_memkind_is_os(memkind: mi_memkind_t) -> bool {
    matches!(
        memkind,
        mi_memkind_t::MI_MEM_OS | mi_memkind_t::MI_MEM_OS_HUGE | mi_memkind_t::MI_MEM_OS_REMAP
    )
}

pub const MI_PADDING_SIZE: usize = if MI_PADDING != 0 {
    core::mem::size_of::<mi_padding_t>()
} else {
    0
};
pub const MI_PADDING_WSIZE: usize = if MI_PADDING_SIZE == 0 {
    0
} else {
    MI_PADDING_SIZE.div_ceil(core::mem::size_of::<*const c_void>())
};

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
pub struct mi_block_s {
    pub next: mi_encoded_t,
}

pub type mi_block_t = mi_block_s;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_padding_s {
    pub canary: u32,
    pub delta: u32,
}

pub type mi_padding_t = mi_padding_s;

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

#[repr(C)]
pub struct mi_arena_pages_s {
    pub pages: *mut mi_bitmap_t,
    pub pages_abandoned: [*mut mi_bitmap_t; MI_ARENA_BIN_COUNT],
}

pub type mi_arena_pages_t = mi_arena_pages_s;

#[repr(C)]
pub struct mi_subproc_s {
    pub subproc_seq: usize,
    pub next: *mut mi_subproc_t,
    pub prev: *mut mi_subproc_t,
    pub arena_count: AtomicUsize,
    pub arenas: [*mut mi_arena_t; MI_MAX_ARENAS],
    pub arena_reserve_lock: mi_lock_t,
    pub purge_expire: AtomicI64,
    pub heap_main: *mut mi_heap_t,
    pub heaps: *mut mi_heap_t,
    pub heaps_lock: mi_lock_t,
    pub thread_count: AtomicUsize,
    pub thread_total_count: AtomicUsize,
    pub heap_count: AtomicUsize,
    pub heap_total_count: AtomicUsize,
    pub memid: crate::internal::mi_memid_t,
    pub stats: mi_stats_t,
}

pub type mi_subproc_t = mi_subproc_s;

#[repr(C)]
pub struct mi_tld_s {
    pub thread_id: mi_threadid_t,
    pub thread_seq: usize,
    pub numa_node: i32,
    pub subproc: *mut mi_subproc_t,
    pub theaps: *mut mi_theap_t,
    pub theaps_lock: mi_lock_t,
    pub recurse: bool,
    pub is_in_threadpool: bool,
    pub memid: crate::internal::mi_memid_t,
}

pub type mi_tld_t = mi_tld_s;
unsafe impl Send for mi_tld_s {}
unsafe impl Sync for mi_tld_s {}

#[repr(C)]
pub struct mi_arena_s {
    pub memid: crate::internal::mi_memid_t,
    pub subproc: *mut mi_subproc_t,
    pub arena_idx: usize,
    pub slice_count: usize,
    pub info_slices: usize,
    pub numa_node: i32,
    pub is_exclusive: bool,
    pub _padding: [u8; 7],
    pub purge_expire: AtomicI64,
    pub commit_fun: Option<mi_commit_fun_t>,
    pub commit_fun_arg: *mut c_void,
    pub total_size: usize,
    pub parent: *mut mi_arena_t,
    pub slices_free: *mut mi_bbitmap_t,
    pub slices_committed: *mut mi_bitmap_t,
    pub slices_dirty: *mut mi_bitmap_t,
    pub slices_purge: *mut mi_bitmap_t,
    pub pages_meta: *mut mi_page_t,
    pub pages_main: mi_arena_pages_t,
}

pub type mi_arena_t = mi_arena_s;

#[repr(C)]
pub struct mi_heap_s {
    pub subproc: *mut mi_subproc_t,
    pub heap_seq: usize,
    pub next: *mut mi_heap_t,
    pub prev: *mut mi_heap_t,
    pub theap: mi_thread_local_t,
    pub exclusive_arena: *mut mi_arena_t,
    pub numa_node: i32,
    pub _padding: u32,
    pub theaps: *mut mi_theap_t,
    pub theaps_lock: mi_lock_t,
    pub abandoned_count: [AtomicUsize; MI_BIN_COUNT],
    pub os_abandoned_pages: *mut mi_page_t,
    pub os_abandoned_pages_lock: mi_lock_t,
    pub arena_pages: [AtomicPtr<c_void>; MI_MAX_ARENAS],
    pub arena_pages_lock: mi_lock_t,
    pub stats: mi_stats_t,
}

pub type mi_heap_t = mi_heap_s;

#[repr(C)]
pub struct mi_page_s {
    pub xthread_id: mi_threadid_t,
    pub free: *mut mi_block_t,
    pub used: u16,
    pub capacity: u16,
    pub reserved: u16,
    pub retire_expire: u8,
    pub free_is_zero: bool,
    pub local_free: *mut mi_block_t,
    pub xthread_free: mi_thread_free_t,
    pub block_size: usize,
    pub page_start: *mut u8,
    pub keys: [usize; 2],
    pub theap: *mut mi_theap_t,
    pub heap: *mut mi_heap_t,
    pub next: *mut mi_page_t,
    pub prev: *mut mi_page_t,
    pub slice_committed: usize,
    pub memid: crate::internal::mi_memid_t,
}
unsafe impl Send for mi_page_s {}
unsafe impl Sync for mi_page_s {}

#[repr(C)]
pub struct mi_theap_s {
    pub tld: *mut mi_tld_t,
    pub heap: *mut mi_heap_t,
    pub refcount: usize,
    pub heartbeat: u64,
    pub cookie: usize,
    pub random: mi_random_ctx_t,
    pub page_count: usize,
    pub page_retired_min: usize,
    pub page_retired_max: usize,
    pub pages_full_size: usize,
    pub generic_count: c_long,
    pub generic_collect_count: c_long,
    pub tnext: *mut mi_theap_t,
    pub tprev: *mut mi_theap_t,
    pub hnext: *mut mi_theap_t,
    pub hprev: *mut mi_theap_t,
    pub page_full_retain: c_long,
    pub allow_page_reclaim: bool,
    pub allow_page_abandon: bool,
    #[cfg(feature = "guarded")]
    pub guarded_size_min: usize,
    #[cfg(feature = "guarded")]
    pub guarded_size_max: usize,
    #[cfg(feature = "guarded")]
    pub guarded_sample_rate: usize,
    #[cfg(feature = "guarded")]
    pub guarded_sample_count: usize,
    pub pages_free_direct: [*mut mi_page_t; MI_PAGES_DIRECT],
    pub pages: [mi_page_queue_t; MI_BIN_COUNT],
    pub memid: crate::internal::mi_memid_t,
    pub stats: mi_stats_t,
}
unsafe impl Send for mi_theap_s {}
unsafe impl Sync for mi_theap_s {}
unsafe impl Send for mi_arena_pages_s {}
unsafe impl Sync for mi_arena_pages_s {}
unsafe impl Send for mi_subproc_s {}
unsafe impl Sync for mi_subproc_s {}
unsafe impl Send for mi_arena_s {}
unsafe impl Sync for mi_arena_s {}
unsafe impl Send for mi_heap_s {}
unsafe impl Sync for mi_heap_s {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_tls_slot_t {
    pub version: usize,
    pub value: *mut c_void,
}

unsafe impl Send for mi_tls_slot_t {}
unsafe impl Sync for mi_tls_slot_t {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_thread_locals_t {
    pub count: usize,
    pub slots: [mi_tls_slot_t; 1],
}

unsafe impl Send for mi_thread_locals_t {}
unsafe impl Sync for mi_thread_locals_t {}

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
