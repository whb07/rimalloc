//! Global / thread initialization

// Retains translated TLS/stat initialization macros that are active only on
// specific platform startup paths.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

mod cpu;
mod subproc_registry;
mod tls;
#[cfg(feature = "guarded")]
use core::ffi::c_long;
use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::ptr;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use crate::arena::_mi_arenas_unsafe_destroy_all;
use crate::atomic::{
    mi_atomic_do_once_enter, mi_atomic_once_s, mi_atomic_once_t, mi_lock_acquire, mi_lock_done,
    mi_lock_init, mi_lock_release, mi_lock_t,
};
use crate::core_types::{
    BorrowedSubproc, Errno, HeapPtr, HeapVisitCallback, HeapVisitCallbackArg, MainHeap,
    MainSubproc, MiLockGuard, MiLockPtr, SubprocPtr, ThreadHeapLocal, ThreadHeapPtr,
    ThreadHeapState, ThreadId, TldPtr,
};
#[cfg(feature = "guarded")]
use crate::core_types::{
    GuardedConfig, GuardedSampleCount, GuardedSampleRate, GuardedSizeMax, GuardedSizeMin,
};
use crate::heap::{_mi_heap_force_destroy, mi_heap_destroy};
use crate::internal::{
    _mi_allocator_done, _mi_allocator_init, _mi_meta_free, _mi_meta_zalloc, mi_memid_t,
};
#[cfg(feature = "guarded")]
use crate::options::mi_option_get_clamp;
use crate::options::{
    _mi_error_message_fmt, _mi_fputs, _mi_options_init, _mi_options_post_init, _mi_verbose_message,
    _mi_warning_message_fmt, mi_option_get, mi_option_is_enabled, mi_option_t,
};
use crate::stats::{
    __mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_stats_merge_into, mi_stat_count_t,
    mi_stats_header_init, mi_stats_t,
};
use crate::theap::{_mi_theap_collect_abandon, _mi_theap_create, _mi_theap_free};
use crate::types::{
    MI_BIN_COUNT, MI_BIN_FULL, MI_BIN_HUGE, MI_PAGES_DIRECT, mi_heap_t, mi_heap_visit_fun,
    mi_page_queue_t, mi_page_t, mi_random_ctx_t, mi_subproc_id_t, mi_subproc_t, mi_theap_t,
    mi_thread_local_t, mi_threadid_t, mi_tld_t,
};
#[cfg(any(
    miri,
    not(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))
))]
pub(crate) use tls::fallback_thread_id;
pub(crate) use tls::{
    _mi_theap_cached_expansion_slot, _mi_theap_cached_key, _mi_theap_cached_slot,
    _mi_theap_default_expansion_slot, _mi_theap_default_key, _mi_theap_default_slot,
    mi_theap_main_tls_peek, mi_theap_main_tls_set,
};

// SAFETY: platform ABI hooks and link-name shims forward into the bounded init lifecycle.
#[cfg(target_os = "linux")]
core::arch::global_asm!(".hidden _mi_theap_empty");
// SAFETY: platform ABI hooks and link-name shims forward into the bounded init lifecycle.
#[cfg(target_os = "linux")]
core::arch::global_asm!(".hidden _mi_cpu_stosb_max");
// SAFETY: platform ABI hooks and link-name shims forward into the bounded init lifecycle.
#[cfg(target_os = "linux")]
core::arch::global_asm!(".hidden subproc_main");

const MI_MAX_ARENAS: usize = 160;
pub const MI_TLD_INVALID: *mut mi_tld_t = core::ptr::dangling_mut::<mi_tld_t>();

#[allow(non_snake_case)]
pub const fn MI_STAT_COUNT_NULL() -> mi_stat_count_t {
    mi_stat_count_t {
        total: 0,
        peak: 0,
        current: 0,
    }
}

#[allow(non_snake_case)]
pub const fn MI_STAT_COUNT<T: Copy>(_stat: T) -> mi_stat_count_t {
    MI_STAT_COUNT_NULL()
}

pub const fn mi_memid_none() -> mi_memid_t {
    crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw()
}

pub const fn mi_memid_static() -> mi_memid_t {
    crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw()
}

#[inline]
fn mi_memid_is_static(memid: mi_memid_t) -> bool {
    matches!(
        crate::core_types::MemId::view(memid),
        crate::core_types::MemIdView::Static(_)
    )
}

#[allow(non_snake_case)]
pub const fn QNULL(sz: usize) -> mi_page_queue_t {
    mi_page_queue_t {
        first: ptr::null_mut(),
        last: ptr::null_mut(),
        count: 0,
        block_size: sz * size_of::<usize>(),
    }
}

#[allow(non_snake_case)]
pub const fn MI_STATS_NULL() -> mi_stats_t {
    const ZERO_COUNT: mi_stat_count_t = mi_stat_count_t {
        total: 0,
        peak: 0,
        current: 0,
    };
    const ZERO_COUNTER: crate::stats::mi_stat_counter_t =
        crate::stats::mi_stat_counter_t { total: 0 };
    mi_stats_t {
        size: size_of::<mi_stats_t>(),
        version: crate::stats::MI_STAT_VERSION,
        pages: ZERO_COUNT,
        reserved: ZERO_COUNT,
        committed: ZERO_COUNT,
        reset: ZERO_COUNTER,
        purged: ZERO_COUNTER,
        page_committed: ZERO_COUNT,
        pages_abandoned: ZERO_COUNT,
        threads: ZERO_COUNT,
        malloc_normal: ZERO_COUNT,
        malloc_huge: ZERO_COUNT,
        malloc_requested: ZERO_COUNT,
        mmap_calls: ZERO_COUNTER,
        commit_calls: ZERO_COUNTER,
        reset_calls: ZERO_COUNTER,
        purge_calls: ZERO_COUNTER,
        arena_count: ZERO_COUNTER,
        malloc_normal_count: ZERO_COUNTER,
        malloc_huge_count: ZERO_COUNTER,
        malloc_guarded_count: ZERO_COUNTER,
        arena_rollback_count: ZERO_COUNTER,
        arena_purges: ZERO_COUNTER,
        pages_extended: ZERO_COUNTER,
        pages_retire: ZERO_COUNTER,
        page_searches: ZERO_COUNTER,
        page_searches_count: ZERO_COUNTER,
        segments: ZERO_COUNT,
        segments_abandoned: ZERO_COUNT,
        segments_cache: ZERO_COUNT,
        _segments_reserved: ZERO_COUNT,
        heaps: ZERO_COUNT,
        theaps: ZERO_COUNT,
        pages_reclaim_on_alloc: ZERO_COUNTER,
        pages_reclaim_on_free: ZERO_COUNTER,
        pages_reabandon_full: ZERO_COUNTER,
        pages_unabandon_busy_wait: ZERO_COUNTER,
        heaps_delete_wait: ZERO_COUNTER,
        _stat_reserved: [ZERO_COUNT; 4],
        _stat_counter_reserved: [ZERO_COUNTER; 4],
        malloc_bins: [ZERO_COUNT; MI_BIN_HUGE + 1],
        page_bins: [ZERO_COUNT; MI_BIN_HUGE + 1],
        chunk_bins: [ZERO_COUNT; 6],
    }
}

static mut subprocs_lock: mi_lock_t = mi_lock_t::new();
static PROCESS_INIT: mi_atomic_once_t = mi_atomic_once_s::new();
static OPTIONS_POST_INIT_STATE: AtomicUsize = AtomicUsize::new(0);
static PROCESS_DONE: AtomicBool = AtomicBool::new(false);
static OS_PRELOADING: AtomicBool = AtomicBool::new(true);
static SUBPROC_TOTAL_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg_attr(target_os = "linux", unsafe(no_mangle))]
static mut subproc_main: MiSubprocPrefix = MiSubprocPrefix {
    subproc_seq: 0,
    next: ptr::null_mut(),
    prev: ptr::null_mut(),
    arena_count: AtomicUsize::new(0),
    arenas: [ptr::null_mut(); MI_MAX_ARENAS],
    arena_reserve_lock: mi_lock_t::new(),
    purge_expire: core::sync::atomic::AtomicI64::new(0),
    heap_main: ptr::null_mut(),
    heaps: ptr::null_mut(),
    heaps_lock: mi_lock_t::new(),
    thread_count: AtomicUsize::new(0),
    thread_total_count: AtomicUsize::new(0),
    heap_count: AtomicUsize::new(0),
    heap_total_count: AtomicUsize::new(0),
    memid: mi_memid_none(),
    stats: MI_STATS_NULL(),
};
static mut subprocs: *mut mi_subproc_t = ptr::addr_of_mut!(subproc_main)
    .cast::<MiSubprocPrefix>()
    .cast::<mi_subproc_t>();

