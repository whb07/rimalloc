//! Statistics

#![allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc
)]

use core::ffi::{CStr, c_char, c_void};
use core::str;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};
use core::{mem::size_of, ptr};

use crate::atomic::{
    mi_atomic_addi64_relaxed, mi_atomic_maxi64_relaxed, mi_atomic_void_addi64_relaxed, mi_lock_t,
};
use crate::init::_mi_subproc_main;
use crate::internal::{_mi_memzero, _mi_theap_heap, mi_memid_t, mi_theap_is_initialized};
use crate::mimalloc::{mi_good_size, mi_rezalloc};
use crate::mimalloc::{mi_heap_main, mi_subproc_current, mi_subproc_visit_heaps};
use crate::options::{_mi_fprintf, _mi_fputs};
use crate::os::_mi_os_numa_node_count;
use crate::page::{
    _mi_bin_size, MI_LARGE_MAX_OBJ_SIZE, MI_LARGE_PAGE_SIZE, MI_MEDIUM_MAX_OBJ_SIZE,
    MI_MEDIUM_PAGE_SIZE, MI_SMALL_MAX_OBJ_SIZE, MI_SMALL_PAGE_SIZE,
};
use crate::prim::{
    _mi_heap_theap_peek, _mi_prim_clock_now, _mi_prim_process_info, _mi_theap_default,
};
use crate::types::{
    MI_BIN_COUNT, MI_MALLOC_VERSION, MI_STAT, mi_heap_t, mi_heap_visit_fun, mi_output_fun,
    mi_page_t, mi_process_info_t, mi_subproc_id_t, mi_subproc_t, mi_theap_t,
};

pub const MI_STAT_VERSION: usize = 5;
pub const MI_BIN_HUGE: usize = 73;
pub const MI_LARGE_MAX_OBJ_WSIZE: usize = 8192 * size_of::<usize>();
pub const MIMALLOC_STATS_H: () = ();

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
pub type mi_stat_count_s = mi_stat_count_t;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_stat_counter_t {
    pub total: i64,
}
pub type mi_stat_counter_s = mi_stat_counter_t;

#[macro_export]
macro_rules! MI_STAT_COUNT {
    ($name:ident) => {
        pub $name: $crate::stats::mi_stat_count_t,
    };
}

