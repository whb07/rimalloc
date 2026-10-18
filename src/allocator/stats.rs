//! Statistics

// Retains upstream stats aliases and header marker constants for C parity.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize};
use core::{mem::size_of, ptr};

use crate::atomic::mi_lock_t;
use crate::core_types::{
    BorrowedHeap, HeapPtr, OutputCallback, OutputCallbackArg, ThreadHeapLocal, ThreadHeapPtr,
};
use crate::internal::memid::mi_memid_t;
use crate::internal::mi_theap_is_initialized;
use crate::mimalloc::{mi_heap_main, mi_subproc_current, mi_subproc_visit_heaps};
use crate::prim::{_mi_heap_theap_peek, _mi_theap_default};
use crate::types::{
    MI_BIN_COUNT, MI_MAX_ARENAS, MI_STAT, mi_heap_t, mi_heap_visit_fun, mi_output_fun, mi_page_t,
    mi_subproc_id_t, mi_subproc_t, mi_theap_t,
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

/// Upstream `MI_STAT_FIELDS()` X-macro: invokes `$callback!` once with every scalar statistic of
/// `mi_stats_t` in declaration order, each tagged `count` (`mi_stat_count_t`) or `counter`
/// (`mi_stat_counter_t`).
macro_rules! mi_stat_fields {
    ($callback:ident) => {
        $callback! {
            count pages,
            count reserved,
            count committed,
            counter reset,
            counter purged,
            count page_committed,
            count pages_abandoned,
            count threads,
            count malloc_normal,
            count malloc_huge,
            count malloc_requested,

            counter mmap_calls,
            counter commit_calls,
            counter reset_calls,
            counter purge_calls,
            counter arena_count,
            counter malloc_normal_count,
            counter malloc_huge_count,
            counter malloc_guarded_count,

            counter arena_rollback_count,
            counter arena_purges,
            counter pages_extended,
            counter pages_retire,
            counter page_searches,
            counter page_searches_count,

            count segments,
            count segments_abandoned,
            count segments_cache,
            count _segments_reserved,

            count heaps,
            count theaps,
            counter pages_reclaim_on_alloc,
            counter pages_reclaim_on_free,
            counter pages_reabandon_full,
            counter pages_unabandon_busy_wait,
            counter heaps_delete_wait,
        }
    };
}
pub(crate) use mi_stat_fields;

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

// The hand-written `mi_stats_t` must declare exactly the `mi_stat_fields!` list, contiguously and
// in the same order (the field set and kinds are also checked by the `MI_STATS_NULL` literal).
macro_rules! assert_stat_fields_layout {
    ($($kind:ident $field:ident,)*) => {
        const _: () = {
            let mut offset = core::mem::offset_of!(mi_stats_t, version) + size_of::<usize>();
            $(
                assert!(core::mem::offset_of!(mi_stats_t, $field) == offset);
                offset += assert_stat_fields_layout!(@ $kind);
            )*
            assert!(offset == core::mem::offset_of!(mi_stats_t, _stat_reserved));
        };
    };
    (@ count) => { size_of::<mi_stat_count_t>() };
    (@ counter) => { size_of::<mi_stat_counter_t>() };
}
mi_stat_fields!(assert_stat_fields_layout);

impl Default for mi_stats_t {
    fn default() -> Self {
        // SAFETY: `mi_stats_t` is plain old data; all-zero bytes are valid for every field.
        unsafe { core::mem::zeroed() }
    }
}

mod counters;
mod process;
mod render;

use crate::allocator::init::MI_STATS_NULL;
pub(crate) use counters::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt,
    __mi_stat_counter_increase, __mi_stat_counter_increase_mt, __mi_stat_decrease,
    __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt, _mi_stats_merge_into,
};
use counters::{mi_stats_add, mi_stats_copy};
pub use counters::{mi_stats_header_init, mi_stats_init};
pub(crate) use process::{_mi_clock_end, _mi_clock_now, _mi_clock_start, _mi_stats_init};
use render::_mi_stats_print;

