//! Mechanical translation of `include/mimalloc/internal.h`.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    uninhabited_static,
    unused_imports,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::{c_char, c_int, c_void};
use core::mem::{size_of, zeroed};
use core::ptr;
use std::os::raw::c_long;

pub use crate::arena::{MI_ARENA_BCHUNK_BITS, MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE};
pub use crate::bits::{
    MI_ARCH_X64, MI_ARCH_X86, MI_GiB, MI_INTPTR_SIZE, MI_KiB, MI_MAX_VABITS, MI_MiB,
    MI_PAGE_MAP_FLAT, MI_SIZE_BITS, MI_SIZE_SIZE, mi_rotl, mi_rotl32, mi_rotr,
};
pub use crate::page::{
    _mi_page_unfull, MI_LARGE_MAX_OBJ_SIZE, MI_MAX_ALLOC_SIZE, MI_MEDIUM_PAGE_SIZE,
    MI_OS_PAGE_SIZE, MI_PADDING_SIZE, MI_SMALL_PAGE_SIZE,
};
pub use crate::stats::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase,
    __mi_stat_adjust_increase_mt, __mi_stat_counter_increase, __mi_stat_counter_increase_mt,
    __mi_stat_decrease, __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt,
};
pub use crate::types::*;

pub const MI_INTERNAL_H: () = ();

pub const MI_DEBUG: usize = if cfg!(debug_assertions) { 1 } else { 0 };
pub const MI_SECURE: usize = 0;
pub const MI_TRACK_ASAN: usize = 0;
pub const MI_TRACK_ENABLED: usize = 0;
pub const MI_TRACK_ENABLED_FULL: usize = 0;
pub const MI_GUARDED: usize = 0;
pub const MI_PAGE_META_IS_SEPARATED: usize = if MI_PAGE_MAP_FLAT != 0 { 0 } else { 1 };
pub const MI_BIG_ENDIAN: usize = if cfg!(target_endian = "big") { 1 } else { 0 };
pub const __wasi__: usize = if cfg!(target_os = "wasi") { 1 } else { 0 };

pub const mi_decl_cache_align: usize = 64;
pub const mi_decl_forceinline: () = ();
pub const mi_decl_thread: () = ();
pub const mi_decl_weak: () = ();
pub const mi_decl_cold: () = ();
pub const mi_decl_noinline: () = ();
pub const mi_decl_noreturn: () = ();
pub const mi_decl_hidden: () = ();
pub const mi_decl_externc: () = ();
pub const mi_decl_maybe_unused: () = ();
pub const MI_INIT4: usize = 4;
pub const MI_INIT8: usize = 8;
pub const MI_INIT16: usize = 16;
pub const MI_INIT32: usize = 32;
pub const MI_INIT64: usize = 64;
pub const MI_INIT128: usize = 128;
pub const MI_INIT256: usize = 256;
pub const MI_INIT74: usize = 74;
pub const MI_INIT5: usize = 5;
pub const MI_INIT6: usize = 6;

pub const MI_PAGE_MAP_SUB_SHIFT: usize = 13;
pub const MI_PAGE_MAP_SUB_COUNT: usize = 1usize << MI_PAGE_MAP_SUB_SHIFT;
pub const MI_PAGE_MAP_SHIFT: usize = MI_MAX_VABITS - MI_PAGE_MAP_SUB_SHIFT - MI_ARENA_SLICE_SHIFT;
pub const MI_PAGE_MAP_COUNT: usize = 1usize << MI_PAGE_MAP_SHIFT;

pub const MI_BLOCK_TAG_ALIGNED: usize = 0;
pub const MI_BLOCK_TAG_GUARDED: usize = !MI_BLOCK_TAG_ALIGNED;

pub type mi_submap_t = *mut *mut mi_page_t;

macro_rules! __has_builtin {
    ($x:expr) => {
        false
    };
}

#[macro_export]
macro_rules! mi_trace_message {
    ($($arg:tt)*) => {{
        if $crate::internal::MI_DEBUG > 0 {
            unsafe { $crate::internal::_mi_trace_message($($arg)*) }
        }
    }};
}