static mut tld_empty: mi_tld_t = mi_tld_t {
    thread_id: 0,
    thread_seq: 0,
    numa_node: 0,
    subproc: ptr::addr_of_mut!(subproc_main)
        .cast::<MiSubprocPrefix>()
        .cast::<mi_subproc_t>(),
    theaps: ptr::null_mut(),
    theaps_lock: mi_lock_t::new(),
    recurse: false,
    is_in_threadpool: false,
    memid: mi_memid_static(),
};

#[repr(C)]
pub struct MiHeapMainPrefix {
    subproc: *mut mi_subproc_t,
    heap_seq: usize,
    next: *mut mi_heap_t,
    prev: *mut mi_heap_t,
    theap: mi_thread_local_t,
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

static mut tld_main: mi_tld_t = mi_tld_t {
    thread_id: 0,
    thread_seq: 0,
    numa_node: 0,
    subproc: ptr::addr_of_mut!(subproc_main).cast::<mi_subproc_t>(),
    theaps: ptr::addr_of_mut!(theap_main),
    theaps_lock: mi_lock_t::new(),
    recurse: false,
    is_in_threadpool: false,
    memid: mi_memid_static(),
};

pub static _mi_page_empty: mi_page_t = mi_page_t {
    xthread_id: 0,
    free: ptr::null_mut(),
    used: 0,
    capacity: 0,
    reserved: 0,
    retire_expire: 0,
    free_is_zero: false,
    local_free: ptr::null_mut(),
    xthread_free: 0,
    block_size: 0,
    page_start: ptr::null_mut(),
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    keys: [0, 0],
    theap: ptr::null_mut(),
    heap: ptr::null_mut(),
    next: ptr::null_mut(),
    prev: ptr::null_mut(),
    slice_committed: crate::arena::MI_ARENA_SLICE_SIZE,
    memid: mi_memid_static(),
};

#[allow(non_snake_case)]
pub const fn MI_PAGE_EMPTY() -> *mut mi_page_t {
    ptr::addr_of!(_mi_page_empty).cast_mut()
}

#[allow(non_snake_case)]
pub const fn MI_SMALL_PAGES_EMPTY() -> [*mut mi_page_t; MI_PAGES_DIRECT] {
    [MI_PAGE_EMPTY(); MI_PAGES_DIRECT]
}

const MI_PAGE_QUEUE_WORD_SIZES: [usize; MI_BIN_COUNT] = [
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
    114_688,
    131_072,
    163_840,
    196_608,
    229_376,
    262_144,
    327_680,
    393_216,
    458_752,
    524_288,
    crate::stats::MI_LARGE_MAX_OBJ_WSIZE + 1,
    crate::stats::MI_LARGE_MAX_OBJ_WSIZE + 2,
];

#[allow(non_snake_case)]
pub const fn MI_PAGE_QUEUES_EMPTY() -> [mi_page_queue_t; MI_BIN_COUNT] {
    let mut queues = [QNULL(0); MI_BIN_COUNT];
    let mut i = 0;
    while i < MI_BIN_COUNT {
        queues[i] = QNULL(MI_PAGE_QUEUE_WORD_SIZES[i]);
        i += 1;
    }
    queues
}

const MI_RANDOM_CTX_EMPTY: mi_random_ctx_t = mi_random_ctx_t {
    input: [0; 16],
    output: [0; 16],
    output_available: 0,
    weak: true,
};

// SAFETY: empty thread-heap sentinel stays static and typed before TLS exposure.
#[unsafe(no_mangle)]
pub(crate) static _mi_theap_empty: mi_theap_t = mi_theap_t {
    tld: ptr::addr_of!(tld_empty).cast_mut(),
    heap: ptr::null_mut(),
    refcount: 1,
    heartbeat: 0,
    cookie: 0,
    random: MI_RANDOM_CTX_EMPTY,
    page_count: 0,
    page_retired_min: MI_BIN_FULL,
    page_retired_max: 0,
    pages_full_size: 0,
    generic_count: 0,
    generic_collect_count: 0,
    tnext: ptr::null_mut(),
    tprev: ptr::null_mut(),
    hnext: ptr::null_mut(),
    hprev: ptr::null_mut(),
    page_full_retain: 0,
    allow_page_reclaim: false,
    allow_page_abandon: true,
    #[cfg(feature = "guarded")]
    guarded_size_min: 0,
    #[cfg(feature = "guarded")]
    guarded_size_max: 0,
    #[cfg(feature = "guarded")]
    guarded_sample_rate: 0,
    #[cfg(feature = "guarded")]
    guarded_sample_count: 1,
    pages_free_direct: MI_SMALL_PAGES_EMPTY(),
    pages: MI_PAGE_QUEUES_EMPTY(),
    memid: mi_memid_static(),
    stats: MI_STATS_NULL(),
};

pub(crate) static _mi_theap_empty_wrong: mi_theap_t = mi_theap_t {
    tld: ptr::addr_of!(tld_empty).cast_mut(),
    heap: ptr::null_mut(),
    refcount: 1,
    heartbeat: 0,
    cookie: 0,
    random: MI_RANDOM_CTX_EMPTY,
    page_count: 0,
    page_retired_min: MI_BIN_FULL,
    page_retired_max: 0,
    pages_full_size: 0,
    generic_count: 0,
    generic_collect_count: 0,
    tnext: ptr::null_mut(),
    tprev: ptr::null_mut(),
    hnext: ptr::null_mut(),
    hprev: ptr::null_mut(),
    page_full_retain: 0,
    allow_page_reclaim: false,
    allow_page_abandon: true,
    #[cfg(feature = "guarded")]
    guarded_size_min: 0,
    #[cfg(feature = "guarded")]
    guarded_size_max: 0,
    #[cfg(feature = "guarded")]
    guarded_sample_rate: 0,
    #[cfg(feature = "guarded")]
    guarded_sample_count: 1,
    pages_free_direct: MI_SMALL_PAGES_EMPTY(),
    pages: MI_PAGE_QUEUES_EMPTY(),
    memid: mi_memid_static(),
    stats: MI_STATS_NULL(),
};

static mut theap_main: mi_theap_t = mi_theap_t {
    tld: ptr::addr_of_mut!(tld_main),
    heap: ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>(),
    refcount: 1,
    heartbeat: 0,
    cookie: 0,
    random: mi_random_ctx_t {
        input: [0x846c_a68b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        output: [0; 16],
        output_available: 0,
        weak: true,
    },
    page_count: 0,
    page_retired_min: MI_BIN_FULL,
    page_retired_max: 0,
    pages_full_size: 0,
    generic_count: 0,
    generic_collect_count: 0,
    tnext: ptr::null_mut(),
    tprev: ptr::null_mut(),
    hnext: ptr::null_mut(),
    hprev: ptr::null_mut(),
    page_full_retain: 2,
    allow_page_reclaim: true,
    allow_page_abandon: true,
    #[cfg(feature = "guarded")]
    guarded_size_min: 0,
    #[cfg(feature = "guarded")]
    guarded_size_max: 0,
    #[cfg(feature = "guarded")]
    guarded_sample_rate: 0,
    #[cfg(feature = "guarded")]
    guarded_sample_count: 1,
    pages_free_direct: MI_SMALL_PAGES_EMPTY(),
    pages: MI_PAGE_QUEUES_EMPTY(),
    memid: mi_memid_static(),
    stats: MI_STATS_NULL(),
};

static mut heap_main: MiHeapMainPrefix = MiHeapMainPrefix {
    subproc: ptr::null_mut(),
    heap_seq: 0,
    next: ptr::null_mut(),
    prev: ptr::null_mut(),
    theap: 0,
    exclusive_arena: ptr::null_mut(),
    numa_node: 0,
    _padding: 0,
    theaps: ptr::null_mut(),
    theaps_lock: mi_lock_t::new(),
    abandoned_count: [const { AtomicUsize::new(0) }; MI_BIN_COUNT],
    os_abandoned_pages: ptr::null_mut(),
    os_abandoned_pages_lock: mi_lock_t::new(),
    arena_pages: [const { AtomicPtr::new(ptr::null_mut()) }; MI_MAX_ARENAS],
    arena_pages_lock: mi_lock_t::new(),
    stats: MI_STATS_NULL(),
};

// SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
#[unsafe(no_mangle)]
pub(crate) static mut _mi_cpu_stosb_max: usize = 0;

pub(crate) static mut _mi_process_is_initialized: bool = false;

pub(crate) static mut _mi_cpu_has_popcnt: bool = false;

#[repr(C)]
struct MiSubprocPrefix {
    subproc_seq: usize,
    next: *mut mi_subproc_t,
    prev: *mut mi_subproc_t,
    arena_count: AtomicUsize,
    arenas: [*mut c_void; MI_MAX_ARENAS],
    arena_reserve_lock: mi_lock_t,
    purge_expire: core::sync::atomic::AtomicI64,
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

impl MiSubprocPrefix {
    fn new_with_memid(seq: crate::core_types::SubprocSeq, memid: mi_memid_t) -> Self {
        let mut stats = mi_stats_t::default();
        // SAFETY: `stats` points to a valid local `mi_stats_t`.
        // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
        unsafe { mi_stats_header_init(&mut stats) };
        Self {
            subproc_seq: seq.get(),
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
            arena_count: AtomicUsize::new(0),
            arenas: [ptr::null_mut(); MI_MAX_ARENAS],
            arena_reserve_lock: mi_lock_t::new(),
            purge_expire: core::sync::atomic::AtomicI64::new(0),
            heap_main: ptr::null_mut(),
            heaps: ptr::null_mut(),
            heaps_lock: mi_lock_t::new(),
            thread_count: AtomicUsize::new(0),
            thread_total_count: AtomicUsize::new(0),
            heap_count: AtomicUsize::new(0),
            heap_total_count: AtomicUsize::new(0),
            memid,
            stats,
        }
    }
}

unsafe fn mi_subproc_prefix(subproc: *mut mi_subproc_t) -> *mut MiSubprocPrefix {
    // SAFETY: subproc prefix projection uses validated subproc pointer witnesses.
    unsafe { mi_subproc_prefix_typed(SubprocPtr::new_unchecked(subproc)) }
}

unsafe fn mi_subproc_prefix_typed(subproc: SubprocPtr) -> *mut MiSubprocPrefix {
    subproc.as_ptr().cast::<MiSubprocPrefix>()
}

unsafe fn mi_subproc_heap_main_raw(subproc: *mut mi_subproc_t) -> *mut mi_heap_t {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        return ptr::null_mut();
    };
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    let subproc = unsafe { BorrowedSubproc::new(subproc) };
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    unsafe { mi_subproc_heap_main_typed(subproc) }
        .map(HeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

unsafe fn mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> Option<HeapPtr> {
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    let subproc = unsafe { mi_subproc_prefix_typed(subproc.ptr()) };
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    HeapPtr::new(unsafe { ptr::addr_of!((*subproc).heap_main).read_unaligned() })
}

unsafe fn mi_heap_subproc_raw(heap: *const mi_heap_t) -> *mut mi_subproc_t {
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    unsafe { ptr::addr_of!((*(heap.cast::<MiHeapMainPrefix>())).subproc).read_unaligned() }
}

// SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
/// # Safety
/// The returned opaque identifier must only be passed back to this crate's subproc functions.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_subproc_to_id(subproc: *mut mi_subproc_t) -> mi_subproc_id_t {
    let subproc = SubprocPtr::new(subproc);
    _mi_subproc_to_id_typed(subproc)
}

fn _mi_subproc_to_id_typed(subproc: Option<SubprocPtr>) -> mi_subproc_id_t {
    mi_subproc_id_t::from_ptr(subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut()))
}

// SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
#[used]
static MI_SUBPROC_TO_ID_KEEP: unsafe extern "C" fn(*mut mi_subproc_t) -> mi_subproc_id_t =
    _mi_subproc_to_id;

unsafe fn mi_subproc_destroy_meta(subproc: *mut mi_subproc_t) {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        return;
    };
    // SAFETY: subproc metadata destruction is serialized by the subproc registry lock.
    unsafe { mi_subproc_destroy_meta_typed(subproc) };
}

const _: unsafe fn(*mut mi_subproc_t) = mi_subproc_destroy_meta;

unsafe fn mi_subproc_destroy_meta_typed(subproc: SubprocPtr) {
    // SAFETY: subproc metadata destruction is serialized by the subproc registry lock.
    let subproc = unsafe { mi_subproc_prefix_typed(subproc) };
    // SAFETY: `subproc` points to metadata allocated by `mi_subproc_new`.
    // SAFETY: subproc metadata destruction is serialized by the subproc registry lock.
    unsafe {
        mi_lock_done(ptr::addr_of_mut!((*subproc).arena_reserve_lock));
        mi_lock_done(ptr::addr_of_mut!((*subproc).heaps_lock));
        _mi_meta_free(
            subproc.cast(),
            size_of::<MiSubprocPrefix>(),
            (*subproc).memid,
        );
    }
}

#[cfg(all(not(miri), target_os = "linux", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn mi_subproc_main_addr() -> *mut mi_subproc_t {
    let main: *mut mi_subproc_t;
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe {
        core::arch::asm!(
            "lea {main}, [rip + subproc_main]",
            main = lateout(reg) main,
            options(nostack, preserves_flags)
        );
    }
    main
}