#[unsafe(no_mangle)]
pub extern "C" fn mi_stats_get_bin_size(bin: usize) -> usize {
    counters::stats_get_bin_size(bin)
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_process_info_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    // SAFETY: the compatibility boundary forwards the callback contract.
    unsafe { render::mi_process_info_print_out_impl(out, arg) }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_process_info_print() {
    render::mi_process_info_print_impl()
}

/// # Safety
/// `stats` must be null or point to a live, aligned initialized `mi_stats_t` that remains readable
/// and stable against mutation during the initial value copy. If `output_buf` is non-null it must be
/// uniquely writable for `output_size` bytes for the duration of this call.
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_subproc_stats_get_json(
    subproc_id: mi_subproc_id_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    let mut stats = MI_STATS_NULL();
    // SAFETY: the compatibility boundary validates the live subproc before rendering its snapshot.
    unsafe {
        if !mi_subproc_stats_get(subproc_id, ptr::addr_of_mut!(stats)) {
            return ptr::null_mut();
        }
        mi_stats_get_json_from(ptr::addr_of!(stats), buf_size, buf)
    }
}

/// # Safety
/// `heap` must be null or identify a live initialized heap that is not torn down or reassociated
/// during this call. Any associated local thread heap must be current-thread owned and exclusively
/// available for destructive stats merge. If `buf` is non-null it must be uniquely writable for
/// `buf_size` bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_get_json(
    heap: *mut mi_heap_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    let mut stats = MI_STATS_NULL();
    // SAFETY: the caller keeps the heap live and any current-local source exclusive while its live
    // counters are atomically copied into the stack snapshot; null retains main-heap semantics.
    if !unsafe { mi_heap_stats_get(heap, ptr::addr_of_mut!(stats)) } {
        return ptr::null_mut();
    }
    // SAFETY: `stats` is a stable initialized stack value and the caller supplies the output buffer.
    unsafe { mi_stats_get_json_from(ptr::addr_of!(stats), buf_size, buf) }
}

/// # Safety
/// If `buf` is non-null it must be writable for `buf_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get_json(buf_size: usize, buf: *mut c_char) -> *mut c_char {
    // SAFETY: the current subproc is live and the caller provides the output buffer contract.
    unsafe { mi_subproc_stats_get_json(mi_subproc_current(), buf_size, buf) }
}

/// # Safety
/// `stats` must be null or point to a live, aligned initialized `mi_stats_t` that remains readable
/// and stable against mutation during the initial value copy. If `buf` is non-null it must be
/// uniquely writable for `buf_size` bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_as_json(
    stats: *mut mi_stats_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    // SAFETY: the compatibility boundary forwards the caller's stats and buffer contracts.
    unsafe { mi_stats_get_json_from(stats.cast_const(), buf_size, buf) }
}

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

/// # Safety
/// `theap` must remain a live initialized current-thread local thread heap with a live associated
/// heap. Its local statistics must be exclusively accessible, the heap destination statistics must
/// remain atomic-compatible, and neither allocation nor their association may be torn down during
/// the destructive merge. Protocol atomic updates to the heap destination are permitted.
unsafe fn mi_stats_merge_local_theap_to_heap(theap: ThreadHeapLocal) -> *const mi_stats_t {
    // SAFETY: the contract keeps the provided current-local thread heap and its association live.
    let Some(heap) = (unsafe { theap.ptr().heap() }) else {
        crate::internal::mi_assert_internal!(false);
        return core::ptr::null();
    };
    // SAFETY: the local witness provides exclusive readable/writable source statistics.
    let stats = unsafe { ptr::addr_of_mut!((*theap.ptr().as_ptr()).stats) };
    // SAFETY: the associated heap remains live with atomic-compatible destination statistics.
    let heap_stats =
        unsafe { ptr::addr_of_mut!((*heap.as_ptr().cast::<MiHeapStatsPrefix>()).stats) };
    // SAFETY: the source is exclusively owned by this thread; the destination remains live and all
    // concurrent destination access uses the atomic statistics protocol.
    unsafe { _mi_stats_merge_into(heap_stats, stats) };
    heap_stats.cast_const()
}