#[macro_export]
macro_rules! mi_assert {
    ($expr:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 {
            debug_assert!($expr);
        }
    }};
    ($expr:expr, $($arg:tt)+) => {{
        if $crate::internal::MI_DEBUG > 0 {
            debug_assert!($expr, $($arg)+);
        }
    }};
}

#[macro_export]
macro_rules! mi_assert_internal {
    ($($arg:tt)*) => {{
        if $crate::internal::MI_DEBUG > 1 {
            $crate::mi_assert!($($arg)*);
        }
    }};
}

#[macro_export]
macro_rules! mi_assert_expensive {
    ($($arg:tt)*) => {{
        if $crate::internal::MI_DEBUG > 2 {
            $crate::mi_assert!($($arg)*);
        }
    }};
}

#[macro_export]
macro_rules! MI_UNUSED {
    ($x:expr) => {{
        let _ = &$x;
    }};
}

#[macro_export]
macro_rules! MI_UNUSED_RELEASE {
    ($x:expr) => {{
        if $crate::internal::MI_DEBUG <= 1 {
            $crate::MI_UNUSED!($x);
        }
    }};
}

#[macro_export]
macro_rules! MI_INIT4 {
    ($x:expr) => {
        $x, $x, $x, $x
    };
}

#[macro_export]
macro_rules! MI_INIT8 {
    ($x:expr) => {
        $crate::MI_INIT4!($x), $crate::MI_INIT4!($x)
    };
}

#[macro_export]
macro_rules! MI_INIT16 {
    ($x:expr) => {
        $crate::MI_INIT8!($x), $crate::MI_INIT8!($x)
    };
}

#[macro_export]
macro_rules! MI_INIT32 {
    ($x:expr) => {
        $crate::MI_INIT16!($x), $crate::MI_INIT16!($x)
    };
}

#[macro_export]
macro_rules! MI_INIT64 {
    ($x:expr) => {
        $crate::MI_INIT32!($x), $crate::MI_INIT32!($x)
    };
}

#[macro_export]
macro_rules! MI_INIT128 {
    ($x:expr) => {
        $crate::MI_INIT64!($x), $crate::MI_INIT64!($x)
    };
}

#[macro_export]
macro_rules! MI_INIT256 {
    ($x:expr) => {
        $crate::MI_INIT128!($x), $crate::MI_INIT128!($x)
    };
}

#[macro_export]
macro_rules! MI_INIT74 {
    ($x:expr) => {
        $crate::MI_INIT64!($x), $crate::MI_INIT8!($x), $x, $x
    };
}

#[macro_export]
macro_rules! MI_INIT5 {
    ($x:expr) => {
        $crate::MI_INIT4!($x), $x
    };
}

#[macro_export]
macro_rules! MI_INIT6 {
    ($x:expr) => {
        $crate::MI_INIT4!($x), $x, $x
    };
}

#[macro_export]
macro_rules! _mi_memzero_var {
    ($x:expr) => {{
        unsafe { core::ptr::write_bytes(core::ptr::addr_of_mut!($x), 0, 1) };
    }};
}

#[macro_export]
macro_rules! mi_theap_stat_counter_increase {
    ($theap:expr, $stat:ident, $amount:expr) => {{ unsafe { $crate::internal::__mi_stat_counter_increase(&mut (*$theap).stats.$stat, $amount) } }};
}

#[macro_export]
macro_rules! mi_theap_stat_decrease {
    ($theap:expr, $stat:ident, $amount:expr) => {{ unsafe { $crate::internal::__mi_stat_decrease(&mut (*$theap).stats.$stat, $amount) } }};
}

#[macro_export]
macro_rules! mi_theap_stat_adjust_decrease {
    ($theap:expr, $stat:ident, $amount:expr) => {{ unsafe { $crate::internal::__mi_stat_adjust_decrease(&mut (*$theap).stats.$stat, $amount) } }};
}

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

pub const MI_ARENA_SLICE_SHIFT: usize = MI_ARENA_SLICE_SIZE.trailing_zeros() as usize;

#[inline]
pub const fn mi_unlikely(x: bool) -> bool {
    x
}

#[inline]
pub const fn mi_likely(x: bool) -> bool {
    x
}

#[inline]
pub const fn MI_UNUSED<T>(_x: &T) {}

