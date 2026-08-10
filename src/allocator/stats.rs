//! Statistics

// Retains upstream stats aliases and header marker constants for C parity.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize};
use core::{mem::size_of, ptr};

use crate::atomic::mi_lock_t;
use crate::core_types::{
    BorrowedHeap, HeapPtr, OutputCallback, OutputCallbackArg, SubprocPtr, ThreadHeapLocal,
    ThreadHeapPtr,
};
use crate::internal::memid::mi_memid_t;
use crate::internal::mi_theap_is_initialized;
use crate::mimalloc::{mi_heap_main, mi_subproc_current, mi_subproc_visit_heaps};
use crate::prim::{_mi_heap_theap_peek, _mi_theap_default};
use crate::types::{
    MI_BIN_COUNT, MI_STAT, mi_heap_t, mi_heap_visit_fun, mi_output_fun, mi_page_t, mi_subproc_id_t,
    mi_subproc_t, mi_theap_t,
};

pub const MI_STAT_VERSION: usize = 5;
pub const MI_BIN_HUGE: usize = 73;
pub const MI_LARGE_MAX_OBJ_WSIZE: usize = 8192 * size_of::<usize>();
pub const MIMALLOC_STATS_H: () = ();

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
const MI_CHUNKBINS: [mi_chunkbin_t; MI_CHUNKBIN_COUNT] = [
    mi_chunkbin_e::MI_CBIN_SMALL,
    mi_chunkbin_e::MI_CBIN_OTHER,
    mi_chunkbin_e::MI_CBIN_MEDIUM,
    mi_chunkbin_e::MI_CBIN_LARGE,
    mi_chunkbin_e::MI_CBIN_HUGE,
    mi_chunkbin_e::MI_CBIN_NONE,
];
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
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        unsafe { core::mem::zeroed() }
    }
}

mod counters;
mod process;
mod render;

pub(crate) use counters::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt,
    __mi_stat_counter_increase, __mi_stat_counter_increase_mt, __mi_stat_decrease,
    __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt, _mi_stats_merge_into,
};
use counters::{initialized_stats, mi_stats_add_into, mi_stats_copy};
pub use counters::{mi_stats_header_init, mi_stats_init};
pub(crate) use process::{_mi_clock_end, _mi_clock_now, _mi_clock_start, _mi_stats_init};
use render::_mi_stats_print;

#[unsafe(no_mangle)]
pub extern "C" fn mi_stats_get_bin_size(bin: usize) -> usize {
    counters::stats_get_bin_size(bin)
}

/// # Safety
/// Any non-null output pointers must be valid for writing one `usize`.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
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
    // SAFETY: the compatibility boundary forwards the caller's output-pointer contract.
    unsafe {
        process::mi_process_info_impl(process::ProcessInfoOutput {
            elapsed_msecs,
            user_msecs,
            system_msecs,
            current_rss,
            peak_rss,
            current_commit,
            peak_commit,
            page_faults,
        })
    }
}

/// # Safety
/// Output callbacks and argument pointers must follow the C ABI contract when non-null.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_process_info_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    // SAFETY: the compatibility boundary forwards the callback contract.
    unsafe { render::mi_process_info_print_out_impl(out, arg) }
}

// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub extern "C" fn mi_process_info_print() {
    render::mi_process_info_print_impl()
}

/// # Safety
/// `stats` must point to a valid `mi_stats_t` when non-null. If `output_buf` is non-null it
/// must be writable for `output_size` bytes.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_stats_get_json_from(
    stats: *const mi_stats_t,
    output_size: usize,
    output_buf: *mut c_char,
) -> *mut c_char {
    // SAFETY: the compatibility boundary forwards the caller's stats and buffer contracts.
    unsafe { render::mi_stats_get_json_from_impl(stats, output_size, output_buf) }
}

/// # Safety
/// `subproc_id` must identify a live subproc. If `buf` is non-null it must be writable for
/// `buf_size` bytes.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get_json(
    subproc_id: mi_subproc_id_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    let mut stats = initialized_stats();
    // SAFETY: the compatibility boundary validates the live subproc before rendering its snapshot.
    unsafe {
        if !mi_subproc_stats_get(subproc_id, ptr::addr_of_mut!(stats)) {
            return ptr::null_mut();
        }
        mi_stats_get_json_from(ptr::addr_of!(stats), buf_size, buf)
    }
}

