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

pub(crate) use crate::alloc::{
    _mi_page_malloc_zero, _mi_theap_malloc_guarded, _mi_theap_malloc_zero,
    _mi_theap_malloc_zero_ex, _mi_theap_realloc_zero,
};
pub(crate) use crate::arena::{
    _mi_arena_from_id, _mi_arena_id_none, _mi_arena_memid_is_suitable, _mi_arenas_alloc,
    _mi_arenas_alloc_aligned, _mi_arenas_collect, _mi_arenas_contain, _mi_arenas_free,
    _mi_arenas_page_abandon, _mi_arenas_page_alloc, _mi_arenas_page_free,
    _mi_arenas_page_try_reabandon_to_mapped, _mi_arenas_page_unabandon,
    _mi_arenas_unsafe_destroy_all, _mi_heap_destroy_pages, _mi_heap_move_pages,
    _mi_heap_visit_blocks,
};
pub use crate::arena::{MI_ARENA_BCHUNK_BITS, MI_ARENA_SLICE_ALIGN, MI_ARENA_SLICE_SIZE};
pub(crate) use crate::arena_meta::{_mi_meta_free, _mi_meta_is_meta_page, _mi_meta_zalloc};
use crate::bits::MI_SIZE_SHIFT;
pub use crate::bits::{
    MI_ARCH_X64, MI_ARCH_X86, MI_GiB, MI_INTPTR_SIZE, MI_KiB, MI_MAX_VABITS, MI_MiB,
    MI_PAGE_MAP_FLAT, MI_SIZE_BITS, MI_SIZE_SIZE, mi_rotl, mi_rotl32, mi_rotr,
};
#[cfg(not(mi_c_malloc_fast))]
pub(crate) use crate::free::_mi_padding_shrink;
pub(crate) use crate::free::_mi_page_ptr_unalign;
pub(crate) use crate::heap::{
    _mi_heap_force_destroy, _mi_heap_theap_get_or_init, _mi_heap_theap_get_peek,
};
pub(crate) use crate::init::{
    _mi_auto_process_done, _mi_auto_process_init, _mi_cpu_stosb_max, _mi_is_heap_main,
    _mi_is_theap_main, _mi_preloading, _mi_subproc, _mi_subproc_from_id, _mi_subproc_heap_main,
    _mi_subproc_main, _mi_theap_cached_set, _mi_theap_default_safe, _mi_theap_default_set,
    _mi_theap_empty_get, _mi_theap_guarded_init, _mi_theap_main_safe, _mi_theap_options_init,
    _mi_thread_done, _mi_thread_id,
};
pub(crate) use crate::libc::{
    _mi_getenv, _mi_streq, _mi_strlcat, _mi_strlcpy, _mi_strlen, _mi_strnicmp, _mi_strnlen,
    _mi_strnstr, _mi_toupper, _mi_vsnprintf,
};
use crate::options::_mi_error_message_fmt;
pub(crate) use crate::options::{
    _mi_error_message, _mi_option_get_fast, _mi_trace_message, _mi_verbose_message,
    _mi_warning_message,
};
pub(crate) use crate::os::{
    _mi_os_alloc, _mi_os_canuse_large_page, _mi_os_commit, _mi_os_commit_ex, _mi_os_decommit,
    _mi_os_free, _mi_os_free_ex, _mi_os_get_aligned_hint, _mi_os_good_alloc_size,
    _mi_os_guard_page_size, _mi_os_has_overcommit, _mi_os_has_virtual_reserve, _mi_os_init,
    _mi_os_large_page_size, _mi_os_minimal_purge_size, _mi_os_page_size, _mi_os_protect,
    _mi_os_purge, _mi_os_purge_ex, _mi_os_reset, _mi_os_reuse, _mi_os_unprotect,
    _mi_os_virtual_address_bits, _mi_os_zalloc,
};
pub(crate) use crate::page::{
    _mi_deferred_free, _mi_page_abandon, _mi_page_free, _mi_page_init, _mi_page_retire,
    _mi_theap_collect_retired,
};
pub use crate::page::{
    _mi_page_free_collect, _mi_page_free_collect_partly, _mi_page_queue_append,
    _mi_page_queue_is_valid, _mi_page_stats_bin, _mi_page_unfull, _mi_theap_page_reclaim,
    MI_LARGE_MAX_OBJ_SIZE, MI_MAX_ALLOC_SIZE, MI_MEDIUM_PAGE_SIZE, MI_OS_PAGE_SIZE,
    MI_PADDING_SIZE, MI_SMALL_PAGE_SIZE, mi_page_flags_set, mi_page_is_abandoned, mi_page_is_huge,
    mi_page_is_in_full, mi_page_set_in_full, mi_page_set_theap, mi_page_theap,
};
pub(crate) use crate::page_map::{
    _mi_page_map_base, _mi_page_map_init, _mi_page_map_register, _mi_page_map_unregister,
    _mi_page_map_unregister_range, _mi_page_map_unsafe_destroy, _mi_safe_ptr_page,
};
pub(crate) use crate::prim::{_mi_allocator_done, _mi_allocator_init};
pub(crate) use crate::random::{
    _mi_os_random_weak, _mi_random_init, _mi_random_init_weak, _mi_random_next,
    _mi_random_reinit_if_weak, _mi_random_split,
};
pub(crate) use crate::stats::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase,
    __mi_stat_adjust_increase_mt, __mi_stat_counter_increase, __mi_stat_counter_increase_mt,
    __mi_stat_decrease, __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt,
};
pub(crate) use crate::theap::{
    _mi_theap_area_visit_blocks, _mi_theap_collect_abandon, _mi_theap_create, _mi_theap_decref,
    _mi_theap_delete, _mi_theap_free, _mi_theap_incref, _mi_theap_random_next,
};
pub use crate::types::*;

