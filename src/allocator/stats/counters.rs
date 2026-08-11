//! Statistics counter initialization, mutation, and aggregation.

use core::{mem::size_of, ptr};

use crate::allocator::page::_mi_bin_size;
use crate::internal::_mi_memzero;
use crate::internal::atomic::{
    mi_atomic_addi64_relaxed, mi_atomic_loadi64_explicit, mi_atomic_maxi64_relaxed,
    mi_memory_order_e,
};
use crate::types::{Bin, StatsBin};

use super::{
    MI_BIN_HUGE, MI_CHUNKBIN_COUNT, MI_STAT, MI_STAT_VERSION, mi_stat_count_t, mi_stat_counter_t,
    mi_stats_t,
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

/// # Safety
/// `stat` must point to live, aligned atomic-compatible count storage, and every concurrent access
/// to its fields must use the atomic statistics protocol.
unsafe fn mi_stat_update_mt(stat: *mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    // SAFETY: `stat` points to valid atomic-compatible `i64` fields. Raw field
    // projection avoids creating an exclusive reference to shared storage.
    unsafe {
        let current = mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).current), amount);
        mi_atomic_maxi64_relaxed(ptr::addr_of_mut!((*stat).peak), current + amount);
        if amount > 0 {
            mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), amount);
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
/// `stat` must point to live, correctly aligned, atomic-compatible counter
/// storage, and all concurrent access to its fields must be atomic.
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
/// `stat` must point to live, correctly aligned, atomic-compatible count
/// storage, and all concurrent access to its fields must be atomic.
// SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_increase_mt(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
    // SAFETY: the caller provides atomic-compatible storage under the MT access protocol.
    unsafe { mi_stat_update_mt(stat, amount as i64) };
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
/// `stat` must point to live, correctly aligned, atomic-compatible count
/// storage, and all concurrent access to its fields must be atomic.
// SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease_mt(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: multithreaded stats counter updates use atomic-compatible counter fields.
    // SAFETY: the caller provides atomic-compatible storage under the MT access protocol.
    unsafe { mi_stat_update_mt(stat, -(amount as i64)) };
}

/// # Safety
/// `stat` must point to valid writable count storage.
// SAFETY: local stats counter updates use caller-provided mutable counter storage.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease(stat: *mut mi_stat_count_t, amount: usize) {
    // SAFETY: local stats counter updates use caller-provided mutable counter storage.
    mi_stat_update(unsafe { &mut *stat }, -(amount as i64));
}

/// # Safety
/// `stat` must point to live, aligned atomic-compatible count storage, and every concurrent access
/// to its fields must use the atomic statistics protocol.
unsafe fn mi_stat_adjust_mt(stat: *mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
    // SAFETY: `stat` points to valid atomic-compatible `i64` fields. Raw field
    // projection avoids creating an exclusive reference to shared storage.
    unsafe {
        mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).current), amount);
        mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), amount);
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
/// `stat` must point to live, correctly aligned, atomic-compatible count
/// storage, and all concurrent access to its fields must be atomic.
// SAFETY: stats adjust helpers update current and total counters without peak tracking.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_increase_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
    // SAFETY: the caller provides atomic-compatible storage under the MT access protocol.
    unsafe { mi_stat_adjust_mt(stat, amount as i64) };
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
/// `stat` must point to live, correctly aligned, atomic-compatible count
/// storage, and all concurrent access to its fields must be atomic.
// SAFETY: stats adjust helpers update current and total counters without peak tracking.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_decrease_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    // SAFETY: stats adjust helpers update current and total counters without peak tracking.
    // SAFETY: the caller provides atomic-compatible storage under the MT access protocol.
    unsafe { mi_stat_adjust_mt(stat, -(amount as i64)) };
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