#[inline]
pub const fn MI_UNUSED_RELEASE<T>(_x: &T) {}

#[inline]
pub unsafe fn _mi_memzero_var<T>(x: *mut T) {
    ptr::write_bytes(x, 0, 1);
}

unsafe extern "C" {
    pub static _mi_page_empty: mi_page_t;
    pub static _mi_theap_empty: mi_theap_t;
    pub static _mi_theap_empty_wrong: mi_theap_t;
    #[cfg(target_pointer_width = "32")]
    pub static mut _mi_page_map: *mut u8;
    #[cfg(not(target_pointer_width = "32"))]
    pub static mut _mi_page_map: *mut mi_submap_t;
    pub static mut _mi_cpu_movsb_max: usize;
    pub static mut _mi_cpu_stosb_max: usize;

    pub fn _mi_vsnprintf(
        buf: *mut c_char,
        bufsize: usize,
        fmt: *const c_char,
        args: *mut c_void,
    ) -> c_int;
    pub fn _mi_snprintf(buf: *mut c_char, buflen: usize, fmt: *const c_char, ...) -> c_int;
    pub fn _mi_toupper(c: c_char) -> c_char;
    pub fn _mi_strnicmp(s: *const c_char, t: *const c_char, n: usize) -> c_int;
    pub fn _mi_strlcpy(dest: *mut c_char, src: *const c_char, dest_size: usize);
    pub fn _mi_strlcat(dest: *mut c_char, src: *const c_char, dest_size: usize);
    pub fn _mi_strlen(s: *const c_char) -> usize;
    pub fn _mi_strnlen(s: *const c_char, max_len: usize) -> usize;
    pub fn _mi_strnstr(s: *mut c_char, max_len: usize, pat: *const c_char) -> *mut c_char;
    pub fn _mi_streq(s: *const c_char, t: *const c_char) -> bool;
    pub fn _mi_getenv(name: *const c_char, result: *mut c_char, result_size: usize) -> bool;

    pub fn _mi_fputs(
        out: Option<mi_output_fun>,
        arg: *mut c_void,
        prefix: *const c_char,
        message: *const c_char,
    );
    pub fn _mi_fprintf(out: Option<mi_output_fun>, arg: *mut c_void, fmt: *const c_char, ...);
    pub fn _mi_raw_message(fmt: *const c_char, ...);
    pub fn _mi_message(fmt: *const c_char, ...);
    pub fn _mi_warning_message(fmt: *const c_char, ...);
    pub fn _mi_verbose_message(fmt: *const c_char, ...);
    pub fn _mi_trace_message(fmt: *const c_char, ...);
    pub fn _mi_options_init();
    pub fn _mi_options_post_init();
    pub fn _mi_option_get_fast(option: crate::options::mi_option_t) -> c_long;
    pub fn _mi_error_message(err: c_int, fmt: *const c_char, ...);

    pub fn _mi_random_init(ctx: *mut mi_random_ctx_t);
    pub fn _mi_random_init_weak(ctx: *mut mi_random_ctx_t);
    pub fn _mi_random_reinit_if_weak(ctx: *mut mi_random_ctx_t);
    pub fn _mi_random_split(ctx: *mut mi_random_ctx_t, new_ctx: *mut mi_random_ctx_t);
    pub fn _mi_random_next(ctx: *mut mi_random_ctx_t) -> usize;
    pub fn _mi_theap_random_next(theap: *mut mi_theap_t) -> usize;
    pub fn _mi_os_random_weak(extra_seed: usize) -> usize;

    pub fn _mi_auto_process_init();
    pub fn _mi_auto_process_done();
    pub fn _mi_is_redirected() -> bool;
    pub fn _mi_allocator_init(message: *mut *const c_char) -> bool;
    pub fn _mi_allocator_done();
    pub fn _mi_is_main_thread() -> bool;
    pub fn _mi_preloading() -> bool;
    pub fn _mi_thread_done(theap: *mut mi_theap_t);
    pub fn _mi_subproc() -> *mut mi_subproc_t;
    pub fn _mi_subproc_main() -> *mut mi_subproc_t;
    pub fn _mi_subproc_heap_main(subproc: *mut mi_subproc_t) -> *mut mi_heap_t;
    pub fn _mi_subproc_from_id(subproc_id: mi_subproc_id_t) -> *mut mi_subproc_t;
    pub fn _mi_thread_id() -> mi_threadid_t;
    pub fn _mi_thread_seq_id() -> usize;
    pub fn _mi_is_heap_main(heap: *const mi_heap_t) -> bool;
    pub fn _mi_is_theap_main(theap: *const mi_theap_t) -> bool;
    pub fn _mi_theap_guarded_init(theap: *mut mi_theap_t);
    pub fn _mi_theap_options_init(theap: *mut mi_theap_t);
    pub fn _mi_theap_default_safe() -> *mut mi_theap_t;
    pub fn _mi_theap_main_safe() -> *mut mi_theap_t;

    pub fn _mi_os_init();
    pub fn _mi_os_alloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void;
    pub fn _mi_os_zalloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void;
    pub fn _mi_os_free(p: *mut c_void, size: usize, memid: mi_memid_t);
    pub fn _mi_os_free_ex(
        p: *mut c_void,
        size: usize,
        still_committed: bool,
        memid: mi_memid_t,
        subproc: *mut mi_subproc_t,
    );
    pub fn _mi_os_page_size() -> usize;
    pub fn _mi_os_guard_page_size() -> usize;
    pub fn _mi_os_good_alloc_size(size: usize) -> usize;
    pub fn _mi_os_has_overcommit() -> bool;
    pub fn _mi_os_has_virtual_reserve() -> bool;
    pub fn _mi_os_virtual_address_bits() -> usize;
    pub fn _mi_os_minimal_purge_size() -> usize;
    pub fn _mi_os_reset(addr: *mut c_void, size: usize) -> bool;
    pub fn _mi_os_decommit(addr: *mut c_void, size: usize) -> bool;
    pub fn _mi_os_reuse(p: *mut c_void, size: usize);
    pub fn _mi_os_commit(p: *mut c_void, size: usize, is_zero: *mut bool) -> bool;
    pub fn _mi_os_commit_ex(
        addr: *mut c_void,
        size: usize,
        is_zero: *mut bool,
        stat_size: usize,
    ) -> bool;
    pub fn _mi_os_protect(addr: *mut c_void, size: usize) -> bool;
    pub fn _mi_os_unprotect(addr: *mut c_void, size: usize) -> bool;
    pub fn _mi_os_purge(p: *mut c_void, size: usize) -> bool;
    pub fn _mi_os_purge_ex(
        p: *mut c_void,
        size: usize,
        allow_reset: bool,
        stats_size: usize,
        commit_fun: *mut mi_commit_fun_t,
        commit_fun_arg: *mut c_void,
    ) -> bool;
    pub fn _mi_os_get_aligned_hint(try_alignment: usize, size: usize) -> *mut c_void;
    pub fn _mi_os_canuse_large_page(size: usize, alignment: usize) -> bool;
    pub fn _mi_os_large_page_size() -> usize;

    pub fn _mi_arena_id_none() -> mi_arena_id_t;
    pub fn _mi_arenas_contain(p: *const c_void) -> bool;
    pub fn _mi_meta_is_meta_page(p: *mut c_void) -> bool;
    pub fn _mi_page_map_init() -> bool;
    pub fn _mi_page_map_unregister_range(start: *mut c_void, size: usize);
}

