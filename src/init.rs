//! Global / thread initialization

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc
)]

use core::ffi::{c_long, c_void};
use core::mem::{MaybeUninit, size_of};
use core::ptr;
use std::sync::Once;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release, mi_lock_t};
use crate::bits::MI_SIZE_SIZE;
use crate::internal::{_mi_allocator_done, _mi_error_message, mi_memid_t, mi_memkind_t};
use crate::options::{_mi_warning_message, mi_option_get, mi_option_get_clamp, mi_option_t};
use crate::stats::{_mi_stats_merge_into, mi_stat_count_t, mi_stats_header_init, mi_stats_t};
use crate::types::{
    MI_BIN_COUNT, MI_BIN_FULL, MI_BIN_HUGE, MI_PAGES_DIRECT, mi_heap_t, mi_heap_visit_fun,
    mi_page_queue_t, mi_page_t, mi_random_ctx_t, mi_subproc_id_t, mi_subproc_t, mi_theap_t,
    mi_thread_local_t, mi_threadid_t, mi_tld_t,
};
#[cfg(windows)]
use crate::{prim::MI_TLS_EXPANSION_SLOT, types::EFAULT};

pub const MI_TLS_DIRECT_SLOTS: usize = 64;
pub const MI_TLS_EXPANSION_SLOTS: usize = 1024;
pub const MI_TLS_DIRECT_FIRST: usize = if MI_SIZE_SIZE == 4 {
    0x0E10 / MI_SIZE_SIZE
} else {
    0x1480 / MI_SIZE_SIZE
};
pub const MI_TLS_INITIAL_SLOT: usize = 5;
const MI_MAX_ARENAS: usize = 160;
#[allow(clippy::manual_dangling_ptr)]
pub const MI_TLD_INVALID: *mut mi_tld_t = 1usize as *mut mi_tld_t;
pub const TLS_OUT_OF_INDEXES: u32 = u32::MAX;

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

