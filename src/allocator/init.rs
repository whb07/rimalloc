//! Global / thread initialization

// Retains translated TLS/stat initialization macros that are active only on
// specific platform startup paths.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

mod cpu;
mod subproc;
mod subproc_registry;
mod thread;
mod tls;
use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use crate::allocator::options::{
    _mi_fputs, _mi_options_post_init, _mi_verbose_message, mi_option_get, mi_option_is_enabled,
    mi_option_t,
};
use crate::allocator::stats::{
    __mi_stat_increase_mt, mi_stat_count_t, mi_stats_header_init, mi_stats_t,
};
use crate::atomic::{
    mi_atomic_do_once_enter, mi_atomic_once_s, mi_atomic_once_t, mi_lock_init, mi_lock_t,
};
use crate::core_types::{BorrowedSubproc, HeapPtr, SubprocPtr};
use crate::internal::memid::mi_memid_t;
use crate::internal::{_mi_allocator_done, _mi_allocator_init};
use crate::types::{
    MI_BIN_COUNT, MI_BIN_FULL, MI_BIN_HUGE, MI_MAX_ARENAS, MI_PAGES_DIRECT, mi_heap_t,
    mi_heap_visit_fun, mi_page_queue_t, mi_page_t, mi_random_ctx_t, mi_subproc_id_t, mi_subproc_t,
    mi_theap_t, mi_thread_local_t, mi_threadid_t, mi_tld_t,
};
#[cfg(any(
    miri,
    not(any(
        all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")),
        all(target_os = "macos", target_arch = "aarch64")
    ))
))]
pub(crate) use tls::fallback_thread_id;
pub(crate) use tls::{
    _mi_theap_cached_expansion_slot, _mi_theap_cached_key, _mi_theap_cached_slot,
    _mi_theap_default_expansion_slot, _mi_theap_default_key, _mi_theap_default_slot,
    mi_theap_main_tls_peek, mi_theap_main_tls_set,
};

#[cfg(target_os = "linux")]
core::arch::global_asm!(".hidden _mi_theap_empty");
#[cfg(target_os = "linux")]
core::arch::global_asm!(".hidden _mi_cpu_stosb_max");
#[cfg(target_os = "linux")]
core::arch::global_asm!(".hidden subproc_main");