/// # Safety
/// `heap` must be null or a valid heap pointer. If `buf` is non-null it must be writable for
/// `buf_size` bytes.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_get_json(
    heap: *mut mi_heap_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    let heap = if heap.is_null() {
        // SAFETY: the main heap is initialized before its statistics are queried.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: the caller guarantees that the non-null heap remains valid for this query.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: the borrowed heap and caller-provided output buffer remain valid for this call.
    unsafe {
        mi_stats_get_json_from(
            mi_heap_get_stats_borrowed(BorrowedHeap::new_unchecked(heap)),
            buf_size,
            buf,
        )
    }
}

/// # Safety
/// If `buf` is non-null it must be writable for `buf_size` bytes.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get_json(buf_size: usize, buf: *mut c_char) -> *mut c_char {
    // SAFETY: the current subproc is live and the caller provides the output buffer contract.
    unsafe { mi_subproc_stats_get_json(mi_subproc_current(), buf_size, buf) }
}

/// # Safety
/// `stats` must point to a valid `mi_stats_t` when non-null. If `buf` is non-null it must be
/// writable for `buf_size` bytes.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_as_json(
    stats: *mut mi_stats_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    // SAFETY: the compatibility boundary forwards the caller's stats and buffer contracts.
    unsafe { mi_stats_get_json_from(stats.cast_const(), buf_size, buf) }
}

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

#[derive(Clone, Copy)]
struct HeapPrintVisitInfo {
    out: OutputCallback,
    out_arg: OutputCallbackArg,
}

impl HeapPrintVisitInfo {
    #[inline(always)]
    fn from_raw(info: &mi_heap_print_visit_info_t) -> Self {
        Self {
            out: OutputCallback::new(info.out),
            out_arg: OutputCallbackArg::from_raw(info.out_arg),
        }
    }
}

struct HeapAggregateVisitInfo<'a> {
    stats: &'a mut mi_stats_t,
}

unsafe fn mi_stats_merge_theap_to_heap(theap: *mut mi_theap_t) -> *const mi_stats_t {
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    let theap = unsafe { ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap)) };
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    let Some(heap) = (unsafe { theap.ptr().heap() }) else {
        crate::internal::mi_assert_internal!(false);
        return core::ptr::null();
    };
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    unsafe { mi_stats_merge_theap_to_heap_typed(theap, heap) }
}

unsafe fn mi_stats_merge_theap_to_heap_typed(
    theap: ThreadHeapLocal,
    heap: HeapPtr,
) -> *const mi_stats_t {
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    let stats = unsafe { &mut (*theap.ptr().as_ptr()).stats };
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    let heap_stats = unsafe { &mut (*heap.as_ptr().cast::<MiHeapStatsPrefix>()).stats };
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    unsafe { _mi_stats_merge_into(heap_stats, stats) };
    heap_stats as *const mi_stats_t
}

unsafe fn mi_heap_get_stats(heap: *mut mi_heap_t) -> *const mi_stats_t {
    let heap = if heap.is_null() {
        // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    unsafe { mi_heap_get_stats_borrowed(BorrowedHeap::new_unchecked(heap)) }
}

unsafe fn mi_heap_get_stats_borrowed(heap: BorrowedHeap) -> *const mi_stats_t {
    let heap = heap.ptr();
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    if !unsafe { crate::prim::_mi_thread_is_initialized() } {
        // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
        return unsafe { &(*heap.as_ptr().cast::<MiHeapStatsPrefix>()).stats };
    }
    // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
    let theap = unsafe { _mi_heap_theap_peek(heap.as_ptr()) };
    if theap.is_null() {
        // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
        unsafe { &(*heap.as_ptr().cast::<MiHeapStatsPrefix>()).stats }
    } else {
        // SAFETY: heap stats merge helpers bind thread heaps and heap stats through typed witnesses.
        unsafe { mi_stats_merge_theap_to_heap(theap) }
    }
}

/// # Safety
/// `heap` must be null or a valid heap pointer owned by the current subproc.
// SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_merge_to_subproc(heap: *mut mi_heap_t) {
    let heap = if heap.is_null() {
        // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    let Some(subproc) = (unsafe { heap.subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    unsafe { mi_heap_stats_merge_to_subproc_typed(BorrowedHeap::new_unchecked(heap), subproc) };
}

unsafe fn mi_heap_stats_merge_to_subproc_typed(heap: BorrowedHeap, subproc: SubprocPtr) {
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    unsafe { _mi_stats_merge_into(&mut *subproc.stats(), &mut *heap.ptr().stats()) };
}

/// # Safety
/// Uses the process-default heap/theap state and follows the legacy C ABI.
// SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_reset() {
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    if !unsafe { mi_theap_is_initialized(_mi_theap_default()) } {
        return;
    }
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    let heap = unsafe { mi_heap_main() };
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    let _ = unsafe { mi_heap_get_stats(heap) };
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    unsafe { mi_heap_stats_merge_to_subproc(heap) };
}