pub const MI_INTERNAL_H: () = ();
pub const _Atomic: () = ();

pub const MI_DEBUG: usize = if cfg!(debug_assertions) { 2 } else { 0 };
pub const MI_SECURE: usize = if cfg!(feature = "secure") { 4 } else { 0 };
pub const MI_TRACK_ASAN: usize = crate::track::MI_TRACK_ASAN;
pub const MI_TRACK_ENABLED: usize = crate::track::MI_TRACK_ENABLED;
pub const MI_TRACK_ENABLED_FULL: usize = 0;
pub const MI_GUARDED: usize = if cfg!(feature = "guarded") { 1 } else { 0 };
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

#[cold]
pub(crate) fn _mi_rust_assert_fail(assertion: *const c_char, fname: *const c_char, line: u32) -> ! {
    unsafe { crate::options::_mi_assert_fail(assertion, fname, line, ptr::null()) }
}

macro_rules! __has_builtin {
    ($x:expr) => {
        false
    };
}

#[macro_export]
macro_rules! mi_debug_assert {
    ($expr:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 && !$expr {
            $crate::internal::_mi_rust_assert_fail(
                concat!(stringify!($expr), "\0").as_ptr().cast(),
                concat!(file!(), "\0").as_ptr().cast(),
                line!(),
            )
        }
    }};
    ($expr:expr, $($arg:tt)+) => {{
        $crate::mi_debug_assert!($expr);
    }};
}

#[macro_export]
macro_rules! mi_debug_assert_eq {
    ($left:expr, $right:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 {
            let left = &$left;
            let right = &$right;
            if !(*left == *right) {
                $crate::internal::_mi_rust_assert_fail(
                    concat!(stringify!($left), " == ", stringify!($right), "\0")
                        .as_ptr()
                        .cast(),
                    concat!(file!(), "\0").as_ptr().cast(),
                    line!(),
                )
            }
        }
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        $crate::mi_debug_assert_eq!($left, $right);
    }};
}

#[macro_export]
macro_rules! mi_trace_message {
    ($($arg:tt)*) => {{
        if $crate::internal::MI_DEBUG > 0 {
            $crate::internal::_mi_trace_message(format_args!($($arg)*))
        }
    }};
}

