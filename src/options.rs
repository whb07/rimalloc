//! Options, stats, random, thread, init helpers

#![allow(non_camel_case_types, non_upper_case_globals)]

use crate::types::MI_MALLOC_VERSION;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_option_e {
    mi_option_show_errors = 0,
    mi_option_show_stats = 1,
    mi_option_verbose = 2,
    mi_option_deprecated_eager_commit = 3,
    mi_option_arena_eager_commit = 4,
    mi_option_purge_decommits = 5,
    mi_option_allow_large_os_pages = 6,
    mi_option_reserve_huge_os_pages = 7,
    mi_option_reserve_huge_os_pages_at = 8,
    mi_option_reserve_os_memory = 9,
    mi_option_deprecated_segment_cache = 10,
    mi_option_deprecated_page_reset = 11,
    mi_option_deprecated_abandoned_page_purge = 12,
    mi_option_deprecated_segment_reset = 13,
    mi_option_deprecated_eager_commit_delay = 14,
    mi_option_purge_delay = 15,
    mi_option_use_numa_nodes = 16,
    mi_option_disallow_os_alloc = 17,
    mi_option_os_tag = 18,
    mi_option_max_errors = 19,
    mi_option_max_warnings = 20,
    mi_option_deprecated_max_segment_reclaim = 21,
    mi_option_destroy_on_exit = 22,
    mi_option_arena_reserve = 23,
    mi_option_arena_purge_mult = 24,
    mi_option_deprecated_purge_extend_delay = 25,
    mi_option_disallow_arena_alloc = 26,
    mi_option_retry_on_oom = 27,
    mi_option_visit_abandoned = 28,
    mi_option_guarded_min = 29,
    mi_option_guarded_max = 30,
    mi_option_guarded_precise = 31,
    mi_option_guarded_sample_rate = 32,
    mi_option_guarded_sample_seed = 33,
    mi_option_generic_collect = 34,
    mi_option_page_reclaim_on_free = 35,
    mi_option_page_full_retain = 36,
    mi_option_page_max_candidates = 37,
    mi_option_max_vabits = 38,
    mi_option_pagemap_commit = 39,
    mi_option_page_commit_on_demand = 40,
    mi_option_page_max_reclaim = 41,
    mi_option_page_cross_thread_max_reclaim = 42,
    mi_option_allow_thp = 43,
    mi_option_minimal_purge_size = 44,
    mi_option_arena_max_object_size = 45,
    mi_option_arena_is_numa_local = 46,
    _mi_option_last = 47,
}

pub type mi_option_t = mi_option_e;

pub const mi_option_large_os_pages: mi_option_t = mi_option_e::mi_option_allow_large_os_pages;
pub const mi_option_eager_region_commit: mi_option_t = mi_option_e::mi_option_arena_eager_commit;
pub const mi_option_reset_decommits: mi_option_t = mi_option_e::mi_option_purge_decommits;
pub const mi_option_reset_delay: mi_option_t = mi_option_e::mi_option_purge_delay;
pub const mi_option_limit_os_alloc: mi_option_t = mi_option_e::mi_option_disallow_os_alloc;

pub extern "C" fn mi_version() -> i32 {
    MI_MALLOC_VERSION
}
