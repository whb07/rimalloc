//! Statistics counter initialization, mutation, and aggregation.

use core::{mem::size_of, ptr};

use crate::allocator::init::MI_STATS_NULL;
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
/// - Pointer validity: `stats` must be non-null, properly aligned for
///   `mi_stats_t`, and valid for writes to `size` and `version`.
/// - Lifetime: `stats` must remain live for the duration of the call.
/// - Aliasing: `stats` must be exclusively writable for these header fields;
///   no concurrent access to `size`/`version` may occur.
/// - Initialization: storage must be allocated for `mi_stats_t` (may be
///   uninitialized beyond the header) and writable for `usize` header fields.
/// - Ownership: caller retains ownership; this function does not allocate or
///   free.
/// - Threading: caller must synchronize concurrent access to `stats`; no
///   internal synchronization is performed.
/// - Unwind: must not unwind.
pub unsafe fn mi_stats_header_init(stats: *mut mi_stats_t) {
    unsafe {
        (*stats).size = core::mem::size_of::<mi_stats_t>();
        (*stats).version = MI_STAT_VERSION;
    }
}

/// # Safety
/// - Pointer validity: `stats` must be non-null, aligned for `mi_stats_t`,
///   and valid for `size_of::<mi_stats_t>` bytes.
/// - Lifetime: `stats` must remain live for the whole call.
/// - Aliasing: `stats` must be exclusively writable; no other pointer may
///   access it concurrently.
/// - Initialization: allocated storage is zeroed before header initialization.
/// - Ownership: caller retains ownership.
/// - Threading: caller must prevent concurrent access to `stats`.
/// - Unwind: must not unwind.
pub unsafe fn mi_stats_init(stats: *mut mi_stats_t) {
    unsafe {
        ptr::write_bytes(stats, 0, 1);
    }
    unsafe {
        mi_stats_header_init(stats);
    }
}

pub(super) fn stats_get_bin_size(bin: usize) -> usize {
    u16::try_from(bin)
        .ok()
        .and_then(StatsBin::new)
        .map_or(0, mi_stats_get_bin_size_typed)
}

pub(super) fn mi_stats_get_bin_size_typed(bin: StatsBin) -> usize {
    _mi_bin_size(stats_bin_as_bin(bin).as_usize())
}

/// # Safety
/// - Pointer validity: `stat` must be non-null, aligned for
///   `mi_stat_count_t`, and valid for `current`/`peak`/`total`.
/// - Lifetime: `stat` must remain live for the call.
/// - Aliasing: no exclusive reference may alias `stat`; atomic field
///   projection via `addr_of_mut!` avoids creating `&mut`.
/// - Initialization: `stat` fields must be initialized atomic-compatible
///   `i64` storage.
/// - Ownership: caller retains ownership.
/// - Threading: every concurrent access to `stat` fields must use the atomic
///   statistics protocol (relaxed atomics).
/// - Unwind: must not unwind.
unsafe fn mi_stat_update_mt(stat: *mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
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
/// - Pointer validity: `stat` must be non-null, aligned for
///   `mi_stat_counter_t`, and valid for `total`.
/// - Lifetime: `stat` must remain live for the call.
/// - Aliasing: no concurrent exclusive access; atomic projection avoids
///   exclusive alias.
/// - Initialization: `total` must be atomic-compatible initialized storage.
/// - Ownership: caller retains ownership.
/// - Threading: all concurrent access must be atomic (relaxed).
/// - Unwind: must not unwind.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_counter_increase_mt(
    stat: *mut mi_stat_counter_t,
    amount: usize,
) {
    unsafe {
        mi_atomic_addi64_relaxed(ptr::addr_of_mut!((*stat).total), amount as i64);
    }
}

#[inline(never)]
pub(crate) extern "C" fn __mi_stat_counter_increase(stat: &mut mi_stat_counter_t, amount: usize) {
    stat.total += amount as i64;
}

/// # Safety
/// - Pointer validity: `stat` must be non-null, aligned for
///   `mi_stat_count_t`, and valid for count fields.
/// - Lifetime: live for the call.
/// - Aliasing: atomic projection without exclusive alias.
/// - Initialization: atomic-compatible fields.
/// - Ownership: caller retains ownership.
/// - Threading: all concurrent access must use atomic protocol.
/// - Unwind: must not unwind.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_increase_mt(stat: *mut mi_stat_count_t, amount: usize) {
    unsafe { mi_stat_update_mt(stat, amount as i64) };
}