/// # Safety
/// `heap` must remain live and initialized without teardown or reassociation. If it has a local
/// thread heap, that thread heap must be current-thread owned and exclusively accessible for its
/// destructive stats merge. Heap statistics must remain atomic-compatible; protocol atomic updates
/// are permitted.
unsafe fn mi_heap_get_stats_borrowed(heap: BorrowedHeap) -> *const mi_stats_t {
    let heap = heap.ptr();
    // SAFETY: reading current-thread initialization state does not access the borrowed heap.
    if !unsafe { crate::prim::_mi_thread_is_initialized() } {
        // SAFETY: the borrowed heap remains live; raw projection avoids a shared reference to its
        // concurrently atomically updated statistics.
        return unsafe { ptr::addr_of!((*heap.as_ptr().cast::<MiHeapStatsPrefix>()).stats) };
    }
    // SAFETY: the borrowed live heap remains associated while its current-thread theap is queried.
    let theap = unsafe { _mi_heap_theap_peek(heap.as_ptr()) };
    if theap.is_null() {
        // SAFETY: the borrowed heap remains live; raw projection avoids a shared reference to its
        // concurrently atomically updated statistics.
        unsafe { ptr::addr_of!((*heap.as_ptr().cast::<MiHeapStatsPrefix>()).stats) }
    } else {
        // SAFETY: `_mi_heap_theap_peek` returned this current thread's live
        // local thread heap for the borrowed heap.
        let theap = unsafe { ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap)) };
        // SAFETY: the current-local witness keeps the thread-heap source exclusive, while the
        // borrowed associated heap and its atomic-compatible destination remain live.
        unsafe { mi_stats_merge_local_theap_to_heap(theap) }
    }
}

/// # Safety
/// `heap` must be null or identify a live initialized heap with a live associated subprocess.
/// The heap stats source must be exclusively available: no counter updates, other merge/reset, or
/// teardown may race the destructive merge and zeroing. Concurrent access to the subprocess
/// destination is permitted only through the atomic stats protocol.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_merge_to_subproc(heap: *mut mi_heap_t) {
    let heap = if heap.is_null() {
        // SAFETY: the runtime keeps the initialized main heap live for this destructive merge.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: the caller supplies the non-null live initialized heap.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: the caller keeps the heap and its stable subprocess association live.
    let Some(subproc) = (unsafe { heap.subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: the caller keeps the source heap stats exclusively available for
    // destructive merge and the matching subprocess destination live.
    unsafe {
        _mi_stats_merge_into(
            subproc.stats(),
            BorrowedHeap::new_unchecked(heap).ptr().stats(),
        )
    };
}

/// # Safety
/// The process-default heap, current-local thread heap, and subprocess must remain live. The caller
/// must exclude concurrent main-heap counter updates, merge/reset operations, and teardown while
/// its statistics are destructively merged and zeroed. Concurrent subprocess destination updates
/// are permitted only through the atomic stats protocol.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_reset() {
    // SAFETY: the contract keeps the default thread heap live for this initialization query.
    if !unsafe { mi_theap_is_initialized(_mi_theap_default()) } {
        return;
    }
    // SAFETY: the initialized runtime keeps the main heap live through the merge/reset epoch.
    let heap = unsafe { HeapPtr::new_unchecked(mi_heap_main()) };
    // SAFETY: current-thread stats may be destructively merged into the live
    // main heap before its source counters are reset.
    let _ = unsafe { mi_heap_get_stats_borrowed(BorrowedHeap::new_unchecked(heap)) };
    // SAFETY: the contract excludes main-heap updates and teardown while the live heap source is
    // destructively merged into its live atomic subprocess destination.
    unsafe { mi_heap_stats_merge_to_subproc(heap.as_ptr()) };
}

/// # Safety
/// The process-default heap, current-local thread heap, and subprocess must remain live. The caller
/// must exclude concurrent main-heap counter updates, merge/reset operations, and teardown while
/// its statistics are destructively merged and zeroed. Concurrent subprocess destination updates
/// are permitted only through the atomic stats protocol.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_merge() {
    // SAFETY: merging is identical to `mi_stats_reset` and forwards the same caller contract.
    unsafe { mi_stats_reset() };
}

/// # Safety
/// `heap` must be null or identify a live initialized heap that remains allocated and associated
/// during the snapshot. Any current-local thread heap must be exclusively available for destructive
/// merge. A non-null output callback and its opaque argument must remain valid for synchronous
/// invocation and the callback must not unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_print_out(
    heap: *mut mi_heap_t,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let heap = if heap.is_null() {
        // SAFETY: the runtime keeps the initialized main heap live during this synchronous print.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: the caller supplies the non-null live initialized heap.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: the caller contract supplies the heap/current-local merge epoch and callback validity;
    // the raw values are wrapped only for this synchronous dispatch.
    unsafe {
        mi_heap_stats_print_out_typed(
            BorrowedHeap::new_unchecked(heap),
            OutputCallback::new(out),
            OutputCallbackArg::from_raw(arg),
        )
    };
}

