//! Benchmark-only hotspot counters.

use core::ffi::{c_char, c_ulonglong, c_void};
use core::fmt;
use core::fmt::Write as _;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::options::mi_option_t;
use crate::types::{mi_block_t, mi_page_t};

const COUNTER_COUNT: usize = 37;

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
    RemoteFreeCasRetries,
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
    "remote_free_cas_retries",
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
pub(crate) fn remote_free_cas_retry() {
    inc(Counter::RemoteFreeCasRetries, 1);
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

const PAGE_BATCH_FORCE: u8 = 0;
const PAGE_BATCH_LOCAL: u8 = 1;
const PAGE_BATCH_REMOTE: u8 = 2;
const PAGE_BATCH_MAX_LEN: usize = 128;
const PAGE_BATCH_MAX_DESTINATION: usize = 3;

struct PageBatchBenchFixture {
    page_storage: *mut mi_page_t,
    blocks: *mut mi_block_t,
    page: crate::core_types::PagePtr,
    len: usize,
    destination_len: usize,
    kind: u8,
}

impl PageBatchBenchFixture {
    unsafe fn allocate(kind: u8, len: usize, destination_len: usize) -> *mut Self {
        if kind > PAGE_BATCH_REMOTE
            || len == 0
            || len > PAGE_BATCH_MAX_LEN
            || destination_len > PAGE_BATCH_MAX_DESTINATION
        {
            return ptr::null_mut();
        }
        let Some(thread_id) = crate::core_types::ThreadId::new(crate::prim::_mi_prim_thread_id())
        else {
            return ptr::null_mut();
        };
        let block_count = len + destination_len;
        // SAFETY[AUDIT:bench-page-batch-fixture]: benchmark fixture storage is
        // allocated before timing, checked for null, and released by destroy.
        let page_storage =
            unsafe { crate::mimalloc::mi_zalloc(mem::size_of::<mi_page_t>()) }.cast::<mi_page_t>();
        let blocks =
            unsafe { crate::mimalloc::mi_zalloc(block_count * mem::size_of::<mi_block_t>()) }
                .cast::<mi_block_t>();
        // SAFETY[AUDIT:bench-page-batch-fixture]: the opaque fixture allocation
        // is checked below and initialized exactly once before publication.
        let fixture_storage =
            unsafe { crate::mimalloc::mi_zalloc(mem::size_of::<Self>()) }.cast::<Self>();
        if page_storage.is_null() || blocks.is_null() || fixture_storage.is_null() {
            // SAFETY[AUDIT:bench-page-batch-fixture]: each value is either null
            // or an allocation returned above and has not been published.
            unsafe {
                crate::mimalloc::mi_free(fixture_storage.cast());
                crate::mimalloc::mi_free(blocks.cast());
                crate::mimalloc::mi_free(page_storage.cast());
            }
            return ptr::null_mut();
        }
        // SAFETY[AUDIT:bench-page-batch-fixture]: all storage is live and
        // uniquely owned; the page fields initialize exactly the batch paths.
        unsafe {
            let page = crate::core_types::PagePtr::new_unchecked(page_storage);
            (*page_storage).capacity = block_count as u16;
            (*page_storage).reserved = block_count as u16;
            (*page_storage).block_size = mem::size_of::<mi_block_t>();
            (*page_storage).page_start = blocks.cast();
            page.set_keys(crate::core_types::PageKeys::disabled());
            page.thread_id_cell()
                .store_owner(crate::core_types::PageOwnerId::Thread(thread_id));
            ptr::write(
                fixture_storage,
                Self {
                    page_storage,
                    blocks,
                    page,
                    len,
                    destination_len,
                    kind,
                },
            );
        }
        fixture_storage
    }

    fn block(&self, slot: usize) -> crate::core_types::BlockPtr {
        // SAFETY[AUDIT:bench-page-batch-fixture]: callers bound slots by the
        // configured batch plus destination lengths.
        unsafe { crate::core_types::BlockPtr::new_unchecked(self.blocks.add(slot)) }
    }

    unsafe fn link_range(&self, start: usize, len: usize) {
        if len == 0 {
            return;
        }
        for slot in start + 1..start + len {
            // SAFETY[AUDIT:bench-page-batch-fixture]: adjacent blocks belong to
            // this fixture page and use its page-local encoding keys.
            unsafe {
                self.block(slot - 1)
                    .set_next(self.page, Some(self.block(slot)))
            };
        }
        // SAFETY[AUDIT:bench-page-batch-fixture]: the bounded range tail is
        // writable fixture storage and terminates the detached chain.
        unsafe { self.block(start + len - 1).set_next(self.page, None) };
    }

    unsafe fn prepare(&self) {
        // SAFETY[AUDIT:bench-page-batch-fixture]: each range is disjoint,
        // page-homogeneous, and bounded by the allocated fixture storage.
        unsafe {
            self.link_range(0, self.len);
            self.link_range(self.len, self.destination_len);
        }
        let destination = if self.destination_len == 0 {
            None
        } else {
            Some(self.block(self.len))
        };
        // SAFETY[AUDIT:bench-page-batch-fixture]: preparation runs outside the
        // timed region with exclusive fixture access and restores accounting.
        unsafe {
            self.page.set_used(crate::core_types::UsedBlockCount::new(
                if self.kind == PAGE_BATCH_REMOTE {
                    self.len as u16
                } else {
                    0
                },
            ));
            if self.kind == PAGE_BATCH_REMOTE {
                self.page
                    .set_free(crate::core_types::AvailableFreeList::empty());
                self.page.set_local_free(match destination {
                    Some(head) => crate::core_types::LocalFreeList::from_head(head),
                    None => crate::core_types::LocalFreeList::empty(),
                });
            } else {
                self.page
                    .set_local_free(crate::core_types::LocalFreeList::from_head(self.block(0)));
                self.page.set_free(match destination {
                    Some(head) => crate::core_types::AvailableFreeList::from_head(head),
                    None => crate::core_types::AvailableFreeList::empty(),
                });
            }
        }
    }

    unsafe fn run(&self) -> bool {
        if self.kind == PAGE_BATCH_FORCE {
            // SAFETY[AUDIT:bench-page-batch-transition]: the synthetic page has a
            // valid local list and exclusive current-thread ownership.
            unsafe { crate::page::_mi_page_free_collect(self.page_storage, true) };
            return true;
        }
        // SAFETY[AUDIT:bench-page-batch-transition]: preparation assigns this
        // synthetic page exclusively to the current benchmark thread.
        let owner = unsafe { crate::core_types::assume_local_page(self.page) };
        if self.kind == PAGE_BATCH_LOCAL {
            // SAFETY[AUDIT:bench-page-batch-transition]: the prepared local list
            // and destination are valid page-homogeneous chains.
            let Ok(Some(batch)) = (unsafe { owner.detach_local_free_batch() }) else {
                return false;
            };
            // SAFETY[AUDIT:bench-page-batch-transition]: the available destination
            // is a valid list owned by the exclusive fixture page.
            let available = unsafe { self.page.free() };
            // SAFETY[AUDIT:bench-page-batch-transition]: the detached batch and
            // available destination belong to the same exclusive fixture page.
            let available = unsafe { batch.prepend_to_available(&owner, available) };
            // SAFETY[AUDIT:bench-page-batch-transition]: the splice result is the
            // fixture page's new uniquely owned available list.
            unsafe { self.page.set_free(available) };
            true
        } else {
            // SAFETY[AUDIT:bench-page-batch-transition]: the prepared remote chain
            // is detached, nonempty, capacity-bounded, and counted in used.
            let Ok(batch) = (unsafe {
                crate::core_types::RemoteFreeBatch::from_detached(
                    self.page,
                    self.block(0),
                    self.page.capacity(),
                    self.page.used(),
                )
            }) else {
                return false;
            };
            // SAFETY[AUDIT:bench-page-batch-transition]: the local destination
            // was prepared as a valid page-homogeneous fixture chain.
            let local = unsafe { self.page.local_free() };
            // SAFETY[AUDIT:bench-page-batch-transition]: the detached remote
            // batch and local destination share exclusive fixture ownership.
            let _ = unsafe { batch.collect_into_local_page(&owner, local) };
            true
        }
    }
}

/// Create an opaque page-batch slow-path benchmark fixture.
// SAFETY[AUDIT:bench-page-batch-ffi]: the returned handle is either null or a
// uniquely owned fixture released by `mi_bench_page_batch_fixture_destroy`.
#[unsafe(no_mangle)]
pub extern "C" fn mi_bench_page_batch_fixture_new(
    kind: u8,
    len: usize,
    destination_len: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:bench-page-batch-ffi]: the constructor validates all scalar
    // bounds before allocating or initializing the opaque fixture.
    unsafe { PageBatchBenchFixture::allocate(kind, len, destination_len).cast() }
}

