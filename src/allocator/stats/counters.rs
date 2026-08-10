//! Statistics counter initialization, mutation, and aggregation.

use core::{mem::size_of, ptr};

use crate::allocator::page::_mi_bin_size;
use crate::internal::_mi_memzero;
use crate::internal::atomic::{
    mi_atomic_addi64_relaxed, mi_atomic_maxi64_relaxed, mi_atomic_void_addi64_relaxed,
};
use crate::types::{Bin, StatsBin};

use super::{
    MI_BIN_HUGE, MI_STAT, MI_STAT_VERSION, mi_stat_count_t, mi_stat_counter_t, mi_stats_t,
};

/// # Safety
/// `stats` must point to a valid writable `mi_stats_t`.
pub unsafe fn mi_stats_header_init(stats: *mut mi_stats_t) {
    // SAFETY: caller guarantees `stats` is a valid writable `mi_stats_t`.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        (*stats).size = core::mem::size_of::<mi_stats_t>();
        (*stats).version = MI_STAT_VERSION;
    }
}

/// # Safety
/// `stats` must point to a valid writable `mi_stats_t`.
pub unsafe fn mi_stats_init(stats: *mut mi_stats_t) {
    // SAFETY: caller guarantees `stats` is a valid writable `mi_stats_t`.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        ptr::write_bytes(stats, 0, 1);
        mi_stats_header_init(stats);
    }
}

pub(super) fn stats_get_bin_size(bin: usize) -> usize {
    let Some(typed_bin) = u16::try_from(bin).ok().and_then(StatsBin::new) else {
        return 0;
    };
    mi_stats_get_bin_size_from_raw(bin, typed_bin).get()
}

fn mi_stats_get_bin_size_from_raw(bin: usize, typed_bin: StatsBin) -> StatBinSize {
    crate::internal::mi_debug_assert!(bin == typed_bin.as_usize());
    StatBinSize(_mi_bin_size(bin))
}

pub(super) fn mi_stats_get_bin_size_typed(bin: StatsBin) -> StatBinSize {
    StatBinSize(_mi_bin_size(stats_bin_as_bin(bin).as_usize()))
}

pub(super) fn initialized_stats() -> mi_stats_t {
    let mut stats = mi_stats_t::default();
    // SAFETY: `stats` is a fresh writable `mi_stats_t`.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { mi_stats_init(&mut stats) };
    stats
}

fn mi_stat_update_mt(stat: &mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    // SAFETY: `stat` points to valid `i64` fields and these helpers operate atomically on them.
    // SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
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

/// # Safety
/// `stat` must point to valid writable counter storage.
// SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_counter_increase_mt(
    stat: *mut mi_stat_counter_t,
    amount: usize,
) {
    // SAFETY: `stat.total` is a valid atomic-compatible `i64` storage location.
    // SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
    unsafe {
        mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), amount as i64);
    }
}