#[cfg(any(miri, not(all(target_os = "linux", target_arch = "x86_64"))))]
#[inline(always)]
unsafe fn mi_subproc_main_addr() -> *mut mi_subproc_t {
    ptr::addr_of_mut!(subproc_main)
        .cast::<MiSubprocPrefix>()
        .cast::<mi_subproc_t>()
}

unsafe fn mi_subproc_main_addr_typed() -> MainSubproc {
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe { MainSubproc::new_unchecked(SubprocPtr::new_unchecked(mi_subproc_main_addr())) }
}

fn mi_subproc_main_init() {
    // SAFETY: one-time process initialization for local subproc scaffolding.
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe {
        let main = mi_subproc_main_addr();
        let main_prefix = mi_subproc_prefix(main);
        if !mi_memid_is_static((*main_prefix).memid) {
            _mi_options_init();
            (*main_prefix).memid =
                crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw();
            (*main_prefix).heaps = ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>();
            (*main_prefix).heap_total_count.store(1, Ordering::Relaxed);
            (*main_prefix).heap_count.store(1, Ordering::Relaxed);
            (*main_prefix).heap_main = ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>();
            __mi_stat_increase_mt(&mut (*main_prefix).stats.heaps, 1);
            mi_stats_header_init(ptr::addr_of_mut!((*main_prefix).stats));
            mi_lock_init(ptr::addr_of_mut!((*main_prefix).arena_reserve_lock));
            mi_lock_init(ptr::addr_of_mut!((*main_prefix).heaps_lock));
            mi_lock_init(ptr::addr_of_mut!(subprocs_lock));
            mi_lock_init(ptr::addr_of_mut!(tld_empty.theaps_lock));
        }
    }
}

unsafe fn mi_subproc_unsafe_destroy(subproc: *mut mi_subproc_t, acquire_subprocs_lock: bool) {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        return;
    };
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    unsafe { mi_subproc_unsafe_destroy_typed(subproc, acquire_subprocs_lock) };
}