/// Restore an opaque fixture outside the benchmark timed region.
///
/// # Safety
/// `fixture` must be null or a live handle returned by the matching constructor,
/// and operations on a non-null handle must be serialized.
// SAFETY[AUDIT:bench-page-batch-ffi]: callers must pass a live handle returned
// by the matching constructor and serialize prepare/run/destroy operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_page_batch_fixture_prepare(fixture: *mut c_void) -> bool {
    // SAFETY[AUDIT:bench-page-batch-ffi]: the function contract requires a
    // live fixture handle for the duration of this shared borrow.
    let Some(fixture) = (unsafe { fixture.cast::<PageBatchBenchFixture>().as_ref() }) else {
        return false;
    };
    unsafe { fixture.prepare() };
    true
}

/// Run one prepared page-batch slow-path transition.
///
/// # Safety
/// `fixture` must be a live, prepared handle with no concurrent operation on
/// the same fixture.
// SAFETY[AUDIT:bench-page-batch-ffi]: callers must pass a live prepared handle
// and must not race another operation on the same fixture.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_page_batch_fixture_run(fixture: *mut c_void) -> bool {
    // SAFETY[AUDIT:bench-page-batch-ffi]: the function contract requires a
    // live fixture handle for the duration of this shared borrow.
    let Some(fixture) = (unsafe { fixture.cast::<PageBatchBenchFixture>().as_ref() }) else {
        return false;
    };
    unsafe { fixture.run() }
}

