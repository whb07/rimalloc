//! Benchmark-only hotspot counters.

use core::ffi::{c_char, c_ulonglong};
use core::fmt;
use core::fmt::Write as _;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::options::mi_option_t;

const COUNTER_COUNT: usize = 36;

#[repr(usize)]
#[derive(Clone, Copy)]
enum Counter {
    PageFreeCollectCalls = 0,
    PageFreeCollectForce,
    PageFreeCollectNoForce,
    PageFreeCollectLocalEmpty,
    PageFreeCollectMovedToFree,
    PageFreeCollectForceSplice,
    PageFreeCollectNoopLocalBlocked,
    PageFreeCollectPartlyCalls,
    PageFreeCollectPartlyNullHead,
    PageFreeCollectPartlyDetached,
    PageFreeCollectPartlyFinalCollect,
    FreeListExtendCalls,
    FreeListExtendBlocks,
    AbandonedReclaimAttempts,
    AbandonedReclaimSuccess,
    AbandonedReclaimFailThreadUninit,
    AbandonedReclaimFailNoTheap,
    AbandonedReclaimFailReclaimDisabled,
    AbandonedReclaimFailQueueLimit,
    ReabandonAttempts,
    ReabandonSuccess,
    ReabandonFailMostlyUsed,
    ReabandonFailNonArena,
    ReabandonFailAlreadyMapped,
    PageQueueSearches,
    PageQueueCandidatesScanned,
    PageQueueSuccess,
    PageQueueFailedExtend,
    BitmapFindClearAttempts,
    BitmapFindClearSuccess,
    BitmapFindClearFail,
    OptionReadsTotal,
    OptionReadsPageReclaimOnFree,
    OptionReadsPageMaxReclaim,
    OptionReadsPageCrossThreadMaxReclaim,
    OptionReadsPageMaxCandidates,
}

const COUNTER_NAMES: [&str; COUNTER_COUNT] = [
    "page_free_collect_calls",
    "page_free_collect_force",
    "page_free_collect_no_force",
    "page_free_collect_local_empty",
    "page_free_collect_moved_to_free",
    "page_free_collect_force_splice",
    "page_free_collect_noop_local_blocked",
    "page_free_collect_partly_calls",
    "page_free_collect_partly_null_head",
    "page_free_collect_partly_detached",
    "page_free_collect_partly_final_collect",
    "free_list_extend_calls",
    "free_list_extend_blocks",
    "abandoned_reclaim_attempts",
    "abandoned_reclaim_success",
    "abandoned_reclaim_fail_thread_uninit",
    "abandoned_reclaim_fail_no_theap",
    "abandoned_reclaim_fail_reclaim_disabled",
    "abandoned_reclaim_fail_queue_limit",
    "reabandon_attempts",
    "reabandon_success",
    "reabandon_fail_mostly_used",
    "reabandon_fail_non_arena",
    "reabandon_fail_already_mapped",
    "page_queue_searches",
    "page_queue_candidates_scanned",
    "page_queue_success",
    "page_queue_failed_extend",
    "bitmap_find_clear_attempts",
    "bitmap_find_clear_success",
    "bitmap_find_clear_fail",
    "option_reads_total",
    "option_reads_page_reclaim_on_free",
    "option_reads_page_max_reclaim",
    "option_reads_page_cross_thread_max_reclaim",
    "option_reads_page_max_candidates",
];

static COUNTERS: [AtomicU64; COUNTER_COUNT] = [const { AtomicU64::new(0) }; COUNTER_COUNT];

#[inline(always)]
fn inc(counter: Counter, amount: u64) {
    COUNTERS[counter as usize].fetch_add(amount, Ordering::Relaxed);
}

#[inline(always)]
pub(crate) fn page_free_collect_call(force: bool) {
    inc(Counter::PageFreeCollectCalls, 1);
    inc(
        if force {
            Counter::PageFreeCollectForce
        } else {
            Counter::PageFreeCollectNoForce
        },
        1,
    );
}

#[inline(always)]
pub(crate) fn page_free_collect_local_empty() {
    inc(Counter::PageFreeCollectLocalEmpty, 1);
}

#[inline(always)]
pub(crate) fn page_free_collect_moved_to_free(force_splice: bool) {
    inc(Counter::PageFreeCollectMovedToFree, 1);
    if force_splice {
        inc(Counter::PageFreeCollectForceSplice, 1);
    }
}

#[inline(always)]
pub(crate) fn page_free_collect_noop_local_blocked() {
    inc(Counter::PageFreeCollectNoopLocalBlocked, 1);
}

#[inline(always)]
pub(crate) fn page_free_collect_partly_call(head_is_null: bool) {
    inc(Counter::PageFreeCollectPartlyCalls, 1);
    if head_is_null {
        inc(Counter::PageFreeCollectPartlyNullHead, 1);
    }
}

#[inline(always)]
pub(crate) fn page_free_collect_partly_detached() {
    inc(Counter::PageFreeCollectPartlyDetached, 1);
}

#[inline(always)]
pub(crate) fn page_free_collect_partly_final_collect() {
    inc(Counter::PageFreeCollectPartlyFinalCollect, 1);
}