#[inline]
pub const fn _mi_is_power_of_two(x: usize) -> bool {
    (x & x.wrapping_sub(1)) == 0
}

#[inline]
pub fn _mi_is_aligned(p: *const c_void, alignment: usize) -> bool {
    alignment == 0 || (p as usize).is_multiple_of(alignment)
}

#[inline]
pub const fn _mi_align_up(sz: usize, alignment: usize) -> usize {
    let mask = alignment.wrapping_sub(1);
    if alignment != 0 && (alignment & mask) == 0 {
        (sz + mask) & !mask
    } else if alignment == 0 {
        sz
    } else {
        ((sz + mask) / alignment) * alignment
    }
}

#[inline]
pub const fn _mi_align_down(sz: usize, alignment: usize) -> usize {
    let mask = alignment.wrapping_sub(1);
    if alignment != 0 && (alignment & mask) == 0 {
        sz & !mask
    } else if alignment == 0 {
        sz
    } else {
        (sz / alignment) * alignment
    }
}

#[inline]
pub fn _mi_align_up_ptr(p: *const c_void, alignment: usize) -> *mut c_void {
    _mi_align_up(p as usize, alignment) as *mut c_void
}

#[inline]
pub fn _mi_align_down_ptr(p: *const c_void, alignment: usize) -> *mut c_void {
    _mi_align_down(p as usize, alignment) as *mut c_void
}