/// Destroy an opaque page-batch fixture outside the timed region.
///
/// # Safety
/// `fixture` must be null or a live unique handle and must not be used after a
/// non-null handle is destroyed.
// SAFETY[AUDIT:bench-page-batch-ffi]: callers must pass a live unique handle or
// null and must not use a non-null handle after this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_page_batch_fixture_destroy(fixture: *mut c_void) {
    let fixture = fixture.cast::<PageBatchBenchFixture>();
    if fixture.is_null() {
        return;
    }
    // SAFETY[AUDIT:bench-page-batch-ffi]: the handle uniquely owns all three
    // allocator-backed storage regions and none carry drop glue.
    unsafe {
        let page_storage = (*fixture).page_storage;
        let blocks = (*fixture).blocks;
        crate::mimalloc::mi_free(blocks.cast());
        crate::mimalloc::mi_free(page_storage.cast());
        crate::mimalloc::mi_free(fixture.cast());
    }
}

#[cfg(test)]
pub(crate) fn option_read_counts_for_tests() -> (u64, u64, u64, u64, u64) {
    let load = |counter: Counter| COUNTERS[counter as usize].load(Ordering::Relaxed);
    (
        load(Counter::OptionReadsTotal),
        load(Counter::OptionReadsPageReclaimOnFree),
        load(Counter::OptionReadsPageMaxReclaim),
        load(Counter::OptionReadsPageCrossThreadMaxReclaim),
        load(Counter::OptionReadsPageMaxCandidates),
    )
}