/// # Safety
/// `stat` must point to valid writable counter storage.
// SAFETY: local stats counter updates use caller-provided mutable counter storage.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_counter_increase(
    stat: *mut mi_stat_counter_t,
    amount: usize,
) {
    // SAFETY: local stats counter updates use caller-provided mutable counter storage.
    unsafe {
        (*stat).total += amount as i64;
    }
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_increase_mt(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
    mi_stat_update_mt(unsafe { &mut *stat }, amount as i64);
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: local stats counter updates use caller-provided mutable counter storage.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_increase(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: local stats counter updates use caller-provided mutable counter storage.
    mi_stat_update(unsafe { &mut *stat }, amount as i64);
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease_mt(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
    mi_stat_update_mt(unsafe { &mut *stat }, -(amount as i64));
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: local stats counter updates use caller-provided mutable counter storage.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: local stats counter updates use caller-provided mutable counter storage.
    mi_stat_update(unsafe { &mut *stat }, -(amount as i64));
}

fn mi_stat_adjust_mt(stat: &mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    // SAFETY: `stat` points to valid `i64` fields and these helpers operate atomically on them.
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
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

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: stats adjust helpers update current and total counters without peak tracking.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_increase_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
    mi_stat_adjust_mt(unsafe { &mut *stat }, amount as i64);
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: stats adjust helpers update current and total counters without peak tracking.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_increase(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
    mi_stat_adjust(unsafe { &mut *stat }, amount as i64);
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: stats adjust helpers update current and total counters without peak tracking.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_decrease_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
    mi_stat_adjust_mt(unsafe { &mut *stat }, -(amount as i64));
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: stats adjust helpers update current and total counters without peak tracking.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_decrease(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
    mi_stat_adjust(unsafe { &mut *stat }, -(amount as i64));
}

pub(super) fn mi_stat_count_add_mt(stat: *mut mi_stat_count_t, src: *const mi_stat_count_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
    // SAFETY: both pointers are valid for atomic-compatible `i64` field access.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
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
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        mi_atomic_void_addi64_relaxed(
            ptr::addr_of_mut!((*stat).total),
            ptr::addr_of!((*src).total),
        );
    }
}

// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[inline(never)]
unsafe extern "C" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t) {
    if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src) {
        return;
    }
    // SAFETY: null and self-add cases were checked above.
    // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
    unsafe { mi_stats_add_nonnull(stats, src) };
}

#[inline(never)]
unsafe fn mi_stats_add_nonnull(stats: *mut mi_stats_t, src: *const mi_stats_t) {
    macro_rules! MI_STAT_COUNT {
        ($field:ident) => {
            // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
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
            // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
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
        // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
        let stats_malloc_bins =
            unsafe { ptr::addr_of_mut!((*stats).malloc_bins).cast::<mi_stat_count_t>() };
        // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
        let src_malloc_bins =
            unsafe { ptr::addr_of!((*src).malloc_bins).cast::<mi_stat_count_t>() };
        for i in 0..=MI_BIN_HUGE {
            // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
            let bin = unsafe { StatsBin::new_unchecked(i as u16) };
            // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
            mi_stat_count_add_mt(unsafe { stats_malloc_bins.add(bin.as_usize()) }, unsafe {
                src_malloc_bins.add(bin.as_usize())
            });
        }
    }
    // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
    let stats_page_bins =
        unsafe { ptr::addr_of_mut!((*stats).page_bins).cast::<mi_stat_count_t>() };
    // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
    let src_page_bins = unsafe { ptr::addr_of!((*src).page_bins).cast::<mi_stat_count_t>() };
    for i in 0..=MI_BIN_HUGE {
        // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
        let bin = unsafe { StatsBin::new_unchecked(i as u16) };
        // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
        mi_stat_count_add_mt(unsafe { stats_page_bins.add(bin.as_usize()) }, unsafe {
            src_page_bins.add(bin.as_usize())
        });
    }
}

pub(super) fn mi_stats_add_into(to: &mut mi_stats_t, from: &mi_stats_t) {
    // SAFETY: references are valid `mi_stats_t` values; the checked raw helper preserves C's
    // null/self guard at the compatibility boundary.
    // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
    unsafe { mi_stats_add(to as *mut mi_stats_t, from as *const mi_stats_t) };
}

pub(super) fn mi_stats_copy(stats_to: *mut mi_stats_t, stats_from: *const mi_stats_t) -> bool {
    // SAFETY: callers must pass valid pointers when non-null; header checks gate the copy.
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
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

/// # Safety
/// `to` and `from` must point to valid writable `mi_stats_t` values.
// SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_stats_merge_into(to: *mut mi_stats_t, from: *mut mi_stats_t) {
    if to.is_null() || from.is_null() || core::ptr::eq(to, from) {
        return;
    }
    // SAFETY: caller guarantees valid pointers to `mi_stats_t` values.
    // SAFETY: raw stats merge bodies update validated counter fields and reset source buffers.
    unsafe {
        mi_stats_add_nonnull(to, from);
        _mi_memzero(from.cast(), size_of::<mi_stats_t>());
    }
}

#[derive(Clone, Copy)]
pub(super) struct StatBinSize(usize);

impl StatBinSize {
    #[inline(always)]
    pub(super) fn as_bytes(self) -> i64 {
        self.0 as i64
    }

    #[inline(always)]
    pub(super) fn get(self) -> usize {
        self.0
    }
}

#[inline(always)]
pub(super) fn stats_bin_iter() -> impl Iterator<Item = StatsBin> {
    (0..=MI_BIN_HUGE).map(|index| {
        crate::internal::mi_debug_assert!(u16::try_from(index).is_ok());
        // SAFETY: the range is bounded by `MI_BIN_HUGE`.
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        unsafe { StatsBin::new_unchecked(index as u16) }
    })
}

#[inline(always)]
fn stats_bin_as_bin(bin: StatsBin) -> Bin {
    // SAFETY: every stats bin is within mimalloc's regular bin table.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { Bin::new_unchecked(bin.get()) }
}