pub const MI_TLD_INVALID: *mut mi_tld_t = core::ptr::dangling_mut::<mi_tld_t>();

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
        crate::core_types::MemIdView::Static
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
    const ZERO_COUNTER: crate::allocator::stats::mi_stat_counter_t =
        crate::allocator::stats::mi_stat_counter_t { total: 0 };
    mi_stats_t {
        size: size_of::<mi_stats_t>(),
        version: crate::allocator::stats::MI_STAT_VERSION,
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
    slice_committed: crate::allocator::arena::MI_ARENA_SLICE_SIZE,
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
    crate::allocator::stats::MI_LARGE_MAX_OBJ_WSIZE + 1,
    crate::allocator::stats::MI_LARGE_MAX_OBJ_WSIZE + 2,
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
    // Starts non-static so `thread::theap_main_init` runs once and marks it static. C 3.3.2
    // initializes it with `MI_MEMID_STATIC`, which skips that initialization (`UPSTREAM_DIFF.md`).
    memid: mi_memid_none(),
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

#[unsafe(no_mangle)]
pub(crate) static mut _mi_cpu_stosb_max: usize = 0;

pub(crate) static mut _mi_process_is_initialized: bool = false;

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

/// Project a possibly null or dangling subprocess pointer to the corresponding
/// prefix address without accessing it. The raw result grants no validity,
/// liveness, or dereference permission.
fn mi_subproc_prefix(subproc: *mut mi_subproc_t) -> *mut MiSubprocPrefix {
    subproc.cast::<MiSubprocPrefix>()
}

/// Project a non-null subprocess pointer to its internal prefix address.
/// The returned raw pointer conveys no liveness or dereference permission.
fn mi_subproc_prefix_typed(subproc: SubprocPtr) -> *mut MiSubprocPrefix {
    subproc.as_ptr().cast::<MiSubprocPrefix>()
}

unsafe fn mi_subproc_heap_main_raw(subproc: *mut mi_subproc_t) -> *mut mi_heap_t {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        return ptr::null_mut();
    };
    let subproc = unsafe { BorrowedSubproc::new(subproc) };
    unsafe { subproc::heap_main_typed(subproc) }
        .map(HeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

/// Convert a possibly null subprocess pointer value into its opaque ID without
/// dereferencing or validating it. The returned ID conveys no liveness or
/// dereference permission and is only interpreted by subprocess entrypoints.
#[inline(never)]
pub(crate) extern "C" fn _mi_subproc_to_id(subproc: *mut mi_subproc_t) -> mi_subproc_id_t {
    subproc::to_id(SubprocPtr::new(subproc))
}

#[inline(never)]
pub(crate) extern "C" fn _mi_thread_id() -> mi_threadid_t {
    tls::current_thread_id()
}

#[inline(always)]
fn mi_is_main_thread_fast() -> bool {
    let main_tid = unsafe { tld_main.thread_id };
    main_tid == 0 || main_tid == tls::current_thread_id()
}

#[inline(never)]
pub extern "C" fn _mi_is_main_thread() -> bool {
    mi_is_main_thread_fast()
}

#[unsafe(no_mangle)]
/// Release allocator thread-local state for the current thread. Repeated calls
/// after the thread is already uninitialized are ignored.
pub extern "C" fn mi_thread_done() {
    // SAFETY: the null raw argument selects the current thread's allocator TLS;
    // the internal boundary validates locality and state before teardown.
    unsafe { crate::internal::_mi_thread_done(ptr::null_mut()) };
}

pub(crate) unsafe extern "C" fn _mi_thread_done(theap: *mut mi_theap_t) {
    unsafe { thread::done(theap) };
}

#[inline(never)]
pub(crate) extern "C" fn _mi_preloading() -> bool {
    OS_PRELOADING.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_is_redirected() -> bool {
    unsafe { crate::prim::_mi_is_redirected() }
}

/// # Safety
/// `theap` may be null. A nonnull value must remain live initialized allocator
/// metadata with stable heap/TLD ownership through the locality probe. If it is
/// current-thread local, its guarded sampling fields and any random state used
/// for a zero seed must be exclusively mutable; incompatible reentry, concurrent
/// local allocation/configuration, and teardown are excluded. Protocol remote
/// frees remain permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_guarded_set_sample_rate(
    theap: *mut mi_theap_t,
    sample_rate: usize,
    seed: usize,
) {
    #[cfg(feature = "guarded")]
    // SAFETY: the public contract keeps any nonnull heap and its ownership
    // metadata live through the internal locality probe and grants exclusive
    // local guarded/random mutation when classification succeeds.
    unsafe {
        thread::guarded_set_sample_rate(theap, sample_rate, seed)
    };
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        let _ = sample_rate;
        let _ = seed;
    }
}

/// # Safety
/// `theap` may be null. A nonnull value must remain live initialized allocator
/// metadata with stable heap/TLD ownership through the locality probe. If it is
/// current-thread local, its guarded size fields must be exclusively mutable;
/// incompatible reentry, concurrent local allocation/configuration, and
/// teardown are excluded. Protocol remote frees remain permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_guarded_set_size_bound(
    theap: *mut mi_theap_t,
    min: usize,
    max: usize,
) {
    #[cfg(feature = "guarded")]
    // SAFETY: the public contract keeps any nonnull heap and its ownership
    // metadata live through the internal locality probe and grants exclusive
    // local guarded-size mutation when classification succeeds.
    unsafe {
        thread::guarded_set_size_bound(theap, min, max)
    };
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        let _ = min;
        let _ = max;
    }
}

/// # Safety
/// `theap` may be null. With `guarded` enabled, a nonnull value must remain live
/// initialized allocator metadata with stable heap/TLD ownership through the
/// locality probe. Current-thread guarded fields and random state must be
/// exclusively mutable. Option storage and any registered diagnostic callback
/// plus opaque argument must remain valid; callback unwind, incompatible
/// reentry, and teardown are excluded. Concurrent atomic option updates and
/// protocol remote frees remain permitted. Without `guarded`, the pointer is
/// not accessed.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_guarded_init(theap: *mut mi_theap_t) {
    // SAFETY: the raw boundary contract supplies the exact live/locality,
    // option, guarded-field, and random-state obligations of the worker.
    unsafe { thread::guarded_init(theap) };
}

/// # Safety
/// `theap` may be null. A nonnull value must remain live initialized allocator
/// metadata with stable heap/TLD ownership through the locality probe. If it is
/// current-thread local, its allocation-policy fields must be exclusively
/// mutable. Option storage and any registered diagnostic callback plus opaque
/// argument must remain valid; callback unwind, incompatible reentry, and
/// teardown are excluded. Concurrent atomic option updates and protocol remote
/// frees remain permitted.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_options_init(theap: *mut mi_theap_t) {
    // SAFETY: the raw boundary contract supplies the worker's exact live,
    // locality, option-access, and exclusive policy-field mutation obligations.
    unsafe { thread::options_init(theap) };
}