#[inline(never)]
pub(crate) extern "C" fn __mi_stat_increase(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_update(stat, amount as i64);
}

/// # Safety
/// - Pointer validity: `stat` non-null, aligned, atomic-compatible count
///   storage.
/// - Lifetime: live for call.
/// - Aliasing: atomic projection.
/// - Initialization: atomic-compatible fields.
/// - Ownership: caller retains ownership.
/// - Threading: atomic protocol for all concurrent access.
/// - Unwind: must not unwind.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_decrease_mt(stat: *mut mi_stat_count_t, amount: usize) {
    unsafe { mi_stat_update_mt(stat, -(amount as i64)) };
}

#[inline(never)]
pub(crate) extern "C" fn __mi_stat_decrease(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_update(stat, -(amount as i64));
}

/// # Safety
/// - Pointer validity: `stat` non-null, aligned, atomic-compatible.
/// - Lifetime: live.
/// - Aliasing: atomic projection avoids exclusive alias.
/// - Initialization: atomic-compatible `current`/`total`.
/// - Ownership: caller retains.
/// - Threading: atomic protocol.
/// - Unwind: must not unwind.
unsafe fn mi_stat_adjust_mt(stat: *mut mi_stat_count_t, amount: i64) {
    if amount == 0 {
        return;
    }
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
/// - Pointer validity: `stat` must be non-null, aligned for
///   `mi_stat_count_t`, and valid for `current`/`total`.
/// - Lifetime: live for the call.
/// - Aliasing: atomic projection without exclusive alias.
/// - Initialization: atomic-compatible fields.
/// - Ownership: caller retains ownership.
/// - Threading: all concurrent access must use the atomic protocol; this
///   helper updates `current` and `total` without peak tracking.
/// - Unwind: must not unwind.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_increase_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    unsafe { mi_stat_adjust_mt(stat, amount as i64) };
}

/// # Safety
/// - Pointer validity: `stat` non-null, aligned, atomic-compatible count
///   storage for `current`/`total`.
/// - Lifetime: live.
/// - Aliasing: atomic projection.
/// - Initialization: atomic-compatible fields.
/// - Ownership: caller retains.
/// - Threading: atomic protocol.
/// - Unwind: must not unwind.
#[inline(never)]
pub(crate) unsafe extern "C" fn __mi_stat_adjust_decrease_mt(
    stat: *mut mi_stat_count_t,
    amount: usize,
) {
    unsafe { mi_stat_adjust_mt(stat, -(amount as i64)) };
}

#[inline(never)]
pub(crate) extern "C" fn __mi_stat_adjust_decrease(stat: &mut mi_stat_count_t, amount: usize) {
    mi_stat_adjust(stat, -(amount as i64));
}

