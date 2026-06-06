//! Statistics

#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::{mem::size_of, ptr};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

use crate::atomic::{
    mi_atomic_addi64_relaxed, mi_atomic_maxi64_relaxed, mi_atomic_void_addi64_relaxed,
};

pub const MI_STAT_VERSION: usize = 5;
pub const MI_BIN_HUGE: usize = 73;
pub const MI_LARGE_MAX_OBJ_WSIZE: usize = 8192 * size_of::<usize>();

const MI_BIN_WORD_SIZES: [usize; MI_BIN_HUGE + 1] = [
    1,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    10,
    12,
    14,
    16,
    20,
    24,
    28,
    32,
    40,
    48,
    56,
    64,
    80,
    96,
    112,
    128,
    160,
    192,
    224,
    256,
    320,
    384,
    448,
    512,
    640,
    768,
    896,
    1024,
    1280,
    1536,
    1792,
    2048,
    2560,
    3072,
    3584,
    4096,
    5120,
    6144,
    7168,
    8192,
    10240,
    12288,
    14336,
    16384,
    20480,
    24576,
    28672,
    32768,
    40960,
    49152,
    57344,
    65536,
    81920,
    98304,
    114688,
    131072,
    163840,
    196608,
    229376,
    262144,
    327680,
    393216,
    458752,
    524288,
    MI_LARGE_MAX_OBJ_WSIZE + 1,
];

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

pub const MI_CBIN_SMALL: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_SMALL;
pub const MI_CBIN_OTHER: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_OTHER;
pub const MI_CBIN_MEDIUM: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_MEDIUM;
pub const MI_CBIN_LARGE: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_LARGE;
pub const MI_CBIN_HUGE: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_HUGE;
pub const MI_CBIN_NONE: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_NONE;
pub const MI_CBIN_COUNT: mi_chunkbin_t = mi_chunkbin_e::MI_CBIN_COUNT;

const MI_CHUNKBIN_COUNT: usize = mi_chunkbin_e::MI_CBIN_COUNT as usize;
static CLOCK_ORIGIN: OnceLock<Instant> = OnceLock::new();
static CLOCK_DIFF: AtomicI64 = AtomicI64::new(0);
static PROCESS_START: AtomicI64 = AtomicI64::new(0);

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

pub(crate) fn bin_block_size(bin: usize) -> usize {
    MI_BIN_WORD_SIZES[bin] * size_of::<usize>()
}

pub extern "C" fn mi_stats_get_bin_size(bin: usize) -> usize {
    if bin > MI_BIN_HUGE {
        return 0;
    }
    bin_block_size(bin)
}

fn mi_stat_update_mt(stat: &mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    // SAFETY: `stat` points to valid `i64` fields and these helpers operate atomically on them.
    unsafe {
        let current = mi_atomic_addi64_relaxed(&mut stat.current, amount);
        mi_atomic_maxi64_relaxed(&mut stat.peak, current + amount);
        if amount > 0 {
            mi_atomic_addi64_relaxed(&mut stat.total, amount);
        }
    }
}

fn mi_stat_update(stat: &mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    stat.current += amount;
    if stat.current > stat.peak {
        stat.peak = stat.current;
    }
    if amount > 0 {
        stat.total += amount;
    }
}

fn __mi_stat_counter_increase_mt(stat: &mut mi_stat_counter_t, amount: usize) {
    // SAFETY: `stat.total` is a valid atomic-compatible `i64` storage location.
    unsafe {
        mi_atomic_addi64_relaxed(&mut stat.total, amount as i64);
    }
}

fn __mi_stat_counter_increase(stat: &mut mi_stat_counter_t, amount: usize) {
    stat.total += amount as i64;
}

fn __mi_stat_increase_mt(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_update_mt(stat, amount as i64);
}

fn __mi_stat_increase(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_update(stat, amount as i64);
}

fn __mi_stat_decrease_mt(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_update_mt(stat, -(amount as i64));
}

fn __mi_stat_decrease(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_update(stat, -(amount as i64));
}

fn mi_stat_adjust_mt(stat: &mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    // SAFETY: `stat` points to valid `i64` fields and these helpers operate atomically on them.
    unsafe {
        mi_atomic_addi64_relaxed(&mut stat.current, amount);
        mi_atomic_addi64_relaxed(&mut stat.total, amount);
    }
}