#[inline(always)]
pub(crate) fn free_list_extend(blocks: usize) {
    inc(Counter::FreeListExtendCalls, 1);
    inc(Counter::FreeListExtendBlocks, blocks as u64);
}

#[inline(always)]
pub(crate) fn abandoned_reclaim_attempt() {
    inc(Counter::AbandonedReclaimAttempts, 1);
}

#[inline(always)]
pub(crate) fn abandoned_reclaim_success() {
    inc(Counter::AbandonedReclaimSuccess, 1);
}

#[inline(always)]
pub(crate) fn abandoned_reclaim_fail_thread_uninit() {
    inc(Counter::AbandonedReclaimFailThreadUninit, 1);
}

#[inline(always)]
pub(crate) fn abandoned_reclaim_fail_no_theap() {
    inc(Counter::AbandonedReclaimFailNoTheap, 1);
}

#[inline(always)]
pub(crate) fn abandoned_reclaim_fail_reclaim_disabled() {
    inc(Counter::AbandonedReclaimFailReclaimDisabled, 1);
}

#[inline(always)]
pub(crate) fn abandoned_reclaim_fail_queue_limit() {
    inc(Counter::AbandonedReclaimFailQueueLimit, 1);
}

#[inline(always)]
pub(crate) fn reabandon_attempt() {
    inc(Counter::ReabandonAttempts, 1);
}

#[inline(always)]
pub(crate) fn reabandon_success() {
    inc(Counter::ReabandonSuccess, 1);
}

#[inline(always)]
pub(crate) fn reabandon_fail_mostly_used() {
    inc(Counter::ReabandonFailMostlyUsed, 1);
}

#[inline(always)]
pub(crate) fn reabandon_fail_non_arena() {
    inc(Counter::ReabandonFailNonArena, 1);
}

#[inline(always)]
pub(crate) fn reabandon_fail_already_mapped() {
    inc(Counter::ReabandonFailAlreadyMapped, 1);
}

#[inline(always)]
pub(crate) fn page_queue_search(candidates_scanned: usize, success: bool, failed_extend: bool) {
    inc(Counter::PageQueueSearches, 1);
    inc(
        Counter::PageQueueCandidatesScanned,
        candidates_scanned as u64,
    );
    if success {
        inc(Counter::PageQueueSuccess, 1);
    }
    if failed_extend {
        inc(Counter::PageQueueFailedExtend, 1);
    }
}

#[inline(always)]
pub(crate) fn bitmap_find_clear(result: bool) {
    inc(Counter::BitmapFindClearAttempts, 1);
    inc(
        if result {
            Counter::BitmapFindClearSuccess
        } else {
            Counter::BitmapFindClearFail
        },
        1,
    );
}

#[inline(always)]
pub(crate) fn option_read(option: mi_option_t) {
    inc(Counter::OptionReadsTotal, 1);
    let counter = if option == mi_option_t::mi_option_page_reclaim_on_free {
        Some(Counter::OptionReadsPageReclaimOnFree)
    } else if option == mi_option_t::mi_option_page_max_reclaim {
        Some(Counter::OptionReadsPageMaxReclaim)
    } else if option == mi_option_t::mi_option_page_cross_thread_max_reclaim {
        Some(Counter::OptionReadsPageCrossThreadMaxReclaim)
    } else if option == mi_option_t::mi_option_page_max_candidates {
        Some(Counter::OptionReadsPageMaxCandidates)
    } else {
        None
    };
    if let Some(counter) = counter {
        inc(counter, 1);
    }
}

#[repr(C)]
pub struct mi_bench_diagnostics_snapshot_t {
    pub values: [c_ulonglong; COUNTER_COUNT],
}

// SAFETY[AUDIT:bench-diagnostics-ffi]: the exported reset symbol has no
// caller-provided pointers and only resets diagnostic counters with relaxed
// stores.
#[unsafe(no_mangle)]
pub extern "C" fn mi_bench_diagnostics_reset() {
    for counter in &COUNTERS {
        counter.store(0, Ordering::Relaxed);
    }
}

/// # Safety
/// `out` must be null or point to writable storage for one snapshot.
// SAFETY[AUDIT:bench-diagnostics-ffi]: the exported snapshot symbol preserves
// the C ABI, rejects null output pointers, and writes exactly one snapshot when
// the caller provides writable storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_diagnostics_snapshot(
    out: *mut mi_bench_diagnostics_snapshot_t,
) -> bool {
    if out.is_null() {
        return false;
    }
    // SAFETY[AUDIT:bench-diagnostics-ffi]: `out` is non-null and this function's
    // safety contract requires writable storage for one snapshot.
    unsafe {
        for (idx, counter) in COUNTERS.iter().enumerate() {
            (*out).values[idx] = counter.load(Ordering::Relaxed) as c_ulonglong;
        }
    }
    true
}

struct JsonBuf {
    buf: *mut c_char,
    len: usize,
    written: usize,
    needed: usize,
}

impl JsonBuf {
    fn new(buf: *mut c_char, len: usize) -> Self {
        Self {
            buf,
            len,
            written: 0,
            needed: 0,
        }
    }