/// # Safety
/// - Pointer validity: `stat` and `src` must be non-null, aligned for
///   `mi_stat_count_t`, and valid for their fields.
/// - Lifetime: both pointers must remain live for the call.
/// - Aliasing: `stat` must be writable and distinct-checked; `src` is read
///   via `addr_of!` without creating a shared reference.
/// - Initialization: both counters must be atomic-compatible initialized
///   storage.
/// - Ownership: caller retains both; no transfer.
/// - Threading: every concurrent access to either counter must use the atomic
///   statistics protocol; loads are relaxed and need not be coherent across
///   fields.
/// - Unwind: must not unwind.
pub(super) unsafe fn mi_stat_count_add_mt(stat: *mut mi_stat_count_t, src: *const mi_stat_count_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
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
/// - Pointer validity: `stat` and `src` non-null, aligned for
///   `mi_stat_counter_t`, valid for `total`.
/// - Lifetime: both live for the call.
/// - Aliasing: `stat` writable, `src` read via `addr_of!`.
/// - Initialization: atomic-compatible.
/// - Ownership: caller retains.
/// - Threading: atomic protocol for concurrent access.
/// - Unwind: must not unwind.
unsafe fn mi_stat_counter_add_mt(stat: *mut mi_stat_counter_t, src: *const mi_stat_counter_t) {
    if ptr::eq(stat.cast_const(), src) {
        return;
    }
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
/// - Pointer validity: `stats` and `src` must be null-checked before use;
///   when non-null they must be valid, aligned `mi_stats_t` storage; `stats`
///   writable.
/// - Lifetime: both must remain live for the call.
/// - Aliasing: `stats` and `src` must not overlap when both non-null; caller
///   checks self-add.
/// - Initialization: `mi_stats_t` instances must be initialized header-wise but
///   counters are atomic-compatible.
/// - Ownership: caller retains.
/// - Threading: each counter is either exclusively stable or accessed
///   concurrently only via atomic stats protocol.
/// - Unwind: must not unwind.
#[inline(never)]
pub(super) unsafe extern "C" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t) {
    if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src) {
        return;
    }
    macro_rules! add_stat_fields {
        ($($kind:ident $field:ident,)*) => {
            $(add_stat_fields!(@ $kind $field);)*
        };
        (@ count $field:ident) => {
            unsafe {
                mi_stat_count_add_mt(
                    ptr::addr_of_mut!((*stats).$field),
                    ptr::addr_of!((*src).$field),
                )
            }
        };
        (@ counter $field:ident) => {
            unsafe {
                mi_stat_counter_add_mt(
                    ptr::addr_of_mut!((*stats).$field),
                    ptr::addr_of!((*src).$field),
                )
            }
        };
    }
    mi_stat_fields!(add_stat_fields);

    if MI_STAT > 1 {
        let stats_malloc_bins =
            unsafe { ptr::addr_of_mut!((*stats).malloc_bins).cast::<mi_stat_count_t>() };
        let src_malloc_bins =
            unsafe { ptr::addr_of!((*src).malloc_bins).cast::<mi_stat_count_t>() };
        for i in 0..=MI_BIN_HUGE {
            let bin = unsafe { StatsBin::new_unchecked(i as u16) };
            unsafe {
                mi_stat_count_add_mt(
                    stats_malloc_bins.add(bin.as_usize()),
                    src_malloc_bins.add(bin.as_usize()),
                )
            };
        }
    }
    let stats_page_bins =
        unsafe { ptr::addr_of_mut!((*stats).page_bins).cast::<mi_stat_count_t>() };
    let src_page_bins = unsafe { ptr::addr_of!((*src).page_bins).cast::<mi_stat_count_t>() };
    for i in 0..=MI_BIN_HUGE {
        let bin = unsafe { StatsBin::new_unchecked(i as u16) };
        unsafe {
            mi_stat_count_add_mt(
                stats_page_bins.add(bin.as_usize()),
                src_page_bins.add(bin.as_usize()),
            )
        };
    }
}

/// # Safety
/// - Pointer validity: `stats_to` non-null aligned writable `mi_stats_t`;
///   `stats_from` may be null or live aligned initialized stats.
/// - Lifetime: `stats_to` live for write; `stats_from` live if non-null.
/// - Aliasing: `stats_to` exclusive for final `ptr::write`.
/// - Initialization: `stats_to` size/version validated; source immutable
///   size field.
/// - Ownership: caller retains; no allocation.
/// - Threading: source counters either exclusive or atomic protocol.
/// - Unwind: must not unwind.
pub(super) unsafe fn mi_stats_copy(
    stats_to: *mut mi_stats_t,
    stats_from: *const mi_stats_t,
) -> bool {
    if stats_to.is_null() {
        return false;
    }
    let (to_size, to_version) = unsafe { ((*stats_to).size, (*stats_to).version) };
    if to_size != size_of::<mi_stats_t>() || to_version != MI_STAT_VERSION || stats_from.is_null() {
        return false;
    }
    let from_size = unsafe { (*stats_from).size };
    if from_size != to_size {
        return false;
    }

    let snapshot = unsafe { mi_stats_snapshot_atomic(stats_from) };
    unsafe { ptr::write(stats_to, snapshot) };
    true
}