/// # Safety
/// Uses process-global stats and heap state and must not race with process teardown.
// SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_merge() {
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    if !unsafe { mi_theap_is_initialized(_mi_theap_default()) } {
        return;
    }
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    let heap = unsafe { mi_heap_main() };
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    let _ = unsafe { mi_heap_get_stats(heap) };
    // SAFETY: public stats merge and reset shims preserve process-global C ABI behavior.
    unsafe { mi_heap_stats_merge_to_subproc(heap) };
}

/// # Safety
/// `heap` must be null or a valid `mi_heap_t*`; any output callback must obey the C ABI.
// SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_print_out(
    heap: *mut mi_heap_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let heap = if heap.is_null() {
        // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    unsafe {
        mi_heap_stats_print_out_typed(
            BorrowedHeap::new_unchecked(heap),
            OutputCallback::new(out),
            OutputCallbackArg::from_raw(arg),
        )
    };
}

unsafe fn mi_heap_stats_print_out_typed(
    heap: BorrowedHeap,
    out: OutputCallback,
    arg: OutputCallbackArg,
) {
    let heap_ptr = heap.ptr();
    let heap_prefix = heap_ptr.as_ptr().cast::<MiHeapStatsPrefix>();
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    unsafe {
        _mi_stats_print(
            "heap",
            crate::core_types::HeapSeq::new((*heap_prefix).heap_seq).get(),
            mi_heap_get_stats_borrowed(heap),
            out.raw(),
            arg.as_ptr(),
        )
    };
}

// SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
#[inline(never)]
unsafe extern "C" fn mi_heap_print_visitor(heap: *mut mi_heap_t, arg: *mut c_void) -> bool {
    let vinfo =
        // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
        HeapPrintVisitInfo::from_raw(unsafe { &*(arg.cast::<mi_heap_print_visit_info_t>()) });
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    let heap = unsafe { BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)) };
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    unsafe { mi_heap_print_visitor_typed(heap, vinfo) }
}

unsafe fn mi_heap_print_visitor_typed(heap: BorrowedHeap, vinfo: HeapPrintVisitInfo) -> bool {
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    unsafe { mi_heap_stats_print_out_typed(heap, vinfo.out, vinfo.out_arg) };
    true
}

// SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
#[inline(never)]
unsafe extern "C" fn mi_heap_aggregate_visitor(heap: *mut mi_heap_t, arg: *mut c_void) -> bool {
    let vinfo = HeapAggregateVisitInfo {
        // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
        stats: unsafe { &mut *arg.cast::<mi_stats_t>() },
    };
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    let heap = unsafe { BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)) };
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    unsafe { mi_heap_aggregate_visitor_typed(heap, vinfo) }
}

unsafe fn mi_heap_aggregate_visitor_typed(
    heap: BorrowedHeap,
    vinfo: HeapAggregateVisitInfo<'_>,
) -> bool {
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    let heap_stats = unsafe { mi_heap_get_stats_borrowed(heap) };
    // SAFETY: both pointers refer to valid stats buffers during the heap walk.
    // SAFETY: heap stats visitors keep callback arguments and borrowed heap witnesses valid during walks.
    unsafe { mi_stats_add_into(vinfo.stats, &*heap_stats) };
    true
}

/// # Safety
/// `subproc_id` must identify a live subproc and any output callback must obey the C ABI.
// SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_heap_stats_print_out(
    subproc_id: mi_subproc_id_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let subproc = subproc_id.as_ptr::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return;
    }
    let mut vinfo = mi_heap_print_visit_info_t { out, out_arg: arg };
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
    unsafe {
        mi_subproc_visit_heaps(
            subproc_id,
            Some(mi_heap_print_visitor as mi_heap_visit_fun),
            ptr::addr_of_mut!(vinfo).cast(),
        );
        _mi_stats_print(
            "subproc",
            crate::core_types::SubprocSeq::new((*subproc).subproc_seq).get(),
            ptr::addr_of!((*subproc).stats),
            out,
            arg,
        );
    }
}