fn mi_stat_adjust(stat: &mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    stat.current += amount;
    stat.total += amount;
}

fn __mi_stat_adjust_increase_mt(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_adjust_mt(stat, amount as i64);
}

fn __mi_stat_adjust_increase(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_adjust(stat, amount as i64);
}

fn __mi_stat_adjust_decrease_mt(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_adjust_mt(stat, -(amount as i64));
}

fn __mi_stat_adjust_decrease(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_adjust(stat, -(amount as i64));
}

fn mi_stat_count_add_mt(stat: &mut mi_stat_count_t, src: &mi_stat_count_t) {
    if ptr::eq(stat, src) {
        return;
    }
    // SAFETY: both pointers are valid for atomic-compatible `i64` field access.
    unsafe {
        mi_atomic_void_addi64_relaxed(&mut stat.total, &src.total);
        let prev_current = mi_atomic_addi64_relaxed(&mut stat.current, src.current);
        mi_atomic_maxi64_relaxed(&mut stat.peak, prev_current + src.peak);
    }
}

fn mi_stat_counter_add_mt(stat: &mut mi_stat_counter_t, src: &mi_stat_counter_t) {
    if ptr::eq(stat, src) {
        return;
    }
    // SAFETY: both pointers are valid for atomic-compatible `i64` field access.
    unsafe {
        mi_atomic_void_addi64_relaxed(&mut stat.total, &src.total);
    }
}

fn mi_stats_add(stats: &mut mi_stats_t, src: &mi_stats_t) {
    if ptr::eq(stats, src) {
        return;
    }

    mi_stat_count_add_mt(&mut stats.pages, &src.pages);
    mi_stat_count_add_mt(&mut stats.reserved, &src.reserved);
    mi_stat_count_add_mt(&mut stats.committed, &src.committed);
    mi_stat_counter_add_mt(&mut stats.reset, &src.reset);
    mi_stat_counter_add_mt(&mut stats.purged, &src.purged);
    mi_stat_count_add_mt(&mut stats.page_committed, &src.page_committed);
    mi_stat_count_add_mt(&mut stats.pages_abandoned, &src.pages_abandoned);
    mi_stat_count_add_mt(&mut stats.threads, &src.threads);
    mi_stat_count_add_mt(&mut stats.malloc_normal, &src.malloc_normal);
    mi_stat_count_add_mt(&mut stats.malloc_huge, &src.malloc_huge);
    mi_stat_count_add_mt(&mut stats.malloc_requested, &src.malloc_requested);
    mi_stat_counter_add_mt(&mut stats.mmap_calls, &src.mmap_calls);
    mi_stat_counter_add_mt(&mut stats.commit_calls, &src.commit_calls);
    mi_stat_counter_add_mt(&mut stats.reset_calls, &src.reset_calls);
    mi_stat_counter_add_mt(&mut stats.purge_calls, &src.purge_calls);
    mi_stat_counter_add_mt(&mut stats.arena_count, &src.arena_count);
    mi_stat_counter_add_mt(&mut stats.malloc_normal_count, &src.malloc_normal_count);
    mi_stat_counter_add_mt(&mut stats.malloc_huge_count, &src.malloc_huge_count);
    mi_stat_counter_add_mt(&mut stats.malloc_guarded_count, &src.malloc_guarded_count);
    mi_stat_counter_add_mt(&mut stats.arena_rollback_count, &src.arena_rollback_count);
    mi_stat_counter_add_mt(&mut stats.arena_purges, &src.arena_purges);
    mi_stat_counter_add_mt(&mut stats.pages_extended, &src.pages_extended);
    mi_stat_counter_add_mt(&mut stats.pages_retire, &src.pages_retire);
    mi_stat_counter_add_mt(&mut stats.page_searches, &src.page_searches);
    mi_stat_counter_add_mt(&mut stats.page_searches_count, &src.page_searches_count);
    mi_stat_count_add_mt(&mut stats.segments, &src.segments);
    mi_stat_count_add_mt(&mut stats.segments_abandoned, &src.segments_abandoned);
    mi_stat_count_add_mt(&mut stats.segments_cache, &src.segments_cache);
    mi_stat_count_add_mt(&mut stats._segments_reserved, &src._segments_reserved);
    mi_stat_count_add_mt(&mut stats.heaps, &src.heaps);
    mi_stat_count_add_mt(&mut stats.theaps, &src.theaps);
    mi_stat_counter_add_mt(
        &mut stats.pages_reclaim_on_alloc,
        &src.pages_reclaim_on_alloc,
    );
    mi_stat_counter_add_mt(&mut stats.pages_reclaim_on_free, &src.pages_reclaim_on_free);
    mi_stat_counter_add_mt(&mut stats.pages_reabandon_full, &src.pages_reabandon_full);
    mi_stat_counter_add_mt(
        &mut stats.pages_unabandon_busy_wait,
        &src.pages_unabandon_busy_wait,
    );
    mi_stat_counter_add_mt(&mut stats.heaps_delete_wait, &src.heaps_delete_wait);

    for (dst, add) in stats
        ._stat_reserved
        .iter_mut()
        .zip(src._stat_reserved.iter())
    {
        mi_stat_count_add_mt(dst, add);
    }
    for (dst, add) in stats
        ._stat_counter_reserved
        .iter_mut()
        .zip(src._stat_counter_reserved.iter())
    {
        mi_stat_counter_add_mt(dst, add);
    }
    for (dst, add) in stats.malloc_bins.iter_mut().zip(src.malloc_bins.iter()) {
        mi_stat_count_add_mt(dst, add);
    }
    for (dst, add) in stats.page_bins.iter_mut().zip(src.page_bins.iter()) {
        mi_stat_count_add_mt(dst, add);
    }
    for (dst, add) in stats.chunk_bins.iter_mut().zip(src.chunk_bins.iter()) {
        mi_stat_count_add_mt(dst, add);
    }
}