unsafe fn mi_subproc_unsafe_destroy_typed(subproc: SubprocPtr, acquire_subprocs_lock: bool) {
    let subproc_raw = subproc.as_ptr();
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    let main_subproc = unsafe { mi_subproc_main_addr_typed().ptr() };
    let is_main = ptr::eq(subproc_raw, main_subproc.as_ptr());
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    let subproc_prefix = unsafe { mi_subproc_prefix_typed(subproc) };
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    let borrowed_subproc = unsafe { BorrowedSubproc::new(subproc) };
    if acquire_subprocs_lock {
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        unsafe { mi_lock_acquire(ptr::addr_of_mut!(subprocs_lock)) };
    }
    // SAFETY: registry membership rewires only the lock-protected subproc head and prev/next links.
    unsafe { subproc_registry::unlink_locked(subproc) };
    if acquire_subprocs_lock {
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        unsafe { mi_lock_release(ptr::addr_of_mut!(subprocs_lock)) };
    }

    loop {
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        let heaps_lock = unsafe { MiLockGuard::acquire(subproc.heaps_lock()) };
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        let heap = unsafe { (*subproc_prefix).heaps };
        drop(heaps_lock);
        if heap.is_null() {
            break;
        }
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        let main_heap = unsafe { mi_subproc_heap_main_typed(borrowed_subproc) }
            .map(HeapPtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        if !ptr::eq(heap, main_heap) {
            // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
            unsafe { mi_heap_destroy(heap) };
        } else {
            // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
            unsafe { _mi_heap_force_destroy(heap) };
            break;
        }
    }

    {
        let subproc = subproc_raw;
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        unsafe { _mi_arenas_unsafe_destroy_all(subproc) };
    }

    if is_main {
        let subproc = subproc_raw;
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        unsafe { crate::page_map::_mi_page_map_unsafe_destroy(subproc) };
    } else {
        // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
        let main_subproc = unsafe { mi_subproc_prefix_typed(main_subproc) };
        // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
        unsafe {
            _mi_stats_merge_into(
                ptr::addr_of_mut!((*main_subproc).stats),
                ptr::addr_of_mut!((*subproc_prefix).stats),
            );
            mi_subproc_destroy_meta_typed(subproc);
        }
    }
}

unsafe fn mi_subprocs_unsafe_destroy_all() {
    mi_subproc_main_init();
    // SAFETY: registry teardown walks remaining subprocs while holding the registry lock.
    let main_subproc = unsafe { _mi_subproc_main() };
    // SAFETY: registry teardown walks remaining subprocs while holding the registry lock.
    unsafe {
        {
            let lock =
                MiLockPtr::from_nonnull(NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock)));
            let _lock = MiLockGuard::acquire(lock);
            let mut subproc = subproc_registry::head_locked();
            while let Some(current) = subproc {
                let next = subproc_registry::next_locked(current);
                if !ptr::eq(current.as_ptr(), main_subproc) {
                    mi_subproc_unsafe_destroy(current.as_ptr(), false);
                }
                subproc = next;
            }
        }
        mi_subproc_unsafe_destroy(main_subproc, true);
    }
}

#[inline(never)]
pub(crate) extern "C" fn _mi_thread_id() -> mi_threadid_t {
    tls::current_thread_id()
}

#[used]
static MI_THREAD_ID_KEEP: extern "C" fn() -> mi_threadid_t = _mi_thread_id;

#[inline(always)]
fn mi_is_main_thread_fast() -> bool {
    // SAFETY: main-thread identity compares initialized thread metadata with the current thread ID.
    let main_tid = unsafe { tld_main.thread_id };
    main_tid == 0 || main_tid == tls::current_thread_id()
}

#[inline(never)]
pub extern "C" fn _mi_is_main_thread() -> bool {
    mi_is_main_thread_fast()
}

// SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
#[unsafe(no_mangle)]
/// # Safety
/// Delegates to the internal thread teardown routine for the current thread.
pub unsafe extern "C" fn mi_thread_done() {
    // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
    unsafe { crate::internal::_mi_thread_done(ptr::null_mut()) };
}

// SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
/// # Safety
/// `theap` must be null or the current thread's thread-local heap.
pub(crate) unsafe extern "C" fn _mi_thread_done(theap: *mut mi_theap_t) {
    let mut default_theap = theap;
    if default_theap.is_null() {
        default_theap = mi_theap_main_tls_peek();
        if default_theap.is_null() {
            // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
            default_theap = unsafe { crate::prim::_mi_theap_default() };
            crate::internal::mi_assert_internal!(
                // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
                default_theap.is_null() || unsafe { _mi_is_theap_main(default_theap) }
            );
        }
    }

    let Some(default_theap) = ThreadHeapPtr::new(default_theap) else {
        return;
    };
    // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
    unsafe { _mi_thread_done_typed(default_theap) };
}

unsafe fn _mi_thread_done_typed(default_theap: ThreadHeapPtr) {
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let theap = match unsafe { default_theap.state() } {
        ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => theap,
        ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return,
    };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let current = unsafe { ThreadId::new_unchecked(crate::prim::_mi_prim_thread_id()) };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let Ok(default_theap) = (unsafe { theap.try_local(current) }) else {
        return;
    };
    crate::threadlocal::_mi_thread_locals_thread_done();

    let default_theap = default_theap.ptr().as_ptr();
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let tld = unsafe { (*default_theap).tld };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let subproc = unsafe { (*tld).subproc };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if heap.is_null() {
        mi_heap_main_init();
        // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
        heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    unsafe {
        __mi_stat_decrease_mt(
            ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
            1,
        )
    };

    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    if unsafe { (*tld).thread_id } != crate::prim::_mi_prim_thread_id() {
        return;
    }

    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    unsafe { mi_thread_theaps_done(tld) };
    mi_tld_free(tld);
}

#[inline(never)]
pub(crate) extern "C" fn _mi_preloading() -> bool {
    OS_PRELOADING.load(Ordering::Relaxed)
}

// SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
#[unsafe(no_mangle)]
pub extern "C" fn mi_is_redirected() -> bool {
    unsafe { crate::prim::_mi_is_redirected() }
}

/// # Safety
/// `theap` must be null or point to a valid thread heap owned by this allocator.
// SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_guarded_set_sample_rate(
    theap: *mut mi_theap_t,
    sample_rate: usize,
    seed: usize,
) {
    #[cfg(feature = "guarded")]
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        if let Some(theap) = ThreadHeapPtr::new(theap) {
            let current = ThreadId::new_unchecked(crate::prim::_mi_prim_thread_id());
            if let Ok(theap) = theap.try_local(current) {
                mi_theap_guarded_set_sample_rate_typed(
                    theap,
                    GuardedSampleRate::new(sample_rate),
                    seed,
                );
            }
        }
    }
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        let _ = sample_rate;
        let _ = seed;
    }
}

#[cfg(feature = "guarded")]
unsafe fn mi_theap_guarded_set_sample_rate_typed(
    theap: ThreadHeapLocal,
    sample_rate: GuardedSampleRate,
    seed: usize,
) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        (*theap).guarded_sample_rate = sample_rate.get();
        let mut sample_count = GuardedSampleCount::new(sample_rate.get());
        if sample_rate.get() > 1 {
            let seed = if seed == 0 {
                crate::internal::_mi_theap_random_next(theap)
            } else {
                seed
            };
            sample_count = GuardedSampleCount::new((seed % sample_rate.get()) + 1);
        }
        (*theap).guarded_sample_count = sample_count.get();
    }
}

/// # Safety
/// `theap` must be null or point to a valid thread heap owned by this allocator.
// SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_guarded_set_size_bound(
    theap: *mut mi_theap_t,
    min: usize,
    max: usize,
) {
    #[cfg(feature = "guarded")]
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        if let Some(theap) = ThreadHeapPtr::new(theap) {
            let current = ThreadId::new_unchecked(crate::prim::_mi_prim_thread_id());
            if let Ok(theap) = theap.try_local(current) {
                let min = GuardedSizeMin::new(min);
                let max = GuardedSizeMax::new(if min.get() > max { min.get() } else { max });
                mi_theap_guarded_set_size_bound_typed(theap, min, max);
            }
        }
    }
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        let _ = min;
        let _ = max;
    }
}

#[cfg(feature = "guarded")]
unsafe fn mi_theap_guarded_set_size_bound_typed(
    theap: ThreadHeapLocal,
    min: GuardedSizeMin,
    max: GuardedSizeMax,
) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        (*theap).guarded_size_min = min.get();
        (*theap).guarded_size_max = max.get();
    }
}

// SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_guarded_init(theap: *mut mi_theap_t) {
    #[cfg(feature = "guarded")]
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        if let Some(theap) = ThreadHeapPtr::new(theap) {
            let current = ThreadId::new_unchecked(crate::prim::_mi_prim_thread_id());
            if let Ok(theap) = theap.try_local(current) {
                let sample_rate =
                    mi_option_get_clamp(mi_option_t::mi_option_guarded_sample_rate, 0, c_long::MAX)
                        as usize;
                let mut sample_count = sample_rate;
                if sample_rate > 1 {
                    let seed = mi_option_get(mi_option_t::mi_option_guarded_sample_seed) as usize;
                    let seed = if seed == 0 {
                        crate::internal::_mi_theap_random_next(theap.ptr().as_ptr())
                    } else {
                        seed
                    };
                    sample_count = (seed % sample_rate) + 1;
                }
                let size_min =
                    mi_option_get_clamp(mi_option_t::mi_option_guarded_min, 0, c_long::MAX)
                        as usize;
                let size_max =
                    mi_option_get_clamp(mi_option_t::mi_option_guarded_max, 0, c_long::MAX)
                        as usize;
                let config = GuardedConfig::from_clamped_raw(
                    size_min,
                    if size_min > size_max {
                        size_min
                    } else {
                        size_max
                    },
                    sample_rate,
                    sample_count,
                );
                _mi_theap_guarded_init_typed(theap, config);
            }
        }
    }
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(feature = "guarded")]
unsafe fn _mi_theap_guarded_init_typed(theap: ThreadHeapLocal, config: GuardedConfig) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        (*theap).guarded_size_min = config.size_min().get();
        (*theap).guarded_size_max = config.size_max().get();
        (*theap).guarded_sample_rate = config.sample_rate().get();
        (*theap).guarded_sample_count = config.sample_count().get();
    }
}

// SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
/// # Safety
/// `theap` must be valid when non-null.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_options_init(theap: *mut mi_theap_t) {
    if let Some(theap) = ThreadHeapPtr::new(theap) {
        // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
        let current = unsafe { ThreadId::new_unchecked(crate::prim::_mi_prim_thread_id()) };
        // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
        if let Ok(theap) = unsafe { theap.try_local(current) } {
            // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
            unsafe { _mi_theap_options_init_typed(theap) };
        }
    }
}

unsafe fn _mi_theap_options_init_typed(theap: ThreadHeapLocal) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        let policy = crate::allocator::policy::ActivePolicy::thread_heap_policy();
        (*theap).allow_page_reclaim = policy.allow_page_reclaim();
        (*theap).allow_page_abandon = policy.allow_page_abandon();
        (*theap).page_full_retain = policy.page_full_retain().get();
    }
}

fn mi_tld_main_init() {
    // SAFETY: mutates the statically allocated main-thread tld during one-time lazy init.
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        if tld_main.thread_id == 0 {
            tld_main.thread_id = crate::prim::_mi_prim_thread_id();
            mi_lock_init(ptr::addr_of_mut!(tld_main.theaps_lock));
        }
    }
}

fn mi_theap_main_init() {
    // SAFETY: mutates the statically allocated main theap during one-time lazy init.
    // SAFETY: main thread-heap static metadata is initialized once before publication.
    unsafe {
        if !mi_memid_is_static(theap_main.memid) {
            theap_main.memid =
                crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw();
            #[cfg(any(target_os = "macos", windows))]
            crate::internal::_mi_random_init_weak(ptr::addr_of_mut!(theap_main.random));
            #[cfg(not(any(target_os = "macos", windows)))]
            crate::internal::_mi_random_init(ptr::addr_of_mut!(theap_main.random));
            theap_main.cookie =
                crate::internal::_mi_theap_random_next(ptr::addr_of_mut!(theap_main));
            _mi_theap_options_init(ptr::addr_of_mut!(theap_main));
            _mi_theap_guarded_init(ptr::addr_of_mut!(theap_main));
        }
    }
}

fn mi_heap_main_init() {
    // SAFETY: mutates the statically allocated main heap during one-time lazy init.
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe {
        if heap_main.subproc.is_null() {
            heap_main.subproc = ptr::addr_of_mut!(subproc_main)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>();
            heap_main.theaps = ptr::addr_of_mut!(theap_main);

            mi_theap_main_init();
            mi_subproc_main_init();
            mi_tld_main_init();

            mi_lock_init(ptr::addr_of_mut!(heap_main.theaps_lock));
            mi_lock_init(ptr::addr_of_mut!(heap_main.os_abandoned_pages_lock));
            mi_lock_init(ptr::addr_of_mut!(heap_main.arena_pages_lock));
        }
    }
}

#[inline(always)]
fn mi_tld_alloc() -> *mut mi_tld_t {
    let main_subproc = ptr::addr_of_mut!(subproc_main)
        .cast::<MiSubprocPrefix>()
        .cast::<mi_subproc_t>();
    if mi_is_main_thread_fast() {
        mi_tld_main_init();
        // SAFETY: main-thread bookkeeping mutates only the process-global main subproc counters.
        // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
        unsafe {
            let subproc = mi_subproc_prefix(main_subproc);
            (*subproc).thread_count.fetch_add(1, Ordering::Relaxed);
            ptr::addr_of_mut!(tld_main)
        }
    } else {
        mi_tld_alloc_non_main(main_subproc)
    }
}

const _: fn() -> *mut mi_tld_t = mi_tld_alloc;

#[inline(always)]
fn mi_tld_alloc_non_main(main_subproc: *mut mi_subproc_t) -> *mut mi_tld_t {
    let Some(main_subproc) = SubprocPtr::new(main_subproc) else {
        return ptr::null_mut();
    };
    mi_tld_alloc_non_main_typed(main_subproc)
        .map(TldPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

#[inline(always)]
fn mi_tld_alloc_non_main_typed(main_subproc: SubprocPtr) -> Option<TldPtr> {
    // SAFETY: allocates a fresh zeroed tld block and initializes its local fields.
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        let mut memid = mi_memid_t::default();
        let tld_raw =
            _mi_meta_zalloc(size_of::<mi_tld_t>(), ptr::addr_of_mut!(memid)).cast::<mi_tld_t>();
        let Some(tld) = TldPtr::new(tld_raw) else {
            _mi_error_message_fmt(
                Errno::ENOMEM,
                format_args!("unable to allocate memory for thread local data\n"),
            );
            return None;
        };
        ptr::write(tld.memid_ptr(), memid);
        tld.set_thread_heaps(None);
        mi_lock_init(tld.thread_heaps_lock());
        tld.set_subproc(Some(main_subproc));
        tld.set_numa_node(crate::os::_mi_os_numa_node_typed());
        tld.set_thread_id(Some(ThreadId::new_unchecked(
            crate::prim::_mi_prim_thread_id(),
        )));
        let thread_seq = main_subproc
            .thread_total_count_cell()
            .fetch_add(1, Ordering::Relaxed);
        tld.set_thread_seq(crate::core_types::ThreadSeq::new(thread_seq));
        tld.set_is_in_threadpool(crate::prim::_mi_prim_thread_is_in_threadpool());
        main_subproc
            .thread_count_cell()
            .fetch_add(1, Ordering::Relaxed);
        Some(tld)
    }
}

#[inline(never)]
fn mi_tld_free(tld: *mut mi_tld_t) {
    if tld.is_null() || tld == MI_TLD_INVALID {
        return;
    }
    // SAFETY: `tld` must point to a valid local `mi_tld_t`.
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        mi_lock_done(ptr::addr_of_mut!((*tld).theaps_lock));
        if !(*tld).subproc.is_null() {
            let subproc = mi_subproc_prefix((*tld).subproc);
            (*subproc).thread_count.fetch_sub(1, Ordering::Relaxed);
        }
        if !ptr::eq(tld, ptr::addr_of_mut!(tld_main)) {
            _mi_meta_free(tld.cast::<c_void>(), size_of::<mi_tld_t>(), (*tld).memid);
        }
    }
}

// SAFETY: main subproc and main heap publication initialize process-global static metadata once.
/// # Safety
/// Returns a borrowed process-global opaque subproc pointer.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_subproc_main() -> *mut mi_subproc_t {
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe { mi_subproc_main_addr_typed().ptr().as_ptr() }
}

#[used]
static MI_SUBPROC_MAIN_KEEP: unsafe extern "C" fn() -> *mut mi_subproc_t = _mi_subproc_main;

// SAFETY: default thread-heap association validates local ownership before TLS publication.
/// # Safety
/// `theap` must be a valid default theap for the current thread.
pub(crate) unsafe extern "C" fn _mi_theap_default_set(theap: *mut mi_theap_t) {
    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    unsafe { tls::theap_default_set(theap) };
}