/// # Safety
/// `heap` must remain live and initialized without teardown or reassociation. Any associated local
/// thread heap must be current-thread owned and exclusive for destructive merge; concurrent heap
/// counter updates must use the atomic stats protocol. A non-null output callback and its opaque
/// argument must remain valid for synchronous invocation and must not unwind. While invoked from a
/// subprocess heap traversal, it must not reenter operations that create, destroy, or relink heaps
/// in that subprocess.
unsafe fn mi_heap_stats_print_out_typed(
    heap: BorrowedHeap,
    out: OutputCallback,
    arg: OutputCallbackArg,
) {
    let heap_ptr = heap.ptr();
    let heap_prefix = heap_ptr.as_ptr().cast::<MiHeapStatsPrefix>();
    // SAFETY: the borrowed heap remains live and any current-local source is exclusively merged.
    // `_mi_stats_print` atomically snapshots the returned heap counters before its first callback.
    let heap_stats = unsafe { mi_heap_get_stats_borrowed(heap) };
    // SAFETY: the heap stats and sequence prefix remain live while `_mi_stats_print` takes its
    // atomic snapshot; the caller-provided callback/argument contract is forwarded unchanged.
    unsafe {
        _mi_stats_print(
            "heap",
            crate::core_types::HeapSeq::new((*heap_prefix).heap_seq).get(),
            heap_stats,
            out.raw(),
            arg.as_ptr(),
        )
    };
}

/// # Safety
/// `heap` must identify a live initialized heap held stable by the subprocess heap traversal.
/// `arg` must point to a live readable `mi_heap_print_visit_info_t` for this synchronous call. Its
/// output callback and opaque argument must satisfy `mi_heap_stats_print_out_typed`; the callback
/// must not unwind or reenter subprocess heap-registry mutation while the traversal lock is held.
#[inline(never)]
unsafe extern "C" fn mi_heap_print_visitor(heap: *mut mi_heap_t, arg: *mut c_void) -> bool {
    // SAFETY: the visitor contract keeps this internal stack visit-info readable until return; the
    // temporary reference and copied adapter do not escape this invocation.
    let vinfo =
        HeapPrintVisitInfo::from_raw(unsafe { &*(arg.cast::<mi_heap_print_visit_info_t>()) });
    // SAFETY: the traversal keeps the non-null heap live; this witness is used synchronously only.
    let heap = unsafe { BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)) };
    // SAFETY: the traversal contract keeps the borrowed heap and internal copied adapter live for
    // this call; the opaque output argument is governed solely by its caller-provided contract.
    unsafe { mi_heap_stats_print_out_typed(heap, vinfo.out, vinfo.out_arg) };
    true
}

/// # Safety
/// `heap` must identify a live initialized heap held stable by the subprocess heap traversal, with
/// any local thread-heap stats exclusively mergeable by this thread. `arg` must point to one live,
/// aligned, exclusively writable initialized `mi_stats_t` accumulator. Heap stats fields may be
/// concurrently updated during this call only through the atomic stats protocol. This C callback
/// must not unwind.
#[inline(never)]
unsafe extern "C" fn mi_heap_aggregate_visitor(heap: *mut mi_heap_t, arg: *mut c_void) -> bool {
    // SAFETY: the visitor caller supplies one aligned uniquely writable stats
    // accumulator for the duration of the synchronous traversal.
    let stats = arg.cast::<mi_stats_t>();
    // SAFETY: the traversal keeps the non-null heap live; this witness is used synchronously only.
    let heap = unsafe { BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)) };
    // SAFETY: the borrowed heap and any current-local source satisfy the destructive merge epoch;
    // the returned raw destination remains live and atomically accessible during accumulation.
    let heap_stats = unsafe { mi_heap_get_stats_borrowed(heap) };
    // SAFETY: the live heap stats source remains readable and distinct from the unique
    // traversal accumulator while their initialized counters are combined.
    unsafe { mi_stats_add(stats, heap_stats) };
    true
}