#[macro_export]
macro_rules! mi_assert {
    ($expr:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 {
            $crate::mi_debug_assert!($expr);
        }
    }};
    ($expr:expr, $($arg:tt)+) => {{
        if $crate::internal::MI_DEBUG > 0 {
            $crate::mi_debug_assert!($expr, $($arg)+);
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
    ($theap:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_counter_increase(
                core::ptr::addr_of_mut!((*$theap).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_theap_stat_decrease {
    ($theap:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_decrease(
                core::ptr::addr_of_mut!((*$theap).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_theap_stat_adjust_decrease {
    ($theap:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_adjust_decrease(
                core::ptr::addr_of_mut!((*$theap).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_heap_stat_decrease {
    ($heap:expr, $stat:ident, $amount:expr) => {{ unsafe { $crate::internal::__mi_stat_decrease_mt(&mut (*$heap).stats.$stat, $amount) } }};
}

#[macro_export]
macro_rules! mi_subproc_stat_decrease {
    ($subproc:expr, $stat:ident, $amount:expr) => {{ unsafe { $crate::internal::__mi_stat_decrease_mt(&mut (*$subproc).stats.$stat, $amount) } }};
}

#[macro_export]
macro_rules! mi_heap_stat_adjust_decrease {
    ($heap:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_adjust_decrease_mt(
                core::ptr::addr_of_mut!((*$heap).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_subproc_stat_adjust_decrease {
    ($subproc:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_adjust_decrease_mt(
                core::ptr::addr_of_mut!((*$subproc).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_heap_stat_counter_increase {
    ($heap:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_counter_increase_mt(
                core::ptr::addr_of_mut!((*$heap).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_subproc_stat_counter_increase {
    ($subproc:expr, $stat:ident, $amount:expr) => {{
        unsafe {
            $crate::internal::__mi_stat_counter_increase_mt(
                core::ptr::addr_of_mut!((*$subproc).stats.$stat),
                $amount,
            )
        }
    }};
}

#[macro_export]
macro_rules! mi_os_stat_counter_increase {
    ($stat:ident, $amount:expr) => {{ $crate::mi_subproc_stat_counter_increase!($crate::internal::_mi_subproc(), $stat, $amount) }};
}

#[macro_export]
macro_rules! mi_os_stat_decrease {
    ($stat:ident, $amount:expr) => {{ $crate::mi_subproc_stat_decrease!($crate::internal::_mi_subproc(), $stat, $amount) }};
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

pub const MI_ARENA_SLICE_SHIFT: usize = 13 + MI_SIZE_SHIFT;
pub const MI_BCHUNK_BITS_SHIFT: usize = 6 + MI_SIZE_SHIFT;

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
    pub fn _mi_snprintf(buf: *mut c_char, buflen: usize, fmt: *const c_char, ...) -> c_int;

    pub fn _mi_fputs(
        out: Option<mi_output_fun>,
        arg: *mut c_void,
        prefix: *const c_char,
        message: *const c_char,
    );
    pub fn _mi_fprintf(out: Option<mi_output_fun>, arg: *mut c_void, fmt: *const c_char, ...);
    pub fn _mi_raw_message(fmt: *const c_char, ...);
    pub fn _mi_message(fmt: *const c_char, ...);
    pub fn _mi_options_init();
    pub fn _mi_options_post_init();

    pub fn _mi_is_redirected() -> bool;
    pub fn _mi_is_main_thread() -> bool;
    pub fn _mi_thread_seq_id() -> usize;

    pub fn _mi_malloc_generic(
        theap: *mut mi_theap_t,
        size: usize,
        zero_huge_alignment: usize,
        usable: *mut usize,
    ) -> *mut c_void;
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

#[inline(always)]
pub(crate) fn mi_direct_bin_from_size(size: usize) -> crate::core_types::DirectBin {
    let idx = _mi_wsize_from_size(size);
    crate::mi_debug_assert!(idx < crate::types::MI_PAGES_DIRECT);
    crate::core_types::DirectBin::new(idx as u16)
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
        _mi_error_message_fmt(
            75,
            format_args!("allocation request is too large ({count} * {size} bytes)\n"),
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
    let idx = mi_direct_bin_from_size(size).as_usize();
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
    crate::atomic::mi_atomic_load_ptr_acquire(_mi_page_map_base().add(idx))
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub unsafe fn _mi_ptr_page_ex(p: *const c_void, valid: *mut bool) -> *mut mi_page_t {
    let idx = _mi_page_map_index(p);
    let ofs = *_mi_page_map_base().add(idx) as usize;
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
pub unsafe fn mi_page_tld(page: *const mi_page_t) -> *mut mi_tld_t {
    crate::mi_assert_internal!(!crate::page::mi_page_is_abandoned(page));
    crate::mi_assert_internal!(!(*page).theap.is_null());
    (*(*page).theap).tld
}

#[inline]
pub unsafe fn mi_page_heap(page: *const mi_page_t) -> *mut mi_heap_t {
    let mut heap = (*page).heap;
    if mi_likely(heap.is_null()) {
        heap = crate::mimalloc::mi_heap_main();
    }
    crate::mi_assert_internal!(!heap.is_null());
    heap
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
    matches!(
        crate::core_types::MemId::view((*page).memid),
        crate::core_types::MemIdView::Arena(_)
    )
}

#[inline]
pub unsafe fn mi_page_is_singleton(page: *const mi_page_t) -> bool {
    (*page).reserved == 1
}

#[inline]
pub(crate) unsafe fn mi_page_usable_block_size(
    page: *const mi_page_t,
) -> crate::core_types::UsableSize {
    // SAFETY: mi_page_block_size is never zero for an initialized page.
    let block_size = crate::core_types::BlockSize::new_unchecked(mi_page_block_size(page));
    block_size.usable()
}

#[inline]
pub unsafe fn mi_page_meta_is_separated(page: *const mi_page_t) -> bool {
    if MI_PAGE_META_IS_SEPARATED != 0 {
        mi_page_is_in_arena(page)
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
pub unsafe fn mi_page_immediate_available(page: *const mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    !(*page).free.is_null()
}

#[inline]
pub unsafe fn mi_page_is_expandable(page: *const mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_internal!((*page).capacity <= (*page).reserved);
    (*page).capacity < (*page).reserved
}

#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    let full = (*page).reserved == (*page).used;
    crate::mi_assert_internal!(!full || (*page).free.is_null());
    full
}

#[inline]
pub unsafe fn mi_page_is_mostly_used(page: *const mi_page_t) -> bool {
    if page.is_null() {
        return true;
    }
    let frac = (*page).reserved / 8u16;
    (*page).reserved - (*page).used <= frac
}

#[inline]
pub unsafe fn mi_page_is_used_at_frac(page: *const mi_page_t, n: u16) -> bool {
    if page.is_null() {
        return true;
    }
    let frac = (*page).reserved / n;
    (*page).reserved - (*page).used <= frac
}

#[inline]
pub unsafe fn mi_page_queue(theap: *const mi_theap_t, size: usize) -> *mut mi_page_queue_t {
    let pq = ptr::addr_of!(
        (*(theap as *mut mi_theap_t)).pages[crate::page::_mi_bin_typed(size).as_usize()]
    )
    .cast_mut();
    if size <= MI_LARGE_MAX_OBJ_SIZE {
        crate::mi_assert_internal!((*pq).block_size <= MI_LARGE_MAX_OBJ_SIZE);
    }
    pq
}

#[inline]
pub unsafe fn mi_page_set_has_interior_pointers(page: *mut mi_page_t, has_aligned: bool) {
    crate::page::mi_page_flags_set(page, has_aligned, MI_PAGE_HAS_INTERIOR_POINTERS);
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
pub unsafe fn mi_page_has_interior_pointers(page: *const mi_page_t) -> bool {
    (mi_page_flags(page) & MI_PAGE_HAS_INTERIOR_POINTERS) != 0
}

#[inline]
pub unsafe fn mi_page_is_abandoned_mapped(page: *const mi_page_t) -> bool {
    mi_page_thread_id(page) == MI_THREADID_ABANDONED_MAPPED
}

#[inline]
pub unsafe fn mi_page_set_abandoned_mapped(page: *mut mi_page_t) {
    crate::mi_assert_internal!(crate::page::mi_page_is_abandoned(page));
    crate::atomic::mi_atomic_or_relaxed(&mut (*page).xthread_id, MI_THREADID_ABANDONED_MAPPED);
}

#[inline]
pub unsafe fn mi_page_clear_abandoned_mapped(page: *mut mi_page_t) {
    crate::mi_assert_internal!(mi_page_is_abandoned_mapped(page));
    crate::atomic::mi_atomic_and_relaxed(&mut (*page).xthread_id, MI_PAGE_FLAG_MASK);
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
pub unsafe fn mi_page_has_any_available(page: *const mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null() && (*page).reserved > 0);
    (*page).used < (*page).reserved || !mi_page_thread_free(page).is_null()
}

#[inline]
pub unsafe fn mi_page_is_owned(page: *const mi_page_t) -> bool {
    mi_tf_is_owned(crate::atomic::mi_atomic_load_explicit(
        &(*(page as *mut mi_page_t)).xthread_free,
        crate::atomic::mi_memory_order_e::mi_memory_order_relaxed,
    ))
}

#[inline]
pub unsafe fn mi_page_claim_ownership(page: *mut mi_page_t) -> bool {
    let old = crate::atomic::mi_atomic_fetch_or_explicit(
        &mut (*page).xthread_free,
        1,
        crate::atomic::mi_memory_order_e::mi_memory_order_acq_rel,
    );
    (old & 1) == 0
}

#[inline]
pub unsafe fn mi_block_ptr_is_guarded(block: *const mi_block_t, p: *const c_void) -> bool {
    if MI_GUARDED == 0 {
        MI_UNUSED(&block);
        MI_UNUSED(&p);
        return false;
    }
    let offset = p.cast::<u8>().offset_from(block.cast::<u8>());
    offset >= size_of::<mi_block_t>() as isize && (*block.cast_mut()).next == MI_BLOCK_TAG_GUARDED
}

#[inline]
pub unsafe fn mi_theap_malloc_use_guarded(theap: *mut mi_theap_t, size: usize) -> bool {
    if MI_GUARDED == 0 {
        MI_UNUSED(&theap);
        MI_UNUSED(&size);
        return false;
    }
    if theap.is_null() {
        return false;
    }
    #[cfg(feature = "guarded")]
    {
        let count = (*theap).guarded_sample_count.wrapping_sub(1);
        if mi_likely(count != 0) {
            (*theap).guarded_sample_count = count;
            false
        } else if size >= (*theap).guarded_size_min && size <= (*theap).guarded_size_max {
            (*theap).guarded_sample_count = (*theap).guarded_sample_rate;
            (*theap).guarded_sample_rate != 0
        } else {
            if (*theap).guarded_sample_rate != 0 {
                (*theap).guarded_sample_count = 1;
            }
            false
        }
    }
    #[cfg(not(feature = "guarded"))]
    {
        false
    }
}

#[inline(always)]
pub unsafe fn mi_rep_movsb(dst: *mut c_void, src: *const c_void, n: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        core::arch::asm!(
            "rep movsb",
            inout("rdi") dst => _,
            inout("rcx") n => _,
            inout("rsi") src => _,
            options(nostack, preserves_flags),
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), n);
    }
}

#[inline]
pub unsafe fn mi_rep_stosb(dst: *mut c_void, val: u8, n: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        core::arch::asm!(
            "rep stosb",
            inout("rdi") dst => _,
            inout("rcx") n => _,
            in("al") val,
            options(nostack, preserves_flags),
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ptr::write_bytes(dst.cast::<u8>(), val, n);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn mi_rep_movsq(dst: *mut c_void, src: *const c_void, count: usize) {
    core::arch::asm!(
        "rep movsq",
        inout("rdi") dst => _,
        inout("rcx") count => _,
        inout("rsi") src => _,
        options(nostack, preserves_flags),
    );
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
    let mut raw: mi_memid_t = zeroed();
    raw.memkind = memkind;
    raw
}

#[inline]
pub unsafe fn _mi_memid_none() -> mi_memid_t {
    crate::core_types::MemId::<crate::core_types::NoneMem>::none().raw
}

#[inline]
pub unsafe fn _mi_memid_create_os(
    base: *mut c_void,
    size: usize,
    committed: bool,
    is_zero: bool,
    is_large: bool,
) -> mi_memid_t {
    crate::core_types::MemId::<crate::core_types::OsMem>::os(
        crate::core_types::OsMemPtr::new(base),
        crate::core_types::ReservedBytes(size),
        crate::core_types::InitiallyCommitted(committed),
        crate::core_types::InitiallyZero(is_zero),
        crate::core_types::Pinned(is_large),
    )
    .raw
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
pub unsafe fn mi_is_in_same_page(p: *const c_void, q: *const c_void) -> bool {
    let page = unsafe { _mi_ptr_page(p) };
    unsafe { mi_page_contains_address(page, q) }
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
pub unsafe fn mi_page_keys(page: *const mi_page_t) -> [usize; 2] {
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    {
        unsafe { (*page).keys }
    }

    #[cfg(not(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    )))]
    {
        let _ = page;
        [0, 0]
    }
}

#[inline]
pub unsafe fn mi_page_set_keys(page: *mut mi_page_t, key0: usize, key1: usize) {
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    unsafe {
        (*page).keys[0] = key0;
        (*page).keys[1] = key1;
    }

    #[cfg(not(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    )))]
    {
        let _ = (page, key0, key1);
    }
}

#[inline]
pub unsafe fn mi_block_nextx(
    null: *const c_void,
    block: *const mi_block_t,
    keys: &[usize; 2],
) -> *mut mi_block_t {
    unsafe { crate::track::mi_track_mem_defined(block.cast_mut().cast(), size_of::<mi_block_t>()) };
    let next = if MI_ENCODE_FREELIST != 0 {
        mi_ptr_decode(null, (*block).next, keys).cast::<mi_block_t>()
    } else {
        (*block).next as *mut mi_block_t
    };
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
    (*block).next = if MI_ENCODE_FREELIST != 0 {
        mi_ptr_encode(null, next.cast(), keys)
    } else {
        next as usize
    };
    unsafe { crate::track::mi_track_mem_noaccess(block.cast(), size_of::<mi_block_t>()) };
}

#[inline]
pub unsafe fn mi_block_next(page: *const mi_page_t, block: *const mi_block_t) -> *mut mi_block_t {
    let keys = unsafe { mi_page_keys(page) };
    if MI_ENCODE_FREELIST == 0 {
        return unsafe { mi_block_nextx(page.cast(), block, &keys) };
    }
    let mut next = unsafe { mi_block_nextx(page.cast(), block, &keys) };
    if !next.is_null() && !mi_is_in_same_page(block.cast(), next.cast()) {
        _mi_error_message_fmt(
            14,
            format_args!(
                "corrupted free list entry of size {}b at {block:p}: value 0x{:x}\n",
                mi_page_block_size(page),
                next as usize
            ),
        );
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
    let keys = unsafe { mi_page_keys(page) };
    unsafe { mi_block_set_nextx(page.cast(), block, next, &keys) };
}

#[inline]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn mi_cpu_movsb_max() -> usize {
    let max: usize;
    unsafe {
        core::arch::asm!(
            "mov {max}, qword ptr [rip + _mi_cpu_movsb_max]",
            max = lateout(reg) max,
            options(nostack, readonly, preserves_flags)
        );
    }
    max
}

#[inline]
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
unsafe fn mi_cpu_movsb_max() -> usize {
    crate::init::_mi_cpu_movsb_max
}

#[inline]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn mi_cpu_stosb_max() -> usize {
    let max: usize;
    unsafe {
        core::arch::asm!(
            "mov {max}, qword ptr [rip + _mi_cpu_stosb_max]",
            max = lateout(reg) max,
            options(nostack, readonly, preserves_flags)
        );
    }
    max
}

#[inline]
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
unsafe fn mi_cpu_stosb_max() -> usize {
    crate::init::_mi_cpu_stosb_max
}

#[inline]
pub unsafe fn _mi_memcpy(dst: *mut c_void, src: *const c_void, n: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        if n <= mi_cpu_movsb_max() {
            mi_rep_movsb(dst, src, n);
            return;
        }
    }
    ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), n);
}

#[inline]
pub unsafe fn _mi_memset(dst: *mut c_void, val: i32, n: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        if n <= mi_cpu_stosb_max() {
            mi_rep_stosb(dst, val as u8, n);
            return;
        }
    }
    ptr::write_bytes(dst, val as u8, n);
}

#[inline]
pub unsafe fn _mi_memcpy_aligned(dst: *mut c_void, src: *const c_void, n: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        let movsb_max = mi_cpu_movsb_max();
        if n > movsb_max && n.is_multiple_of(size_of::<usize>()) {
            mi_rep_movsq(dst, src, n / size_of::<usize>());
            return;
        }
        if n <= movsb_max {
            mi_rep_movsb(dst, src, n);
            return;
        }
    }
    ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), n);
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