    fn finish(&mut self) -> usize {
        if !self.buf.is_null() && self.len > 0 {
            let nul_at = self.written.min(self.len - 1);
            // SAFETY[AUDIT:bench-diagnostics-ffi]: non-null `buf` with
            // `len > 0` provides at least one writable byte, and `nul_at` is
            // clamped below `len`.
            unsafe { *self.buf.add(nul_at) = 0 };
        }
        self.needed
    }
}

impl fmt::Write for JsonBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.needed += s.len();
        if self.buf.is_null() || self.len == 0 {
            return Ok(());
        }
        let cap = self.len - 1;
        let available = cap.saturating_sub(self.written);
        let copy_len = available.min(s.len());
        if copy_len > 0 {
            // SAFETY[AUDIT:bench-diagnostics-ffi]: `copy_len` is bounded by the
            // remaining writable capacity before the reserved NUL byte, and the
            // source `str` cannot overlap the caller-provided output buffer.
            unsafe {
                ptr::copy_nonoverlapping(
                    s.as_ptr(),
                    self.buf.cast::<u8>().add(self.written),
                    copy_len,
                );
            }
            self.written += copy_len;
        }
        Ok(())
    }
}

/// # Safety
/// If `buf` is non-null, it must point to `buf_len` writable bytes.
// SAFETY[AUDIT:bench-diagnostics-ffi]: the exported JSON symbol accepts a
// nullable caller buffer, writes at most `buf_len - 1` bytes when present, and
// always NUL-terminates non-empty buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_diagnostics_json(buf: *mut c_char, buf_len: usize) -> usize {
    let mut out = JsonBuf::new(buf, buf_len);
    let _ = out.write_char('{');
    for (idx, name) in COUNTER_NAMES.iter().enumerate() {
        if idx != 0 {
            let _ = out.write_char(',');
        }
        let value = COUNTERS[idx].load(Ordering::Relaxed);
        let _ = write!(out, "\"{name}\":{value}");
    }
    let _ = out.write_char('}');
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK.lock().expect("bench diagnostics test lock")
    }

    #[test]
    fn snapshot_rejects_null_and_copies_counter_values() {
        let _guard = test_lock();
        mi_bench_diagnostics_reset();

        let counter = Counter::ReabandonFailAlreadyMapped;
        let sentinel = u64::MAX / 4;
        COUNTERS[counter as usize].store(sentinel, Ordering::Relaxed);

        // SAFETY[AUDIT:bench-diagnostics-ffi]: null output is an explicitly
        // supported rejection path for the C ABI snapshot helper.
        assert!(!unsafe { mi_bench_diagnostics_snapshot(core::ptr::null_mut()) });

        let mut snapshot = mi_bench_diagnostics_snapshot_t {
            values: [0; COUNTER_COUNT],
        };
        // SAFETY[AUDIT:bench-diagnostics-ffi]: `snapshot` is writable storage
        // for exactly one C ABI snapshot.
        assert!(unsafe { mi_bench_diagnostics_snapshot(&mut snapshot) });
        assert!(snapshot.values[counter as usize] >= sentinel as c_ulonglong);

        mi_bench_diagnostics_reset();
    }

    #[test]
    fn json_reports_needed_length_and_nul_terminates_truncated_buffers() {
        let _guard = test_lock();
        mi_bench_diagnostics_reset();

        // SAFETY[AUDIT:bench-diagnostics-ffi]: null buffer with zero length is
        // the size-query path and performs no writes.
        let null_needed = unsafe { mi_bench_diagnostics_json(core::ptr::null_mut(), 0) };
        assert!(null_needed > COUNTER_COUNT);

        let mut zero_len = [0xA5_u8; 1];
        // SAFETY[AUDIT:bench-diagnostics-ffi]: a non-null buffer with zero
        // length is permitted and must not be written.
        let zero_needed =
            unsafe { mi_bench_diagnostics_json(zero_len.as_mut_ptr().cast::<c_char>(), 0) };
        assert!(zero_needed >= null_needed);
        assert_eq!(zero_len[0], 0xA5);

        let mut one_byte = [0xA5_u8; 1];
        // SAFETY[AUDIT:bench-diagnostics-ffi]: `one_byte` provides one
        // writable byte, which is reserved for the NUL terminator.
        let one_needed =
            unsafe { mi_bench_diagnostics_json(one_byte.as_mut_ptr().cast::<c_char>(), 1) };
        assert!(one_needed >= null_needed);
        assert_eq!(one_byte[0], 0);

        let mut small = [0xA5_u8; 8];
        // SAFETY[AUDIT:bench-diagnostics-ffi]: `small` is a valid writable
        // byte buffer and the writer must truncate before its trailing NUL.
        let needed =
            unsafe { mi_bench_diagnostics_json(small.as_mut_ptr().cast::<c_char>(), small.len()) };
        assert!(needed >= small.len());
        assert_eq!(&small[..7], b"{\"page_");
        assert_eq!(small[7], 0);

        mi_bench_diagnostics_reset();
    }
}
