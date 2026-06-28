//! Mechanical translation of `include/mimalloc.h`.
//!
//! Public declarations in this module keep the C header pointer shape. Raw
//! pointer parameters and returns use `*mut` or `*const`, including nullable
//! cases inherited from mimalloc. Nullability follows the upstream C contract:
//! allocation failure returns null where C returns null, `mi_free` accepts
//! null, realloc-family functions accept null only where C treats that as
//! allocation, and metadata/out-pointer arguments must be valid and non-null
//! unless their specific C contract allows null. This module does not use
//! `NonNull` in raw C signatures.

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::{c_char, c_int, c_long, c_void};

#[cfg(windows)]
pub type c_wchar = u16;
#[cfg(not(windows))]
pub type c_wchar = i32;

pub use crate::mi_stats_t_decl;
pub use crate::options::{
    mi_option_abandoned_page_purge, mi_option_abandoned_reclaim_on_free,
    mi_option_decommit_extend_delay, mi_option_eager_commit, mi_option_eager_commit_delay,
    mi_option_eager_region_commit, mi_option_large_os_pages, mi_option_limit_os_alloc,
    mi_option_max_segment_reclaim, mi_option_purge_extend_delay, mi_option_reset_decommits,
    mi_option_reset_delay, mi_option_t,
};
pub use crate::stats::{
    MI_BIN_HUGE, MI_CBIN_COUNT, MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_NONE,
    MI_CBIN_OTHER, MI_CBIN_SMALL, MI_STAT_VERSION, mi_chunkbin_t, mi_stat_count_t,
    mi_stat_counter_t, mi_stats_header_init, mi_stats_init, mi_stats_t,
};
pub use crate::types::{
    MI_MALLOC_VERSION, MI_MAX_ALIGN_SIZE, MI_SMALL_SIZE_MAX, MI_SMALL_WSIZE_MAX, mi_arena_id_t,
    mi_block_visit_fun, mi_commit_fun_t, mi_deferred_free_fun, mi_error_fun, mi_heap_area_t,
    mi_heap_t, mi_heap_visit_fun, mi_output_fun, mi_subproc_id_t, mi_theap_t,
};

pub const _mi_option_last: mi_option_t = crate::options::mi_option_t::_mi_option_last;
pub const MIMALLOC_H: () = ();
pub const mi_attr_noexcept: () = ();
pub const mi_decl_nodiscard: () = ();
pub const mi_decl_export: () = ();
pub const mi_decl_restrict: () = ();
pub const mi_attr_malloc: () = ();
pub const mi_cdecl: () = ();
pub const mi_attr_alloc_size: () = ();
pub const mi_attr_alloc_size2: () = ();
pub const mi_attr_alloc_align: () = ();
pub const MI_HAS_HEAP_STL_ALLOCATOR: usize = 1;