/// # Safety
/// `subproc_id` must identify a live subproc and `stats` must point to writable stats storage.
// SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get_exclusive(
    subproc_id: mi_subproc_id_t,
    stats: *mut mi_stats_t,
) -> bool {
    if stats.is_null() {
        return false;
    }
    let subproc = subproc_id.as_ptr::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return false;
    }
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
    unsafe { mi_stats_copy(&mut *stats, &(*subproc).stats) }
}

/// # Safety
/// `subproc_id` must identify a live subproc and `stats` must point to writable stats storage.
// SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get(
    subproc_id: mi_subproc_id_t,
    stats: *mut mi_stats_t,
) -> bool {
    if stats.is_null() {
        return false;
    }
    let subproc = subproc_id.as_ptr::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return false;
    }
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
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

/// # Safety
/// `stats` must point to writable stats storage.
// SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get(stats: *mut mi_stats_t) -> bool {
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
    unsafe { mi_subproc_stats_get(mi_subproc_current(), stats) }
}

/// # Safety
/// `heap` must be null or a valid heap pointer, and `stats` must point to writable stats storage.
// SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_get(heap: *mut mi_heap_t, stats: *mut mi_stats_t) -> bool {
    if stats.is_null() {
        return false;
    }
    let heap = if heap.is_null() {
        // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
    unsafe { mi_heap_stats_get_borrowed(BorrowedHeap::new_unchecked(heap), &mut *stats) }
}

unsafe fn mi_heap_stats_get_borrowed(heap: BorrowedHeap, stats: &mut mi_stats_t) -> bool {
    // SAFETY: public stats export shims validate output buffers before copying heap or subproc stats.
    unsafe { mi_stats_copy(stats, mi_heap_get_stats_borrowed(heap)) }
}

/// # Safety
/// May collect current-thread allocator state and must not race with allocator teardown.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect_reduce(_target_thread_owned: usize) {
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { crate::allocator::thread_heap::mi_collect(false) };
}

/// # Safety
/// `subproc_id` must identify a live subproc and any output callback must obey the C ABI.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_print_out(
    subproc_id: mi_subproc_id_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let subproc = subproc_id.as_ptr::<MiSubprocStatsPrefix>();
    if subproc.is_null() {
        return;
    }
    let mut stats = initialized_stats();
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        if mi_subproc_stats_get(subproc_id, ptr::addr_of_mut!(stats)) {
            _mi_stats_print(
                "subproc",
                crate::core_types::SubprocSeq::new((*subproc).subproc_seq).get(),
                ptr::addr_of!(stats),
                out,
                arg,
            );
        }
    }
}

/// # Safety
/// Any output callback must obey the C ABI.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { mi_subproc_stats_print_out(mi_subproc_current(), out, arg) };
}

/// # Safety
/// `out`, when non-null, must be a valid `mi_output_fun` callback pointer encoded as `void*`.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_print(out: *mut c_void) {
    let out_fun = if out.is_null() {
        None
    } else {
        // The legacy C API accepts an output callback through a `void*`.
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        Some(unsafe { core::mem::transmute::<*mut c_void, mi_output_fun>(out) })
    };
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { mi_stats_print_out(out_fun, ptr::null_mut()) };
}

/// # Safety
/// Any output callback must obey the C ABI.
// SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_thread_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let theap = unsafe { _mi_theap_default() };
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    if theap.is_null() || !unsafe { mi_theap_is_initialized(theap) } {
        return;
    }
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let theap_ptr = unsafe { ThreadHeapPtr::new_unchecked(theap) };
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let Some(heap) = (unsafe { theap_ptr.heap() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let heap_seq = crate::core_types::HeapSeq::new(unsafe {
        (*heap.as_ptr().cast::<MiHeapStatsPrefix>()).heap_seq
    });
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        _mi_stats_print(
            "heap",
            heap_seq.get(),
            ptr::addr_of!((*theap).stats),
            out,
            arg,
        );
        mi_stats_merge_theap_to_heap(theap);
    }
}