#[macro_export]
macro_rules! MI_STAT_FIELDS {
    () => {
        $crate::MI_STAT_COUNT!(pages)
        $crate::MI_STAT_COUNT!(reserved)
        $crate::MI_STAT_COUNT!(committed)
        $crate::MI_STAT_COUNTER!(reset)
        $crate::MI_STAT_COUNTER!(purged)
        $crate::MI_STAT_COUNT!(page_committed)
        $crate::MI_STAT_COUNT!(pages_abandoned)
        $crate::MI_STAT_COUNT!(threads)
        $crate::MI_STAT_COUNT!(malloc_normal)
        $crate::MI_STAT_COUNT!(malloc_huge)
        $crate::MI_STAT_COUNT!(malloc_requested)
        $crate::MI_STAT_COUNTER!(mmap_calls)
        $crate::MI_STAT_COUNTER!(commit_calls)
        $crate::MI_STAT_COUNTER!(reset_calls)
        $crate::MI_STAT_COUNTER!(purge_calls)
        $crate::MI_STAT_COUNTER!(arena_count)
        $crate::MI_STAT_COUNTER!(malloc_normal_count)
        $crate::MI_STAT_COUNTER!(malloc_huge_count)
        $crate::MI_STAT_COUNTER!(malloc_guarded_count)
        $crate::MI_STAT_COUNTER!(arena_rollback_count)
        $crate::MI_STAT_COUNTER!(arena_purges)
        $crate::MI_STAT_COUNTER!(pages_extended)
        $crate::MI_STAT_COUNTER!(pages_retire)
        $crate::MI_STAT_COUNTER!(page_searches)
        $crate::MI_STAT_COUNTER!(page_searches_count)
        $crate::MI_STAT_COUNT!(segments)
        $crate::MI_STAT_COUNT!(segments_abandoned)
        $crate::MI_STAT_COUNT!(segments_cache)
        $crate::MI_STAT_COUNT!(_segments_reserved)
        $crate::MI_STAT_COUNT!(heaps)
        $crate::MI_STAT_COUNT!(theaps)
        $crate::MI_STAT_COUNTER!(pages_reclaim_on_alloc)
        $crate::MI_STAT_COUNTER!(pages_reclaim_on_free)
        $crate::MI_STAT_COUNTER!(pages_reabandon_full)
        $crate::MI_STAT_COUNTER!(pages_unabandon_busy_wait)
        $crate::MI_STAT_COUNTER!(heaps_delete_wait)
    };
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
pub type mi_stats_s = mi_stats_t;

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

#[macro_export]
macro_rules! mi_stats_t_decl {
    ($name:ident) => {
        let mut $name = $crate::stats::mi_stats_t::default();
        // SAFETY: the macro creates a fresh local `mi_stats_t`.
        unsafe {
            $crate::stats::mi_stats_init(&mut $name);
        }
    };
}

#[macro_export]
macro_rules! MI_STAT_COUNTER {
    ($name:ident) => {
        pub $name: $crate::stats::mi_stat_counter_t,
    };
}

pub(crate) fn bin_block_size(bin: usize) -> usize {
    MI_BIN_WORD_SIZES[bin] * size_of::<usize>()
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_stats_get_bin_size(bin: usize) -> usize {
    if bin > MI_BIN_HUGE {
        return 0;
    }
    _mi_bin_size(bin)
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

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_counter_increase_mt(
    stat: *mut mi_stat_counter_t,
    amount: usize,
) {
    // SAFETY: `stat.total` is a valid atomic-compatible `i64` storage location.
    unsafe {
        mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), amount as i64);
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_counter_increase(
    stat: *mut mi_stat_counter_t,
    amount: usize,
) {
    unsafe {
        (*stat).total += amount as i64;
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_increase_mt(stat: *mut mi_stat_count_t, amount: usize) {
    mi_stat_update_mt(unsafe { &mut *stat }, amount as i64);
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_increase(stat: *mut mi_stat_count_t, amount: usize) {
    mi_stat_update(unsafe { &mut *stat }, amount as i64);
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease_mt(stat: *mut mi_stat_count_t, amount: usize) {
    mi_stat_update_mt(unsafe { &mut *stat }, -(amount as i64));
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease(stat: *mut mi_stat_count_t, amount: usize) {
    mi_stat_update(unsafe { &mut *stat }, -(amount as i64));
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

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_increase_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    mi_stat_adjust_mt(unsafe { &mut *stat }, amount as i64);
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_increase(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    mi_stat_adjust(unsafe { &mut *stat }, amount as i64);
}

#[used]
static MI_STAT_ADJUST_INCREASE_KEEP: unsafe extern "C" fn(*mut mi_stat_count_t, usize) =
    __mi_stat_adjust_increase;

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_decrease_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    mi_stat_adjust_mt(unsafe { &mut *stat }, -(amount as i64));
}

#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_decrease(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    mi_stat_adjust(unsafe { &mut *stat }, -(amount as i64));
}

fn mi_stat_count_add_mt(stat: *mut mi_stat_count_t, src: *const mi_stat_count_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
    // SAFETY: both pointers are valid for atomic-compatible `i64` field access.
    unsafe {
        mi_atomic_void_addi64_relaxed(
            ptr::addr_of_mut!((*stat).total),
            ptr::addr_of!((*src).total),
        );
        let prev_current =
            mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).current), (*src).current);
        mi_atomic_maxi64_relaxed(ptr::addr_of_mut!((*stat).peak), prev_current + (*src).peak);
    }
}

fn mi_stat_counter_add_mt(stat: *mut mi_stat_counter_t, src: *const mi_stat_counter_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
    // SAFETY: both pointers are valid for atomic-compatible `i64` field access.
    unsafe {
        mi_atomic_void_addi64_relaxed(
            ptr::addr_of_mut!((*stat).total),
            ptr::addr_of!((*src).total),
        );
    }
}

#[inline(never)]
unsafe extern "C" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t) {
    if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src) {
        return;
    }

    macro_rules! MI_STAT_COUNT {
        ($field:ident) => {
            unsafe {
                mi_stat_count_add_mt(
                    ptr::addr_of_mut!((*stats).$field),
                    ptr::addr_of!((*src).$field),
                )
            }
        };
    }
    macro_rules! MI_STAT_COUNTER {
        ($field:ident) => {
            unsafe {
                mi_stat_counter_add_mt(
                    ptr::addr_of_mut!((*stats).$field),
                    ptr::addr_of!((*src).$field),
                )
            }
        };
    }

    MI_STAT_COUNT!(pages);
    MI_STAT_COUNT!(reserved);
    MI_STAT_COUNT!(committed);
    MI_STAT_COUNTER!(reset);
    MI_STAT_COUNTER!(purged);
    MI_STAT_COUNT!(page_committed);
    MI_STAT_COUNT!(pages_abandoned);
    MI_STAT_COUNT!(threads);
    MI_STAT_COUNT!(malloc_normal);
    MI_STAT_COUNT!(malloc_huge);
    MI_STAT_COUNT!(malloc_requested);
    MI_STAT_COUNTER!(mmap_calls);
    MI_STAT_COUNTER!(commit_calls);
    MI_STAT_COUNTER!(reset_calls);
    MI_STAT_COUNTER!(purge_calls);
    MI_STAT_COUNTER!(arena_count);
    MI_STAT_COUNTER!(malloc_normal_count);
    MI_STAT_COUNTER!(malloc_huge_count);
    MI_STAT_COUNTER!(malloc_guarded_count);
    MI_STAT_COUNTER!(arena_rollback_count);
    MI_STAT_COUNTER!(arena_purges);
    MI_STAT_COUNTER!(pages_extended);
    MI_STAT_COUNTER!(pages_retire);
    MI_STAT_COUNTER!(page_searches);
    MI_STAT_COUNTER!(page_searches_count);
    MI_STAT_COUNT!(segments);
    MI_STAT_COUNT!(segments_abandoned);
    MI_STAT_COUNT!(segments_cache);
    MI_STAT_COUNT!(_segments_reserved);
    MI_STAT_COUNT!(heaps);
    MI_STAT_COUNT!(theaps);
    MI_STAT_COUNTER!(pages_reclaim_on_alloc);
    MI_STAT_COUNTER!(pages_reclaim_on_free);
    MI_STAT_COUNTER!(pages_reabandon_full);
    MI_STAT_COUNTER!(pages_unabandon_busy_wait);
    MI_STAT_COUNTER!(heaps_delete_wait);

    if MI_STAT > 1 {
        let stats_malloc_bins =
            unsafe { ptr::addr_of_mut!((*stats).malloc_bins).cast::<mi_stat_count_t>() };
        let src_malloc_bins =
            unsafe { ptr::addr_of!((*src).malloc_bins).cast::<mi_stat_count_t>() };
        for i in 0..=MI_BIN_HUGE {
            mi_stat_count_add_mt(unsafe { stats_malloc_bins.add(i) }, unsafe {
                src_malloc_bins.add(i)
            });
        }
    }
    let stats_page_bins =
        unsafe { ptr::addr_of_mut!((*stats).page_bins).cast::<mi_stat_count_t>() };
    let src_page_bins = unsafe { ptr::addr_of!((*src).page_bins).cast::<mi_stat_count_t>() };
    for i in 0..=MI_BIN_HUGE {
        mi_stat_count_add_mt(unsafe { stats_page_bins.add(i) }, unsafe {
            src_page_bins.add(i)
        });
    }
}

fn mi_stats_add_into(to: &mut mi_stats_t, from: &mi_stats_t) {
    if ptr::eq(to, from) {
        return;
    }
    unsafe { mi_stats_add(to as *mut mi_stats_t, from as *const mi_stats_t) };
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
    unsafe { _mi_prim_clock_now() }
}

#[inline(never)]
pub extern "C" fn _mi_clock_now() -> i64 {
    clock_now_msecs()
}

#[inline(never)]
pub extern "C" fn _mi_clock_start() -> i64 {
    let current = CLOCK_DIFF.load(Ordering::Relaxed);
    if current == 0 {
        let t0 = _mi_clock_now();
        let diff = _mi_clock_now() - t0;
        let _ = CLOCK_DIFF.compare_exchange(0, diff, Ordering::Relaxed, Ordering::Relaxed);
    }
    _mi_clock_now()
}

#[inline(never)]
pub extern "C" fn _mi_clock_end(start: i64) -> i64 {
    _mi_clock_now() - start - CLOCK_DIFF.load(Ordering::Relaxed)
}

#[inline(never)]
pub extern "C" fn _mi_stats_init() {
    if PROCESS_START.load(Ordering::Relaxed) == 0 {
        let start = _mi_clock_start();
        let _ = PROCESS_START.compare_exchange(0, start, Ordering::Relaxed, Ordering::Relaxed);
    }
}

#[used]
static MI_STATS_INIT_KEEP: extern "C" fn() = _mi_stats_init;

/// # Safety
/// `to` and `from` must point to valid writable `mi_stats_t` values.
#[inline(never)]
pub unsafe extern "C" fn _mi_stats_merge_into(to: *mut mi_stats_t, from: *mut mi_stats_t) {
    if to.is_null() || from.is_null() || core::ptr::eq(to, from) {
        return;
    }
    // SAFETY: caller guarantees valid pointers to `mi_stats_t` values.
    unsafe {
        mi_stats_add(to, from);
        _mi_memzero(from.cast(), size_of::<mi_stats_t>());
    }
}

#[repr(C)]
struct buffered_s {
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    buf: *mut c_char,
    used: usize,
    count: usize,
}

type buffered_t = buffered_s;

const MI_MAX_ARENAS: usize = 160;

#[repr(C)]
struct MiHeapStatsPrefix {
    subproc: *mut mi_subproc_t,
    heap_seq: usize,
    next: *mut mi_heap_t,
    prev: *mut mi_heap_t,
    theap: usize,
    exclusive_arena: *mut c_void,
    numa_node: i32,
    _padding: u32,
    theaps: *mut mi_theap_t,
    theaps_lock: mi_lock_t,
    abandoned_count: [AtomicUsize; MI_BIN_COUNT],
    os_abandoned_pages: *mut mi_page_t,
    os_abandoned_pages_lock: mi_lock_t,
    arena_pages: [AtomicPtr<c_void>; MI_MAX_ARENAS],
    arena_pages_lock: mi_lock_t,
    stats: mi_stats_t,
}

#[repr(C)]
struct MiSubprocStatsPrefix {
    subproc_seq: usize,
    next: *mut mi_subproc_t,
    prev: *mut mi_subproc_t,
    arena_count: AtomicUsize,
    arenas: [*mut c_void; MI_MAX_ARENAS],
    arena_reserve_lock: mi_lock_t,
    purge_expire: AtomicI64,
    heap_main: *mut mi_heap_t,
    heaps: *mut mi_heap_t,
    heaps_lock: mi_lock_t,
    thread_count: AtomicUsize,
    thread_total_count: AtomicUsize,
    heap_count: AtomicUsize,
    heap_total_count: AtomicUsize,
    memid: mi_memid_t,
    stats: mi_stats_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mi_heap_print_visit_info_s {
    pub out: Option<mi_output_fun>,
    pub out_arg: *mut c_void,
}

pub type mi_heap_print_visit_info_t = mi_heap_print_visit_info_s;

fn c_char_buffer_str(buf: &[c_char]) -> &str {
    // SAFETY: fixed formatting buffers are written only from ASCII format fragments and
    // always NUL-terminated by `mi_vsnprintf_args`.
    unsafe { str::from_utf8_unchecked(CStr::from_ptr(buf.as_ptr()).to_bytes()) }
}

fn format_amount(buf: &mut [c_char; 32], mut n: i64, unit: i64) {
    let suffix = if unit <= 0 { " " } else { "B" };
    let base = if unit == 0 { 1000 } else { 1024 };
    if unit > 0 {
        n = n.saturating_mul(unit);
    }

    let pos = if n < 0 { n.saturating_neg() } else { n };
    if pos < base {
        if n != 1 || suffix != "B" {
            let amount_suffix = if n == 0 { "" } else { suffix };
            crate::libc::mi_vsnprintf_args(
                buf.as_mut_ptr(),
                buf.len(),
                format_args!("{n}   {amount_suffix:<3}"),
            );
        } else {
            buf[0] = 0;
        }
    } else {
        let mut divider = base;
        let mut magnitude = "K";
        if pos >= divider * base {
            divider *= base;
            magnitude = "M";
        }
        if pos >= divider * base {
            divider *= base;
            magnitude = "G";
        }
        let tens = n / (divider / 10);
        let whole = tens / 10;
        let frac1 = (tens % 10).abs();
        let unitdesc = if base == 1024 {
            match magnitude {
                "K" => "KiB",
                "M" => "MiB",
                _ => "GiB",
            }
        } else {
            match magnitude {
                "K" => "K ",
                "M" => "M ",
                _ => "G ",
            }
        };
        crate::libc::mi_vsnprintf_args(
            buf.as_mut_ptr(),
            buf.len(),
            format_args!("{whole}.{frac1} {unitdesc:<3}"),
        );
    }
}

fn mi_printf_amount(
    n: i64,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    fmt: *const c_char,
) {
    let mut buf = [0 as c_char; 32];
    format_amount(&mut buf, n, unit);
    let rendered = c_char_buffer_str(&buf);
    if fmt.is_null() {
        _mi_fprintf(out, arg, format_args!("{rendered:>12}"));
        return;
    }
    // SAFETY: `fmt` is expected to point to a valid NUL-terminated C string.
    let fmt_bytes = unsafe { CStr::from_ptr(fmt) }.to_bytes();
    if fmt_bytes == b"%s" {
        _mi_fprintf(out, arg, format_args!("{rendered}"));
    } else {
        _mi_fprintf(out, arg, format_args!("{rendered:>12}"));
    }
}

fn mi_print_amount(n: i64, unit: i64, out: Option<mi_output_fun>, arg: *mut c_void) {
    mi_printf_amount(n, unit, out, arg, ptr::null());
}

fn mi_print_count(n: i64, unit: i64, out: Option<mi_output_fun>, arg: *mut c_void) {
    if unit == 1 {
        _mi_fprintf(out, arg, format_args!("{:>12}", " "));
    } else {
        mi_print_amount(n, 0, out, arg);
    }
}

fn mi_stat_print_ex(
    stat: &mi_stat_count_t,
    msg: &str,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    notok: Option<&str>,
) {
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    if unit != 0 {
        if unit > 0 {
            mi_print_amount(stat.peak, unit, out, arg);
            mi_print_amount(stat.total, unit, out, arg);
            mi_print_amount(stat.current, unit, out, arg);
            mi_print_amount(unit, 1, out, arg);
            mi_print_count(stat.total, unit, out, arg);
        } else {
            mi_print_amount(stat.peak, -1, out, arg);
            mi_print_amount(stat.total, -1, out, arg);
            mi_print_amount(stat.current, -1, out, arg);
            if unit == -1 {
                _mi_fprintf(out, arg, format_args!("{:>24}", ""));
            } else {
                mi_print_amount(-unit, 1, out, arg);
                mi_print_count(stat.total / -unit, 0, out, arg);
            }
        }
        if stat.current != 0 {
            _mi_fprintf(
                out,
                arg,
                format_args!("  {}\n", notok.unwrap_or("not all freed")),
            );
        } else {
            _mi_fprintf(out, arg, format_args!("  ok\n"));
        }
    } else {
        mi_print_amount(stat.peak, 0, out, arg);
        mi_print_amount(stat.total, 0, out, arg);
        mi_print_amount(stat.current, 0, out, arg);
        _mi_fprintf(out, arg, format_args!("\n"));
    }
}

fn mi_stat_print(
    stat: &mi_stat_count_t,
    msg: &str,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    mi_stat_print_ex(stat, msg, unit, out, arg, None);
}

fn mi_stat_total_print(
    stat: &mi_stat_count_t,
    msg: &str,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    _mi_fprintf(out, arg, format_args!("{:>12}", " "));
    mi_print_amount(stat.total, unit, out, arg);
    _mi_fprintf(out, arg, format_args!("\n"));
}

fn mi_stat_counter_print(
    stat: &mi_stat_counter_t,
    msg: &str,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    mi_print_amount(stat.total, 0, out, arg);
    _mi_fprintf(out, arg, format_args!("\n"));
}

fn mi_stat_counter_print_size(
    stat: &mi_stat_counter_t,
    msg: &str,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    mi_print_amount(stat.total, 1, out, arg);
    _mi_fprintf(out, arg, format_args!("\n"));
}

fn mi_stat_average_print(
    count: i64,
    total: i64,
    msg: &str,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let avg_tens = if count == 0 { 0 } else { total * 10 / count };
    let avg_whole = avg_tens / 10;
    let avg_frac1 = avg_tens % 10;
    _mi_fprintf(
        out,
        arg,
        format_args!("  {msg:<10}: {avg_whole:>5}.{avg_frac1} avg\n"),
    );
}

fn mi_print_header(name: &str, out: Option<mi_output_fun>, arg: *mut c_void) {
    _mi_fprintf(
        out,
        arg,
        format_args!(
            " {name:<11} {:>11} {:>11} {:>11} {:>11} {:>11}\n",
            "peak   ", "total   ", "current   ", "block   ", "total#   "
        ),
    );
}

fn mi_stats_print_bins(
    bins: &[mi_stat_count_t],
    max: usize,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) -> bool {
    let mut found = false;
    for (i, stat) in bins.iter().enumerate().take(max + 1) {
        if stat.total > 0 {
            found = true;
            let unit = _mi_bin_size(i);
            let pagekind = if unit <= MI_SMALL_MAX_OBJ_SIZE {
                "S"
            } else if unit <= MI_MEDIUM_MAX_OBJ_SIZE {
                "M"
            } else if unit <= MI_LARGE_MAX_OBJ_SIZE {
                "L"
            } else {
                "H"
            };
            let mut label = [0 as c_char; 64];
            crate::libc::mi_vsnprintf_args(
                label.as_mut_ptr(),
                label.len(),
                format_args!("bin{pagekind:>2}  {i:>3}"),
            );
            mi_stat_print(stat, c_char_buffer_str(&label), unit as i64, out, arg);
        }
    }
    if found {
        _mi_fprintf(out, arg, format_args!("\n"));
    }
    found
}

fn mi_buffered_flush(buf: &mut buffered_t) {
    if buf.buf.is_null() {
        return;
    }
    // SAFETY: `buf.buf` points to writable storage of at least `count + 1` bytes.
    unsafe {
        *buf.buf.add(buf.used) = 0;
        _mi_fputs(buf.out, buf.arg, ptr::null(), buf.buf);
    }
    buf.used = 0;
}

unsafe extern "C" fn mi_buffered_out(msg: *const c_char, arg: *mut c_void) {
    if msg.is_null() || arg.is_null() {
        return;
    }
    // SAFETY: caller passes a valid `buffered_t` as `arg` and `msg` is a NUL-terminated string.
    unsafe {
        let buf = &mut *(arg as *mut buffered_t);
        let mut src = msg;
        while *src != 0 {
            let c = *src;
            if buf.used >= buf.count {
                mi_buffered_flush(buf);
            }
            *buf.buf.add(buf.used) = c;
            buf.used += 1;
            if c == b'\n' as c_char {
                mi_buffered_flush(buf);
            }
            src = src.add(1);
        }
    }
}

unsafe fn _mi_stats_print(
    name: &str,
    id: usize,
    stats: *const mi_stats_t,
    out0: Option<mi_output_fun>,
    arg0: *mut c_void,
) {
    let Some(stats) = (unsafe { stats.as_ref() }) else {
        return;
    };

    let mut buf = [0 as c_char; 256];
    let mut buffer = buffered_t {
        out: out0,
        arg: arg0,
        buf: ptr::null_mut(),
        used: 0,
        count: 255,
    };
    buffer.buf = buf.as_mut_ptr();
    let out: Option<mi_output_fun> = Some(mi_buffered_out as mi_output_fun);
    let arg = (&mut buffer as *mut buffered_t).cast::<c_void>();

    _mi_fprintf(out, arg, format_args!("{name} {id}\n"));

    if stats.malloc_normal.total + stats.malloc_huge.total != 0 {
        mi_print_header("blocks", out, arg);
        mi_stats_print_bins(&stats.malloc_bins, MI_BIN_HUGE, out, arg);
        mi_stat_print(
            &stats.malloc_normal,
            "binned",
            if stats.malloc_normal_count.total == 0 {
                -1
            } else {
                1
            },
            out,
            arg,
        );
        mi_stat_print(
            &stats.malloc_huge,
            "huge",
            if stats.malloc_huge_count.total == 0 {
                -1
            } else {
                1
            },
            out,
            arg,
        );
        let mut total = mi_stat_count_t::default();
        mi_stat_count_add_mt(&mut total, &stats.malloc_normal);
        mi_stat_count_add_mt(&mut total, &stats.malloc_huge);
        mi_stat_print_ex(&total, "total", 1, out, arg, Some(""));
        mi_stat_total_print(&stats.malloc_requested, "malloc req", 1, out, arg);
        _mi_fprintf(out, arg, format_args!("\n"));
    }

    if stats.pages.total != 0 {
        mi_print_header("pages", out, arg);
        mi_stat_print_ex(&stats.page_committed, "touched", 1, out, arg, Some(""));
        mi_stat_print(&stats.pages, "pages", 0, out, arg);
        mi_stat_print(&stats.pages_abandoned, "abandoned", 0, out, arg);
        mi_stat_counter_print(&stats.pages_reclaim_on_alloc, "reclaima", out, arg);
        mi_stat_counter_print(&stats.pages_reclaim_on_free, "reclaimf", out, arg);
        mi_stat_counter_print(&stats.pages_reabandon_full, "reabandon", out, arg);
        mi_stat_counter_print(&stats.pages_unabandon_busy_wait, "waits", out, arg);
        mi_stat_counter_print(&stats.pages_extended, "extended", out, arg);
        mi_stat_counter_print(&stats.pages_retire, "retire", out, arg);
        mi_stat_average_print(
            stats.page_searches_count.total,
            stats.page_searches.total,
            "searches",
            out,
            arg,
        );
        _mi_fprintf(out, arg, format_args!("\n"));
    }

    if stats.arena_count.total > 0 {
        mi_print_header("arenas", out, arg);
        mi_stat_print_ex(&stats.reserved, "reserved", 1, out, arg, Some(""));
        mi_stat_print_ex(&stats.committed, "committed", 1, out, arg, Some(""));
        mi_stat_counter_print_size(&stats.reset, "reset", out, arg);
        mi_stat_counter_print_size(&stats.purged, "purged", out, arg);
        mi_stat_counter_print(&stats.arena_count, "arenas", out, arg);
        mi_stat_counter_print(&stats.arena_rollback_count, "rollback", out, arg);
        mi_stat_counter_print(&stats.mmap_calls, "mmaps", out, arg);
        mi_stat_counter_print(&stats.commit_calls, "commits", out, arg);
        mi_stat_counter_print(&stats.reset_calls, "resets", out, arg);
        mi_stat_counter_print(&stats.purge_calls, "purges", out, arg);
        mi_stat_counter_print(&stats.malloc_guarded_count, "guarded", out, arg);
        mi_stat_print_ex(&stats.theaps, "theaps", 0, out, arg, Some(""));
        mi_stat_print_ex(&stats.heaps, "heaps", 0, out, arg, Some(""));
        mi_stat_counter_print(&stats.heaps_delete_wait, "heap waits", out, arg);
        _mi_fprintf(out, arg, format_args!("\n"));

        mi_print_header("process", out, arg);
        mi_stat_print_ex(&stats.threads, "threads", 0, out, arg, Some(""));
        _mi_fprintf(
            out,
            arg,
            format_args!("  {:<10}: {:>5}\n", "numa nodes", unsafe {
                _mi_os_numa_node_count()
            }),
        );
        unsafe { mi_process_info_print_out(out, arg) };
    }
    _mi_fprintf(out, arg, format_args!("\n"));
}

unsafe fn mi_stats_merge_theap_to_heap(theap: *mut mi_theap_t) -> *const mi_stats_t {
    let stats = unsafe { &mut (*theap).stats };
    let heap = unsafe { _mi_theap_heap(theap) }.cast::<MiHeapStatsPrefix>();
    let heap_stats = unsafe { &mut (*heap).stats };
    unsafe { _mi_stats_merge_into(heap_stats, stats) };
    heap_stats as *const mi_stats_t
}

unsafe fn mi_heap_get_stats(mut heap: *mut mi_heap_t) -> *const mi_stats_t {
    if heap.is_null() {
        heap = unsafe { mi_heap_main() };
    }
    if !unsafe { crate::prim::_mi_thread_is_initialized() } {
        return unsafe { &(*heap.cast::<MiHeapStatsPrefix>()).stats };
    }
    let theap = unsafe { _mi_heap_theap_peek(heap) };
    if theap.is_null() {
        unsafe { &(*heap.cast::<MiHeapStatsPrefix>()).stats }
    } else {
        unsafe { mi_stats_merge_theap_to_heap(theap) }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_merge_to_subproc(mut heap: *mut mi_heap_t) {
    if heap.is_null() {
        heap = unsafe { mi_heap_main() };
    }
    let heap = heap.cast::<MiHeapStatsPrefix>();
    let subproc = unsafe { (*heap).subproc.cast::<MiSubprocStatsPrefix>() };
    unsafe { _mi_stats_merge_into(&mut (*subproc).stats, &mut (*heap).stats) };
}

/// # Safety
/// Uses the process-default heap/theap state and follows the legacy C ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_reset() {
    if !unsafe { mi_theap_is_initialized(_mi_theap_default()) } {
        return;
    }
    let heap = unsafe { mi_heap_main() };
    let _ = unsafe { mi_heap_get_stats(heap) };
    unsafe { mi_heap_stats_merge_to_subproc(heap) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_merge() {
    if !unsafe { mi_theap_is_initialized(_mi_theap_default()) } {
        return;
    }
    let heap = unsafe { mi_heap_main() };
    let _ = unsafe { mi_heap_get_stats(heap) };
    unsafe { mi_heap_stats_merge_to_subproc(heap) };
}

/// # Safety
/// `heap` must be null or a valid `mi_heap_t*`; any output callback must obey the C ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_print_out(
    mut heap: *mut mi_heap_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    if heap.is_null() {
        heap = unsafe { mi_heap_main() };
    }
    let heap_prefix = heap.cast::<MiHeapStatsPrefix>();
    unsafe {
        _mi_stats_print(
            "heap",
            (*heap_prefix).heap_seq,
            mi_heap_get_stats(heap),
            out,
            arg,
        )
    };
}

#[inline(never)]
unsafe extern "C" fn mi_heap_print_visitor(heap: *mut mi_heap_t, arg: *mut c_void) -> bool {
    let vinfo = unsafe { &*(arg.cast::<mi_heap_print_visit_info_t>()) };
    unsafe { mi_heap_stats_print_out(heap, vinfo.out, vinfo.out_arg) };
    true
}

#[inline(never)]
unsafe extern "C" fn mi_heap_aggregate_visitor(heap: *mut mi_heap_t, arg: *mut c_void) -> bool {
    let stats = unsafe { &mut *arg.cast::<mi_stats_t>() };
    let heap_stats = unsafe { mi_heap_get_stats(heap) };
    // SAFETY: both pointers refer to valid stats buffers during the heap walk.
    unsafe { mi_stats_add_into(stats, &*heap_stats) };
    true
}

#[used]
static MI_HEAP_PRINT_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_print_visitor;

#[used]
static MI_HEAP_AGGREGATE_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_aggregate_visitor;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_heap_stats_print_out(
    subproc_id: mi_subproc_id_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let subproc = subproc_id._mi_subproc_id.cast::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return;
    }
    let mut vinfo = mi_heap_print_visit_info_t { out, out_arg: arg };
    unsafe {
        mi_subproc_visit_heaps(
            subproc_id,
            Some(mi_heap_print_visitor as mi_heap_visit_fun),
            ptr::addr_of_mut!(vinfo).cast(),
        );
        _mi_stats_print(
            "subproc",
            (*subproc).subproc_seq,
            ptr::addr_of!((*subproc).stats),
            out,
            arg,
        );
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get_exclusive(
    subproc_id: mi_subproc_id_t,
    stats: *mut mi_stats_t,
) -> bool {
    if stats.is_null() {
        return false;
    }
    let subproc = subproc_id._mi_subproc_id.cast::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return false;
    }
    unsafe { mi_stats_copy(&mut *stats, &(*subproc).stats) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get(
    subproc_id: mi_subproc_id_t,
    stats: *mut mi_stats_t,
) -> bool {
    if stats.is_null() {
        return false;
    }
    let subproc = subproc_id._mi_subproc_id.cast::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return false;
    }
    unsafe {
        if !mi_stats_copy(&mut *stats, &(*subproc).stats) {
            return false;
        }
        mi_subproc_visit_heaps(
            subproc_id,
            Some(mi_heap_aggregate_visitor as mi_heap_visit_fun),
            stats.cast(),
        );
    }
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get(stats: *mut mi_stats_t) -> bool {
    unsafe { mi_subproc_stats_get(mi_subproc_current(), stats) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_get(heap: *mut mi_heap_t, stats: *mut mi_stats_t) -> bool {
    if stats.is_null() {
        return false;
    }
    unsafe { mi_stats_copy(stats, mi_heap_get_stats(heap)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect_reduce(_target_thread_owned: usize) {
    unsafe { crate::theap::mi_collect(false) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_print_out(
    subproc_id: mi_subproc_id_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let subproc = subproc_id._mi_subproc_id.cast::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return;
    }
    crate::mi_stats_t_decl!(stats);
    unsafe {
        if mi_subproc_stats_get(subproc_id, ptr::addr_of_mut!(stats)) {
            _mi_stats_print(
                "subproc",
                (*subproc).subproc_seq,
                ptr::addr_of!(stats),
                out,
                arg,
            );
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    unsafe { mi_subproc_stats_print_out(mi_subproc_current(), out, arg) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_print(out: *mut c_void) {
    let out_fun = if out.is_null() {
        None
    } else {
        // The legacy C API accepts an output callback through a `void*`.
        Some(unsafe { core::mem::transmute::<*mut c_void, mi_output_fun>(out) })
    };
    unsafe { mi_stats_print_out(out_fun, ptr::null_mut()) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_thread_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    let theap = unsafe { _mi_theap_default() };
    if theap.is_null() || !unsafe { mi_theap_is_initialized(theap) } {
        return;
    }
    let heap = unsafe { _mi_theap_heap(theap) };
    let heap_seq = unsafe { (*heap.cast::<MiHeapStatsPrefix>()).heap_seq };
    unsafe {
        _mi_stats_print("heap", heap_seq, ptr::addr_of!((*theap).stats), out, arg);
        mi_stats_merge_theap_to_heap(theap);
    }
}

fn clamp_msecs_to_size(value: i64) -> usize {
    if value <= 0 {
        0
    } else {
        value.min(isize::MAX as i64) as usize
    }
}

/// # Safety
/// Output callbacks and argument pointers must follow the C ABI contract when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_process_info_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    let mut elapsed = 0usize;
    let mut user_time = 0usize;
    let mut sys_time = 0usize;
    let mut current_rss = 0usize;
    let mut peak_rss = 0usize;
    let mut current_commit = 0usize;
    let mut peak_commit = 0usize;
    let mut page_faults = 0usize;
    // SAFETY: local pointers below are valid writable outputs.
    unsafe {
        mi_process_info(
            &mut elapsed,
            &mut user_time,
            &mut sys_time,
            &mut current_rss,
            &mut peak_rss,
            &mut current_commit,
            &mut peak_commit,
            &mut page_faults,
        );
    }
    _mi_fprintf(
        out,
        arg,
        format_args!(
            "  {:<10}: {:>5}.{:03} s\n",
            "elapsed",
            elapsed / 1000,
            elapsed % 1000
        ),
    );
    _mi_fprintf(
        out,
        arg,
        format_args!(
            "  {:<10}: user: {}.{:03} s, system: {}.{:03} s, faults: {}, peak rss: ",
            "process",
            user_time / 1000,
            user_time % 1000,
            sys_time / 1000,
            sys_time % 1000,
            page_faults
        ),
    );
    mi_printf_amount(peak_rss as i64, 1, out, arg, c"%s".as_ptr());
    if peak_commit > 0 {
        _mi_fprintf(out, arg, format_args!(", peak commit: "));
        mi_printf_amount(peak_commit as i64, 1, out, arg, c"%s".as_ptr());
    }
    _mi_fprintf(out, arg, format_args!("\n"));
}

/// # Safety
/// Any non-null output pointers must be valid for writing one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_process_info(
    elapsed_msecs: *mut usize,
    user_msecs: *mut usize,
    system_msecs: *mut usize,
    current_rss: *mut usize,
    peak_rss: *mut usize,
    current_commit: *mut usize,
    peak_commit: *mut usize,
    page_faults: *mut usize,
) {
    let mut pinfo = mi_process_info_t {
        elapsed: _mi_clock_end(PROCESS_START.load(Ordering::Relaxed)),
        ..Default::default()
    };
    let subproc = unsafe { _mi_subproc_main() }.cast::<MiSubprocStatsPrefix>();
    if let Some(subproc) = unsafe { subproc.as_ref() } {
        pinfo.current_commit = subproc.stats.committed.current.max(0) as usize;
        pinfo.peak_commit = subproc.stats.committed.peak.max(0) as usize;
    }
    pinfo.current_rss = pinfo.current_commit;
    pinfo.peak_rss = pinfo.peak_commit;
    // SAFETY: `pinfo` is a valid mutable primitive info struct.
    unsafe {
        _mi_prim_process_info(&mut pinfo);
        if !elapsed_msecs.is_null() {
            *elapsed_msecs = clamp_msecs_to_size(pinfo.elapsed);
        }
        if !user_msecs.is_null() {
            *user_msecs = clamp_msecs_to_size(pinfo.utime);
        }
        if !system_msecs.is_null() {
            *system_msecs = clamp_msecs_to_size(pinfo.stime);
        }
        if !current_rss.is_null() {
            *current_rss = pinfo.current_rss;
        }
        if !peak_rss.is_null() {
            *peak_rss = pinfo.peak_rss;
        }
        if !current_commit.is_null() {
            *current_commit = pinfo.current_commit;
        }
        if !peak_commit.is_null() {
            *peak_commit = pinfo.peak_commit;
        }
        if !page_faults.is_null() {
            *page_faults = pinfo.page_faults;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_process_info_print() {
    // SAFETY: null output selects the default output sink.
    unsafe { mi_process_info_print_out(None, ptr::null_mut()) }
}

#[repr(C)]
struct mi_json_buf_s {
    buf: *mut c_char,
    size: usize,
    used: usize,
    can_realloc: bool,
}

type mi_json_buf_t = mi_json_buf_s;

impl Default for mi_json_buf_s {
    fn default() -> Self {
        Self {
            buf: ptr::null_mut(),
            size: 0,
            used: 0,
            can_realloc: true,
        }
    }
}

fn mi_json_buf_expand(hbuf: &mut mi_json_buf_t) -> bool {
    if !hbuf.buf.is_null() && hbuf.size > 0 {
        // SAFETY: `buf` is valid for `size` bytes when non-null.
        unsafe { *hbuf.buf.add(hbuf.size - 1) = 0 };
    }
    if hbuf.size > usize::MAX / 2 || !hbuf.can_realloc {
        return false;
    }
    let newsize = if hbuf.size == 0 {
        unsafe { mi_good_size(12 * 1024) }
    } else {
        2 * hbuf.size
    };
    // SAFETY: `mi_rezalloc` accepts null for fresh allocation and preserves the old contents.
    let newbuf = unsafe { mi_rezalloc(hbuf.buf.cast(), newsize).cast::<c_char>() };
    if newbuf.is_null() {
        return false;
    }
    hbuf.buf = newbuf;
    hbuf.size = newsize;
    true
}

fn mi_json_buf_print(hbuf: &mut mi_json_buf_t, msg: &str) {
    if hbuf.used + 1 >= hbuf.size && !hbuf.can_realloc {
        return;
    }
    for byte in msg.bytes() {
        if hbuf.used + 1 >= hbuf.size && !mi_json_buf_expand(hbuf) {
            return;
        }
        crate::mi_debug_assert!(hbuf.used < hbuf.size);
        // SAFETY: capacity is ensured just above and `used < size`.
        unsafe { *hbuf.buf.add(hbuf.used) = byte as c_char };
        hbuf.used += 1;
    }
    crate::mi_debug_assert!(hbuf.used < hbuf.size);
    // SAFETY: capacity is ensured above and the trailing NUL fits.
    unsafe { *hbuf.buf.add(hbuf.used) = 0 };
}

fn mi_json_buf_print_count_bin(
    hbuf: &mut mi_json_buf_t,
    prefix: &str,
    stat: &mi_stat_count_t,
    bin: usize,
    add_comma: bool,
) {
    let binsize = mi_stats_get_bin_size(bin);
    let pagesize = if binsize <= MI_SMALL_MAX_OBJ_SIZE {
        MI_SMALL_PAGE_SIZE
    } else if binsize <= MI_MEDIUM_MAX_OBJ_SIZE {
        MI_MEDIUM_PAGE_SIZE
    } else if binsize <= MI_LARGE_MAX_OBJ_SIZE {
        MI_LARGE_PAGE_SIZE
    } else {
        0
    };
    let mut buf = [0 as c_char; 128];
    crate::libc::mi_vsnprintf_args(
        buf.as_mut_ptr(),
        buf.len(),
        format_args!(
            "{prefix}{{ \"total\": {}, \"peak\": {}, \"current\": {}, \"block_size\": {binsize}, \"page_size\": {pagesize} }}{}\n",
            stat.total,
            stat.peak,
            stat.current,
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_count_cbin(
    hbuf: &mut mi_json_buf_t,
    prefix: &str,
    stat: &mi_stat_count_t,
    bin: mi_chunkbin_t,
    add_comma: bool,
) {
    let cbin = match bin {
        mi_chunkbin_e::MI_CBIN_SMALL => "S",
        mi_chunkbin_e::MI_CBIN_MEDIUM => "M",
        mi_chunkbin_e::MI_CBIN_LARGE => "L",
        mi_chunkbin_e::MI_CBIN_HUGE => "H",
        mi_chunkbin_e::MI_CBIN_OTHER => "X",
        _ => " ",
    };
    let mut buf = [0 as c_char; 128];
    crate::libc::mi_vsnprintf_args(
        buf.as_mut_ptr(),
        buf.len(),
        format_args!(
            "{prefix}{{ \"total\": {}, \"peak\": {}, \"current\": {}, \"bin\": \"{cbin}\" }}{}\n",
            stat.total,
            stat.peak,
            stat.current,
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_count(
    hbuf: &mut mi_json_buf_t,
    prefix: &str,
    stat: &mi_stat_count_t,
    add_comma: bool,
) {
    let mut buf = [0 as c_char; 128];
    crate::libc::mi_vsnprintf_args(
        buf.as_mut_ptr(),
        buf.len(),
        format_args!(
            "{prefix}{{ \"total\": {}, \"peak\": {}, \"current\": {} }}{}\n",
            stat.total,
            stat.peak,
            stat.current,
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_count_value(hbuf: &mut mi_json_buf_t, name: &str, stat: &mi_stat_count_t) {
    let mut buf = [0 as c_char; 128];
    crate::libc::mi_vsnprintf_args(buf.as_mut_ptr(), buf.len(), format_args!("  \"{name}\": "));
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
    mi_json_buf_print_count(hbuf, "", stat, true);
}

#[inline(never)]
fn mi_json_buf_print_value(hbuf: &mut mi_json_buf_t, name: &str, val: i64) {
    let mut buf = [0 as c_char; 128];
    crate::libc::mi_vsnprintf_args(
        buf.as_mut_ptr(),
        buf.len(),
        format_args!("  \"{name}\": {val},\n"),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

#[used]
static MI_JSON_BUF_PRINT_VALUE_KEEP: fn(&mut mi_json_buf_t, &str, i64) = mi_json_buf_print_value;

fn mi_json_buf_print_size(hbuf: &mut mi_json_buf_t, name: &str, val: usize, add_comma: bool) {
    let mut buf = [0 as c_char; 128];
    crate::libc::mi_vsnprintf_args(
        buf.as_mut_ptr(),
        buf.len(),
        format_args!(
            "    \"{name}\": {val}{}\n",
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_counter_value(hbuf: &mut mi_json_buf_t, name: &str, stat: &mi_stat_counter_t) {
    mi_json_buf_print_value(hbuf, name, stat.total);
}

fn append_stats_json(hbuf: &mut mi_json_buf_t, stats: &mi_stats_t) {
    macro_rules! MI_STAT_COUNT {
        ($field:ident) => {
            mi_json_buf_print_count_value(hbuf, stringify!($field), &stats.$field)
        };
    }

    MI_STAT_COUNT!(pages);
    MI_STAT_COUNT!(reserved);
    MI_STAT_COUNT!(committed);
    mi_json_buf_print_counter_value(hbuf, "reset", &stats.reset);
    mi_json_buf_print_counter_value(hbuf, "purged", &stats.purged);
    MI_STAT_COUNT!(page_committed);
    MI_STAT_COUNT!(pages_abandoned);
    MI_STAT_COUNT!(threads);
    MI_STAT_COUNT!(malloc_normal);
    MI_STAT_COUNT!(malloc_huge);
    MI_STAT_COUNT!(malloc_requested);
    mi_json_buf_print_counter_value(hbuf, "mmap_calls", &stats.mmap_calls);
    mi_json_buf_print_counter_value(hbuf, "commit_calls", &stats.commit_calls);
    mi_json_buf_print_counter_value(hbuf, "reset_calls", &stats.reset_calls);
    mi_json_buf_print_counter_value(hbuf, "purge_calls", &stats.purge_calls);
    mi_json_buf_print_counter_value(hbuf, "arena_count", &stats.arena_count);
    mi_json_buf_print_counter_value(hbuf, "malloc_normal_count", &stats.malloc_normal_count);
    mi_json_buf_print_counter_value(hbuf, "malloc_huge_count", &stats.malloc_huge_count);
    mi_json_buf_print_counter_value(hbuf, "malloc_guarded_count", &stats.malloc_guarded_count);
    mi_json_buf_print_counter_value(hbuf, "arena_rollback_count", &stats.arena_rollback_count);
    mi_json_buf_print_counter_value(hbuf, "arena_purges", &stats.arena_purges);
    mi_json_buf_print_counter_value(hbuf, "pages_extended", &stats.pages_extended);
    mi_json_buf_print_counter_value(hbuf, "pages_retire", &stats.pages_retire);
    mi_json_buf_print_counter_value(hbuf, "page_searches", &stats.page_searches);
    mi_json_buf_print_counter_value(hbuf, "page_searches_count", &stats.page_searches_count);
    MI_STAT_COUNT!(segments);
    MI_STAT_COUNT!(segments_abandoned);
    MI_STAT_COUNT!(segments_cache);
    MI_STAT_COUNT!(_segments_reserved);
    MI_STAT_COUNT!(heaps);
    MI_STAT_COUNT!(theaps);
    mi_json_buf_print_counter_value(
        hbuf,
        "pages_reclaim_on_alloc",
        &stats.pages_reclaim_on_alloc,
    );
    mi_json_buf_print_counter_value(hbuf, "pages_reclaim_on_free", &stats.pages_reclaim_on_free);
    mi_json_buf_print_counter_value(hbuf, "pages_reabandon_full", &stats.pages_reabandon_full);
    mi_json_buf_print_counter_value(
        hbuf,
        "pages_unabandon_busy_wait",
        &stats.pages_unabandon_busy_wait,
    );
    mi_json_buf_print_counter_value(hbuf, "heaps_delete_wait", &stats.heaps_delete_wait);
}

/// # Safety
/// `stats` must point to a valid `mi_stats_t` when non-null. If `output_buf` is non-null it
/// must be writable for `output_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get_json_from(
    stats: *const mi_stats_t,
    output_size: usize,
    output_buf: *mut c_char,
) -> *mut c_char {
    // SAFETY: caller provides a valid stats pointer when non-null.
    let Some(stats) = (unsafe { stats.as_ref() }) else {
        return ptr::null_mut();
    };
    if stats.size != size_of::<mi_stats_t>() || stats.version != MI_STAT_VERSION {
        return ptr::null_mut();
    }

    let mut hbuf = mi_json_buf_t::default();
    if output_size > 0 && !output_buf.is_null() {
        // SAFETY: caller provides `output_buf` writable for `output_size` bytes.
        unsafe { ptr::write_bytes(output_buf, 0, output_size) };
        hbuf.buf = output_buf;
        hbuf.size = output_size;
        hbuf.can_realloc = false;
    } else if !mi_json_buf_expand(&mut hbuf) {
        return ptr::null_mut();
    }
    mi_json_buf_print(&mut hbuf, "{\n");
    mi_json_buf_print_value(&mut hbuf, "stat_version", MI_STAT_VERSION as i64);
    mi_json_buf_print_value(&mut hbuf, "mimalloc_version", MI_MALLOC_VERSION as i64);

    mi_json_buf_print(&mut hbuf, "  \"process\": {\n");
    let mut elapsed = 0usize;
    let mut user_time = 0usize;
    let mut sys_time = 0usize;
    let mut current_rss = 0usize;
    let mut peak_rss = 0usize;
    let mut current_commit = 0usize;
    let mut peak_commit = 0usize;
    let mut page_faults = 0usize;
    // SAFETY: local pointers below are valid writable outputs.
    unsafe {
        mi_process_info(
            &mut elapsed,
            &mut user_time,
            &mut sys_time,
            &mut current_rss,
            &mut peak_rss,
            &mut current_commit,
            &mut peak_commit,
            &mut page_faults,
        );
    }
    mi_json_buf_print_size(&mut hbuf, "elapsed_msecs", elapsed, true);
    mi_json_buf_print_size(&mut hbuf, "user_msecs", user_time, true);
    mi_json_buf_print_size(&mut hbuf, "system_msecs", sys_time, true);
    mi_json_buf_print_size(&mut hbuf, "page_faults", page_faults, true);
    mi_json_buf_print_size(&mut hbuf, "rss_current", current_rss, true);
    mi_json_buf_print_size(&mut hbuf, "rss_peak", peak_rss, true);
    mi_json_buf_print_size(&mut hbuf, "commit_current", current_commit, true);
    mi_json_buf_print_size(&mut hbuf, "commit_peak", peak_commit, false);
    mi_json_buf_print(&mut hbuf, "  },\n");

    append_stats_json(&mut hbuf, stats);

    mi_json_buf_print(&mut hbuf, "  \"malloc_bins\": [\n");
    for i in 0..=MI_BIN_HUGE {
        mi_json_buf_print_count_bin(
            &mut hbuf,
            "    ",
            &stats.malloc_bins[i],
            i,
            i != MI_BIN_HUGE,
        );
    }
    mi_json_buf_print(&mut hbuf, "  ],\n");
    mi_json_buf_print(&mut hbuf, "  \"page_bins\": [\n");
    for i in 0..=MI_BIN_HUGE {
        mi_json_buf_print_count_bin(&mut hbuf, "    ", &stats.page_bins[i], i, i != MI_BIN_HUGE);
    }
    mi_json_buf_print(&mut hbuf, "  ],\n");
    mi_json_buf_print(&mut hbuf, "  \"chunk_bins\": [\n");
    for i in 0..MI_CHUNKBIN_COUNT {
        mi_json_buf_print_count_cbin(
            &mut hbuf,
            "    ",
            &stats.chunk_bins[i],
            unsafe { core::mem::transmute::<i32, mi_chunkbin_t>(i as i32) },
            i + 1 != MI_CHUNKBIN_COUNT,
        );
    }
    mi_json_buf_print(&mut hbuf, "  ]\n");
    mi_json_buf_print(&mut hbuf, "}\n");

    if hbuf.used >= hbuf.size {
        if hbuf.can_realloc {
            // SAFETY: the buffer came from mimalloc when `can_realloc` is true.
            unsafe { crate::mimalloc::mi_free(hbuf.buf.cast()) };
        }
        ptr::null_mut()
    } else {
        hbuf.buf
    }
}

/// # Safety
/// `stats` must point to a valid `mi_stats_t` when non-null. If `buf` is non-null it must be
/// writable for `buf_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get_json(
    subproc_id: mi_subproc_id_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    crate::mi_stats_t_decl!(stats);
    unsafe {
        if !mi_subproc_stats_get(subproc_id, ptr::addr_of_mut!(stats)) {
            return ptr::null_mut();
        }
        mi_stats_get_json_from(ptr::addr_of!(stats), buf_size, buf)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_get_json(
    heap: *mut mi_heap_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    unsafe { mi_stats_get_json_from(mi_heap_get_stats(heap), buf_size, buf) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get_json(buf_size: usize, buf: *mut c_char) -> *mut c_char {
    unsafe { mi_subproc_stats_get_json(mi_subproc_current(), buf_size, buf) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_as_json(
    stats: *mut mi_stats_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    unsafe { mi_stats_get_json_from(stats.cast_const(), buf_size, buf) }
}