// SAFETY[AUDIT:c-header-macro-reentry]: these are public C header compatibility
// macros. They intentionally expand through the public raw C-shaped facade, not
// crate-private internals, so downstream macro expansion observes header-like
// allocation semantics.
#[macro_export]
macro_rules! mi_malloc_tp {
    ($tp:ty) => {{ $crate::raw::mi_malloc(core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_zalloc_tp {
    ($tp:ty) => {{ $crate::raw::mi_zalloc(core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_calloc_tp {
    ($tp:ty, $count:expr) => {{ $crate::raw::mi_calloc($count, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_mallocn_tp {
    ($tp:ty, $count:expr) => {{ $crate::raw::mi_mallocn($count, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_reallocn_tp {
    ($tp:ty, $p:expr, $count:expr) => {{ $crate::raw::mi_reallocn($p as *mut _, $count, core::mem::size_of::<$tp>()) as *mut $tp }};
    ($p:expr, $tp:ty, $count:expr) => {{ $crate::raw::mi_reallocn($p as *mut _, $count, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_heap_malloc_tp {
    ($tp:ty, $heap:expr) => {{ $crate::raw::mi_heap_malloc($heap, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_heap_zalloc_tp {
    ($tp:ty, $heap:expr) => {{ $crate::raw::mi_heap_zalloc($heap, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_heap_calloc_tp {
    ($tp:ty, $heap:expr, $count:expr) => {{ $crate::raw::mi_heap_calloc($heap, $count, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_heap_mallocn_tp {
    ($tp:ty, $heap:expr, $count:expr) => {{ $crate::raw::mi_heap_mallocn($heap, $count, core::mem::size_of::<$tp>()) as *mut $tp }};
}

#[macro_export]
macro_rules! mi_heap_reallocn_tp {
    ($tp:ty, $heap:expr, $p:expr, $count:expr) => {{
        $crate::raw::mi_heap_reallocn($heap, $p as *mut _, $count, core::mem::size_of::<$tp>())
            as *mut $tp
    }};
}

#[macro_export]
macro_rules! mi_heap_recalloc_tp {
    ($tp:ty, $heap:expr, $p:expr, $count:expr) => {{
        $crate::raw::mi_heap_recalloc($heap, $p as *mut _, $count, core::mem::size_of::<$tp>())
            as *mut $tp
    }};
}

unsafe extern "C" {
    pub fn mi_malloc(size: usize) -> *mut c_void;
    pub fn mi_calloc(count: usize, size: usize) -> *mut c_void;
    pub fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;
    pub fn mi_expand(p: *mut c_void, newsize: usize) -> *mut c_void;

    pub fn mi_free(p: *mut c_void);
    pub fn mi_strdup(s: *const c_char) -> *mut c_char;
    pub fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char;
    pub fn mi_realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;

    pub fn mi_malloc_small(size: usize) -> *mut c_void;
    pub fn mi_zalloc_small(size: usize) -> *mut c_void;
    pub fn mi_zalloc(size: usize) -> *mut c_void;

    pub fn mi_mallocn(count: usize, size: usize) -> *mut c_void;
    pub fn mi_reallocn(p: *mut c_void, count: usize, size: usize) -> *mut c_void;
    pub fn mi_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void;

    pub fn mi_usable_size(p: *const c_void) -> usize;
    pub fn mi_good_size(size: usize) -> usize;
    pub fn mi_free_small(p: *mut c_void);

    pub fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;
    pub fn mi_malloc_aligned_at(size: usize, alignment: usize, offset: usize) -> *mut c_void;
    pub fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;
    pub fn mi_zalloc_aligned_at(size: usize, alignment: usize, offset: usize) -> *mut c_void;
    pub fn mi_calloc_aligned(count: usize, size: usize, alignment: usize) -> *mut c_void;
    pub fn mi_calloc_aligned_at(
        count: usize,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;
    pub fn mi_realloc_aligned(p: *mut c_void, newsize: usize, alignment: usize) -> *mut c_void;
    pub fn mi_realloc_aligned_at(
        p: *mut c_void,
        newsize: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;

    pub fn mi_register_deferred_free(deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void);
    pub fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void);
    pub fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void);

    pub fn mi_collect(force: bool);
    pub fn mi_version() -> c_int;
    pub fn mi_options_print();
    pub fn mi_process_info_print();
    pub fn mi_options_print_out(out: Option<mi_output_fun>, arg: *mut c_void);
    pub fn mi_process_info_print_out(out: Option<mi_output_fun>, arg: *mut c_void);
    pub fn mi_process_info(
        elapsed_msecs: *mut usize,
        user_msecs: *mut usize,
        system_msecs: *mut usize,
        current_rss: *mut usize,
        peak_rss: *mut usize,
        current_commit: *mut usize,
        peak_commit: *mut usize,
        page_faults: *mut usize,
    );

    pub fn mi_process_init();
    pub fn mi_process_done();
    pub fn mi_thread_init();
    pub fn mi_thread_done();
    pub fn mi_thread_set_in_threadpool();

    pub fn mi_umalloc(size: usize, block_size: *mut usize) -> *mut c_void;
    pub fn mi_ucalloc(count: usize, size: usize, block_size: *mut usize) -> *mut c_void;
    pub fn mi_urealloc(
        p: *mut c_void,
        newsize: usize,
        block_size_pre: *mut usize,
        block_size_post: *mut usize,
    ) -> *mut c_void;
    pub fn mi_ufree(p: *mut c_void, block_size: *mut usize);

    pub fn mi_umalloc_aligned(size: usize, alignment: usize, block_size: *mut usize)
    -> *mut c_void;
    pub fn mi_uzalloc_aligned(size: usize, alignment: usize, block_size: *mut usize)
    -> *mut c_void;

    pub fn mi_umalloc_small(size: usize, block_size: *mut usize) -> *mut c_void;
    pub fn mi_uzalloc_small(size: usize, block_size: *mut usize) -> *mut c_void;

    pub fn mi_heap_new() -> *mut mi_heap_t;
    pub fn mi_heap_delete(heap: *mut mi_heap_t);
    pub fn mi_heap_destroy(heap: *mut mi_heap_t);
    pub fn mi_heap_set_numa_affinity(heap: *mut mi_heap_t, numa_node: c_int);
    pub fn mi_heap_collect(heap: *mut mi_heap_t, force: bool);

    pub fn mi_heap_main() -> *mut mi_heap_t;
    pub fn mi_heap_of(p: *const c_void) -> *mut mi_heap_t;
    pub fn mi_heap_contains(heap: *const mi_heap_t, p: *const c_void) -> bool;
    pub fn mi_any_heap_contains(p: *const c_void) -> bool;

    pub fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void;
    pub fn mi_heap_zalloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void;
    pub fn mi_heap_calloc(heap: *mut mi_heap_t, count: usize, size: usize) -> *mut c_void;
    pub fn mi_heap_mallocn(heap: *mut mi_heap_t, count: usize, size: usize) -> *mut c_void;
    pub fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void;
    pub fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void;

    pub fn mi_heap_realloc(heap: *mut mi_heap_t, p: *mut c_void, newsize: usize) -> *mut c_void;
    pub fn mi_heap_reallocn(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        count: usize,
        size: usize,
    ) -> *mut c_void;
    pub fn mi_heap_reallocf(heap: *mut mi_heap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

    pub fn mi_heap_strdup(heap: *mut mi_heap_t, s: *const c_char) -> *mut c_char;
    pub fn mi_heap_strndup(heap: *mut mi_heap_t, s: *const c_char, n: usize) -> *mut c_char;
    pub fn mi_heap_realpath(
        heap: *mut mi_heap_t,
        fname: *const c_char,
        resolved_name: *mut c_char,
    ) -> *mut c_char;

    pub fn mi_heap_malloc_aligned(
        heap: *mut mi_heap_t,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_heap_malloc_aligned_at(
        heap: *mut mi_heap_t,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;
    pub fn mi_heap_zalloc_aligned(
        heap: *mut mi_heap_t,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_heap_zalloc_aligned_at(
        heap: *mut mi_heap_t,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;
    pub fn mi_heap_calloc_aligned(
        heap: *mut mi_heap_t,
        count: usize,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_heap_calloc_aligned_at(
        heap: *mut mi_heap_t,
        count: usize,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;
    pub fn mi_heap_realloc_aligned(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newsize: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_heap_realloc_aligned_at(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newsize: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;

    pub fn mi_rezalloc(p: *mut c_void, newsize: usize) -> *mut c_void;
    pub fn mi_recalloc(p: *mut c_void, newcount: usize, size: usize) -> *mut c_void;
    pub fn mi_rezalloc_aligned(p: *mut c_void, newsize: usize, alignment: usize) -> *mut c_void;
    pub fn mi_rezalloc_aligned_at(
        p: *mut c_void,
        newsize: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;
    pub fn mi_recalloc_aligned(
        p: *mut c_void,
        newcount: usize,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_recalloc_aligned_at(
        p: *mut c_void,
        newcount: usize,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;

    pub fn mi_heap_rezalloc(heap: *mut mi_heap_t, p: *mut c_void, newsize: usize) -> *mut c_void;
    pub fn mi_heap_recalloc(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newcount: usize,
        size: usize,
    ) -> *mut c_void;
    pub fn mi_heap_rezalloc_aligned(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newsize: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_heap_rezalloc_aligned_at(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newsize: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;
    pub fn mi_heap_recalloc_aligned(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newcount: usize,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_heap_recalloc_aligned_at(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        newcount: usize,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;

    pub fn mi_heap_visit_blocks(
        heap: *mut mi_heap_t,
        visit_blocks: bool,
        visitor: Option<mi_block_visit_fun>,
        arg: *mut c_void,
    ) -> bool;
    pub fn mi_heap_visit_abandoned_blocks(
        heap: *mut mi_heap_t,
        visit_blocks: bool,
        visitor: Option<mi_block_visit_fun>,
        arg: *mut c_void,
    ) -> bool;

    pub fn mi_is_redirected() -> bool;
    pub fn mi_reserve_huge_os_pages_interleave(
        pages: usize,
        numa_nodes: usize,
        timeout_msecs: usize,
    ) -> c_int;
    pub fn mi_reserve_huge_os_pages_at(
        pages: usize,
        numa_node: c_int,
        timeout_msecs: usize,
    ) -> c_int;

    pub fn mi_reserve_os_memory(size: usize, commit: bool, allow_large: bool) -> c_int;
    pub fn mi_manage_os_memory(
        start: *mut c_void,
        size: usize,
        is_committed: bool,
        is_pinned: bool,
        is_zero: bool,
        numa_node: c_int,
    ) -> bool;

    pub fn mi_debug_show_arenas();
    pub fn mi_arenas_print();
    pub fn mi_arena_min_alignment() -> usize;
    pub fn mi_arena_min_size() -> usize;

    pub fn mi_arena_area(arena_id: mi_arena_id_t, size: *mut usize) -> *mut c_void;
    pub fn mi_reserve_huge_os_pages_at_ex(
        pages: usize,
        numa_node: c_int,
        timeout_msecs: usize,
        exclusive: bool,
        arena_id: *mut mi_arena_id_t,
    ) -> c_int;
    pub fn mi_reserve_os_memory_ex(
        size: usize,
        commit: bool,
        allow_large: bool,
        exclusive: bool,
        arena_id: *mut mi_arena_id_t,
    ) -> c_int;
    pub fn mi_manage_os_memory_ex(
        start: *mut c_void,
        size: usize,
        is_committed: bool,
        is_pinned: bool,
        is_zero: bool,
        numa_node: c_int,
        exclusive: bool,
        arena_id: *mut mi_arena_id_t,
    ) -> bool;
    pub fn mi_arena_contains(arena_id: mi_arena_id_t, p: *const c_void) -> bool;
    pub fn mi_heap_new_in_arena(arena_id: mi_arena_id_t) -> *mut mi_heap_t;

    pub fn mi_subproc_main() -> mi_subproc_id_t;
    pub fn mi_subproc_current() -> mi_subproc_id_t;
    pub fn mi_subproc_new() -> mi_subproc_id_t;
    pub fn mi_subproc_destroy(subproc: mi_subproc_id_t);
    pub fn mi_subproc_add_current_thread(subproc: mi_subproc_id_t);
    pub fn mi_subproc_visit_heaps(
        subproc: mi_subproc_id_t,
        visitor: Option<mi_heap_visit_fun>,
        arg: *mut c_void,
    ) -> bool;

    pub fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t;
    pub fn mi_theap_set_default(theap: *mut mi_theap_t) -> *mut mi_theap_t;
    pub fn mi_theap_get_default() -> *mut mi_theap_t;
    pub fn mi_theap_collect(theap: *mut mi_theap_t, force: bool);

    pub fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void;
    pub fn mi_theap_zalloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void;
    pub fn mi_theap_calloc(theap: *mut mi_theap_t, count: usize, size: usize) -> *mut c_void;
    pub fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void;
    pub fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void;
    pub fn mi_theap_malloc_aligned(
        theap: *mut mi_theap_t,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_theap_realloc(theap: *mut mi_theap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

    pub fn mi_theap_guarded_set_sample_rate(
        theap: *mut mi_theap_t,
        sample_rate: usize,
        seed: usize,
    );
    pub fn mi_theap_guarded_set_size_bound(theap: *mut mi_theap_t, min: usize, max: usize);

    pub fn mi_manage_memory(
        start: *mut c_void,
        size: usize,
        is_committed: bool,
        is_pinned: bool,
        is_zero: bool,
        numa_node: c_int,
        exclusive: bool,
        commit_fun: Option<mi_commit_fun_t>,
        commit_fun_arg: *mut c_void,
        arena_id: *mut mi_arena_id_t,
    ) -> bool;

    pub fn mi_unsafe_heap_page_is_under_utilized(
        heap: *mut mi_heap_t,
        p: *mut c_void,
        perc_threshold: usize,
    ) -> bool;

    pub fn mi_check_owned(p: *const c_void) -> bool;
    pub fn mi_thread_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void);
    pub fn mi_is_in_heap_region(p: *const c_void) -> bool;
    pub fn mi_theap_visit_blocks(
        theap: *const mi_theap_t,
        visit_blocks: bool,
        visitor: Option<mi_block_visit_fun>,
        arg: *mut c_void,
    ) -> bool;

    pub fn mi_reserve_huge_os_pages(
        pages: usize,
        max_secs: f64,
        pages_reserved: *mut usize,
    ) -> c_int;
    pub fn mi_collect_reduce(target_thread_owned: usize);

    pub fn mi_stats_reset();
    pub fn mi_stats_merge();
    pub fn mi_stats_get_bin_size(bin: usize) -> usize;
    pub fn mi_stats_get(stats: *mut mi_stats_t) -> bool;
    pub fn mi_stats_print(out: *mut c_void);
    pub fn mi_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void);
    pub fn mi_heap_stats_get(heap: *mut mi_heap_t, stats: *mut mi_stats_t) -> bool;
    pub fn mi_heap_stats_print_out(
        heap: *mut mi_heap_t,
        out: Option<mi_output_fun>,
        arg: *mut c_void,
    );
    pub fn mi_heap_stats_merge_to_subproc(heap: *mut mi_heap_t);
    pub fn mi_subproc_stats_get(subproc_id: mi_subproc_id_t, stats: *mut mi_stats_t) -> bool;
    pub fn mi_subproc_stats_get_json(
        subproc_id: mi_subproc_id_t,
        buf_size: usize,
        buf: *mut c_char,
    ) -> *mut c_char;
    pub fn mi_subproc_stats_print_out(
        subproc_id: mi_subproc_id_t,
        out: Option<mi_output_fun>,
        arg: *mut c_void,
    );
    pub fn mi_subproc_heap_stats_print_out(
        subproc_id: mi_subproc_id_t,
        out: Option<mi_output_fun>,
        arg: *mut c_void,
    );
    pub fn mi_stats_get_json(buf_size: usize, buf: *mut c_char) -> *mut c_char;
    pub fn mi_heap_stats_get_json(
        heap: *mut mi_heap_t,
        buf_size: usize,
        buf: *mut c_char,
    ) -> *mut c_char;
    pub fn mi_stats_as_json(
        stats: *mut mi_stats_t,
        buf_size: usize,
        buf: *mut c_char,
    ) -> *mut c_char;
    pub fn mi_subproc_stats_get_exclusive(
        subproc_id: mi_subproc_id_t,
        stats: *mut mi_stats_t,
    ) -> bool;

    pub fn mi_option_is_enabled(option: mi_option_t) -> bool;
    pub fn mi_option_enable(option: mi_option_t);
    pub fn mi_option_disable(option: mi_option_t);
    pub fn mi_option_set_enabled(option: mi_option_t, enable: bool);
    pub fn mi_option_set_enabled_default(option: mi_option_t, enable: bool);

    pub fn mi_option_get(option: mi_option_t) -> c_long;
    pub fn mi_option_get_clamp(option: mi_option_t, min: c_long, max: c_long) -> c_long;
    pub fn mi_option_get_size(option: mi_option_t) -> usize;
    pub fn mi_option_set(option: mi_option_t, value: c_long);
    pub fn mi_option_set_default(option: mi_option_t, value: c_long);

    pub fn mi_cfree(p: *mut c_void);
    pub fn mi__expand(p: *mut c_void, newsize: usize) -> *mut c_void;
    pub fn mi_malloc_size(p: *const c_void) -> usize;
    pub fn mi_malloc_good_size(size: usize) -> usize;
    pub fn mi_malloc_usable_size(p: *const c_void) -> usize;

    pub fn mi_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int;
    pub fn mi_memalign(alignment: usize, size: usize) -> *mut c_void;
    pub fn mi_valloc(size: usize) -> *mut c_void;
    pub fn mi_pvalloc(size: usize) -> *mut c_void;
    pub fn mi_aligned_alloc(alignment: usize, size: usize) -> *mut c_void;

    pub fn mi_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void;
    pub fn mi_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int;
    pub fn mi_aligned_recalloc(
        p: *mut c_void,
        newcount: usize,
        size: usize,
        alignment: usize,
    ) -> *mut c_void;
    pub fn mi_aligned_offset_recalloc(
        p: *mut c_void,
        newcount: usize,
        size: usize,
        alignment: usize,
        offset: usize,
    ) -> *mut c_void;

    pub fn mi_free_size(p: *mut c_void, size: usize);
    pub fn mi_free_size_aligned(p: *mut c_void, size: usize, alignment: usize);
    pub fn mi_free_aligned(p: *mut c_void, alignment: usize);
    pub fn mi_dupenv_s(buf: *mut *mut c_char, size: *mut usize, name: *const c_char) -> c_int;

    pub fn mi_wdupenv_s(buf: *mut *mut c_wchar, size: *mut usize, name: *const c_wchar) -> c_int;
    pub fn mi_wcsdup(s: *const c_wchar) -> *mut c_wchar;
    pub fn mi_mbsdup(s: *const u8) -> *mut u8;

    pub fn mi_new(size: usize) -> *mut c_void;
    pub fn mi_new_aligned(size: usize, alignment: usize) -> *mut c_void;
    pub fn mi_new_nothrow(size: usize) -> *mut c_void;
    pub fn mi_new_aligned_nothrow(size: usize, alignment: usize) -> *mut c_void;
    pub fn mi_new_n(count: usize, size: usize) -> *mut c_void;
    pub fn mi_new_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;
    pub fn mi_new_reallocn(p: *mut c_void, newcount: usize, size: usize) -> *mut c_void;

    pub fn mi_heap_alloc_new(heap: *mut mi_heap_t, size: usize) -> *mut c_void;
    pub fn mi_heap_alloc_new_n(heap: *mut mi_heap_t, count: usize, size: usize) -> *mut c_void;
}