/// # Safety
/// - Pointer validity: `stats_from` non-null aligned initialized `mi_stats_t`.
/// - Lifetime: live for snapshot duration.
/// - Aliasing: source read via `addr_of!`, no exclusive alias needed.
/// - Initialization: source header and counter fields initialized.
/// - Ownership: caller retains.
/// - Threading: counters either exclusive or atomic protocol during snapshot.
/// - Unwind: must not unwind.
pub(super) unsafe fn mi_stats_snapshot_atomic(stats_from: *const mi_stats_t) -> mi_stats_t {
    let (from_size, from_version) = unsafe { ((*stats_from).size, (*stats_from).version) };
    let mut snapshot = MI_STATS_NULL();
    snapshot.size = from_size;
    snapshot.version = from_version;
    macro_rules! snapshot_stat_fields {
        ($($kind:ident $field:ident,)*) => {
            $(snapshot_stat_fields!(@ $kind $field);)*
        };
        (@ count $field:ident) => {
            snapshot.$field =
                unsafe { mi_stat_count_snapshot(ptr::addr_of!((*stats_from).$field)) };
        };
        (@ counter $field:ident) => {
            snapshot.$field =
                unsafe { mi_stat_counter_snapshot(ptr::addr_of!((*stats_from).$field)) };
        };
    }
    mi_stat_fields!(snapshot_stat_fields);

    for i in 0..snapshot._stat_reserved.len() {
        snapshot._stat_reserved[i] = unsafe {
            mi_stat_count_snapshot(
                ptr::addr_of!((*stats_from)._stat_reserved)
                    .cast::<mi_stat_count_t>()
                    .add(i),
            )
        };
    }
    for i in 0..snapshot._stat_counter_reserved.len() {
        snapshot._stat_counter_reserved[i] = unsafe {
            mi_stat_counter_snapshot(
                ptr::addr_of!((*stats_from)._stat_counter_reserved)
                    .cast::<mi_stat_counter_t>()
                    .add(i),
            )
        };
    }
    for i in 0..=MI_BIN_HUGE {
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
/// - Pointer validity: `src` non-null, aligned for `mi_stat_count_t`, valid
///   for `total`/`peak`/`current`.
/// - Lifetime: live for this operation.
/// - Aliasing: reads via `addr_of!` without shared alias.
/// - Initialization: atomic-compatible initialized fields.
/// - Ownership: caller retains.
/// - Threading: either exclusive or atomic protocol for concurrent access.
/// - Unwind: must not unwind.
unsafe fn mi_stat_count_snapshot(src: *const mi_stat_count_t) -> mi_stat_count_t {
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
/// - Pointer validity: `src` non-null, aligned for `mi_stat_counter_t`.
/// - Lifetime: live.
/// - Aliasing: `addr_of!` read.
/// - Initialization: atomic-compatible `total`.
/// - Ownership: caller retains.
/// - Threading: exclusive or atomic protocol.
/// - Unwind: must not unwind.
unsafe fn mi_stat_counter_snapshot(src: *const mi_stat_counter_t) -> mi_stat_counter_t {
    mi_stat_counter_t {
        total: unsafe {
            mi_atomic_loadi64_explicit(
                ptr::addr_of!((*src).total),
                mi_memory_order_e::mi_memory_order_relaxed,
            )
        },
    }
}

/// # Safety
/// - Pointer validity: `to` must be non-null, aligned for `mi_stats_t`,
///   valid for all counters; `from` must be non-null, aligned, writable.
/// - Lifetime: both remain live for the call.
/// - Aliasing: `to` and `from` must not alias; `to` is atomic-compatible,
///   `from` is exclusively owned for zeroing.
/// - Initialization: both `mi_stats_t` are initialized with valid headers.
/// - Ownership: caller retains both; `from` is reset to zero on return.
/// - Threading: `to` counters may be concurrently updated only via atomic
///   protocol; `from` must have no concurrent access.
/// - Unwind: must not unwind.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_stats_merge_into(to: *mut mi_stats_t, from: *mut mi_stats_t) {
    crate::internal::mi_assert_internal!(!to.is_null() && !from.is_null());
    if core::ptr::eq(to, from) {
        return;
    }
    unsafe {
        mi_stats_add(to, from);
    }
    unsafe {
        _mi_memzero(from.cast(), size_of::<mi_stats_t>());
    }
}

#[inline(always)]
pub(super) fn stats_bin_iter() -> impl Iterator<Item = StatsBin> {
    (0..=MI_BIN_HUGE).map(|index| {
        crate::internal::mi_debug_assert!(u16::try_from(index).is_ok());
        unsafe { StatsBin::new_unchecked(index as u16) }
    })
}

#[inline(always)]
fn stats_bin_as_bin(bin: StatsBin) -> Bin {
    unsafe { Bin::new_unchecked(bin.get()) }
}

#[cfg(test)]
mod tests {
    use core::{cell::UnsafeCell, ptr};
    use std::sync::Arc;

    use super::{
        __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt, __mi_stat_decrease_mt,
        __mi_stat_increase_mt, MI_STATS_NULL, mi_stat_count_t, mi_stats_copy,
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
        let source = UnsafeCell::new(MI_STATS_NULL());
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
        let mut snapshot = MI_STATS_NULL();

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
        let source = UnsafeCell::new(MI_STATS_NULL());
        let source_raw = source.get();
        // SAFETY: this is the fixture's only raw pointer and initialization precedes the snapshot.
        unsafe { (*source_raw).version = super::MI_STAT_VERSION.wrapping_add(1) };
        let mut snapshot = MI_STATS_NULL();

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
        let source = UnsafeCell::new(MI_STATS_NULL());
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
