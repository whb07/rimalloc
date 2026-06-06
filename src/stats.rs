//! Statistics

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ptr;

pub const MI_STAT_VERSION: usize = 5;
pub const MI_BIN_HUGE: usize = 73;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_stat_count_t {
    pub total: i64,
    pub peak: i64,
    pub current: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_stat_counter_t {
    pub total: i64,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_chunkbin_e {
    MI_CBIN_SMALL = 0,
    MI_CBIN_OTHER = 1,
    MI_CBIN_MEDIUM = 2,
    MI_CBIN_LARGE = 3,
    MI_CBIN_HUGE = 4,
    MI_CBIN_NONE = 5,
    MI_CBIN_COUNT = 6,
}

pub type mi_chunkbin_t = mi_chunkbin_e;

const MI_CHUNKBIN_COUNT: usize = mi_chunkbin_e::MI_CBIN_COUNT as usize;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_stats_t {
    pub size: usize,
    pub version: usize,

    pub pages: mi_stat_count_t,
    pub reserved: mi_stat_count_t,
    pub committed: mi_stat_count_t,
    pub reset: mi_stat_counter_t,
    pub purged: mi_stat_counter_t,
    pub page_committed: mi_stat_count_t,
    pub pages_abandoned: mi_stat_count_t,
    pub threads: mi_stat_count_t,
    pub malloc_normal: mi_stat_count_t,
    pub malloc_huge: mi_stat_count_t,
    pub malloc_requested: mi_stat_count_t,

    pub mmap_calls: mi_stat_counter_t,
    pub commit_calls: mi_stat_counter_t,
    pub reset_calls: mi_stat_counter_t,
    pub purge_calls: mi_stat_counter_t,
    pub arena_count: mi_stat_counter_t,
    pub malloc_normal_count: mi_stat_counter_t,
    pub malloc_huge_count: mi_stat_counter_t,
    pub malloc_guarded_count: mi_stat_counter_t,

    pub arena_rollback_count: mi_stat_counter_t,
    pub arena_purges: mi_stat_counter_t,
    pub pages_extended: mi_stat_counter_t,
    pub pages_retire: mi_stat_counter_t,
    pub page_searches: mi_stat_counter_t,
    pub page_searches_count: mi_stat_counter_t,

    pub segments: mi_stat_count_t,
    pub segments_abandoned: mi_stat_count_t,
    pub segments_cache: mi_stat_count_t,
    pub _segments_reserved: mi_stat_count_t,

    pub heaps: mi_stat_count_t,
    pub theaps: mi_stat_count_t,
    pub pages_reclaim_on_alloc: mi_stat_counter_t,
    pub pages_reclaim_on_free: mi_stat_counter_t,
    pub pages_reabandon_full: mi_stat_counter_t,
    pub pages_unabandon_busy_wait: mi_stat_counter_t,
    pub heaps_delete_wait: mi_stat_counter_t,

    pub _stat_reserved: [mi_stat_count_t; 4],
    pub _stat_counter_reserved: [mi_stat_counter_t; 4],

    pub malloc_bins: [mi_stat_count_t; MI_BIN_HUGE + 1],
    pub page_bins: [mi_stat_count_t; MI_BIN_HUGE + 1],
    pub chunk_bins: [mi_stat_count_t; MI_CHUNKBIN_COUNT],
}

impl Default for mi_stats_t {
    fn default() -> Self {
        // SAFETY: `mi_stats_t` is plain old data; all-zero bytes are valid for every field.
        unsafe { core::mem::zeroed() }
    }
}

/// # Safety
/// `stats` must point to a valid writable `mi_stats_t`.
pub unsafe fn mi_stats_header_init(stats: *mut mi_stats_t) {
    // SAFETY: caller guarantees `stats` is a valid writable `mi_stats_t`.
    unsafe {
        (*stats).size = core::mem::size_of::<mi_stats_t>();
        (*stats).version = MI_STAT_VERSION;
    }
}

/// # Safety
/// `stats` must point to a valid writable `mi_stats_t`.
pub unsafe fn mi_stats_init(stats: *mut mi_stats_t) {
    // SAFETY: caller guarantees `stats` is a valid writable `mi_stats_t`.
    unsafe {
        ptr::write_bytes(stats, 0, 1);
        mi_stats_header_init(stats);
    }
}