/// # Safety
/// `subproc_id` must identify a live initialized subprocess whose heap registry, heaps, and stats
/// remain allocated during the traversal. Its registry must not be torn down concurrently. A
/// non-null output callback and its opaque argument must remain valid for synchronous invocation
/// and must not unwind. During per-heap callbacks it must not reenter operations that create,
/// destroy, or relink heaps in this subprocess because its heap lock is held.
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
    // SAFETY: the caller keeps the subprocess registry stable; `vinfo` is uniquely live through the
    // synchronous locked traversal and the callback/output contracts forbid unwind and reentry.
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
/// `subproc_id` must identify a live initialized subprocess whose stats storage remains allocated;
/// protocol atomic counter updates are permitted. `stats` must point to a live, aligned,
/// exclusively writable initialized `mi_stats_t` and must not overlap the subprocess stats.
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
    // SAFETY: the destination is exclusive and initialized; the live subprocess source is sampled
    // with atomic field loads and does not overlap it.
    unsafe { mi_stats_copy(stats, ptr::addr_of!((*subproc).stats)) }
}

/// # Safety
/// `subproc_id` must identify a live initialized subprocess whose registry, heaps, and stats remain
/// allocated during synchronous traversal; registry teardown must not race this call. `stats` must
/// point to a live, aligned, exclusively writable initialized `mi_stats_t` that does not overlap a
/// source stats object. Protocol atomic counter updates are permitted.
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
    // SAFETY: the destination is exclusive and initialized; the subprocess source and locked heap
    // traversal remain live, and the internal visitor synchronously accumulates through raw atomics.
    unsafe {
        if !mi_stats_copy(stats, ptr::addr_of!((*subproc).stats)) {
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
/// `stats` must point to a live, aligned, exclusively writable initialized `mi_stats_t` that does
/// not overlap allocator-owned source statistics. The current subprocess and its heap registry must
/// remain live without teardown during the synchronous snapshot; protocol atomic updates are
/// permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_get(stats: *mut mi_stats_t) -> bool {
    // SAFETY: the current subprocess, heap registry, and exclusive destination satisfy the delegated
    // synchronous snapshot contract.
    unsafe { mi_subproc_stats_get(mi_subproc_current(), stats) }
}

/// # Safety
/// `heap` must be null or identify a live initialized heap that is not torn down or reassociated
/// during this call. Any associated local thread heap must be current-thread owned and exclusively
/// available for destructive stats merge. `stats` must point to a live, aligned, exclusively
/// writable initialized `mi_stats_t` that does not overlap the heap stats. Protocol atomic updates
/// to heap counters are permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_get(heap: *mut mi_heap_t, stats: *mut mi_stats_t) -> bool {
    if stats.is_null() {
        return false;
    }
    let heap = if heap.is_null() {
        // SAFETY: the runtime keeps the initialized main heap live during this snapshot.
        unsafe { HeapPtr::new_unchecked(mi_heap_main()) }
    } else {
        // SAFETY: the caller supplies the non-null live initialized heap.
        unsafe { HeapPtr::new_unchecked(heap) }
    };
    // SAFETY: the raw contract keeps the destination exclusively writable and
    // the borrowed heap live while current-thread stats are merged and copied.
    unsafe {
        mi_stats_copy(
            stats,
            mi_heap_get_stats_borrowed(BorrowedHeap::new_unchecked(heap)),
        )
    }
}

/// # Safety
/// May collect current-thread allocator state and must not race with allocator teardown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect_reduce(_target_thread_owned: usize) {
    unsafe { crate::allocator::thread_heap::mi_collect(false) };
}

/// # Safety
/// `subproc_id` must identify a live subproc and any output callback must obey the C ABI.
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
    let mut stats = MI_STATS_NULL();
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    unsafe { mi_subproc_stats_print_out(mi_subproc_current(), out, arg) };
}

/// # Safety
/// `out`, when non-null, must be a valid `mi_output_fun` callback pointer encoded as `void*`.
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

/// # Safety
/// Any output callback must obey the C ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_thread_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    let theap = unsafe { _mi_theap_default() };
    if theap.is_null() || !unsafe { mi_theap_is_initialized(theap) } {
        return;
    }
    let theap_ptr = unsafe { ThreadHeapPtr::new_unchecked(theap) };
    let Some(heap) = (unsafe { theap_ptr.heap() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    let heap_seq = crate::core_types::HeapSeq::new(unsafe {
        (*heap.as_ptr().cast::<MiHeapStatsPrefix>()).heap_seq
    });
    unsafe {
        _mi_stats_print(
            "heap",
            heap_seq.get(),
            ptr::addr_of!((*theap).stats),
            out,
            arg,
        );
        mi_stats_merge_local_theap_to_heap(ThreadHeapLocal::new_unchecked(theap_ptr));
    }
}