/// # Safety
/// `stat` and `src` must point to live, aligned atomic-compatible count storage. `stat` must be
/// writable, and every concurrent access to either counter must use the atomic stats protocol.
pub(super) unsafe fn mi_stat_count_add_mt(stat: *mut mi_stat_count_t, src: *const mi_stat_count_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
    // SAFETY: the contract keeps both counters live and atomic-compatible. Each relaxed source
    // load is race-free; values from different fields need not form one coherent instant.
    unsafe {
        let total = mi_atomic_loadi64_explicit(
            ptr::addr_of!((*src).total),
            mi_memory_order_e::mi_memory_order_relaxed,
        );
        if total != 0 {
            mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), total);
        }
        let current = mi_atomic_loadi64_explicit(
            ptr::addr_of!((*src).current),
            mi_memory_order_e::mi_memory_order_relaxed,
        );
        let peak = mi_atomic_loadi64_explicit(
            ptr::addr_of!((*src).peak),
            mi_memory_order_e::mi_memory_order_relaxed,
        );
        let prev_current = mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).current), current);
        mi_atomic_maxi64_relaxed(ptr::addr_of_mut!((*stat).peak), prev_current + peak);
    }
}

/// # Safety
/// `stat` and `src` must point to live, aligned atomic-compatible counter storage. `stat` must be
/// writable, and every concurrent access to either counter must use the atomic stats protocol.
unsafe fn mi_stat_counter_add_mt(stat: *mut mi_stat_counter_t, src: *const mi_stat_counter_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
    // SAFETY: the contract keeps both counters live and atomic-compatible; the relaxed source load
    // and conditional destination update use the same protocol as concurrent statistics updates.
    unsafe {
        let total = mi_atomic_loadi64_explicit(
            ptr::addr_of!((*src).total),
            mi_memory_order_e::mi_memory_order_relaxed,
        );
        if total != 0 {
            mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), total);
        }
    }
}

/// # Safety
/// Non-null pointers must address live, aligned `mi_stats_t` storage; `stats` must be writable.
/// During this operation, each source/destination counter must either be exclusively stable or have
/// all concurrent access use the allocator's atomic statistics protocol.
#[inline(never)]
unsafe extern "C" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t) {
    if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src) {
        return;
    }
    // SAFETY: the function contract supplies live atomic-compatible source and destination fields;
    // null and self-add cases were handled above.
    unsafe { mi_stats_add_nonnull(stats, src) };
}