fn mi_stats_add_into(to: &mut mi_stats_t, from: &mi_stats_t) {
    if ptr::eq(to, from) {
        return;
    }
    mi_stats_add(to, from);
}

fn mi_stats_copy(stats_to: *mut mi_stats_t, stats_from: *const mi_stats_t) -> bool {
    // SAFETY: callers must pass valid pointers when non-null; header checks gate the copy.
    unsafe {
        if stats_to.is_null()
            || (*stats_to).size != size_of::<mi_stats_t>()
            || (*stats_to).version != MI_STAT_VERSION
        {
            return false;
        }
        if stats_from.is_null() || (*stats_from).size != (*stats_to).size {
            return false;
        }
        *stats_to = *stats_from;
        true
    }
}

fn clock_now_msecs() -> i64 {
    let start = CLOCK_ORIGIN.get_or_init(Instant::now);
    start.elapsed().as_millis() as i64
}

pub extern "C" fn _mi_clock_now() -> i64 {
    clock_now_msecs()
}

pub extern "C" fn _mi_clock_start() -> i64 {
    let current = CLOCK_DIFF.load(Ordering::Relaxed);
    if current == 0 {
        let t0 = _mi_clock_now();
        let diff = _mi_clock_now() - t0;
        let _ = CLOCK_DIFF.compare_exchange(0, diff, Ordering::Relaxed, Ordering::Relaxed);
    }
    _mi_clock_now()
}

pub extern "C" fn _mi_clock_end(start: i64) -> i64 {
    _mi_clock_now() - start - CLOCK_DIFF.load(Ordering::Relaxed)
}

pub extern "C" fn _mi_stats_init() {
    if PROCESS_START.load(Ordering::Relaxed) == 0 {
        let start = _mi_clock_start();
        let _ = PROCESS_START.compare_exchange(0, start, Ordering::Relaxed, Ordering::Relaxed);
    }
}

/// # Safety
/// `to` and `from` must point to valid writable `mi_stats_t` values.
pub unsafe extern "C" fn _mi_stats_merge_into(to: *mut mi_stats_t, from: *mut mi_stats_t) {
    if to.is_null() || from.is_null() || core::ptr::eq(to, from) {
        return;
    }
    // SAFETY: caller guarantees valid pointers to `mi_stats_t` values.
    unsafe {
        mi_stats_add_into(&mut *to, &*from);
        ptr::write_bytes(from, 0, 1);
    }
}