#[cfg(test)]
pub(crate) mod test_support {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) fn lock() -> std::sync::MutexGuard<'static, ()> {
        LOCK.lock().expect("bench diagnostics test lock")
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
/// Both pointers must be non-null and writable for one `usize`.
// SAFETY[AUDIT:bench-occupancy-ffi]: this benchmark-only export validates
// both output pointers and derives occupancy from a local statistics snapshot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_bench_page_occupancy(
    used_bytes_out: *mut usize,
    capacity_bytes_out: *mut usize,
) -> bool {
    if used_bytes_out.is_null() || capacity_bytes_out.is_null() {
        return false;
    }
    let mut stats = crate::stats::mi_stats_t::default();
    // SAFETY[AUDIT:bench-occupancy-ffi]: `stats` is fresh local writable
    // storage and thread initialization takes no caller pointers.
    unsafe {
        crate::stats::mi_stats_init(ptr::addr_of_mut!(stats));
        crate::init::mi_thread_init();
        if !crate::stats::mi_stats_get(ptr::addr_of_mut!(stats)) {
            return false;
        }
    }
    let mut used_bytes = 0usize;
    let mut capacity_bytes = 0usize;
    for bin in 0..crate::types::MI_BIN_HUGE {
        let block_size = crate::stats::mi_stats_get_bin_size(bin);
        let page_size = if block_size <= crate::page::MI_SMALL_MAX_OBJ_SIZE {
            crate::types::MI_SMALL_PAGE_SIZE
        } else if block_size <= crate::page::MI_MEDIUM_MAX_OBJ_SIZE {
            crate::types::MI_MEDIUM_PAGE_SIZE
        } else if block_size <= crate::page::MI_LARGE_MAX_OBJ_SIZE {
            crate::types::MI_LARGE_PAGE_SIZE
        } else {
            0
        };
        let blocks = stats.malloc_bins[bin].current.max(0) as usize;
        let pages = stats.page_bins[bin].current.max(0) as usize;
        used_bytes = used_bytes.saturating_add(blocks.saturating_mul(block_size));
        capacity_bytes = capacity_bytes.saturating_add(pages.saturating_mul(page_size));
    }
    // SAFETY[AUDIT:bench-occupancy-ffi]: both pointers were checked non-null
    // and the function contract requires writable output storage.
    unsafe {
        used_bytes_out.write(used_bytes.min(capacity_bytes));
        capacity_bytes_out.write(capacity_bytes);
    }
    capacity_bytes != 0
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

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        test_support::lock()
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
    fn remote_free_retry_counter_is_reported() {
        let _guard = test_lock();
        mi_bench_diagnostics_reset();
        remote_free_cas_retry();
        let mut snapshot = mi_bench_diagnostics_snapshot_t {
            values: [0; COUNTER_COUNT],
        };
        // SAFETY[AUDIT:bench-diagnostics-ffi]: `snapshot` is writable storage
        // for exactly one diagnostic snapshot.
        assert!(unsafe { mi_bench_diagnostics_snapshot(&mut snapshot) });
        assert_eq!(snapshot.values[Counter::RemoteFreeCasRetries as usize], 1);
        mi_bench_diagnostics_reset();
    }

    #[test]
    fn page_occupancy_rejects_null_and_returns_bounded_totals() {
        let _guard = test_lock();
        let mut used = usize::MAX;
        let mut capacity = usize::MAX;
        // SAFETY[AUDIT:bench-occupancy-ffi]: null outputs exercise the
        // documented rejection paths; valid outputs below are local storage.
        unsafe {
            assert!(!mi_bench_page_occupancy(
                core::ptr::null_mut(),
                &mut capacity
            ));
            assert!(!mi_bench_page_occupancy(&mut used, core::ptr::null_mut()));
            let available = mi_bench_page_occupancy(&mut used, &mut capacity);
            assert_eq!(available, capacity != 0);
        }
        assert!(used <= capacity);
    }

    #[test]
    fn page_batch_fixture_rejects_invalid_inputs_and_runs_each_transition() {
        assert!(mi_bench_page_batch_fixture_new(PAGE_BATCH_LOCAL, 0, 0).is_null());
        assert!(mi_bench_page_batch_fixture_new(PAGE_BATCH_REMOTE + 1, 1, 0).is_null());
        // SAFETY[AUDIT:bench-page-batch-ffi]: null is the documented rejection
        // or no-op path for each opaque-handle function.
        unsafe {
            assert!(!mi_bench_page_batch_fixture_prepare(ptr::null_mut()));
            assert!(!mi_bench_page_batch_fixture_run(ptr::null_mut()));
            mi_bench_page_batch_fixture_destroy(ptr::null_mut());
        }

        for kind in [PAGE_BATCH_FORCE, PAGE_BATCH_LOCAL, PAGE_BATCH_REMOTE] {
            let fixture = mi_bench_page_batch_fixture_new(kind, 8, 3);
            assert!(!fixture.is_null());
            // SAFETY[AUDIT:bench-page-batch-ffi]: the constructor returned a
            // unique live handle used serially and destroyed exactly once.
            unsafe {
                assert!(mi_bench_page_batch_fixture_prepare(fixture));
                assert!(mi_bench_page_batch_fixture_run(fixture));
                mi_bench_page_batch_fixture_destroy(fixture);
            }
        }
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
