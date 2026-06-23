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

#[unsafe(no_mangle)]
pub extern "C" fn mi_bench_diagnostics_reset() {
    for counter in &COUNTERS {
        counter.store(0, Ordering::Relaxed);
    }
}

/// # Safety
/// `out` must be null or point to writable storage for one snapshot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_diagnostics_snapshot(
    out: *mut mi_bench_diagnostics_snapshot_t,
) -> bool {
    if out.is_null() {
        return false;
    }
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
