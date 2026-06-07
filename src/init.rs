//! Global / thread initialization

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc
)]

use core::ffi::{c_char, c_void};
use core::mem::MaybeUninit;
use core::ptr;
use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release, mi_lock_t};
use crate::bits::MI_SIZE_SIZE;
use crate::internal::{_mi_allocator_done, mi_memid_t, mi_memkind_t};
use crate::stats::{mi_stat_count_t, mi_stats_get_json_from, mi_stats_header_init, mi_stats_t};
use crate::types::{
    mi_heap_t, mi_subproc_id_t, mi_subproc_t, mi_theap_t, mi_thread_local_t, mi_threadid_t,
    mi_tld_t,
};

pub const MI_TLS_DIRECT_SLOTS: usize = 64;
pub const MI_TLS_EXPANSION_SLOTS: usize = 1024;
pub const MI_TLS_DIRECT_FIRST: usize = if MI_SIZE_SIZE == 4 {
    0x0E10 / MI_SIZE_SIZE
} else {
    0x1480 / MI_SIZE_SIZE
};
pub const MI_TLS_INITIAL_SLOT: usize = 5;
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

#[allow(dead_code)]
static mut subprocs_lock: mi_lock_t = mi_lock_t::new();
static SUBPROC_MAIN_INIT: Once = Once::new();
static SUBPROC_TOTAL_COUNT: AtomicUsize = AtomicUsize::new(0);
static mut SUBPROC_MAIN: MaybeUninit<MiSubprocPrefix> = MaybeUninit::uninit();
static mut SUBPROCS_HEAD: *mut mi_subproc_t = ptr::null_mut();

#[unsafe(no_mangle)]
pub static mut _mi_cpu_movsb_max: usize = 0;

#[unsafe(no_mangle)]
pub static mut _mi_cpu_stosb_max: usize = 0;

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

const MI_MAX_ARENAS: usize = 160;

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

unsafe fn mi_subproc_to_id(subproc: *mut mi_subproc_t) -> mi_subproc_id_t {
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
            ptr::addr_of_mut!(SUBPROC_MAIN).write(MaybeUninit::new(MiSubprocPrefix::new_main()));
            let main = ptr::addr_of_mut!(SUBPROC_MAIN)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>();
            SUBPROCS_HEAD = main;
            _mi_process_is_initialized = true;
        }
    });
}

unsafe fn mi_subproc_main_ptr() -> *mut mi_subproc_t {
    mi_subproc_main_init();
    ptr::addr_of_mut!(SUBPROC_MAIN)
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
            SUBPROCS_HEAD = (*subproc).next;
        }
        (*subproc).next = ptr::null_mut();
        (*subproc).prev = ptr::null_mut();
    }
}

static MAIN_THREAD_ID: AtomicUsize = AtomicUsize::new(0);
static NEXT_THREAD_ID: AtomicUsize = AtomicUsize::new(4);

thread_local! {
    static THREAD_ID: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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

pub extern "C" fn _mi_thread_id() -> mi_threadid_t {
    current_thread_id()
}

pub extern "C" fn _mi_is_main_thread() -> bool {
    let main_tid = MAIN_THREAD_ID.load(Ordering::Acquire);
    main_tid == 0 || main_tid == current_thread_id()
}

pub extern "C" fn _mi_preloading() -> bool {
    false
}

pub extern "C" fn mi_is_redirected() -> bool {
    false
}

#[cfg(any(target_os = "openbsd", target_os = "android"))]
unsafe extern "C" {
    fn pthread_key_delete(key: mi_thread_local_t) -> i32;
}

#[allow(dead_code)]
fn mi_tls_slots_done() {
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
/// Returns the bounded current subproc pointer for this crate state.
pub unsafe extern "C" fn _mi_subproc() -> *mut mi_subproc_t {
    unsafe { _mi_subproc_main() }
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
    unsafe { mi_subproc_to_id(_mi_subproc_main()) }
}

#[unsafe(no_mangle)]
/// # Safety
/// Returns the current subproc identifier for the bounded crate state.
pub unsafe extern "C" fn mi_subproc_current() -> mi_subproc_id_t {
    unsafe { mi_subproc_to_id(_mi_subproc()) }
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
        (*subproc_ptr).next = SUBPROCS_HEAD;
        if !SUBPROCS_HEAD.is_null() {
            (*mi_subproc_prefix(SUBPROCS_HEAD)).prev = subproc_ptr.cast::<mi_subproc_t>();
        }
        SUBPROCS_HEAD = subproc_ptr.cast::<mi_subproc_t>();
        mi_lock_release(ptr::addr_of_mut!(subprocs_lock));
        mi_subproc_to_id(subproc_ptr.cast::<mi_subproc_t>())
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
    // SAFETY: list mutation is serialized by `subprocs_lock`.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(subprocs_lock));
        mi_subproc_unlink_locked(subproc_prefix);
        mi_lock_release(ptr::addr_of_mut!(subprocs_lock));
        mi_subproc_destroy_boxed(subproc);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Same requirements as `mi_subproc_destroy`.
pub unsafe extern "C" fn mi_subproc_delete(subproc_id: mi_subproc_id_t) {
    unsafe { mi_subproc_destroy(subproc_id) };
}

#[unsafe(no_mangle)]
/// # Safety
/// `subproc_id` must be valid for this crate, and `buf` must be writable for `buf_size` bytes when non-null.
pub unsafe extern "C" fn mi_subproc_stats_get_json(
    subproc_id: mi_subproc_id_t,
    buf_size: usize,
    buf: *mut c_char,
) -> *mut c_char {
    let subproc = unsafe { _mi_subproc_from_id(subproc_id) };
    if subproc.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `stats` lives inside the local prefix for this bounded subproc surface.
    unsafe {
        let stats = ptr::addr_of!((*mi_subproc_prefix(subproc)).stats);
        mi_stats_get_json_from(stats, buf_size, buf)
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Performs process-global teardown for the bounded init/subproc surface and must not race with concurrent mutation of it.
pub unsafe extern "C" fn _mi_auto_process_done() {
    mi_subproc_main_init();
    let main = unsafe { _mi_subproc_main() };
    // SAFETY: process-local teardown is serialized by `subprocs_lock`.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(subprocs_lock));
        let mut subproc = SUBPROCS_HEAD;
        SUBPROCS_HEAD = main;
        let main_prefix = mi_subproc_prefix(main);
        (*main_prefix).next = ptr::null_mut();
        (*main_prefix).prev = ptr::null_mut();
        mi_lock_release(ptr::addr_of_mut!(subprocs_lock));

        while !subproc.is_null() {
            let next = (*mi_subproc_prefix(subproc)).next;
            if !core::ptr::eq(subproc, main) {
                let subproc_prefix = mi_subproc_prefix(subproc);
                if mi_subproc_is_reclaimable(subproc_prefix) {
                    mi_subproc_destroy_boxed(subproc);
                }
            }
            subproc = next;
        }
        _mi_process_is_initialized = false;
        _mi_allocator_done();
    }
}