#[inline]
pub const fn _mi_divide_up(size: usize, divider: usize) -> usize {
    if divider == 0 {
        size
    } else {
        size.div_ceil(divider)
    }
}

#[inline]
pub const fn _mi_clamp(sz: usize, min: usize, max: usize) -> usize {
    if sz < min {
        min
    } else if sz > max {
        max
    } else {
        sz
    }
}

#[inline]
pub unsafe fn mi_mem_is_zero(p: *const c_void, size: usize) -> bool {
    let bytes = p.cast::<u8>();
    let mut i = 0;
    while i < size {
        if *bytes.add(i) != 0 {
            return false;
        }
        i += 1;
    }
    true
}

#[inline]
pub const fn _mi_wsize_from_size(size: usize) -> usize {
    size.div_ceil(size_of::<usize>())
}

#[inline]
pub fn mi_mul_overflow(count: usize, size: usize, total: &mut usize) -> bool {
    match count.checked_mul(size) {
        Some(product) => {
            *total = product;
            false
        }
        None => {
            *total = usize::MAX;
            true
        }
    }
}

#[inline]
pub unsafe fn mi_count_size_overflow(count: usize, size: usize, total: &mut usize) -> bool {
    if count == 1 {
        *total = size;
        false
    } else if !mi_mul_overflow(count, size, total) {
        false
    } else {
        _mi_error_message(
            75,
            c"allocation request is too large (%zu * %zu bytes)\n".as_ptr(),
            count,
            size,
        );
        *total = usize::MAX;
        true
    }
}

#[inline]
pub unsafe fn _mi_theap_heap(theap: *const mi_theap_t) -> *mut mi_heap_t {
    crate::atomic::mi_atomic_load_ptr_acquire(&(*theap).heap)
}

#[inline]
pub unsafe fn mi_theap_is_initialized(theap: *const mi_theap_t) -> bool {
    !theap.is_null() && !unsafe { _mi_theap_heap(theap) }.is_null()
}

#[inline]
pub unsafe fn _mi_theap_get_free_small_page(theap: *mut mi_theap_t, size: usize) -> *mut mi_page_t {
    let idx = _mi_wsize_from_size(size);
    (*theap).pages_free_direct[idx]
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub fn _mi_page_map_index(p: *const c_void) -> usize {
    (p as usize) >> MI_ARENA_SLICE_SHIFT
}

#[cfg(not(target_pointer_width = "32"))]
#[inline]
pub unsafe fn _mi_page_map_index(p: *const c_void, sub_idx: *mut usize) -> usize {
    let u = (p as usize) / MI_ARENA_SLICE_SIZE;
    if !sub_idx.is_null() {
        unsafe { *sub_idx = u % MI_PAGE_MAP_SUB_COUNT };
    }
    u / MI_PAGE_MAP_SUB_COUNT
}

#[cfg(not(target_pointer_width = "32"))]
#[inline]
pub unsafe fn _mi_page_map_at(idx: usize) -> mi_submap_t {
    crate::atomic::mi_atomic_load_ptr_acquire(_mi_page_map.add(idx))
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub unsafe fn _mi_ptr_page_ex(p: *const c_void, valid: *mut bool) -> *mut mi_page_t {
    let idx = _mi_page_map_index(p);
    let ofs = *_mi_page_map.add(idx) as usize;
    if !valid.is_null() {
        *valid = ofs != 0;
    }
    ((((p as usize) >> MI_ARENA_SLICE_SHIFT) + 1 - ofs) << MI_ARENA_SLICE_SHIFT) as *mut mi_page_t
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub unsafe fn _mi_checked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    let mut valid = false;
    let page = _mi_ptr_page_ex(p, &mut valid);
    if valid { page } else { ptr::null_mut() }
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub unsafe fn _mi_unchecked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    _mi_ptr_page_ex(p, ptr::null_mut())
}

#[cfg(not(target_pointer_width = "32"))]
#[inline]
pub unsafe fn _mi_unchecked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    let mut sub_idx = 0usize;
    let idx = _mi_page_map_index(p, &mut sub_idx);
    _mi_page_map_at(idx).add(sub_idx).read()
}

#[cfg(not(target_pointer_width = "32"))]
#[inline]
pub unsafe fn _mi_checked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    let mut sub_idx = 0usize;
    let idx = _mi_page_map_index(p, &mut sub_idx);
    let sub = _mi_page_map_at(idx);
    if sub.is_null() {
        ptr::null_mut()
    } else {
        *sub.add(sub_idx)
    }
}