/// # Safety
/// `stats` and `src` must address distinct live, aligned `mi_stats_t` values; `stats` must be
/// writable. During this operation, each counter must either be exclusively stable or have all
/// concurrent access use the allocator's atomic statistics protocol.
#[inline(never)]
unsafe fn mi_stats_add_nonnull(stats: *mut mi_stats_t, src: *const mi_stats_t) {
    macro_rules! MI_STAT_COUNT {
        ($field:ident) => {
            // SAFETY: the enclosing contract covers this corresponding count-field projection.
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
            // SAFETY: the enclosing contract covers this corresponding counter-field projection.
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
            // SAFETY: the enclosing stats contract covers the corresponding initialized bin
            // counters, and `bin` is within both fixed-size arrays.
            unsafe {
                mi_stat_count_add_mt(
                    stats_malloc_bins.add(bin.as_usize()),
                    src_malloc_bins.add(bin.as_usize()),
                )
            };
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
        // SAFETY: the enclosing stats contract covers the corresponding initialized bin counters,
        // and `bin` is within both fixed-size arrays.
        unsafe {
            mi_stat_count_add_mt(
                stats_page_bins.add(bin.as_usize()),
                src_page_bins.add(bin.as_usize()),
            )
        };
    }
}

/// # Safety
/// `to` and `from` must point to live, aligned `mi_stats_t` values and `to` must be writable. During
/// this operation, each counter must either be exclusively stable or have all concurrent access use
/// the allocator's atomic statistics protocol.
pub(super) unsafe fn mi_stats_add_into(to: *mut mi_stats_t, from: *const mi_stats_t) {
    // SAFETY: the caller supplies the raw atomic-compatible source and destination contracts.
    unsafe { mi_stats_add(to, from) };
}

/// # Safety
/// `stats_to` must be null or point to a live, aligned, exclusively writable `mi_stats_t`.
/// `stats_from` must be null or point to live, aligned statistics storage. During this operation,
/// every source counter must either be exclusively stable or have all concurrent access use the
/// allocator's atomic statistics protocol. Neither allocation may be invalidated during this call.
pub(super) unsafe fn mi_stats_copy(
    stats_to: *mut mi_stats_t,
    stats_from: *const mi_stats_t,
) -> bool {
    if stats_to.is_null() {
        return false;
    }
    // SAFETY: the destination is caller-provided readable storage until the final exclusive write.
    let (to_size, to_version) = unsafe { ((*stats_to).size, (*stats_to).version) };
    if to_size != size_of::<mi_stats_t>() || to_version != MI_STAT_VERSION || stats_from.is_null() {
        return false;
    }
    // SAFETY: size is immutable after source publication and the source remains live.
    let from_size = unsafe { (*stats_from).size };
    if from_size != to_size {
        return false;
    }

    // SAFETY: validation above established the non-null live source; its counters satisfy the
    // function's atomic-or-exclusive access contract.
    let snapshot = unsafe { mi_stats_snapshot_atomic(stats_from) };
    // SAFETY: the contract makes the destination exclusive for this final initialized value write.
    unsafe { ptr::write(stats_to, snapshot) };
    true
}

/// # Safety
/// `stats_from` must point to live, aligned initialized statistics storage. During this operation,
/// every source counter must either be exclusively stable or have all concurrent access use the
/// allocator's atomic statistics protocol. The source may not be invalidated during the snapshot.
pub(super) unsafe fn mi_stats_snapshot_atomic(stats_from: *const mi_stats_t) -> mi_stats_t {
    // SAFETY: the source header is immutable after publication and remains live under the contract.
    let (from_size, from_version) = unsafe { ((*stats_from).size, (*stats_from).version) };
    let mut snapshot = initialized_stats();
    snapshot.size = from_size;
    snapshot.version = from_version;
    macro_rules! MI_STAT_COUNT {
        ($field:ident) => {
            // SAFETY: the source field is live atomic-compatible storage under the function
            // contract; the returned value is written into the fresh exclusive snapshot.
            snapshot.$field =
                unsafe { mi_stat_count_snapshot(ptr::addr_of!((*stats_from).$field)) };
        };
    }
    macro_rules! MI_STAT_COUNTER {
        ($field:ident) => {
            // SAFETY: the source field is live atomic-compatible storage under the function
            // contract; the returned value is written into the fresh exclusive snapshot.
            snapshot.$field =
                unsafe { mi_stat_counter_snapshot(ptr::addr_of!((*stats_from).$field)) };
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

    for i in 0..snapshot._stat_reserved.len() {
        // SAFETY: `i` is in both fixed-size reserved-count arrays and the source remains live.
        snapshot._stat_reserved[i] = unsafe {
            mi_stat_count_snapshot(
                ptr::addr_of!((*stats_from)._stat_reserved)
                    .cast::<mi_stat_count_t>()
                    .add(i),
            )
        };
    }
    for i in 0..snapshot._stat_counter_reserved.len() {
        // SAFETY: `i` is in both fixed-size reserved-counter arrays and the source remains live.
        snapshot._stat_counter_reserved[i] = unsafe {
            mi_stat_counter_snapshot(
                ptr::addr_of!((*stats_from)._stat_counter_reserved)
                    .cast::<mi_stat_counter_t>()
                    .add(i),
            )
        };
    }
    for i in 0..=MI_BIN_HUGE {
        // SAFETY: `i` is in both fixed-size malloc/page bin arrays and the source remains live.
        unsafe {
            snapshot.malloc_bins[i] = mi_stat_count_snapshot(
                ptr::addr_of!((*stats_from).malloc_bins)
                    .cast::<mi_stat_count_t>()
                    .add(i),
            );
            snapshot.page_bins[i] = mi_stat_count_snapshot(
                ptr::addr_of!((*stats_from).page_bins)
                    .cast::<mi_stat_count_t>()
                    .add(i),
            );
        }
    }
    for i in 0..MI_CHUNKBIN_COUNT {
        // SAFETY: `i` is in the fixed-size chunk-bin array and the source remains live.
        snapshot.chunk_bins[i] = unsafe {
            mi_stat_count_snapshot(
                ptr::addr_of!((*stats_from).chunk_bins)
                    .cast::<mi_stat_count_t>()
                    .add(i),
            )
        };
    }
    snapshot
}

/// # Safety
/// `src` must point to live, aligned count storage. During this operation it must either be
/// exclusively stable or have all concurrent field access use the atomic statistics protocol.
unsafe fn mi_stat_count_snapshot(src: *const mi_stat_count_t) -> mi_stat_count_t {
    // SAFETY: the contract covers all three atomic-compatible source field projections.
    unsafe {
        mi_stat_count_t {
            total: mi_atomic_loadi64_explicit(
                ptr::addr_of!((*src).total),
                mi_memory_order_e::mi_memory_order_relaxed,
            ),
            peak: mi_atomic_loadi64_explicit(
                ptr::addr_of!((*src).peak),
                mi_memory_order_e::mi_memory_order_relaxed,
            ),
            current: mi_atomic_loadi64_explicit(
                ptr::addr_of!((*src).current),
                mi_memory_order_e::mi_memory_order_relaxed,
            ),
        }
    }
}

/// # Safety
/// `src` must point to live, aligned counter storage. During this operation it must either be
/// exclusively stable or have all concurrent field access use the atomic statistics protocol.
unsafe fn mi_stat_counter_snapshot(src: *const mi_stat_counter_t) -> mi_stat_counter_t {
    mi_stat_counter_t {
        total: unsafe {
            // SAFETY: the contract covers this atomic-compatible source total-field projection.
            mi_atomic_loadi64_explicit(
                ptr::addr_of!((*src).total),
                mi_memory_order_e::mi_memory_order_relaxed,
            )
        },
    }
}

#[cfg(test)]
mod tests {
    use core::{cell::UnsafeCell, ptr};
    use std::sync::Arc;

    use super::{
        __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt, __mi_stat_decrease_mt,
        __mi_stat_increase_mt, initialized_stats, mi_stat_count_t, mi_stats_copy,
        mi_stats_snapshot_atomic,
    };

    struct SharedStatCount(UnsafeCell<mi_stat_count_t>);

    // SAFETY: concurrent access in this test is restricted to the MT helpers,
    // which access each counter field atomically. Reads occur only after join.
    unsafe impl Sync for SharedStatCount {}

    #[test]
    fn mt_stat_updates_share_atomic_fields() {
        const THREADS: usize = 2;
        const ITERATIONS: usize = 8;

        let stat = Arc::new(SharedStatCount(UnsafeCell::new(mi_stat_count_t::default())));
        std::thread::scope(|scope| {
            for _ in 0..THREADS {
                let stat = Arc::clone(&stat);
                scope.spawn(move || {
                    for _ in 0..ITERATIONS {
                        // SAFETY: `stat` remains live through the scoped join and
                        // all concurrent access uses the atomic MT helper.
                        unsafe { __mi_stat_increase_mt(stat.0.get(), 1) };
                    }
                });
            }
        });

        // SAFETY: both scoped threads have joined, so no concurrent access remains.
        let stat = unsafe { &*stat.0.get() };
        let expected = (THREADS * ITERATIONS) as i64;
        assert_eq!(stat.current, expected);
        assert_eq!(stat.total, expected);
        assert_eq!(stat.peak, expected);
    }

    #[test]
    fn mt_stat_adjustments_update_the_intended_fields() {
        let mut stat = mi_stat_count_t::default();
        let stat_ptr = ptr::addr_of_mut!(stat);

        // SAFETY: `stat_ptr` designates live, exclusively owned counter storage.
        unsafe {
            __mi_stat_adjust_increase_mt(stat_ptr, 3);
            __mi_stat_increase_mt(stat_ptr, 2);
            __mi_stat_decrease_mt(stat_ptr, 1);
            __mi_stat_adjust_decrease_mt(stat_ptr, 1);
        }

        assert_eq!(stat.current, 3);
        assert_eq!(stat.total, 4);
        assert_eq!(stat.peak, 5);
    }

    #[test]
    fn atomic_snapshot_covers_reserved_and_all_bin_families() {
        let source = UnsafeCell::new(initialized_stats());
        let source_raw = source.get();
        // SAFETY: this is the fixture's only raw pointer and all initialization completes before
        // the atomic snapshot starts; no overlapping reference is created.
        unsafe {
            (*source_raw)._stat_reserved[2] = mi_stat_count_t {
                current: 11,
                total: 13,
                peak: 17,
            };
            (*source_raw)._stat_counter_reserved[1].total = 19;
            (*source_raw).malloc_bins[3] = mi_stat_count_t {
                current: 23,
                total: 29,
                peak: 31,
            };
            (*source_raw).page_bins[4].total = 37;
            (*source_raw).chunk_bins[1] = mi_stat_count_t {
                current: 41,
                total: 43,
                peak: 47,
            };
        }
        let mut snapshot = initialized_stats();

        // SAFETY: both values are live and aligned; the destination is exclusive and the source
        // has no concurrent access while the snapshot helper performs its atomic field loads.
        assert!(unsafe { mi_stats_copy(ptr::addr_of_mut!(snapshot), source_raw.cast_const()) });

        assert_eq!(snapshot._stat_reserved[2].total, 13);
        assert_eq!(snapshot._stat_counter_reserved[1].total, 19);
        assert_eq!(snapshot.malloc_bins[3].peak, 31);
        assert_eq!(snapshot.page_bins[4].total, 37);
        assert_eq!(snapshot.chunk_bins[1].current, 41);
    }

    #[test]
    fn atomic_snapshot_preserves_source_header_version() {
        let source = UnsafeCell::new(initialized_stats());
        let source_raw = source.get();
        // SAFETY: this is the fixture's only raw pointer and initialization precedes the snapshot.
        unsafe { (*source_raw).version = super::MI_STAT_VERSION.wrapping_add(1) };
        let mut snapshot = initialized_stats();

        // SAFETY: both values are live and aligned; the destination is exclusive and the source
        // has no concurrent access while its header and fields are sampled.
        assert!(unsafe { mi_stats_copy(ptr::addr_of_mut!(snapshot), source_raw.cast_const()) });
        // SAFETY: the snapshot has completed and no mutation or other access overlaps this read.
        let source = unsafe { &*source_raw };
        assert_eq!(snapshot.size, source.size);
        assert_eq!(snapshot.version, source.version);
    }

    #[test]
    fn atomic_snapshot_accepts_legacy_zero_header_for_rendering() {
        let source = UnsafeCell::new(initialized_stats());
        let source_raw = source.get();
        // SAFETY: this is the fixture's only raw pointer and initialization precedes the snapshot.
        unsafe {
            (*source_raw).size = 0;
            (*source_raw).version = 0;
            (*source_raw).malloc_normal.total = 7;
        }

        // SAFETY: the source remains live and exclusive, with raw provenance suitable for the
        // atomic-compatible field loads used by allocator rendering.
        let snapshot = unsafe { mi_stats_snapshot_atomic(source_raw.cast_const()) };
        assert_eq!(snapshot.size, 0);
        assert_eq!(snapshot.version, 0);
        assert_eq!(snapshot.malloc_normal.total, 7);
    }
}

/// # Safety
/// `to` must point to live, correctly aligned, atomic-compatible `mi_stats_t`
/// storage, and all concurrent access to its fields must be atomic. `from`
/// must point to live, writable `mi_stats_t` storage that is exclusively owned
/// for the duration of the merge and reset.
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