// SAFETY: cached thread-heap association validates local ownership before TLS publication.
/// # Safety
/// `theap` must be valid for the current thread or the empty sentinel.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_cached_set(theap: *mut mi_theap_t) {
    // SAFETY: cached thread-heap association validates local ownership before TLS publication.
    unsafe { tls::theap_cached_set(theap) };
}

// SAFETY: cached thread-heap association validates local ownership before TLS publication.
#[used]
static MI_THEAP_CACHED_SET_KEEP: unsafe extern "C" fn(*mut mi_theap_t) = _mi_theap_cached_set;

// SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
#[unsafe(no_mangle)]
/// # Safety
/// Marks the current thread-local data as belonging to a threadpool thread.
pub unsafe extern "C" fn mi_thread_set_in_threadpool() {
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    let theap = unsafe { _mi_theap_default_safe() };
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        (*(*theap).tld).is_in_threadpool = true;
    }
}

// SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
/// # Safety
/// Returns the current thread default theap, initializing thread state when needed.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_default_safe() -> *mut mi_theap_t {
    // SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
    let theap = unsafe { crate::prim::_mi_theap_default() };
    // SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    // SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
    unsafe { crate::mimalloc::mi_thread_init() };
    // SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
    let theap = unsafe { crate::prim::_mi_theap_default() };
    // SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::mi_theap_is_initialized(theap)
    });
    theap
}

// SAFETY: safe accessors expose current-thread heap TLS through typed witnesses.
#[used]
static MI_THEAP_DEFAULT_SAFE_KEEP: unsafe extern "C" fn() -> *mut mi_theap_t =
    _mi_theap_default_safe;

// SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
/// # Safety
/// Returns the process main theap, ensuring the default path initialized it first.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_main_safe() -> *mut mi_theap_t {
    let mut theap = mi_theap_main_tls_peek();
    if theap.is_null() {
        // SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
        unsafe { crate::mimalloc::mi_thread_init() };
        // SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
        theap = unsafe { crate::prim::_mi_theap_default() };
        crate::internal::mi_assert_internal!(!theap.is_null());
        // SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
        if unsafe { crate::internal::mi_theap_is_initialized(theap) }
            // SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
            && unsafe { crate::internal::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) }
        {
            mi_theap_main_tls_set(ThreadHeapPtr::new(theap));
        }
    }
    crate::internal::mi_assert_internal!(
        !theap.is_null()
            // SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
            && unsafe { crate::internal::mi_theap_is_initialized(theap) }
            // SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
            && unsafe { crate::internal::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) }
    );
    theap
}

// SAFETY: main thread-heap safe accessor initializes and validates main heap TLS before returning it.
#[used]
static MI_THEAP_MAIN_SAFE_KEEP: unsafe extern "C" fn() -> *mut mi_theap_t = _mi_theap_main_safe;

// SAFETY: empty thread-heap sentinel stays static and typed before TLS exposure.
/// # Safety
/// Returns the process-global empty theap sentinel.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_empty_get() -> *mut mi_theap_t {
    ptr::addr_of!(_mi_theap_empty).cast_mut()
}

// SAFETY: empty thread-heap sentinel stays static and typed before TLS exposure.
#[used]
static MI_THEAP_EMPTY_GET_KEEP: unsafe extern "C" fn() -> *mut mi_theap_t = _mi_theap_empty_get;

// SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
/// # Safety
/// Returns the current subproc without forcing thread-local initialization.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_subproc() -> *mut mi_subproc_t {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    unsafe { _mi_subproc_typed().ptr().as_ptr() }
}

unsafe fn _mi_subproc_typed() -> BorrowedSubproc {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    let theap = unsafe { crate::prim::_mi_theap_default_raw() };
    let subproc = if theap.is_null() {
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        unsafe { mi_subproc_main_addr_typed().ptr() }
    } else {
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        let tld = unsafe { TldPtr::new_unchecked((*theap).tld) };
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        unsafe { tld.subproc() }.unwrap_or_else(|| unsafe { mi_subproc_main_addr_typed().ptr() })
    };
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    unsafe { BorrowedSubproc::new(subproc) }
}

// SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
/// # Safety
/// `subproc_id` must have been produced by this crate's subproc API surface.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_subproc_from_id(
    subproc_id: mi_subproc_id_t,
) -> *mut mi_subproc_t {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    unsafe { _mi_subproc_from_id_typed(subproc_id) }
        .map(|subproc| subproc.ptr().as_ptr())
        .unwrap_or(ptr::null_mut())
}

unsafe fn _mi_subproc_from_id_typed(subproc_id: mi_subproc_id_t) -> Option<BorrowedSubproc> {
    SubprocPtr::new(subproc_id.as_ptr::<mi_subproc_t>())
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        .map(|subproc| unsafe { BorrowedSubproc::new(subproc) })
}

#[used]
static MI_SUBPROC_FROM_ID_KEEP: unsafe extern "C" fn(mi_subproc_id_t) -> *mut mi_subproc_t =
    _mi_subproc_from_id;

// SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
#[unsafe(no_mangle)]
/// # Safety
/// Returns an opaque identifier for the main subproc.
pub unsafe extern "C" fn mi_subproc_main() -> mi_subproc_id_t {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    _mi_subproc_to_id_typed(Some(unsafe { mi_subproc_main_addr_typed().ptr() }))
}

// SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
#[unsafe(no_mangle)]
/// # Safety
/// Returns the current subproc identifier for the bounded crate state.
pub unsafe extern "C" fn mi_subproc_current() -> mi_subproc_id_t {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    _mi_subproc_to_id_typed(Some(unsafe { _mi_subproc_typed().ptr() }))
}

// SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
/// # Safety
/// `subproc` must point to a valid local subproc prefix.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_subproc_heap_main(
    subproc: *mut mi_subproc_t,
) -> *mut mi_heap_t {
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    let subproc = unsafe { BorrowedSubproc::new(SubprocPtr::new_unchecked(subproc)) };
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    unsafe { _mi_subproc_heap_main_typed(subproc).ptr().as_ptr() }
}

unsafe fn _mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> MainHeap {
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    let heap = unsafe { mi_subproc_heap_main_typed(subproc) };
    if let Some(heap) = heap {
        // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
        return unsafe { MainHeap::new_unchecked(heap) };
    }
    mi_heap_main_init();
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    let heap = unsafe { mi_subproc_heap_main_typed(subproc) };
    crate::internal::mi_assert_internal!(heap.is_some());
    let Some(heap) = heap else {
        // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
        return unsafe { MainHeap::new_unchecked(HeapPtr::new_unchecked(mi_heap_main())) };
    };
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    unsafe { MainHeap::new_unchecked(heap) }
}

#[used]
static MI_SUBPROC_HEAP_MAIN_KEEP: unsafe extern "C" fn(*mut mi_subproc_t) -> *mut mi_heap_t =
    _mi_subproc_heap_main;

// SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
#[unsafe(no_mangle)]
/// # Safety
/// Returns the current subproc main heap for the bounded init/runtime scaffold.
pub unsafe extern "C" fn mi_heap_main() -> *mut mi_heap_t {
    // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
    let theap = unsafe { crate::prim::_mi_theap_default_raw() };
    let subproc = if theap.is_null() {
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        unsafe { mi_subproc_main_addr() }
    } else {
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        let tld = unsafe { (*theap).tld };
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        unsafe { (*tld).subproc }
    };
    // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
    let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if heap.is_null() {
        mi_heap_main_init();
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        heap = unsafe { mi_subproc_heap_main_raw(subproc) };
        crate::internal::mi_assert_internal!(!heap.is_null());
    }
    heap
}

// SAFETY: main-heap predicates compare validated heap and thread-heap associations.
/// # Safety
/// `heap` must point to a heap prefix compatible with this crate's local scaffold.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_is_heap_main(heap: *const mi_heap_t) -> bool {
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let subproc = unsafe { mi_heap_subproc_raw(heap) };
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if main_heap.is_null() {
        mi_heap_main_init();
        // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
        main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    ptr::eq(main_heap, heap.cast_mut())
}

// SAFETY: main-heap predicates compare validated heap and thread-heap associations.
/// # Safety
/// `theap` must be null or point to a valid thread-heap structure.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_is_theap_main(theap: *const mi_theap_t) -> bool {
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    if !unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return false;
    }
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let heap = unsafe { crate::internal::_mi_theap_heap(theap) };
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let subproc = unsafe { mi_heap_subproc_raw(heap) };
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if main_heap.is_null() {
        mi_heap_main_init();
        // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
        main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    ptr::eq(main_heap, heap)
}