#[inline]
pub unsafe fn _mi_ptr_page(p: *const c_void) -> *mut mi_page_t {
    crate::mi_assert_internal!(p.is_null() || crate::mimalloc::mi_is_in_heap_region(p));
    if MI_DEBUG != 0 || MI_SECURE != 0 || cfg!(target_vendor = "apple") {
        unsafe { _mi_checked_ptr_page(p) }
    } else {
        unsafe { _mi_unchecked_ptr_page(p) }
    }
}

#[inline]
pub unsafe fn mi_page_block_size(page: *const mi_page_t) -> usize {
    crate::mi_assert_internal!((*page).block_size > 0);
    (*page).block_size
}

#[inline]
pub unsafe fn mi_page_start(page: *const mi_page_t) -> *mut u8 {
    (*page).page_start
}

#[inline]
pub unsafe fn mi_page_size(page: *const mi_page_t) -> usize {
    let block_size = unsafe { mi_page_block_size(page) };
    block_size * ((*page).reserved as usize)
}

#[inline]
pub unsafe fn mi_page_area(page: *const mi_page_t, size: *mut usize) -> *mut u8 {
    if !size.is_null() {
        *size = unsafe { mi_page_size(page) };
    }
    unsafe { mi_page_start(page) }
}

#[inline]
pub const fn mi_page_info_size() -> usize {
    _mi_align_up(size_of::<mi_page_t>(), 16)
}

#[inline]
pub unsafe fn mi_page_contains_address(page: *const mi_page_t, p: *const c_void) -> bool {
    let mut psize = 0usize;
    let start = unsafe { mi_page_area(page, &mut psize) };
    start <= p.cast_mut().cast::<u8>() && p.cast::<u8>() < start.add(psize)
}