fn mi_heap_main_init() {
    // SAFETY: mutates the statically allocated main heap during one-time lazy init.
    unsafe {
        if heap_main.subproc.is_null() {
            heap_main.subproc = ptr::addr_of_mut!(subproc_main)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>();
            heap_main.theaps = ptr::addr_of_mut!(theap_main);

            thread::theap_main_init();
            subproc::main_init();
            thread::tld_main_init();

            mi_lock_init(ptr::addr_of_mut!(heap_main.theaps_lock));
            mi_lock_init(ptr::addr_of_mut!(heap_main.os_abandoned_pages_lock));
            mi_lock_init(ptr::addr_of_mut!(heap_main.arena_pages_lock));
        }
    }
}

/// Returns the address of the process-global main subproc metadata. The raw
/// pointer carries no dereference permission; its address remains stable for
/// the process lifetime.
#[inline(never)]
pub(crate) extern "C" fn _mi_subproc_main() -> *mut mi_subproc_t {
    subproc::main_ptr()
}

pub(crate) unsafe extern "C" fn _mi_theap_default_set(theap: *mut mi_theap_t) {
    unsafe { tls::theap_default_set(theap) };
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_cached_set(theap: *mut mi_theap_t) {
    unsafe { tls::theap_cached_set(theap) };
}

#[unsafe(no_mangle)]
/// Mark the current thread-local allocator state as belonging to a threadpool
/// thread. This is a no-op if thread initialization cannot provide that state.
pub extern "C" fn mi_thread_set_in_threadpool() {
    thread::set_in_threadpool();
}

/// Returns the current-thread default theap after successful initialization, or
/// the static empty sentinel if initialization cannot provide one. The raw
/// pointer carries no lifetime or dereference permission.
#[inline(never)]
pub(crate) extern "C" fn _mi_theap_default_safe() -> *mut mi_theap_t {
    thread::default_safe()
}

/// Returns the current thread's theap associated with the main heap after
/// successful initialization, or the static empty sentinel if initialization
/// cannot provide one. The raw pointer carries no lifetime or dereference
/// permission.
#[inline(never)]
pub(crate) extern "C" fn _mi_theap_main_safe() -> *mut mi_theap_t {
    thread::main_safe()
}

/// Returns the current subproc without forcing thread-local initialization.
/// Before a current-thread association exists, this returns the process-global
/// main subproc. The raw pointer is non-owning and grants no dereference rights.
#[inline(never)]
pub(crate) extern "C" fn _mi_subproc() -> *mut mi_subproc_t {
    subproc::current_ptr()
}

// SAFETY: this is the crate's unique definition of the C ABI symbol.
#[unsafe(no_mangle)]
/// Returns an opaque identifier for the process-global main subproc.
pub extern "C" fn mi_subproc_main() -> mi_subproc_id_t {
    subproc::main_id()
}

// SAFETY: this is the crate's unique definition of the C ABI symbol.
#[unsafe(no_mangle)]
/// Returns the current subproc identifier without forcing thread-local
/// initialization. The opaque identifier is non-owning and does not extend the
/// lifetime of a current-thread subprocess association.
pub extern "C" fn mi_subproc_current() -> mi_subproc_id_t {
    subproc::current_id()
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_subproc_heap_main(
    subproc: *mut mi_subproc_t,
) -> *mut mi_heap_t {
    unsafe { subproc::ensure_heap_main(subproc) }
}

// SAFETY: this is the crate's unique definition of the C ABI symbol.
#[unsafe(no_mangle)]
/// Returns the current subprocess main heap, attempting the existing lazy
/// process-main initialization when needed. The raw pointer is non-owning and
/// grants no dereference or lifetime rights; it may remain null when a custom
/// subprocess has no registered main heap.
pub extern "C" fn mi_heap_main() -> *mut mi_heap_t {
    subproc::heap_main()
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_is_heap_main(heap: *const mi_heap_t) -> bool {
    unsafe { subproc::is_heap_main(heap) }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_is_theap_main(theap: *const mi_theap_t) -> bool {
    unsafe { subproc::is_theap_main(theap) }
}

#[unsafe(no_mangle)]
/// Initialize per-thread allocator state for the current thread when it is not
/// already set up. Initialization failure leaves the thread uninitialized.
pub extern "C" fn mi_thread_init() {
    thread::init();
}

#[unsafe(no_mangle)]
/// Allocate a subprocess and return its opaque identifier, or a null identifier
/// if its metadata allocation fails. A nonnull identifier may only be passed
/// back to this crate's subprocess functions.
pub extern "C" fn mi_subproc_new() -> mi_subproc_id_t {
    subproc::new()
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_destroy(subproc_id: mi_subproc_id_t) {
    unsafe { subproc::destroy(subproc_id) };
}

#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
/// # Safety
/// Same requirements as `mi_subproc_destroy`.
pub unsafe extern "C" fn mi_subproc_delete(subproc_id: mi_subproc_id_t) {
    unsafe { mi_subproc_destroy(subproc_id) };
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a local subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_add_current_thread(subproc_id: mi_subproc_id_t) {
    unsafe { subproc::add_current_thread(subproc_id) };
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a local subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_visit_heaps(
    subproc_id: mi_subproc_id_t,
    visitor: Option<mi_heap_visit_fun>,
    arg: *mut c_void,
) -> bool {
    unsafe { subproc::visit_heaps(subproc_id, visitor, arg) }
}

unsafe fn mi_process_init_inner() {
    if let Some(_once) = unsafe { mi_atomic_do_once_enter(ptr::addr_of!(PROCESS_INIT).cast_mut()) }
    {
        unsafe {
            _mi_process_is_initialized = true;
            cpu::mi_detect_cpu_features();
            subproc::main_init();
            crate::allocator::stats::_mi_stats_init();
            crate::os::_mi_os_init();
            mi_heap_main_init();
            crate::allocator::threadlocal::_mi_thread_locals_init();
            let _ = crate::allocator::page_map::_mi_page_map_init();
            crate::prim::_mi_prim_thread_init_auto_done();
            _mi_theap_default_set(ptr::addr_of_mut!(theap_main));
            if !crate::prim::_mi_thread_is_initialized() && !thread::init_default().is_null() {
                let heap = mi_heap_main();
                if !heap.is_null() {
                    __mi_stat_increase_mt(
                        ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
                        1,
                    );
                }
            }
        }
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_auto_process_init() {
    OS_PRELOADING.store(false, Ordering::Relaxed);
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
    if unsafe { crate::prim::_mi_is_redirected() } {
        _mi_verbose_message(format_args!("malloc is redirected.\n"));
    }
    let mut msg: *const c_char = ptr::null();
    unsafe { _mi_allocator_init(ptr::addr_of_mut!(msg)) };
    if !msg.is_null()
        && (mi_option_is_enabled(mi_option_t::mi_option_verbose)
            || mi_option_is_enabled(mi_option_t::mi_option_show_errors))
    {
        unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };
    }
    unsafe { crate::random::_mi_random_reinit_if_weak(ptr::addr_of_mut!(theap_main.random)) };
}

#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global initialization.
pub unsafe extern "C" fn mi_process_init() {
    unsafe { _mi_auto_process_init() };
}

#[inline(never)]
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
pub(crate) unsafe extern "C" fn _mi_auto_process_done() {
    if mi_option_get(mi_option_t::mi_option_destroy_on_exit) > 1 {
        return;
    }
    unsafe { mi_process_done() };
}

#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global teardown.
pub unsafe extern "C" fn mi_process_done() {
    if !unsafe { _mi_process_is_initialized } {
        return;
    }
    if PROCESS_DONE.swap(true, Ordering::AcqRel) {
        return;
    }

    unsafe {
        crate::allocator::threadlocal::_mi_thread_locals_done();
        crate::prim::_mi_prim_thread_done_auto_done();
        if mi_option_is_enabled(mi_option_t::mi_option_destroy_on_exit) {
            subproc::unsafe_destroy_all();
        } else {
            crate::allocator::stats::mi_heap_stats_merge_to_subproc(mi_heap_main());
        }
        if mi_option_is_enabled(mi_option_t::mi_option_show_stats)
            || mi_option_is_enabled(mi_option_t::mi_option_verbose)
        {
            crate::allocator::stats::mi_subproc_stats_print_out(
                mi_subproc_main(),
                None,
                ptr::null_mut(),
            );
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
        unsafe { super::_mi_auto_process_init() };
        assert!(unsafe { crate::os::_mi_os_canuse_large_page(1 << 30, 2 << 20) });
    }

    /// The main theap's one-time initialization runs: options apply to it and its cookie is drawn
    /// from its random state (C 3.3.2 skips both).
    #[test]
    fn main_theap_initialization_applies_options_and_cookie() {
        unsafe { super::_mi_auto_process_init() };
        // SAFETY: process initialization has completed; these reads observe settled fields.
        unsafe {
            let theap = core::ptr::addr_of!(super::theap_main);
            assert!(super::mi_memid_is_static((*theap).memid));
            assert_ne!((*theap).cookie, 0);
            let retain = crate::allocator::options::mi_option_get(
                crate::allocator::options::mi_option_t::mi_option_page_full_retain,
            )
            .clamp(-1, 32);
            assert_eq!((*theap).page_full_retain, retain);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_theap_main_tls_is_thread_local() {
        let previous = super::mi_theap_main_tls_peek();
        let marker = crate::platform::__mi_theap_empty();
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