// SAFETY: main-heap predicates compare validated heap and thread-heap associations.
#[used]
static MI_IS_THEAP_MAIN_KEEP: unsafe extern "C" fn(*const mi_theap_t) -> bool = _mi_is_theap_main;

#[inline(always)]
unsafe fn _mi_thread_init_theap_default() -> *mut mi_theap_t {
    // SAFETY: thread init installs a default heap only after process and TLD setup.
    let mut theap = unsafe { crate::prim::_mi_theap_default() };
    // SAFETY: thread init installs a default heap only after process and TLD setup.
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    if mi_is_main_thread_fast() {
        mi_heap_main_init();
        theap = ptr::addr_of_mut!(theap_main);
    } else {
        let tld = mi_tld_alloc_non_main(
            ptr::addr_of_mut!(subproc_main)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>(),
        );
        if tld.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: thread init installs a default heap only after process and TLD setup.
        theap = unsafe { _mi_theap_create(mi_heap_main(), tld) };
    }
    // SAFETY: thread init installs a default heap only after process and TLD setup.
    unsafe { _mi_theap_default_set(theap) };
    theap
}

unsafe fn mi_thread_theaps_done(tld: *mut mi_tld_t) {
    // SAFETY: thread cleanup resets default, cached, and main TLS state around heap teardown.
    let empty = unsafe { _mi_theap_empty_get() };
    // SAFETY: thread cleanup resets default, cached, and main TLS state around heap teardown.
    unsafe {
        _mi_theap_default_set(empty);
        _mi_theap_cached_set(empty);
        mi_theap_main_tls_set(None);
    }

    // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
    unsafe { mi_lock_acquire(ptr::addr_of_mut!((*tld).theaps_lock)) };
    // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
    let mut theap = unsafe { (*tld).theaps };
    while !theap.is_null() {
        // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
        let next = unsafe { (*theap).tnext };
        // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
        unsafe { _mi_theap_collect_abandon(theap) };
        // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
        crate::internal::mi_assert_internal!(unsafe { (*theap).page_count } == 0);
        theap = next;
    }
    // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
    unsafe { mi_lock_release(ptr::addr_of_mut!((*tld).theaps_lock)) };

    loop {
        let mut all_freed = true;
        // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
        unsafe { mi_lock_acquire(ptr::addr_of_mut!((*tld).theaps_lock)) };
        // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
        let mut theap = unsafe { (*tld).theaps };
        while !theap.is_null() {
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            let next = unsafe { (*theap).tnext };
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            crate::internal::mi_assert_internal!(unsafe { (*theap).page_count } == 0);
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            if !unsafe { _mi_theap_free(theap, true, false) } {
                all_freed = false;
            }
            theap = next;
        }
        // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
        unsafe { mi_lock_release(ptr::addr_of_mut!((*tld).theaps_lock)) };

        if !all_freed {
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            let subproc = unsafe { mi_subproc_prefix((*tld).subproc) };
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            unsafe {
                crate::internal::__mi_stat_counter_increase_mt(
                    ptr::addr_of_mut!((*subproc).stats.heaps_delete_wait),
                    1,
                )
            };
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            unsafe { crate::prim::_mi_prim_thread_yield() };
        } else {
            // SAFETY: thread cleanup drains local heaps and resets TLS under subproc lifecycle ordering.
            crate::internal::mi_assert_internal!(unsafe { (*tld).theaps }.is_null());
            break;
        }
    }

    // SAFETY: thread cleanup resets default, cached, and main TLS state around heap teardown.
    crate::internal::mi_assert_internal!(unsafe { crate::prim::_mi_theap_default() } == empty);
    // SAFETY: thread cleanup resets default, cached, and main TLS state around heap teardown.
    crate::internal::mi_assert_internal!(!unsafe {
        crate::internal::mi_theap_is_initialized(empty)
    }); // SAFETY: thread cleanup resets default, cached, and main TLS state around heap teardown.
}

#[unsafe(no_mangle)]
/// # Safety
/// Initializes per-thread mimalloc state for the current thread when it is not already set up.
// SAFETY: thread init entry coordinates process init, initialization checks, and thread stats.
pub unsafe extern "C" fn mi_thread_init() {
    // SAFETY: thread init entry coordinates process init, initialization checks, and thread stats.
    unsafe { mi_process_init() };
    // SAFETY: thread init entry coordinates process init, initialization checks, and thread stats.
    if unsafe { crate::prim::_mi_thread_is_initialized() } {
        return;
    }
    // SAFETY: thread init entry coordinates process init, initialization checks, and thread stats.
    if unsafe { _mi_thread_init_theap_default() }.is_null() {
        return;
    }
    // SAFETY: thread init entry coordinates process init, initialization checks, and thread stats.
    let heap = unsafe { mi_heap_main() };
    // SAFETY: thread init entry coordinates process init, initialization checks, and thread stats.
    unsafe {
        __mi_stat_increase_mt(
            ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
            1,
        )
    };
}

// SAFETY: subproc creation allocates metadata, initializes locks, and links into the registry under lock.
#[unsafe(no_mangle)]
/// # Safety
/// The returned opaque identifier must only be passed back to this crate's subproc functions.
pub unsafe extern "C" fn mi_subproc_new() -> mi_subproc_id_t {
    mi_subproc_main_init();
    let seq = SUBPROC_TOTAL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let mut memid = mi_memid_t::default();
    // SAFETY: subproc creation allocates metadata, initializes locks, and links into the registry under lock.
    let subproc_ptr = unsafe {
        _mi_meta_zalloc(size_of::<MiSubprocPrefix>(), ptr::addr_of_mut!(memid))
            .cast::<MiSubprocPrefix>()
    };
    if subproc_ptr.is_null() {
        // SAFETY: subproc creation allocates metadata, initializes locks, and links into the registry under lock.
        return unsafe { _mi_subproc_to_id(ptr::null_mut()) };
    }
    // SAFETY: subproc creation allocates metadata, initializes locks, and links into the registry under lock.
    unsafe {
        subproc_ptr.write(MiSubprocPrefix::new_with_memid(
            crate::core_types::SubprocSeq::new(seq),
            memid,
        ))
    };
    // SAFETY: initializing lock words for a freshly allocated local prefix.
    // SAFETY: subproc creation allocates metadata, initializes locks, and links into the registry under lock.
    unsafe {
        mi_lock_init(ptr::addr_of_mut!((*subproc_ptr).arena_reserve_lock));
        mi_lock_init(ptr::addr_of_mut!((*subproc_ptr).heaps_lock));
    }
    // SAFETY: local subproc list mutation is serialized by `subprocs_lock`.
    // SAFETY: subproc creation allocates metadata, initializes locks, and links into the registry under lock.
    unsafe {
        let lock =
            MiLockPtr::from_nonnull(NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock)));
        let _lock = MiLockGuard::acquire(lock);
        subproc_registry::push_front_locked(SubprocPtr::new_unchecked(
            subproc_ptr.cast::<mi_subproc_t>(),
        ));
        _mi_subproc_to_id(subproc_ptr.cast::<mi_subproc_t>())
    }
}

// SAFETY: public subproc destroy entrypoints validate IDs before registry-ordered destruction.
#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_destroy(subproc_id: mi_subproc_id_t) {
    mi_subproc_main_init();
    // SAFETY: raw ID comes from the C ABI surface.
    // SAFETY: public subproc destroy entrypoints validate IDs before registry-ordered destruction.
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    // SAFETY: public subproc destroy entrypoints validate IDs before registry-ordered destruction.
    if subproc.is_null() || core::ptr::eq(subproc, unsafe { _mi_subproc_main() }) {
        return;
    }
    // SAFETY: public subproc destroy entrypoints validate IDs before registry-ordered destruction.
    unsafe { mi_subproc_unsafe_destroy(subproc, true) };
}

// SAFETY: public subproc destroy entrypoints validate IDs before registry-ordered destruction.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
/// # Safety
/// Same requirements as `mi_subproc_destroy`.
pub unsafe extern "C" fn mi_subproc_delete(subproc_id: mi_subproc_id_t) {
    // SAFETY: public subproc destroy entrypoints validate IDs before registry-ordered destruction.
    unsafe { mi_subproc_destroy(subproc_id) };
}

// SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a local subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_add_current_thread(subproc_id: mi_subproc_id_t) {
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    if subproc.is_null() {
        return;
    }
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let theap = unsafe { _mi_theap_default_safe() };
    if theap.is_null() {
        return;
    }
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let tld = unsafe { (*theap).tld };
    if tld.is_null() {
        return;
    }
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let main_subproc = unsafe { _mi_subproc_main() };
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    if !ptr::eq(unsafe { (*tld).subproc }, main_subproc) {
        _mi_warning_message_fmt(format_args!(
            "unable to add thread to the subprocess as it was already in another subprocess (id: {:?})\n",
            subproc
        ));
        return;
    }
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let subproc_prefix = unsafe { mi_subproc_prefix(subproc) };
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let main_prefix = unsafe { mi_subproc_prefix(main_subproc) };
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    unsafe {
        (*tld).subproc = subproc;
        (*tld).thread_seq = crate::core_types::ThreadSeq::new(
            (*subproc_prefix)
                .thread_total_count
                .fetch_add(1, Ordering::Relaxed),
        )
        .get();
        (*main_prefix).thread_count.fetch_sub(1, Ordering::Relaxed);
        (*subproc_prefix)
            .thread_count
            .fetch_add(1, Ordering::Relaxed);
    }
}

// SAFETY: subproc heap visitation locks the heap list and invokes typed heap callbacks.
#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a local subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_visit_heaps(
    subproc_id: mi_subproc_id_t,
    visitor: Option<mi_heap_visit_fun>,
    arg: *mut c_void,
) -> bool {
    let visitor = HeapVisitCallback::new(visitor);
    if !visitor.is_some() {
        return false;
    };
    let arg = HeapVisitCallbackArg::from_raw(arg);
    // SAFETY: subproc heap visitation locks the heap list and invokes typed heap callbacks.
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    if subproc.is_null() {
        return false;
    }
    // SAFETY: subproc heap visitation locks the heap list and invokes typed heap callbacks.
    let subproc_prefix = unsafe { mi_subproc_prefix(subproc) };
    // SAFETY: subproc heap visitation locks the heap list and invokes typed heap callbacks.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!((*subproc_prefix).heaps_lock));
        let mut heap = (*subproc_prefix).heaps;
        let mut ok = true;
        while !heap.is_null() && ok {
            let next = (*(heap.cast::<MiHeapMainPrefix>())).next;
            ok = visitor.invoke(HeapPtr::new_unchecked(heap), arg);
            heap = next;
        }
        mi_lock_release(ptr::addr_of_mut!((*subproc_prefix).heaps_lock));
        ok
    }
}

unsafe fn mi_process_init_inner() {
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    if let Some(_once) = unsafe { mi_atomic_do_once_enter(ptr::addr_of!(PROCESS_INIT).cast_mut()) }
    {
        // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
        unsafe {
            _mi_process_is_initialized = true;
            cpu::mi_detect_cpu_features();
            mi_subproc_main_init();
            crate::stats::_mi_stats_init();
            crate::os::_mi_os_init();
            mi_heap_main_init();
            crate::threadlocal::_mi_thread_locals_init();
            let _ = crate::page_map::_mi_page_map_init();
            crate::prim::_mi_prim_thread_init_auto_done();
            _mi_theap_default_set(ptr::addr_of_mut!(theap_main));
            if !crate::prim::_mi_thread_is_initialized()
                && !_mi_thread_init_theap_default().is_null()
            {
                let heap = mi_heap_main();
                if !heap.is_null() {
                    __mi_stat_increase_mt(
                        &mut (*(heap.cast::<MiHeapMainPrefix>())).stats.threads,
                        1,
                    );
                }
            }
        }
    }
}

// SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
/// # Safety
/// Performs one-time process-global mimalloc initialization for the bounded init/runtime scaffold.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_auto_process_init() {
    OS_PRELOADING.store(false, Ordering::Relaxed);
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    unsafe { mi_process_init_inner() };
    mi_heap_main_init();
    tls::mi_tls_slots_init();
    if OPTIONS_POST_INIT_STATE
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        _mi_options_post_init();
        OPTIONS_POST_INIT_STATE.store(2, Ordering::Release);
    }
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    if unsafe { crate::prim::_mi_is_redirected() } {
        _mi_verbose_message(format_args!("malloc is redirected.\n"));
    }
    let mut msg: *const c_char = ptr::null();
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    unsafe { _mi_allocator_init(ptr::addr_of_mut!(msg)) };
    if !msg.is_null()
        && (mi_option_is_enabled(mi_option_t::mi_option_verbose)
            || mi_option_is_enabled(mi_option_t::mi_option_show_errors))
    {
        // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
        unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };
    }
    // SAFETY: random seeding/reseeding and CPU feature publication use process-local init witnesses.
    unsafe { crate::random::_mi_random_reinit_if_weak(ptr::addr_of_mut!(theap_main.random)) };
}

#[used]
static MI_AUTO_PROCESS_INIT_KEEP: unsafe extern "C" fn() = _mi_auto_process_init;

// SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global initialization.
pub unsafe extern "C" fn mi_process_init() {
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    unsafe { _mi_auto_process_init() };
}

// SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
/// # Safety
/// Performs process-global teardown for the bounded init/subproc surface and must not race with concurrent mutation of it.
#[inline(never)]
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
pub(crate) unsafe extern "C" fn _mi_auto_process_done() {
    if mi_option_get(mi_option_t::mi_option_destroy_on_exit) > 1 {
        return;
    }
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    unsafe { mi_process_done() };
}

#[used]
static MI_AUTO_PROCESS_DONE_KEEP: unsafe extern "C" fn() = _mi_auto_process_done;

// SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global teardown.
pub unsafe extern "C" fn mi_process_done() {
    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    if !unsafe { _mi_process_is_initialized } {
        return;
    }
    if PROCESS_DONE.swap(true, Ordering::AcqRel) {
        return;
    }

    // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
    unsafe {
        crate::threadlocal::_mi_thread_locals_done();
        crate::prim::_mi_prim_thread_done_auto_done();
        if mi_option_is_enabled(mi_option_t::mi_option_destroy_on_exit) {
            mi_subprocs_unsafe_destroy_all();
        } else {
            crate::stats::mi_heap_stats_merge_to_subproc(mi_heap_main());
        }
        if mi_option_is_enabled(mi_option_t::mi_option_show_stats)
            || mi_option_is_enabled(mi_option_t::mi_option_verbose)
        {
            crate::stats::mi_subproc_stats_print_out(mi_subproc_main(), None, ptr::null_mut());
        }
        tls::mi_tls_slots_done();
        _mi_allocator_done();
        OS_PRELOADING.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn auto_process_init_initializes_large_page_config() {
        // SAFETY: process init/teardown uses one-time state, preloading gates, and allocator lifecycle witnesses.
        unsafe { super::_mi_auto_process_init() };
        // SAFETY: process init validation uses process-global initialization witnesses before platform state checks.
        assert!(unsafe { crate::os::_mi_os_canuse_large_page(1 << 30, 2 << 20) });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_theap_main_tls_is_thread_local() {
        let previous = super::mi_theap_main_tls_peek();
        // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
        let marker = unsafe { super::_mi_theap_empty_get() };
        let marker_addr = marker as usize;

        super::mi_theap_main_tls_set(crate::core_types::ThreadHeapPtr::new(marker));
        assert_eq!(super::mi_theap_main_tls_peek(), marker);

        let child = std::thread::spawn(move || {
            let marker = marker_addr as *mut crate::types::mi_theap_t;
            let before = super::mi_theap_main_tls_peek() as usize;
            super::mi_theap_main_tls_set(crate::core_types::ThreadHeapPtr::new(marker));
            let after = super::mi_theap_main_tls_peek() as usize;
            super::mi_theap_main_tls_set(None);
            (before, after)
        })
        .join()
        .unwrap_or((usize::MAX, usize::MAX));

        assert_ne!(child.0, marker_addr);
        assert_eq!(child.1, marker_addr);
        assert_eq!(super::mi_theap_main_tls_peek(), marker);

        super::mi_theap_main_tls_set(crate::core_types::ThreadHeapPtr::new(previous));
    }
}