#[inline]
pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool {
    (*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA
}

#[inline]
pub unsafe fn mi_page_is_singleton(page: *const mi_page_t) -> bool {
    (*page).reserved == 1
}

#[inline]
pub unsafe fn mi_page_usable_block_size(page: *const mi_page_t) -> usize {
    let block_size = unsafe { mi_page_block_size(page) };
    block_size - MI_PADDING_SIZE
}

#[inline]
pub unsafe fn mi_page_meta_is_separated(page: *const mi_page_t) -> bool {
    if MI_PAGE_META_IS_SEPARATED != 0 {
        (*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA
            && page.cast_mut().cast::<c_void>()
                != _mi_align_down_ptr((*page).page_start.cast(), MI_ARENA_SLICE_ALIGN)
    } else {
        false
    }
}

#[inline]
pub unsafe fn mi_page_slice_start(page: *const mi_page_t) -> *mut u8 {
    if mi_page_meta_is_separated(page) {
        _mi_align_down_ptr((*page).page_start.cast(), MI_ARENA_SLICE_ALIGN).cast::<u8>()
    } else {
        page.cast_mut().cast::<u8>()
    }
}

#[inline]
pub unsafe fn mi_page_slice_offset_of(
    page: *const mi_page_t,
    offset_relative_to_page_start: usize,
) -> usize {
    ((*page).page_start as usize - unsafe { mi_page_slice_start(page) } as usize)
        + offset_relative_to_page_start
}

#[inline]
pub unsafe fn mi_page_committed(page: *const mi_page_t) -> usize {
    if (*page).slice_committed == 0 {
        unsafe { mi_page_size(page) }
    } else {
        (*page).slice_committed - unsafe { mi_page_slice_offset_of(page, 0) }
    }
}

#[inline]
pub unsafe fn mi_page_all_free(page: *const mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    (*page).used == 0
}

#[inline]
pub unsafe fn mi_page_xthread_id(page: *const mi_page_t) -> mi_threadid_t {
    crate::atomic::mi_atomic_load_explicit(
        &(*page).xthread_id,
        crate::atomic::mi_memory_order_e::mi_memory_order_relaxed,
    )
}

#[inline]
pub unsafe fn mi_page_thread_id(page: *const mi_page_t) -> mi_threadid_t {
    let xthread_id = unsafe { mi_page_xthread_id(page) };
    xthread_id & !MI_PAGE_FLAG_MASK
}

#[inline]
pub unsafe fn mi_page_flags(page: *const mi_page_t) -> mi_page_flags_t {
    let xthread_id = unsafe { mi_page_xthread_id(page) };
    xthread_id & MI_PAGE_FLAG_MASK
}

#[inline]
pub fn mi_tf_block(tf: mi_thread_free_t) -> *mut mi_block_t {
    (tf & !1usize) as *mut mi_block_t
}

#[inline]
pub const fn mi_tf_is_owned(tf: mi_thread_free_t) -> bool {
    (tf & 1) == 1
}

#[inline]
pub fn mi_tf_create(block: *mut mi_block_t, owned: bool) -> mi_thread_free_t {
    block as usize | if owned { 1 } else { 0 }
}

#[inline]
pub unsafe fn mi_page_thread_free(page: *const mi_page_t) -> *mut mi_block_t {
    mi_tf_block(crate::atomic::mi_atomic_load_explicit(
        &(*page).xthread_free,
        crate::atomic::mi_memory_order_e::mi_memory_order_relaxed,
    ))
}

#[inline]
pub unsafe fn mi_rep_movsb(dst: *mut c_void, src: *const c_void, n: usize) {
    ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), n);
}

#[inline]
pub const fn mi_slice_count_of_size(size: usize) -> usize {
    _mi_divide_up(size, MI_ARENA_SLICE_SIZE)
}

#[inline]
pub const fn mi_size_of_slices(bcount: usize) -> usize {
    bcount * MI_ARENA_SLICE_SIZE
}

#[inline]
pub unsafe fn _mi_memid_create(memkind: mi_memkind_t) -> mi_memid_t {
    let mut memid: mi_memid_t = zeroed();
    memid.memkind = memkind;
    memid
}

#[inline]
pub unsafe fn _mi_memid_none() -> mi_memid_t {
    _mi_memid_create(mi_memkind_t::MI_MEM_NONE)
}

#[inline]
pub unsafe fn _mi_memid_create_os(
    base: *mut c_void,
    size: usize,
    committed: bool,
    is_zero: bool,
    is_large: bool,
) -> mi_memid_t {
    let mut memid = _mi_memid_create(mi_memkind_t::MI_MEM_OS);
    memid.mem.os.base = base;
    memid.mem.os.size = size;
    memid.initially_committed = committed;
    memid.initially_zero = is_zero;
    memid.is_pinned = is_large;
    memid
}

#[inline]
pub unsafe fn _mi_memid_create_meta(
    mpage: *mut c_void,
    block_idx: usize,
    block_count: usize,
) -> mi_memid_t {
    let mut memid = _mi_memid_create(mi_memkind_t::MI_MEM_META);
    memid.mem.meta.meta_page = mpage;
    memid.mem.meta.block_index = block_idx as u32;
    memid.mem.meta.block_count = block_count as u32;
    memid.initially_committed = true;
    memid.initially_zero = true;
    memid.is_pinned = true;
    memid
}

#[inline]
pub const fn _mi_random_shuffle(mut x: usize) -> usize {
    if x == 0 {
        x = 17;
    }
    #[cfg(target_pointer_width = "64")]
    {
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
    }
    #[cfg(target_pointer_width = "32")]
    {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;
    }
    x
}