#[allow(non_snake_case)]
pub const fn MI_MEMID_INIT(kind: mi_memkind_t) -> mi_memid_t {
    mi_memid_t {
        mem: crate::internal::mi_memid_mem_t {
            os: crate::internal::mi_memid_os_info_t {
                base: ptr::null_mut(),
                size: 0,
            },
        },
        memkind: kind,
        is_pinned: true,
        initially_committed: true,
        initially_zero: false,
    }
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

#[allow(dead_code)]
static mut subprocs_lock: mi_lock_t = mi_lock_t::new();
static SUBPROC_MAIN_INIT: Once = Once::new();
static SUBPROC_TOTAL_COUNT: AtomicUsize = AtomicUsize::new(0);
static mut subproc_main: MaybeUninit<MiSubprocPrefix> = MaybeUninit::uninit();
static mut subprocs: *mut mi_subproc_t = ptr::null_mut();

#[allow(dead_code)]
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
    memid: MI_MEMID_INIT(mi_memkind_t::MI_MEM_STATIC),
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

#[allow(dead_code)]
static mut tld_main: mi_tld_t = mi_tld_t {
    thread_id: 0,
    thread_seq: 0,
    numa_node: 0,
    subproc: ptr::addr_of_mut!(subproc_main).cast::<mi_subproc_t>(),
    theaps: ptr::addr_of_mut!(theap_main),
    theaps_lock: mi_lock_t::new(),
    recurse: false,
    is_in_threadpool: false,
    memid: MI_MEMID_INIT(mi_memkind_t::MI_MEM_STATIC),
};

#[unsafe(no_mangle)]
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
    keys: [0, 0],
    theap: ptr::null_mut(),
    heap: ptr::null_mut(),
    next: ptr::null_mut(),
    prev: ptr::null_mut(),
    slice_committed: crate::arena::MI_ARENA_SLICE_SIZE,
    memid: MI_MEMID_INIT(mi_memkind_t::MI_MEM_STATIC),
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
    114688,
    131072,
    163840,
    196608,
    229376,
    262144,
    327680,
    393216,
    458752,
    524288,
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

#[unsafe(no_mangle)]
pub static _mi_theap_empty: mi_theap_t = mi_theap_t {
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
    pages_free_direct: MI_SMALL_PAGES_EMPTY(),
    pages: MI_PAGE_QUEUES_EMPTY(),
    memid: MI_MEMID_INIT(mi_memkind_t::MI_MEM_STATIC),
    stats: MI_STATS_NULL(),
};

#[unsafe(no_mangle)]
pub static _mi_theap_empty_wrong: mi_theap_t = mi_theap_t {
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
    pages_free_direct: MI_SMALL_PAGES_EMPTY(),
    pages: MI_PAGE_QUEUES_EMPTY(),
    memid: MI_MEMID_INIT(mi_memkind_t::MI_MEM_STATIC),
    stats: MI_STATS_NULL(),
};

#[unsafe(no_mangle)]
pub static mut theap_main: mi_theap_t = mi_theap_t {
    tld: ptr::addr_of_mut!(tld_main),
    heap: ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>(),
    refcount: 1,
    heartbeat: 0,
    cookie: 0,
    random: mi_random_ctx_t {
        input: [0x846ca68b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
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
    pages_free_direct: MI_SMALL_PAGES_EMPTY(),
    pages: MI_PAGE_QUEUES_EMPTY(),
    memid: MI_MEMID_INIT(mi_memkind_t::MI_MEM_STATIC),
    stats: MI_STATS_NULL(),
};

#[unsafe(no_mangle)]
pub static mut heap_main: MiHeapMainPrefix = MiHeapMainPrefix {
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
pub static mut _mi_cpu_movsb_max: usize = 0;

#[unsafe(no_mangle)]
pub static mut _mi_cpu_stosb_max: usize = 0;

#[unsafe(no_mangle)]
pub static mut _mi_theap_default_slot: usize = MI_TLS_INITIAL_SLOT;

#[unsafe(no_mangle)]
pub static mut _mi_theap_default_expansion_slot: usize = 0;

#[unsafe(no_mangle)]
pub static mut _mi_theap_cached_slot: usize = MI_TLS_INITIAL_SLOT;

#[unsafe(no_mangle)]
pub static mut _mi_theap_cached_expansion_slot: usize = 0;

#[unsafe(no_mangle)]
pub static mut mi_tls_raw_index_default: u32 = TLS_OUT_OF_INDEXES;

#[unsafe(no_mangle)]
pub static mut mi_tls_raw_index_cached: u32 = TLS_OUT_OF_INDEXES;

#[unsafe(no_mangle)]
pub static mut _mi_theap_default_key: mi_thread_local_t = 0;

#[unsafe(no_mangle)]
pub static mut _mi_theap_cached_key: mi_thread_local_t = 0;

#[unsafe(no_mangle)]
pub static mut __mi_theap_main: *mut mi_theap_t = ptr::null_mut();

#[unsafe(no_mangle)]
pub static mut _mi_process_is_initialized: bool = false;

#[unsafe(no_mangle)]
pub static mut _mi_cpu_has_popcnt: bool = false;

#[repr(C)]
struct MiSubprocPrefix {
    subproc_seq: usize,
    next: *mut mi_subproc_t,
    prev: *mut mi_subproc_t,
    arena_count: AtomicUsize,
    arenas: [*mut c_void; MI_MAX_ARENAS],
    arena_reserve_lock: mi_lock_t,
    purge_expire: std::sync::atomic::AtomicI64,
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
    fn new_main() -> Self {
        let memid = mi_memid_t {
            memkind: mi_memkind_t::MI_MEM_STATIC,
            is_pinned: true,
            initially_committed: true,
            initially_zero: true,
            ..mi_memid_t::default()
        };
        Self::new_with_memid(0, memid)
    }

    fn new_dynamic(seq: usize) -> Self {
        Self::new_with_memid(seq, mi_memid_t::default())
    }

    fn new_with_memid(seq: usize, memid: mi_memid_t) -> Self {
        let mut stats = mi_stats_t::default();
        // SAFETY: `stats` points to a valid local `mi_stats_t`.
        unsafe { mi_stats_header_init(&mut stats) };
        Self {
            subproc_seq: seq,
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
            arena_count: AtomicUsize::new(0),
            arenas: [ptr::null_mut(); MI_MAX_ARENAS],
            arena_reserve_lock: mi_lock_t::new(),
            purge_expire: std::sync::atomic::AtomicI64::new(0),
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
    subproc.cast::<MiSubprocPrefix>()
}

#[unsafe(no_mangle)]
/// # Safety
/// The returned opaque identifier must only be passed back to this crate's subproc functions.
pub unsafe extern "C" fn _mi_subproc_to_id(subproc: *mut mi_subproc_t) -> mi_subproc_id_t {
    mi_subproc_id_t {
        _mi_subproc_id: subproc.cast::<c_void>(),
    }
}

unsafe fn mi_subproc_is_reclaimable(subproc: *mut MiSubprocPrefix) -> bool {
    unsafe {
        (*subproc).heap_main.is_null()
            && (*subproc).heaps.is_null()
            && (*subproc).arena_count.load(Ordering::Acquire) == 0
            && (*subproc).heap_count.load(Ordering::Acquire) == 0
            && (*subproc).thread_count.load(Ordering::Acquire) == 0
    }
}

unsafe fn mi_subproc_destroy_boxed(subproc: *mut mi_subproc_t) {
    let subproc = unsafe { mi_subproc_prefix(subproc) };
    // SAFETY: `subproc` points to a boxed `MiSubprocPrefix` allocated by `mi_subproc_new`.
    unsafe {
        mi_lock_done(ptr::addr_of_mut!((*subproc).arena_reserve_lock));
        mi_lock_done(ptr::addr_of_mut!((*subproc).heaps_lock));
        drop(Box::from_raw(subproc));
    }
}

fn mi_subproc_main_init() {
    SUBPROC_MAIN_INIT.call_once(|| {
        // SAFETY: one-time process initialization for local subproc scaffolding.
        unsafe {
            mi_lock_init(ptr::addr_of_mut!(subprocs_lock));
            ptr::addr_of_mut!(subproc_main).write(MaybeUninit::new(MiSubprocPrefix::new_main()));
            let main = ptr::addr_of_mut!(subproc_main)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>();
            let main_prefix = mi_subproc_prefix(main);
            (*main_prefix).heaps = ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>();
            (*main_prefix).heap_main = ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>();
            (*main_prefix).heap_total_count.store(1, Ordering::Relaxed);
            (*main_prefix).heap_count.store(1, Ordering::Relaxed);
            mi_lock_init(ptr::addr_of_mut!(tld_empty.theaps_lock));
            subprocs = main;
            _mi_process_is_initialized = true;
        }
    });
}

unsafe fn mi_subproc_main_ptr() -> *mut mi_subproc_t {
    mi_subproc_main_init();
    ptr::addr_of_mut!(subproc_main)
        .cast::<MiSubprocPrefix>()
        .cast::<mi_subproc_t>()
}

unsafe fn mi_subproc_unlink_locked(subproc: *mut MiSubprocPrefix) {
    unsafe {
        if !(*subproc).next.is_null() {
            (*mi_subproc_prefix((*subproc).next)).prev = (*subproc).prev;
        }
        if !(*subproc).prev.is_null() {
            (*mi_subproc_prefix((*subproc).prev)).next = (*subproc).next;
        } else {
            subprocs = (*subproc).next;
        }
        (*subproc).next = ptr::null_mut();
        (*subproc).prev = ptr::null_mut();
    }
}

unsafe fn mi_subproc_unsafe_destroy(subproc: *mut mi_subproc_t, acquire_subprocs_lock: bool) {
    if subproc.is_null() {
        return;
    }
    let is_main = ptr::eq(subproc, unsafe { _mi_subproc_main() });
    let subproc_prefix = unsafe { mi_subproc_prefix(subproc) };
    if acquire_subprocs_lock {
        unsafe { mi_lock_acquire(ptr::addr_of_mut!(subprocs_lock)) };
    }
    unsafe { mi_subproc_unlink_locked(subproc_prefix) };
    if acquire_subprocs_lock {
        unsafe { mi_lock_release(ptr::addr_of_mut!(subprocs_lock)) };
    }

    unsafe {
        (*subproc_prefix).heaps = ptr::null_mut();
        (*subproc_prefix).heap_main = ptr::null_mut();
        (*subproc_prefix).thread_count.store(0, Ordering::Relaxed);
        (*subproc_prefix).heap_count.store(0, Ordering::Relaxed);
        (*subproc_prefix).arena_count.store(0, Ordering::Relaxed);
    }

    if is_main {
        unsafe { crate::page_map::_mi_page_map_unsafe_destroy(subproc) };
    } else {
        let main_subproc = unsafe { mi_subproc_prefix(_mi_subproc_main()) };
        unsafe {
            _mi_stats_merge_into(
                ptr::addr_of_mut!((*main_subproc).stats),
                ptr::addr_of_mut!((*subproc_prefix).stats),
            );
            mi_subproc_destroy_boxed(subproc);
        }
    }
}

unsafe fn mi_subprocs_unsafe_destroy_all() {
    mi_subproc_main_init();
    let main_subproc = unsafe { _mi_subproc_main() };
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(subprocs_lock));
        let mut subproc = subprocs;
        while !subproc.is_null() {
            let next = (*mi_subproc_prefix(subproc)).next;
            if !ptr::eq(subproc, main_subproc) {
                mi_subproc_unsafe_destroy(subproc, false);
            }
            subproc = next;
        }
        mi_lock_release(ptr::addr_of_mut!(subprocs_lock));
        mi_subproc_unsafe_destroy(main_subproc, true);
    }
}

static MAIN_THREAD_ID: AtomicUsize = AtomicUsize::new(0);
static NEXT_THREAD_ID: AtomicUsize = AtomicUsize::new(8);

thread_local! {
    static THREAD_ID: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static THEAP_MAIN: std::cell::Cell<*mut mi_theap_t> = const { std::cell::Cell::new(ptr::null_mut()) };
}

#[inline]
pub(crate) fn mi_theap_main_tls_peek() -> *mut mi_theap_t {
    THEAP_MAIN.with(std::cell::Cell::get)
}

#[inline]
pub(crate) fn mi_theap_main_tls_set(theap: *mut mi_theap_t) {
    THEAP_MAIN.with(|slot| slot.set(theap));
}

#[inline]
fn current_thread_id() -> mi_threadid_t {
    THREAD_ID.with(|slot| {
        let tid = slot.get();
        if tid != 0 {
            tid
        } else {
            let tid = NEXT_THREAD_ID.fetch_add(4, Ordering::Relaxed);
            slot.set(tid);
            tid
        }
    })
}

#[allow(dead_code)]
pub(crate) fn mi_mark_main_thread() {
    let tid = current_thread_id();
    let _ = MAIN_THREAD_ID.compare_exchange(0, tid, Ordering::AcqRel, Ordering::Acquire);
}

#[unsafe(no_mangle)]
pub extern "C" fn _mi_thread_id() -> mi_threadid_t {
    current_thread_id()
}

pub extern "C" fn _mi_is_main_thread() -> bool {
    let main_tid = MAIN_THREAD_ID.load(Ordering::Acquire);
    main_tid == 0 || main_tid == current_thread_id()
}

#[unsafe(no_mangle)]
/// # Safety
/// Delegates to the internal thread teardown routine for the current thread.
pub unsafe extern "C" fn mi_thread_done() {
    unsafe { crate::internal::_mi_thread_done(ptr::null_mut()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn _mi_preloading() -> bool {
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_is_redirected() -> bool {
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_theap_guarded_set_sample_rate(
    theap: *mut mi_theap_t,
    sample_rate: usize,
    seed: usize,
) {
    let _ = theap;
    let _ = sample_rate;
    let _ = seed;
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_theap_guarded_set_size_bound(theap: *mut mi_theap_t, min: usize, max: usize) {
    let _ = theap;
    let _ = min;
    let _ = max;
}

#[unsafe(no_mangle)]
pub extern "C" fn _mi_theap_guarded_init(theap: *mut mi_theap_t) {
    let _ = theap;
}

#[unsafe(no_mangle)]
/// # Safety
/// `theap` must be valid when non-null.
pub unsafe extern "C" fn _mi_theap_options_init(theap: *mut mi_theap_t) {
    if theap.is_null() {
        return;
    }
    unsafe {
        (*theap).allow_page_reclaim =
            mi_option_get(mi_option_t::mi_option_page_reclaim_on_free) >= 0;
        (*theap).allow_page_abandon = mi_option_get(mi_option_t::mi_option_page_full_retain) >= 0;
        (*theap).page_full_retain = mi_option_get_clamp(
            mi_option_t::mi_option_page_full_retain,
            -1,
            c_long::from(32),
        );
    }
}

#[allow(dead_code)]
fn mi_tld_main_init() {
    // SAFETY: mutates the statically allocated main-thread tld during one-time lazy init.
    unsafe {
        if tld_main.thread_id == 0 {
            tld_main.thread_id = crate::prim::_mi_prim_thread_id();
            mi_lock_init(ptr::addr_of_mut!(tld_main.theaps_lock));
        }
    }
}

#[allow(dead_code)]
fn mi_theap_main_init() {
    // SAFETY: mutates the statically allocated main theap during one-time lazy init.
    unsafe {
        if theap_main.memid.memkind != mi_memkind_t::MI_MEM_STATIC {
            theap_main.memid = crate::internal::_mi_memid_create(mi_memkind_t::MI_MEM_STATIC);
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

#[allow(dead_code)]
fn mi_heap_main_init() {
    // SAFETY: mutates the statically allocated main heap during one-time lazy init.
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

#[allow(dead_code)]
fn mi_tld_alloc() -> *mut mi_tld_t {
    mi_subproc_main_init();
    if _mi_is_main_thread() {
        mi_tld_main_init();
        // SAFETY: main-thread bookkeeping mutates only the process-global main subproc counters.
        unsafe {
            let subproc = mi_subproc_prefix(mi_subproc_main_ptr());
            (*subproc).thread_count.fetch_add(1, Ordering::Relaxed);
            ptr::addr_of_mut!(tld_main)
        }
    } else {
        // SAFETY: allocates a fresh zeroed tld block and initializes its local fields.
        unsafe {
            let mut memid = mi_memid_t::default();
            let tld =
                crate::internal::_mi_os_zalloc(size_of::<mi_tld_t>(), ptr::addr_of_mut!(memid))
                    .cast::<mi_tld_t>();
            if tld.is_null() {
                _mi_error_message(
                    crate::types::ENOMEM,
                    c"unable to allocate memory for thread local data\n".as_ptr(),
                );
                return ptr::null_mut();
            }
            (*tld).memid = memid;
            (*tld).theaps = ptr::null_mut();
            mi_lock_init(ptr::addr_of_mut!((*tld).theaps_lock));
            (*tld).subproc = mi_subproc_main_ptr();
            (*tld).numa_node = crate::prim::_mi_prim_numa_node() as i32;
            (*tld).thread_id = crate::prim::_mi_prim_thread_id();
            let subproc = mi_subproc_prefix((*tld).subproc);
            (*tld).thread_seq = (*subproc)
                .thread_total_count
                .fetch_add(1, Ordering::Relaxed);
            (*tld).is_in_threadpool = crate::prim::_mi_prim_thread_is_in_threadpool();
            (*subproc).thread_count.fetch_add(1, Ordering::Relaxed);
            tld
        }
    }
}

#[inline(never)]
#[allow(dead_code)]
fn mi_tld_free(tld: *mut mi_tld_t) {
    if tld.is_null() || tld == MI_TLD_INVALID {
        return;
    }
    // SAFETY: `tld` must point to a valid local `mi_tld_t`.
    unsafe {
        mi_lock_done(ptr::addr_of_mut!((*tld).theaps_lock));
        if !(*tld).subproc.is_null() {
            let subproc = mi_subproc_prefix((*tld).subproc);
            (*subproc).thread_count.fetch_sub(1, Ordering::Relaxed);
        }
        if !ptr::eq(tld, ptr::addr_of_mut!(tld_main)) {
            crate::internal::_mi_os_free(tld.cast::<c_void>(), size_of::<mi_tld_t>(), (*tld).memid);
        }
    }
}

unsafe extern "C" {
    fn _mi_theap_collect_abandon(theap: *mut mi_theap_t);
    fn _mi_theap_create(heap: *mut mi_heap_t, tld: *mut mi_tld_t) -> *mut mi_theap_t;
    fn _mi_theap_free(
        theap: *mut mi_theap_t,
        acquire_heap_theaps_lock: bool,
        acquire_tld_theaps_lock: bool,
    ) -> bool;
}

#[cfg(any(target_os = "openbsd", target_os = "android"))]
unsafe extern "C" {
    fn pthread_key_delete(key: mi_thread_local_t) -> i32;
}

#[cfg(windows)]
unsafe extern "system" {
    fn TlsAlloc() -> u32;
    fn TlsFree(dw_tls_index: u32) -> i32;
    fn TlsSetValue(dw_tls_index: u32, lp_tls_value: *mut c_void) -> i32;
}

#[cfg(windows)]
#[allow(dead_code)]
unsafe fn mi_win_tls_slot_alloc(
    slot: *mut usize,
    extended: *mut usize,
    raw_index: *mut u32,
) -> bool {
    let index = unsafe { TlsAlloc() };
    unsafe { raw_index.write(index) };
    if index == TLS_OUT_OF_INDEXES {
        unsafe {
            extended.write(0);
            slot.write(0);
        }
        false
    } else if usize::try_from(index)
        .ok()
        .is_some_and(|i| i < MI_TLS_DIRECT_SLOTS)
    {
        unsafe {
            extended.write(0);
            slot.write(index as usize + MI_TLS_DIRECT_FIRST);
        }
        true
    } else if usize::try_from(index)
        .ok()
        .is_some_and(|i| i < MI_TLS_DIRECT_SLOTS + MI_TLS_EXPANSION_SLOTS - 1)
    {
        unsafe {
            extended.write(index as usize - MI_TLS_DIRECT_SLOTS);
            slot.write(MI_TLS_EXPANSION_SLOT);
        }
        true
    } else {
        unsafe {
            _mi_error_message(
                EFAULT,
                c"returned tls index was too high (%u)\n".as_ptr(),
                index,
            );
            let _ = TlsFree(index);
            raw_index.write(TLS_OUT_OF_INDEXES);
            extended.write(0);
            slot.write(0);
        }
        false
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
unsafe fn mi_win_tls_slot_alloc(
    slot: *mut usize,
    extended: *mut usize,
    raw_index: *mut u32,
) -> bool {
    unsafe {
        raw_index.write(TLS_OUT_OF_INDEXES);
        extended.write(0);
        slot.write(0);
    }
    false
}

#[cfg(windows)]
#[allow(dead_code)]
unsafe fn mi_win_tls_slot_free(raw_index: *mut u32) {
    let index = unsafe { raw_index.read() };
    if index != TLS_OUT_OF_INDEXES {
        unsafe {
            let _ = TlsFree(index);
            raw_index.write(TLS_OUT_OF_INDEXES);
        }
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
unsafe fn mi_win_tls_slot_free(raw_index: *mut u32) {
    unsafe { raw_index.write(TLS_OUT_OF_INDEXES) };
}

#[cfg(windows)]
#[allow(dead_code)]
unsafe fn mi_win_tls_slot_set(slot: usize, extended_slot: usize, value: *mut c_void) {
    crate::mi_assert_internal!(
        (slot >= MI_TLS_DIRECT_FIRST && slot < MI_TLS_DIRECT_FIRST + MI_TLS_DIRECT_SLOTS)
            || slot == MI_TLS_EXPANSION_SLOT
    );
    if slot < MI_TLS_DIRECT_FIRST + MI_TLS_DIRECT_SLOTS {
        crate::prim::mi_prim_tls_slot_set(slot, value);
    } else {
        crate::mi_assert_internal!(extended_slot < MI_TLS_EXPANSION_SLOTS);
        let _ = unsafe { TlsSetValue((extended_slot + MI_TLS_DIRECT_SLOTS) as u32, value) };
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
unsafe fn mi_win_tls_slot_set(_slot: usize, _extended_slot: usize, _value: *mut c_void) {}

#[cfg(any(target_os = "openbsd", target_os = "android"))]
#[allow(dead_code)]
unsafe fn mi_pthread_key_create(pkey: *mut mi_thread_local_t) -> i32 {
    let key = crate::threadlocal::_mi_thread_local_create();
    if key == 0 {
        EFAULT
    } else {
        unsafe { pkey.write(key) };
        0
    }
}

#[cfg(not(any(target_os = "openbsd", target_os = "android")))]
#[allow(dead_code)]
unsafe fn mi_pthread_key_create(pkey: *mut mi_thread_local_t) -> i32 {
    unsafe { pkey.write(0) };
    0
}

#[allow(dead_code)]
fn mi_tls_slots_init() {
    static TLS_SLOTS_INIT: Once = Once::new();
    TLS_SLOTS_INIT.call_once(|| {
        #[cfg(windows)]
        // SAFETY: one-time initialization of process-global TLS slot metadata.
        unsafe {
            let mut ok = mi_win_tls_slot_alloc(
                ptr::addr_of_mut!(_mi_theap_default_slot),
                ptr::addr_of_mut!(_mi_theap_default_expansion_slot),
                ptr::addr_of_mut!(mi_tls_raw_index_default),
            );
            if ok {
                ok = mi_win_tls_slot_alloc(
                    ptr::addr_of_mut!(_mi_theap_cached_slot),
                    ptr::addr_of_mut!(_mi_theap_cached_expansion_slot),
                    ptr::addr_of_mut!(mi_tls_raw_index_cached),
                );
            }
            if !ok {
                _mi_error_message(
                    EFAULT,
                    c"unable to allocate fast TLS user slot (0x%zx)\n".as_ptr(),
                    _mi_theap_cached_slot,
                );
            }
        }

        #[cfg(any(target_os = "openbsd", target_os = "android"))]
        unsafe {
            let mut err = mi_pthread_key_create(ptr::addr_of_mut!(_mi_theap_default_key));
            if err == 0 {
                err = mi_pthread_key_create(ptr::addr_of_mut!(_mi_theap_cached_key));
            }
            if err != 0 {
                _mi_error_message(
                    EFAULT,
                    c"unable to allocate pthread keys (error %d)\n".as_ptr(),
                    err,
                );
            }
        }
    });
}

#[allow(dead_code)]
fn mi_tls_slots_done() {
    #[cfg(windows)]
    unsafe {
        mi_win_tls_slot_free(ptr::addr_of_mut!(mi_tls_raw_index_default));
        mi_win_tls_slot_free(ptr::addr_of_mut!(mi_tls_raw_index_cached));
    }

    #[cfg(any(target_os = "openbsd", target_os = "android"))]
    unsafe {
        if _mi_theap_default_key != 0 {
            let _ = pthread_key_delete(_mi_theap_default_key);
            _mi_theap_default_key = 0;
        }
        if _mi_theap_cached_key != 0 {
            let _ = pthread_key_delete(_mi_theap_cached_key);
            _mi_theap_cached_key = 0;
        }
    }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    not(target_env = "msvc")
))]
#[allow(dead_code)]
fn mi_cpuid(regs4: *mut u32, level: u32, sublevel: u32) -> bool {
    let regs = {
        #[cfg(target_arch = "x86")]
        {
            // Mirrors the GNU C path while preserving the sublevel input.
            unsafe { core::arch::x86::__cpuid_count(level, sublevel) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            // Mirrors the GNU C path while preserving the sublevel input.
            unsafe { core::arch::x86_64::__cpuid_count(level, sublevel) }
        }
    };
    unsafe {
        regs4.add(0).write(regs.eax);
        regs4.add(1).write(regs.ebx);
        regs4.add(2).write(regs.ecx);
        regs4.add(3).write(regs.edx);
    }
    true
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_env = "msvc"))]
#[allow(dead_code)]
fn mi_cpuid(regs4: *mut u32, level: u32, sublevel: u32) -> bool {
    let regs = {
        #[cfg(target_arch = "x86")]
        {
            unsafe { core::arch::x86::__cpuid_count(level, sublevel) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            unsafe { core::arch::x86_64::__cpuid_count(level, sublevel) }
        }
    };
    unsafe {
        regs4.add(0).write(regs.eax);
        regs4.add(1).write(regs.ebx);
        regs4.add(2).write(regs.ecx);
        regs4.add(3).write(regs.edx);
    }
    true
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[allow(dead_code)]
fn mi_cpuid(_regs4: *mut u32, _level: u32, _sublevel: u32) -> bool {
    false
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns a borrowed process-global opaque subproc pointer.
pub unsafe extern "C" fn _mi_subproc_main() -> *mut mi_subproc_t {
    unsafe { mi_subproc_main_ptr() }
}

#[unsafe(no_mangle)]
/// # Safety
/// `theap` must be a valid default theap for the current thread.
pub unsafe extern "C" fn _mi_theap_default_set(theap: *mut mi_theap_t) {
    let theap_old = unsafe { crate::prim::_mi_theap_default() };
    crate::mi_assert_internal!(!theap.is_null());
    crate::mi_assert_internal!(
        unsafe { (*(*theap).tld).thread_id } == 0
            || unsafe { (*(*theap).tld).thread_id } == _mi_thread_id()
    );

    mi_tls_slots_init();
    if crate::prim::MI_TLS_MODEL_THREAD_LOCAL {
        crate::prim::mi_prim_thread_local_default_set(theap);
    } else if crate::prim::MI_TLS_MODEL_FIXED_SLOT {
        crate::prim::mi_prim_tls_slot_set(
            crate::prim::MI_TLS_MODEL_FIXED_SLOT_DEFAULT,
            theap.cast(),
        );
    } else if crate::prim::MI_TLS_MODEL_DYNAMIC_WIN32 {
        unsafe {
            mi_win_tls_slot_set(
                _mi_theap_default_slot,
                _mi_theap_default_expansion_slot,
                theap.cast(),
            );
        }
    } else if crate::prim::MI_TLS_MODEL_DYNAMIC_PTHREADS && unsafe { _mi_theap_default_key } != 0 {
        let _ = crate::threadlocal::_mi_thread_local_set(
            unsafe { _mi_theap_default_key },
            theap.cast(),
        );
    }

    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        unsafe { crate::prim::_mi_prim_thread_associate_default_theap(theap) };
        if unsafe { _mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) } {
            mi_theap_main_tls_set(theap);
        }
    }

    if unsafe { crate::internal::mi_theap_is_initialized(theap_old) }
        && unsafe { _mi_is_heap_main(crate::internal::_mi_theap_heap(theap_old)) }
    {
        mi_theap_main_tls_set(theap_old);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `theap` must be valid for the current thread or the empty sentinel.
pub unsafe extern "C" fn _mi_theap_cached_set(theap: *mut mi_theap_t) {
    let prev = unsafe { crate::prim::_mi_theap_cached() };
    if ptr::eq(prev, theap) {
        return;
    }

    mi_tls_slots_init();
    if crate::prim::MI_TLS_MODEL_THREAD_LOCAL {
        crate::prim::mi_prim_thread_local_cached_set(theap);
    } else if crate::prim::MI_TLS_MODEL_FIXED_SLOT {
        crate::prim::mi_prim_tls_slot_set(
            crate::prim::MI_TLS_MODEL_FIXED_SLOT_CACHED,
            theap.cast(),
        );
    } else if crate::prim::MI_TLS_MODEL_DYNAMIC_WIN32 {
        unsafe {
            mi_win_tls_slot_set(
                _mi_theap_cached_slot,
                _mi_theap_cached_expansion_slot,
                theap.cast(),
            );
        }
    } else if crate::prim::MI_TLS_MODEL_DYNAMIC_PTHREADS && unsafe { _mi_theap_cached_key } != 0 {
        let _ =
            crate::threadlocal::_mi_thread_local_set(unsafe { _mi_theap_cached_key }, theap.cast());
    }

    unsafe {
        crate::internal::_mi_theap_incref(theap);
        crate::internal::_mi_theap_decref(prev);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Marks the current thread-local data as belonging to a threadpool thread.
pub unsafe extern "C" fn mi_thread_set_in_threadpool() {
    let theap = unsafe { _mi_theap_default_safe() };
    unsafe {
        (*(*theap).tld).is_in_threadpool = true;
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the current thread default theap, initializing thread state when needed.
pub unsafe extern "C" fn _mi_theap_default_safe() -> *mut mi_theap_t {
    let theap = unsafe { crate::prim::_mi_theap_default() };
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    unsafe { crate::mimalloc::mi_thread_init() };
    let theap = unsafe { crate::prim::_mi_theap_default() };
    crate::mi_assert_internal!(unsafe { crate::internal::mi_theap_is_initialized(theap) });
    theap
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the process main theap, ensuring the default path initialized it first.
pub unsafe extern "C" fn _mi_theap_main_safe() -> *mut mi_theap_t {
    let mut theap = mi_theap_main_tls_peek();
    if theap.is_null() {
        unsafe { crate::mimalloc::mi_thread_init() };
        theap = unsafe { crate::prim::_mi_theap_default() };
        crate::mi_assert_internal!(!theap.is_null());
        if unsafe { crate::internal::mi_theap_is_initialized(theap) }
            && unsafe { crate::internal::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) }
        {
            mi_theap_main_tls_set(theap);
        }
    }
    crate::mi_assert_internal!(
        !theap.is_null()
            && unsafe { crate::internal::mi_theap_is_initialized(theap) }
            && unsafe { crate::internal::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) }
    );
    theap
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the process-global empty theap sentinel.
pub unsafe extern "C" fn _mi_theap_empty_get() -> *mut mi_theap_t {
    ptr::addr_of!(_mi_theap_empty).cast_mut()
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the current subproc without forcing thread-local initialization.
pub unsafe extern "C" fn _mi_subproc() -> *mut mi_subproc_t {
    let theap = unsafe { crate::prim::_mi_theap_default() };
    if theap.is_null() {
        unsafe { _mi_subproc_main() }
    } else {
        let tld = unsafe { (*theap).tld };
        if tld.is_null() {
            unsafe { _mi_subproc_main() }
        } else {
            unsafe { (*tld).subproc }
        }
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must have been produced by this crate's subproc API surface.
pub unsafe extern "C" fn _mi_subproc_from_id(subproc_id: mi_subproc_id_t) -> *mut mi_subproc_t {
    subproc_id._mi_subproc_id.cast::<mi_subproc_t>()
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns an opaque identifier for the main subproc.
pub unsafe extern "C" fn mi_subproc_main() -> mi_subproc_id_t {
    unsafe { _mi_subproc_to_id(_mi_subproc_main()) }
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the current subproc identifier for the bounded crate state.
pub unsafe extern "C" fn mi_subproc_current() -> mi_subproc_id_t {
    unsafe { _mi_subproc_to_id(_mi_subproc()) }
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc` must be null or point to a valid local subproc prefix.
pub unsafe extern "C" fn _mi_subproc_heap_main(subproc: *mut mi_subproc_t) -> *mut mi_heap_t {
    if subproc.is_null() {
        return ptr::null_mut();
    }
    let prefix = unsafe { mi_subproc_prefix(subproc) };
    let heap = unsafe { (*prefix).heap_main };
    if !heap.is_null() {
        return heap;
    }
    if ptr::eq(subproc, unsafe { _mi_subproc_main() }) {
        mi_heap_main_init();
        unsafe {
            (*prefix).heaps = ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>();
            (*prefix).heap_main = ptr::addr_of_mut!(heap_main).cast::<mi_heap_t>();
            if (*prefix).heap_total_count.load(Ordering::Relaxed) == 0 {
                (*prefix).heap_total_count.store(1, Ordering::Relaxed);
            }
            if (*prefix).heap_count.load(Ordering::Relaxed) == 0 {
                (*prefix).heap_count.store(1, Ordering::Relaxed);
            }
            (*prefix).heap_main
        }
    } else {
        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the current subproc main heap for the bounded init/runtime scaffold.
pub unsafe extern "C" fn mi_heap_main() -> *mut mi_heap_t {
    unsafe { _mi_subproc_heap_main(_mi_subproc()) }
}

#[unsafe(no_mangle)]
/// # Safety
/// `heap` must be null or point to a heap prefix compatible with this crate's local scaffold.
pub unsafe extern "C" fn _mi_is_heap_main(heap: *const mi_heap_t) -> bool {
    if heap.is_null() {
        return false;
    }
    let subproc = unsafe { (*(heap.cast::<MiHeapMainPrefix>())).subproc };
    ptr::eq(unsafe { _mi_subproc_heap_main(subproc) }, heap.cast_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `theap` must be null or point to a valid thread-heap structure.
pub unsafe extern "C" fn _mi_is_theap_main(theap: *const mi_theap_t) -> bool {
    unsafe {
        crate::internal::mi_theap_is_initialized(theap)
            && _mi_is_heap_main(crate::internal::_mi_theap_heap(theap))
    }
}

#[allow(dead_code)]
unsafe fn _mi_thread_init_theap_default() -> *mut mi_theap_t {
    let mut theap = unsafe { crate::prim::_mi_theap_default() };
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    if _mi_is_main_thread() {
        mi_heap_main_init();
        theap = ptr::addr_of_mut!(theap_main);
    } else {
        let tld = mi_tld_alloc();
        if tld.is_null() {
            return ptr::null_mut();
        }
        theap = unsafe { _mi_theap_create(mi_heap_main(), tld) };
    }
    unsafe { _mi_theap_default_set(theap) };
    theap
}

#[allow(dead_code)]
unsafe fn mi_thread_theaps_done(tld: *mut mi_tld_t) {
    let empty = unsafe { _mi_theap_empty_get() };
    unsafe {
        _mi_theap_default_set(empty);
        _mi_theap_cached_set(empty);
        mi_theap_main_tls_set(ptr::null_mut());
    }

    unsafe { mi_lock_acquire(ptr::addr_of_mut!((*tld).theaps_lock)) };
    let mut theap = unsafe { (*tld).theaps };
    while !theap.is_null() {
        let next = unsafe { (*theap).tnext };
        unsafe { _mi_theap_collect_abandon(theap) };
        crate::mi_assert_internal!(unsafe { (*theap).page_count } == 0);
        theap = next;
    }
    unsafe { mi_lock_release(ptr::addr_of_mut!((*tld).theaps_lock)) };

    loop {
        let mut all_freed = true;
        unsafe { mi_lock_acquire(ptr::addr_of_mut!((*tld).theaps_lock)) };
        let mut theap = unsafe { (*tld).theaps };
        while !theap.is_null() {
            let next = unsafe { (*theap).tnext };
            crate::mi_assert_internal!(unsafe { (*theap).page_count } == 0);
            if !unsafe { _mi_theap_free(theap, true, false) } {
                all_freed = false;
            }
            theap = next;
        }
        unsafe { mi_lock_release(ptr::addr_of_mut!((*tld).theaps_lock)) };

        if !all_freed {
            let subproc = unsafe { mi_subproc_prefix((*tld).subproc) };
            crate::internal::__mi_stat_counter_increase_mt(
                unsafe { &mut (*subproc).stats.heaps_delete_wait },
                1,
            );
            unsafe { crate::prim::_mi_prim_thread_yield() };
        } else {
            crate::mi_assert_internal!(unsafe { (*tld).theaps }.is_null());
            break;
        }
    }

    crate::mi_assert_internal!(unsafe { crate::prim::_mi_theap_default() } == empty);
    crate::mi_assert_internal!(!unsafe { crate::internal::mi_theap_is_initialized(empty) });
}

#[unsafe(no_mangle)]
/// # Safety
/// Initializes per-thread mimalloc state for the current thread when it is not already set up.
pub unsafe extern "C" fn mi_thread_init() {
    unsafe { crate::internal::_mi_auto_process_init() };
    if unsafe { crate::prim::_mi_thread_is_initialized() } {
        return;
    }
    if unsafe { _mi_thread_init_theap_default() }.is_null() {
        return;
    }
    let heap = unsafe { mi_heap_main() };
    if !heap.is_null() {
        crate::internal::__mi_stat_increase(
            unsafe { &mut (*(heap.cast::<MiHeapMainPrefix>())).stats.threads },
            1,
        );
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// The returned opaque identifier must only be passed back to this crate's subproc functions.
pub unsafe extern "C" fn mi_subproc_new() -> mi_subproc_id_t {
    mi_subproc_main_init();
    let seq = SUBPROC_TOTAL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let mut subproc = Box::new(MiSubprocPrefix::new_dynamic(seq));
    // SAFETY: initializing lock words for a freshly allocated local prefix.
    unsafe {
        mi_lock_init(ptr::addr_of_mut!(subproc.arena_reserve_lock));
        mi_lock_init(ptr::addr_of_mut!(subproc.heaps_lock));
    }
    let subproc_ptr = Box::into_raw(subproc);
    // SAFETY: local subproc list mutation is serialized by `subprocs_lock`.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(subprocs_lock));
        (*subproc_ptr).next = subprocs;
        if !subprocs.is_null() {
            (*mi_subproc_prefix(subprocs)).prev = subproc_ptr.cast::<mi_subproc_t>();
        }
        subprocs = subproc_ptr.cast::<mi_subproc_t>();
        mi_lock_release(ptr::addr_of_mut!(subprocs_lock));
        _mi_subproc_to_id(subproc_ptr.cast::<mi_subproc_t>())
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a subproc created by this crate; only empty bounded subprocs are reclaimed.
pub unsafe extern "C" fn mi_subproc_destroy(subproc_id: mi_subproc_id_t) {
    mi_subproc_main_init();
    // SAFETY: raw ID comes from the C ABI surface.
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    if subproc.is_null() || core::ptr::eq(subproc, unsafe { _mi_subproc_main() }) {
        return;
    }
    let subproc_prefix = unsafe { mi_subproc_prefix(subproc) };
    if !unsafe { mi_subproc_is_reclaimable(subproc_prefix) } {
        return;
    }
    unsafe { mi_subproc_unsafe_destroy(subproc, true) };
}

#[unsafe(no_mangle)]
/// # Safety
/// Same requirements as `mi_subproc_destroy`.
pub unsafe extern "C" fn mi_subproc_delete(subproc_id: mi_subproc_id_t) {
    unsafe { mi_subproc_destroy(subproc_id) };
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a local subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_add_current_thread(subproc_id: mi_subproc_id_t) {
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    if subproc.is_null() {
        return;
    }
    let theap = unsafe { _mi_theap_default_safe() };
    if theap.is_null() {
        return;
    }
    let tld = unsafe { (*theap).tld };
    if tld.is_null() {
        return;
    }
    let main_subproc = unsafe { _mi_subproc_main() };
    if !ptr::eq(unsafe { (*tld).subproc }, main_subproc) {
        _mi_warning_message(format_args!(
            "unable to add thread to the subprocess as it was already in another subprocess (id: {:?})\n",
            subproc
        ));
        return;
    }
    let subproc_prefix = unsafe { mi_subproc_prefix(subproc) };
    let main_prefix = unsafe { mi_subproc_prefix(main_subproc) };
    unsafe {
        (*tld).subproc = subproc;
        (*tld).thread_seq = (*subproc_prefix)
            .thread_total_count
            .fetch_add(1, Ordering::Relaxed);
        (*main_prefix).thread_count.fetch_sub(1, Ordering::Relaxed);
        (*subproc_prefix)
            .thread_count
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must identify a local subproc created by this crate.
pub unsafe extern "C" fn mi_subproc_visit_heaps(
    subproc_id: mi_subproc_id_t,
    visitor: Option<mi_heap_visit_fun>,
    arg: *mut c_void,
) -> bool {
    let Some(visitor) = visitor else {
        return false;
    };
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    if subproc.is_null() {
        return false;
    }
    let subproc_prefix = unsafe { mi_subproc_prefix(subproc) };
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!((*subproc_prefix).heaps_lock));
        let mut heap = (*subproc_prefix).heaps;
        let mut ok = true;
        while !heap.is_null() && ok {
            let next = (*(heap.cast::<MiHeapMainPrefix>())).next;
            ok = visitor(heap, arg);
            heap = next;
        }
        mi_lock_release(ptr::addr_of_mut!((*subproc_prefix).heaps_lock));
        ok
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Performs one-time process-global mimalloc initialization for the bounded init/runtime scaffold.
pub unsafe extern "C" fn _mi_auto_process_init() {
    if !unsafe { _mi_process_is_initialized } {
        mi_subproc_main_init();
    }
    mi_mark_main_thread();
    mi_tls_slots_init();
}

#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global initialization.
pub unsafe extern "C" fn mi_process_init() {
    unsafe { _mi_auto_process_init() };
}

#[unsafe(no_mangle)]
/// # Safety
/// Performs process-global teardown for the bounded init/subproc surface and must not race with concurrent mutation of it.
pub unsafe extern "C" fn _mi_auto_process_done() {
    mi_subproc_main_init();
    unsafe {
        mi_subprocs_unsafe_destroy_all();
        _mi_process_is_initialized = false;
        _mi_allocator_done();
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global teardown.
pub unsafe extern "C" fn mi_process_done() {
    unsafe { _mi_auto_process_done() };
}