#[inline]
pub fn mi_is_in_same_page(p: *const c_void, q: *const c_void) -> bool {
    _mi_align_down(p as usize, MI_ARENA_SLICE_SIZE)
        == _mi_align_down(q as usize, MI_ARENA_SLICE_SIZE)
}

#[inline]
pub fn mi_ptr_decode(null: *const c_void, x: usize, keys: &[usize; 2]) -> *mut c_void {
    let p = (mi_rotr(x.wrapping_sub(keys[0]), keys[0]) ^ keys[1]) as *mut c_void;
    if ptr::eq(p.cast_const(), null) {
        ptr::null_mut()
    } else {
        p
    }
}

#[inline]
pub fn mi_ptr_encode(null: *const c_void, p: *const c_void, keys: &[usize; 2]) -> usize {
    let x = if p.is_null() {
        null as usize
    } else {
        p as usize
    };
    mi_rotl(x ^ keys[1], keys[0]).wrapping_add(keys[0])
}

#[inline]
pub fn mi_ptr_encode_canary(null: *const c_void, p: *const c_void, keys: &[usize; 2]) -> u32 {
    let x = mi_ptr_encode(null, p, keys) as u32;
    if MI_BIG_ENDIAN != 0 {
        x & 0x00FF_FFFF
    } else {
        x & 0xFFFF_FF00
    }
}

#[inline]
pub unsafe fn mi_block_nextx(
    null: *const c_void,
    block: *const mi_block_t,
    keys: &[usize; 2],
) -> *mut mi_block_t {
    unsafe { crate::track::mi_track_mem_defined(block.cast_mut().cast(), size_of::<mi_block_t>()) };
    let next = mi_ptr_decode(null, (*block).next, keys).cast::<mi_block_t>();
    unsafe {
        crate::track::mi_track_mem_noaccess(block.cast_mut().cast(), size_of::<mi_block_t>())
    };
    next
}

#[inline]
pub unsafe fn mi_block_set_nextx(
    null: *const c_void,
    block: *mut mi_block_t,
    next: *const mi_block_t,
    keys: &[usize; 2],
) {
    unsafe { crate::track::mi_track_mem_undefined(block.cast(), size_of::<mi_block_t>()) };
    (*block).next = mi_ptr_encode(null, next.cast(), keys);
    unsafe { crate::track::mi_track_mem_noaccess(block.cast(), size_of::<mi_block_t>()) };
}

#[inline]
pub unsafe fn mi_block_next(page: *const mi_page_t, block: *const mi_block_t) -> *mut mi_block_t {
    let mut next = unsafe { mi_block_nextx(page.cast(), block, &(*page).keys) };
    if !next.is_null() && !mi_is_in_same_page(block.cast(), next.cast()) {
        unsafe {
            _mi_error_message(
                14,
                c"corrupted free list entry of size %zub at %p: value 0x%zx\n".as_ptr(),
                mi_page_block_size(page),
                block,
                next as usize,
            );
        }
        next = ptr::null_mut();
    }
    next
}

#[inline]
pub unsafe fn mi_block_set_next(
    page: *const mi_page_t,
    block: *mut mi_block_t,
    next: *const mi_block_t,
) {
    unsafe { mi_block_set_nextx(page.cast(), block, next, &(*page).keys) };
}

#[inline]
pub unsafe fn _mi_memcpy(dst: *mut c_void, src: *const c_void, n: usize) {
    ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), n);
}

#[inline]
pub unsafe fn _mi_memset(dst: *mut c_void, val: i32, n: usize) {
    ptr::write_bytes(dst, val as u8, n);
}

#[inline]
pub unsafe fn _mi_memcpy_aligned(dst: *mut c_void, src: *const c_void, n: usize) {
    _mi_memcpy(dst, src, n);
}

#[inline]
pub unsafe fn _mi_memset_aligned(dst: *mut c_void, val: i32, n: usize) {
    _mi_memset(dst, val, n);
}

#[inline]
pub unsafe fn _mi_memzero(dst: *mut c_void, n: usize) {
    _mi_memset(dst, 0, n);
}

#[inline]
pub unsafe fn _mi_memzero_aligned(dst: *mut c_void, n: usize) {
    _mi_memset_aligned(dst, 0, n);
}
