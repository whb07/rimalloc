//! Options, stats, random, thread, init helpers

#![allow(non_camel_case_types, non_upper_case_globals)]

#[cfg(not(target_os = "linux"))]
use core::cell::Cell;
use core::cell::UnsafeCell;
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::fmt;
use core::fmt::Write as _;
use core::mem::size_of;
use core::ptr;
use core::ptr::{NonNull, addr_of_mut};
use core::sync::atomic::{AtomicIsize, AtomicPtr, AtomicU8, AtomicUsize, Ordering};

use crate::arena::{MI_ARENA_BCHUNK_BITS, MI_ARENA_SLICE_SIZE};
use crate::atomic::{mi_lock_acquire, mi_lock_release, mi_lock_s, mi_lock_t};
use crate::bits::{MI_GiB, MI_KiB, MI_MiB, MI_SIZE_BITS};
use crate::core_types::{
    Errno, ErrorCallback, ErrorCallbackArg, MiLockGuard, MiLockPtr, OutputCallback,
    OutputCallbackArg,
};
use crate::init::{_mi_is_main_thread, _mi_preloading, _mi_thread_id};
use crate::internal::{MI_DEBUG, MI_GUARDED};
use crate::libc::_mi_strnlen;
use crate::page::MI_MAX_ALLOC_SIZE;
use crate::prim::_mi_prim_out_stderr;
use crate::types::{
    MI_MALLOC_VERSION, mi_error_fun, mi_option_desc_t, mi_option_init_t, mi_output_fun,
};

unsafe extern "C" {
    #[link_name = "abort"]
    fn c_abort() -> !;
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
unsafe extern "C" {
    #[link_name = "stdout"]
    static mut C_STDOUT: *mut c_void;
    #[link_name = "stderr"]
    static mut C_STDERR: *mut c_void;
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_option_t(i32);

impl mi_option_t {
    pub const mi_option_show_errors: Self = Self(0);
    pub const mi_option_show_stats: Self = Self(1);
    pub const mi_option_verbose: Self = Self(2);
    pub const mi_option_deprecated_eager_commit: Self = Self(3);
    pub const mi_option_arena_eager_commit: Self = Self(4);
    pub const mi_option_purge_decommits: Self = Self(5);
    pub const mi_option_allow_large_os_pages: Self = Self(6);
    pub const mi_option_reserve_huge_os_pages: Self = Self(7);
    pub const mi_option_reserve_huge_os_pages_at: Self = Self(8);
    pub const mi_option_reserve_os_memory: Self = Self(9);
    pub const mi_option_deprecated_segment_cache: Self = Self(10);
    pub const mi_option_deprecated_page_reset: Self = Self(11);
    pub const mi_option_deprecated_abandoned_page_purge: Self = Self(12);
    pub const mi_option_deprecated_segment_reset: Self = Self(13);
    pub const mi_option_deprecated_eager_commit_delay: Self = Self(14);
    pub const mi_option_purge_delay: Self = Self(15);
    pub const mi_option_use_numa_nodes: Self = Self(16);
    pub const mi_option_disallow_os_alloc: Self = Self(17);
    pub const mi_option_os_tag: Self = Self(18);
    pub const mi_option_max_errors: Self = Self(19);
    pub const mi_option_max_warnings: Self = Self(20);
    pub const mi_option_deprecated_max_segment_reclaim: Self = Self(21);
    pub const mi_option_destroy_on_exit: Self = Self(22);
    pub const mi_option_arena_reserve: Self = Self(23);
    pub const mi_option_arena_purge_mult: Self = Self(24);
    pub const mi_option_deprecated_purge_extend_delay: Self = Self(25);
    pub const mi_option_disallow_arena_alloc: Self = Self(26);
    pub const mi_option_retry_on_oom: Self = Self(27);
    pub const mi_option_visit_abandoned: Self = Self(28);
    pub const mi_option_guarded_min: Self = Self(29);
    pub const mi_option_guarded_max: Self = Self(30);
    pub const mi_option_guarded_precise: Self = Self(31);
    pub const mi_option_guarded_sample_rate: Self = Self(32);
    pub const mi_option_guarded_sample_seed: Self = Self(33);
    pub const mi_option_generic_collect: Self = Self(34);
    pub const mi_option_page_reclaim_on_free: Self = Self(35);
    pub const mi_option_page_full_retain: Self = Self(36);
    pub const mi_option_page_max_candidates: Self = Self(37);
    pub const mi_option_max_vabits: Self = Self(38);
    pub const mi_option_pagemap_commit: Self = Self(39);
    pub const mi_option_page_commit_on_demand: Self = Self(40);
    pub const mi_option_page_max_reclaim: Self = Self(41);
    pub const mi_option_page_cross_thread_max_reclaim: Self = Self(42);
    pub const mi_option_allow_thp: Self = Self(43);
    pub const mi_option_minimal_purge_size: Self = Self(44);
    pub const mi_option_arena_max_object_size: Self = Self(45);
    pub const mi_option_arena_is_numa_local: Self = Self(46);
    pub const _mi_option_last: Self = Self(47);

    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self.0
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

pub type mi_option_e = mi_option_t;

pub const mi_option_eager_commit: mi_option_t = mi_option_t::mi_option_deprecated_eager_commit;
pub const mi_option_eager_commit_delay: mi_option_t =
    mi_option_t::mi_option_deprecated_eager_commit_delay;
pub const mi_option_abandoned_page_purge: mi_option_t =
    mi_option_t::mi_option_deprecated_abandoned_page_purge;
pub const mi_option_abandoned_reclaim_on_free: mi_option_t =
    mi_option_t::mi_option_page_reclaim_on_free;
pub const mi_option_max_segment_reclaim: mi_option_t =
    mi_option_t::mi_option_deprecated_max_segment_reclaim;
pub const mi_option_purge_extend_delay: mi_option_t =
    mi_option_t::mi_option_deprecated_purge_extend_delay;
pub const mi_option_decommit_extend_delay: mi_option_t =
    mi_option_t::mi_option_deprecated_purge_extend_delay;
pub const mi_option_large_os_pages: mi_option_t = mi_option_t::mi_option_allow_large_os_pages;
pub const mi_option_eager_region_commit: mi_option_t = mi_option_t::mi_option_arena_eager_commit;
pub const mi_option_reset_decommits: mi_option_t = mi_option_t::mi_option_purge_decommits;
pub const mi_option_reset_delay: mi_option_t = mi_option_t::mi_option_purge_delay;
pub const mi_option_limit_os_alloc: mi_option_t = mi_option_t::mi_option_disallow_os_alloc;

const MI_OPTION_COUNT: usize = mi_option_t::_mi_option_last.as_usize();

const MI_DEFAULT_SHOW_ERRORS: c_long = if cfg!(debug_assertions) { 1 } else { 0 };
const MI_DEFAULT_VERBOSE: c_long = 0;
const MI_DEFAULT_ARENA_EAGER_COMMIT: c_long = 2;
const MI_DEFAULT_ARENA_RESERVE: c_long = if cfg!(target_pointer_width = "64") {
    (1024 * 1024) as c_long
} else {
    (128 * 1024) as c_long
};
const MI_DEFAULT_ARENA_MAX_OBJECT_SIZE: c_long =
    ((MI_SIZE_BITS * MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE) / MI_KiB) as c_long;
const MI_DEFAULT_DISALLOW_ARENA_ALLOC: c_long = 0;
const MI_DEFAULT_ALLOW_LARGE_OS_PAGES: c_long = 0;
const MI_DEFAULT_RESERVE_HUGE_OS_PAGES: c_long = 0;
const MI_DEFAULT_RESERVE_OS_MEMORY: c_long = 0;
const MI_DEFAULT_GUARDED_SAMPLE_RATE: c_long = if MI_GUARDED != 0 && !cfg!(debug_assertions) {
    4000
} else {
    0
};
const MI_DEFAULT_PAGEMAP_COMMIT: c_long = if cfg!(target_os = "macos") { 1 } else { 0 };
const MI_DEFAULT_PAGE_MAX_RECLAIM: c_long = -1;
const MI_DEFAULT_PAGE_CROSS_THREAD_MAX_RECLAIM: c_long = 32;
const MI_DEFAULT_ALLOW_THP: c_long = if cfg!(target_os = "android") { 0 } else { 1 };
const MI_MAX_DELAY_OUTPUT: usize = 16 * 1024;

const fn cstr(bytes: &'static [u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

macro_rules! MI_OPTION {
    ($value:expr, $init:expr, $option:ident, $name:literal) => {
        mi_option_desc_t {
            value: $value as c_long,
            init: $init,
            option: mi_option_t::$option,
            name: cstr(concat!($name, "\0").as_bytes()),
            legacy_name: ptr::null(),
        }
    };
}

macro_rules! MI_OPTION_LEGACY {
    ($value:expr, $init:expr, $option:ident, $name:literal, $legacy:literal) => {
        mi_option_desc_t {
            value: $value as c_long,
            init: $init,
            option: mi_option_t::$option,
            name: cstr(concat!($name, "\0").as_bytes()),
            legacy_name: cstr(concat!($legacy, "\0").as_bytes()),
        }
    };
}

macro_rules! mi_stringifyx {
    ($str:tt) => {
        stringify!($str)
    };
}

const _MI_STRINGIFYX_SENTINEL: &str = mi_stringifyx!(MI_MALLOC_VERSION);

macro_rules! MI_OPTION_VALUE {
    ($value:expr) => {
        AtomicIsize::new($value as isize)
    };
}

static OPTION_VALUES: [AtomicIsize; MI_OPTION_COUNT] = [
    MI_OPTION_VALUE!(MI_DEFAULT_SHOW_ERRORS),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_DEFAULT_VERBOSE),
    MI_OPTION_VALUE!(1),
    MI_OPTION_VALUE!(MI_DEFAULT_ARENA_EAGER_COMMIT),
    MI_OPTION_VALUE!(1),
    MI_OPTION_VALUE!(MI_DEFAULT_ALLOW_LARGE_OS_PAGES),
    MI_OPTION_VALUE!(MI_DEFAULT_RESERVE_HUGE_OS_PAGES),
    MI_OPTION_VALUE!(-1),
    MI_OPTION_VALUE!(MI_DEFAULT_RESERVE_OS_MEMORY),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(1),
    MI_OPTION_VALUE!(1000),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(100),
    MI_OPTION_VALUE!(32),
    MI_OPTION_VALUE!(32),
    MI_OPTION_VALUE!(10),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_DEFAULT_ARENA_RESERVE),
    MI_OPTION_VALUE!(1),
    MI_OPTION_VALUE!(1),
    MI_OPTION_VALUE!(MI_DEFAULT_DISALLOW_ARENA_ALLOC),
    MI_OPTION_VALUE!(400),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_GiB as c_long),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_DEFAULT_GUARDED_SAMPLE_RATE),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(10000),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(2),
    MI_OPTION_VALUE!(4),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_DEFAULT_PAGEMAP_COMMIT),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_DEFAULT_PAGE_MAX_RECLAIM),
    MI_OPTION_VALUE!(MI_DEFAULT_PAGE_CROSS_THREAD_MAX_RECLAIM),
    MI_OPTION_VALUE!(MI_DEFAULT_ALLOW_THP),
    MI_OPTION_VALUE!(0),
    MI_OPTION_VALUE!(MI_DEFAULT_ARENA_MAX_OBJECT_SIZE),
    MI_OPTION_VALUE!(0),
];

static OPTION_INITS: [AtomicU8; MI_OPTION_COUNT] =
    [const { AtomicU8::new(mi_option_init_t::MI_OPTION_UNINIT as u8) }; MI_OPTION_COUNT];

struct LockedOptions(UnsafeCell<[mi_option_desc_t; MI_OPTION_COUNT]>);

unsafe impl Sync for LockedOptions {}

static mut options_lock: mi_lock_t = mi_lock_s::new();
static OPTIONS: LockedOptions = LockedOptions(UnsafeCell::new([
    MI_OPTION!(
        MI_DEFAULT_SHOW_ERRORS,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_show_errors,
        "show_errors"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_show_stats,
        "show_stats"
    ),
    MI_OPTION!(
        MI_DEFAULT_VERBOSE,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_verbose,
        "verbose"
    ),
    MI_OPTION!(
        1,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_eager_commit,
        "deprecated_eager_commit"
    ),
    MI_OPTION_LEGACY!(
        MI_DEFAULT_ARENA_EAGER_COMMIT,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_arena_eager_commit,
        "arena_eager_commit",
        "eager_region_commit"
    ),
    MI_OPTION_LEGACY!(
        1,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_purge_decommits,
        "purge_decommits",
        "reset_decommits"
    ),
    MI_OPTION_LEGACY!(
        MI_DEFAULT_ALLOW_LARGE_OS_PAGES,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_allow_large_os_pages,
        "allow_large_os_pages",
        "large_os_pages"
    ),
    MI_OPTION!(
        MI_DEFAULT_RESERVE_HUGE_OS_PAGES,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_reserve_huge_os_pages,
        "reserve_huge_os_pages"
    ),
    MI_OPTION!(
        -1,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_reserve_huge_os_pages_at,
        "reserve_huge_os_pages_at"
    ),
    MI_OPTION!(
        MI_DEFAULT_RESERVE_OS_MEMORY,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_reserve_os_memory,
        "reserve_os_memory"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_segment_cache,
        "deprecated_segment_cache"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_page_reset,
        "deprecated_page_reset"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_abandoned_page_purge,
        "deprecated_abandoned_page_purge"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_segment_reset,
        "deprecated_segment_reset"
    ),
    MI_OPTION!(
        1,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_eager_commit_delay,
        "deprecated_eager_commit_delay"
    ),
    MI_OPTION_LEGACY!(
        1000,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_purge_delay,
        "purge_delay",
        "reset_delay"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_use_numa_nodes,
        "use_numa_nodes"
    ),
    MI_OPTION_LEGACY!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_disallow_os_alloc,
        "disallow_os_alloc",
        "limit_os_alloc"
    ),
    MI_OPTION!(
        100,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_os_tag,
        "os_tag"
    ),
    MI_OPTION!(
        32,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_max_errors,
        "max_errors"
    ),
    MI_OPTION!(
        32,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_max_warnings,
        "max_warnings"
    ),
    MI_OPTION!(
        10,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_max_segment_reclaim,
        "deprecated_max_segment_reclaim"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_destroy_on_exit,
        "destroy_on_exit"
    ),
    MI_OPTION!(
        MI_DEFAULT_ARENA_RESERVE,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_arena_reserve,
        "arena_reserve"
    ),
    MI_OPTION!(
        1,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_arena_purge_mult,
        "arena_purge_mult"
    ),
    MI_OPTION_LEGACY!(
        1,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_deprecated_purge_extend_delay,
        "deprecated_purge_extend_delay",
        "decommit_extend_delay"
    ),
    MI_OPTION!(
        MI_DEFAULT_DISALLOW_ARENA_ALLOC,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_disallow_arena_alloc,
        "disallow_arena_alloc"
    ),
    MI_OPTION!(
        400,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_retry_on_oom,
        "retry_on_oom"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_visit_abandoned,
        "visit_abandoned"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_guarded_min,
        "guarded_min"
    ),
    MI_OPTION!(
        MI_GiB as c_long,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_guarded_max,
        "guarded_max"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_guarded_precise,
        "guarded_precise"
    ),
    MI_OPTION!(
        MI_DEFAULT_GUARDED_SAMPLE_RATE,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_guarded_sample_rate,
        "guarded_sample_rate"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_guarded_sample_seed,
        "guarded_sample_seed"
    ),
    MI_OPTION!(
        10000,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_generic_collect,
        "generic_collect"
    ),
    MI_OPTION_LEGACY!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_page_reclaim_on_free,
        "page_reclaim_on_free",
        "abandoned_reclaim_on_free"
    ),
    MI_OPTION!(
        2,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_page_full_retain,
        "page_full_retain"
    ),
    MI_OPTION!(
        4,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_page_max_candidates,
        "page_max_candidates"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_max_vabits,
        "max_vabits"
    ),
    MI_OPTION!(
        MI_DEFAULT_PAGEMAP_COMMIT,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_pagemap_commit,
        "pagemap_commit"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_page_commit_on_demand,
        "page_commit_on_demand"
    ),
    MI_OPTION!(
        MI_DEFAULT_PAGE_MAX_RECLAIM,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_page_max_reclaim,
        "page_max_reclaim"
    ),
    MI_OPTION!(
        MI_DEFAULT_PAGE_CROSS_THREAD_MAX_RECLAIM,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_page_cross_thread_max_reclaim,
        "page_cross_thread_max_reclaim"
    ),
    MI_OPTION!(
        MI_DEFAULT_ALLOW_THP,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_allow_thp,
        "allow_thp"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_minimal_purge_size,
        "minimal_purge_size"
    ),
    MI_OPTION!(
        MI_DEFAULT_ARENA_MAX_OBJECT_SIZE,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_arena_max_object_size,
        "arena_max_object_size"
    ),
    MI_OPTION!(
        0,
        mi_option_init_t::MI_OPTION_UNINIT,
        mi_option_arena_is_numa_local,
        "arena_is_numa_local"
    ),
]));

#[cfg(not(target_os = "linux"))]
thread_local! {
    static recurse: Cell<bool> = const { Cell::new(false) };
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn _mi_tls_recurse_get() -> c_int;
    fn _mi_tls_recurse_set(recurse: c_int);
}

struct DelayedOutputBuffer(UnsafeCell<[u8; MI_MAX_DELAY_OUTPUT + 1]>);

unsafe impl Sync for DelayedOutputBuffer {}

static out_buf: DelayedOutputBuffer =
    DelayedOutputBuffer(UnsafeCell::new([0; MI_MAX_DELAY_OUTPUT + 1]));
static out_len: AtomicUsize = AtomicUsize::new(0);
static mut out_buf_lock: mi_lock_t = mi_lock_s::new();
static mi_out_default: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static mi_out_arg: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static mi_max_error_count: AtomicIsize = AtomicIsize::new(16);
static mi_max_warning_count: AtomicIsize = AtomicIsize::new(16);
static error_count: AtomicUsize = AtomicUsize::new(0);
static warning_count: AtomicUsize = AtomicUsize::new(0);

struct LockedErrorHandler(UnsafeCell<ErrorCallback>);

unsafe impl Sync for LockedErrorHandler {}

static mut mi_error_handler_lock: mi_lock_t = mi_lock_s::new();
static mi_error_handler: LockedErrorHandler =
    LockedErrorHandler(UnsafeCell::new(ErrorCallback::none()));
static mi_error_arg: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

#[inline]
fn with_options_table<R>(f: impl FnOnce(&mut [mi_option_desc_t; MI_OPTION_COUNT]) -> R) -> R {
    unsafe {
        let lock = MiLockPtr(NonNull::new_unchecked(addr_of_mut!(options_lock)));
        let _lock = MiLockGuard::acquire(lock);
        f(&mut *OPTIONS.0.get())
    }
}

#[inline]
fn error_handler_load() -> ErrorCallback {
    unsafe {
        let lock = MiLockPtr(NonNull::new_unchecked(addr_of_mut!(mi_error_handler_lock)));
        let _lock = MiLockGuard::acquire(lock);
        *mi_error_handler.0.get()
    }
}

#[inline]
fn error_handler_store(fun: ErrorCallback) {
    unsafe {
        let lock = MiLockPtr(NonNull::new_unchecked(addr_of_mut!(mi_error_handler_lock)));
        let _lock = MiLockGuard::acquire(lock);
        *mi_error_handler.0.get() = fun;
    }
}

#[inline]
pub(crate) fn registered_error_callback() -> (Option<mi_error_fun>, *mut c_void) {
    (
        error_handler_load().raw(),
        mi_error_arg.load(Ordering::Acquire),
    )
}

#[inline]
pub(crate) fn restore_error_callback(snapshot: (Option<mi_error_fun>, *mut c_void)) {
    let (fun, arg) = snapshot;
    error_handler_store(ErrorCallback::new(fun));
    mi_error_arg.store(arg, Ordering::Release);
}

#[inline]
fn option_value_load(idx: usize) -> c_long {
    OPTION_VALUES[idx].load(Ordering::Relaxed) as c_long
}

#[inline]
fn option_value_store(idx: usize, value: c_long) {
    OPTION_VALUES[idx].store(value as isize, Ordering::Relaxed);
}

#[inline]
fn option_init_load(idx: usize) -> mi_option_init_t {
    match OPTION_INITS[idx].load(Ordering::Acquire) {
        2 => mi_option_init_t::MI_OPTION_INITIALIZED,
        1 => mi_option_init_t::MI_OPTION_DEFAULTED,
        _ => mi_option_init_t::MI_OPTION_UNINIT,
    }
}

#[inline]
fn option_init_store(idx: usize, init: mi_option_init_t) {
    OPTION_INITS[idx].store(init as u8, Ordering::Release);
}

#[inline]
fn option_has_size_in_kib(option: mi_option_t) -> bool {
    matches!(
        option,
        mi_option_t::mi_option_reserve_os_memory
            | mi_option_t::mi_option_arena_reserve
            | mi_option_t::mi_option_minimal_purge_size
            | mi_option_t::mi_option_arena_max_object_size
    )
}

#[inline]
fn c_long_bounds() -> (i128, i128) {
    if size_of::<c_long>() == size_of::<i64>() {
        (i64::MIN as i128, i64::MAX as i128)
    } else {
        (i32::MIN as i128, i32::MAX as i128)
    }
}

#[inline]
fn clamp_c_long(x: i128) -> c_long {
    let (min, max) = c_long_bounds();
    let y = if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    };
    y as c_long
}

#[inline]
fn mi_output_is_c_file(out: mi_output_fun) -> bool {
    let p = OutputCallback::new(Some(out)).storage_ptr();
    #[cfg(all(target_os = "linux", not(target_env = "musl")))]
    {
        p == unsafe { C_STDOUT } || p == unsafe { C_STDERR }
    }
    #[cfg(not(all(target_os = "linux", not(target_env = "musl"))))]
    {
        let _ = p;
        false
    }
}

unsafe extern "C" fn mi_out_stderr(msg: *const c_char, arg: *mut c_void) {
    let _ = arg;
    if !msg.is_null() && unsafe { *msg } != 0 {
        unsafe { _mi_prim_out_stderr(msg) };
    }
}

unsafe extern "C" fn mi_out_buf(msg: *const c_char, arg: *mut c_void) {
    let _ = arg;
    if msg.is_null() || out_len.load(Ordering::Acquire) >= MI_MAX_DELAY_OUTPUT {
        return;
    }

    let msg_bytes = unsafe { CStr::from_ptr(msg).to_bytes() };
    let mut n = msg_bytes.len();
    if n == 0 || n >= MI_MAX_DELAY_OUTPUT {
        return;
    }

    unsafe { mi_lock_acquire(addr_of_mut!(out_buf_lock)) };
    let start = out_len.fetch_add(n, Ordering::AcqRel);
    if start < MI_MAX_DELAY_OUTPUT {
        if start + n >= MI_MAX_DELAY_OUTPUT {
            n = MI_MAX_DELAY_OUTPUT - start - 1;
        }
        let buf = unsafe { &mut *out_buf.0.get() };
        buf[start..start + n].copy_from_slice(&msg_bytes[..n]);
    }
    unsafe { mi_lock_release(addr_of_mut!(out_buf_lock)) };
}

fn mi_out_buf_flush(out: OutputCallback, no_more_buf: bool, arg: OutputCallbackArg) {
    unsafe { mi_lock_acquire(addr_of_mut!(out_buf_lock)) };
    let mut count = out_len.fetch_add(
        if no_more_buf { MI_MAX_DELAY_OUTPUT } else { 1 },
        Ordering::AcqRel,
    );
    if count > MI_MAX_DELAY_OUTPUT {
        count = MI_MAX_DELAY_OUTPUT;
    }
    let buf = unsafe { &mut *out_buf.0.get() };
    buf[count] = 0;
    unsafe { out.invoke(buf.as_ptr().cast::<c_char>(), arg) };
    if !no_more_buf {
        buf[count] = b'\n';
    }
    unsafe { mi_lock_release(addr_of_mut!(out_buf_lock)) };
}

unsafe extern "C" fn mi_out_buf_stderr(msg: *const c_char, arg: *mut c_void) {
    unsafe { mi_out_stderr(msg, arg) };
    unsafe { mi_out_buf(msg, arg) };
}

fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback {
    if let Some(arg) = parg {
        *arg = OutputCallbackArg::from_raw(mi_out_arg.load(Ordering::Acquire));
    }
    let callback =
        unsafe { OutputCallback::from_storage_ptr(mi_out_default.load(Ordering::Acquire)) };
    OutputCallback::new(callback.raw().or(Some(mi_out_buf)))
}

fn mi_add_stderr_output() {
    if !mi_out_default.load(Ordering::Acquire).is_null() {
        return;
    }
    mi_out_buf_flush(
        OutputCallback::new(Some(mi_out_stderr)),
        false,
        OutputCallbackArg::null(),
    );
    mi_out_default.store(
        OutputCallback::new(Some(mi_out_buf_stderr)).storage_ptr(),
        Ordering::Release,
    );
    mi_out_arg.store(ptr::null_mut(), Ordering::Release);
}

#[inline(never)]
fn mi_recurse_enter_prim() -> bool {
    #[cfg(target_os = "linux")]
    unsafe {
        if _mi_tls_recurse_get() != 0 {
            false
        } else {
            _mi_tls_recurse_set(1);
            true
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        recurse.with(|slot| {
            if slot.get() {
                false
            } else {
                slot.set(true);
                true
            }
        })
    }
}

#[used]
static MI_RECURSE_ENTER_PRIM_KEEP: fn() -> bool = mi_recurse_enter_prim;

#[inline]
fn mi_recurse_enter() -> bool {
    if cfg!(any(target_vendor = "apple", target_os = "android")) && _mi_preloading() {
        return false;
    }
    mi_recurse_enter_prim()
}

#[inline(never)]
fn mi_recurse_exit_prim() {
    #[cfg(target_os = "linux")]
    unsafe {
        _mi_tls_recurse_set(0);
    }
    #[cfg(not(target_os = "linux"))]
    {
        recurse.with(|slot| slot.set(false));
    }
}

#[used]
static MI_RECURSE_EXIT_PRIM_KEEP: fn() = mi_recurse_exit_prim;

#[inline]
fn mi_recurse_exit() {
    if cfg!(any(target_vendor = "apple", target_os = "android")) && _mi_preloading() {
        return;
    }
    mi_recurse_exit_prim();
}

/// # Safety
///
/// `prefix` and `message`, when non-null, must point to valid nul-terminated
/// C strings. If `out` is supplied, it must be callable with `arg`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_fputs(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    message: *const c_char,
) {
    let use_default = match out {
        None => true,
        Some(out) => mi_output_is_c_file(out),
    };
    if use_default {
        if !mi_recurse_enter() {
            return;
        }
        let mut default_arg = OutputCallbackArg::null();
        let default_out = mi_out_get_default(Some(&mut default_arg));
        if !prefix.is_null() {
            unsafe { default_out.invoke(prefix, default_arg) };
        }
        unsafe { default_out.invoke(message, default_arg) };
        mi_recurse_exit();
    } else if let Some(out) = out {
        let out = OutputCallback::new(Some(out));
        let arg = OutputCallbackArg::from_raw(arg);
        if !prefix.is_null() {
            unsafe { out.invoke(prefix, arg) };
        }
        unsafe { out.invoke(message, arg) };
    }
}

pub(crate) fn _mi_fprintf(out: Option<mi_output_fun>, arg: *mut c_void, args: fmt::Arguments<'_>) {
    let mut text = StackMessage::new();
    if fmt::write(&mut text, args).is_ok() {
        unsafe { _mi_fputs(out, arg, ptr::null(), text.as_c_ptr()) };
    }
}

struct StackMessage {
    bytes: [u8; 1024],
    len: usize,
}

impl StackMessage {
    fn new() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
        }
    }

    fn as_c_ptr(&mut self) -> *const c_char {
        self.bytes[self.len] = 0;
        self.bytes.as_ptr().cast()
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let capacity = self.bytes.len() - 1;
        if self.len >= capacity {
            return;
        }
        let available = capacity - self.len;
        let copy = bytes.len().min(available);
        self.bytes[self.len..self.len + copy].copy_from_slice(&bytes[..copy]);
        self.len += copy;
    }
}

impl fmt::Write for StackMessage {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}

fn mi_vfprintf(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    args: fmt::Arguments<'_>,
) {
    let mut text = StackMessage::new();
    if !mi_recurse_enter() {
        return;
    }
    let wrote = text.write_fmt(args).is_ok();
    mi_recurse_exit();
    if !wrote {
        return;
    }
    unsafe { _mi_fputs(out, arg, prefix, text.as_c_ptr()) };
}

fn mi_vfprintf_thread(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    args: fmt::Arguments<'_>,
) {
    if !prefix.is_null() {
        let prefix_len = unsafe { _mi_strnlen(prefix, 33) };
        if prefix_len <= 32 && !_mi_is_main_thread() {
            let mut tprefix = StackMessage::new();
            let prefix_bytes =
                unsafe { core::slice::from_raw_parts(prefix.cast::<u8>(), prefix_len) };
            tprefix.write_bytes(prefix_bytes);
            let _ = write!(tprefix, "thread 0x{:x}: ", _mi_thread_id() as usize);
            mi_vfprintf(out, arg, tprefix.as_c_ptr(), args);
            return;
        }
    }
    mi_vfprintf(out, arg, prefix, args);
}

fn mi_vfputs_thread(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    message: *const c_char,
) {
    if message.is_null() {
        return;
    }
    if !prefix.is_null() {
        let prefix_len = unsafe { _mi_strnlen(prefix, 33) };
        if prefix_len <= 32 && !_mi_is_main_thread() {
            let mut tprefix = StackMessage::new();
            let prefix_bytes =
                unsafe { core::slice::from_raw_parts(prefix.cast::<u8>(), prefix_len) };
            tprefix.write_bytes(prefix_bytes);
            let _ = write!(tprefix, "thread 0x{:x}: ", _mi_thread_id() as usize);
            unsafe { _mi_fputs(out, arg, tprefix.as_c_ptr(), message) };
            return;
        }
    }
    unsafe { _mi_fputs(out, arg, prefix, message) };
}

#[inline(never)]
pub(crate) fn _mi_raw_message(args: fmt::Arguments<'_>) {
    mi_vfprintf(None, ptr::null_mut(), ptr::null(), args);
}

#[inline(never)]
pub(crate) fn _mi_message(args: fmt::Arguments<'_>) {
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[used]
static MI_RAW_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_raw_message;
#[used]
static MI_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_message;

#[inline(never)]
pub(crate) fn _mi_trace_message(args: fmt::Arguments<'_>) {
    if mi_option_get(mi_option_t::mi_option_verbose) <= 1 {
        return;
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[inline(never)]
pub(crate) fn _mi_verbose_message(args: fmt::Arguments<'_>) {
    if !mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        return;
    }
    mi_vfprintf(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[used]
static MI_TRACE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_trace_message;
#[used]
static MI_VERBOSE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_verbose_message;

fn mi_show_error_message(args: fmt::Arguments<'_>) {
    if !mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        if !mi_option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_errors = mi_max_error_count.load(Ordering::Acquire);
        let count = error_count.fetch_add(1, Ordering::AcqRel);
        if max_errors >= 0 && (count as isize) > max_errors {
            return;
        }
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: error: ".as_ptr(), args);
}

fn mi_show_error_message_raw(message: *const c_char) {
    if !mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        if !mi_option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_errors = mi_max_error_count.load(Ordering::Acquire);
        let count = error_count.fetch_add(1, Ordering::AcqRel);
        if max_errors >= 0 && (count as isize) > max_errors {
            return;
        }
    }
    mi_vfputs_thread(
        None,
        ptr::null_mut(),
        c"mimalloc: error: ".as_ptr(),
        message,
    );
}

#[inline(never)]
pub(crate) fn _mi_warning_message_fmt(args: fmt::Arguments<'_>) {
    if !mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        if !mi_option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_warnings = mi_max_warning_count.load(Ordering::Acquire);
        let count = warning_count.fetch_add(1, Ordering::AcqRel);
        if max_warnings >= 0 && (count as isize) > max_warnings {
            return;
        }
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: warning: ".as_ptr(), args);
}

/// # Safety
/// Matches the internal C helper surface for static C probes. Extra varargs are ignored here.
#[inline(never)]
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
pub(crate) unsafe extern "C" fn _mi_warning_message(fmt: *const c_char) {
    if !mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        if !mi_option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_warnings = mi_max_warning_count.load(Ordering::Acquire);
        let count = warning_count.fetch_add(1, Ordering::AcqRel);
        if max_warnings >= 0 && (count as isize) > max_warnings {
            return;
        }
    }
    mi_vfputs_thread(None, ptr::null_mut(), c"mimalloc: warning: ".as_ptr(), fmt);
}

#[inline(never)]
pub(crate) fn _mi_error_message_fmt(err: Errno, args: fmt::Arguments<'_>) {
    mi_show_error_message(args);
    let handler = error_handler_load();
    let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));
    if handler.is_some() {
        unsafe { handler.invoke(err.get(), arg) };
    } else {
        mi_error_default(err);
    }
}

/// # Safety
/// Matches the internal C helper surface for static C probes. Extra varargs are ignored here.
#[inline(never)]
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
pub(crate) unsafe extern "C" fn _mi_error_message(err: c_int, fmt: *const c_char) {
    let err = Errno::new(err);
    mi_show_error_message_raw(fmt);
    let handler = error_handler_load();
    let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));
    if handler.is_some() {
        unsafe { handler.invoke(err.get(), arg) };
    } else {
        mi_error_default(err);
    }
}

#[used]
static MI_WARNING_MESSAGE_FMT_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_warning_message_fmt;
#[used]
static MI_WARNING_MESSAGE_KEEP: unsafe extern "C" fn(*const c_char) = _mi_warning_message;
#[used]
static MI_ERROR_MESSAGE_FMT_KEEP: for<'a> fn(Errno, fmt::Arguments<'a>) = _mi_error_message_fmt;
#[used]
static MI_ERROR_MESSAGE_KEEP: unsafe extern "C" fn(c_int, *const c_char) = _mi_error_message;

#[inline]
fn build_mimalloc_env_name(buf: &mut [u8; 65], suffix: &[u8]) -> bool {
    const PREFIX: &[u8] = b"mimalloc_";
    let total = PREFIX.len() + suffix.len();
    if total >= buf.len() {
        return false;
    }
    buf[..PREFIX.len()].copy_from_slice(PREFIX);
    buf[PREFIX.len()..total].copy_from_slice(suffix);
    buf[total] = 0;
    true
}

#[inline]
fn get_env_value_stack<'a>(suffix: &[u8], value_buf: &'a mut [c_char; 65]) -> Option<&'a str> {
    let mut env_name = [0u8; 65];
    if !build_mimalloc_env_name(&mut env_name, suffix) {
        return None;
    }
    if !unsafe {
        crate::libc::_mi_getenv(
            env_name.as_ptr().cast(),
            value_buf.as_mut_ptr(),
            value_buf.len(),
        )
    } {
        return None;
    }
    let value = unsafe { CStr::from_ptr(value_buf.as_ptr()) };
    value.to_str().ok()
}

#[inline]
fn option_extra_legacy_names(option: mi_option_t) -> &'static [&'static str] {
    match option {
        mi_option_t::mi_option_deprecated_eager_commit => &["eager_commit"],
        mi_option_t::mi_option_deprecated_eager_commit_delay => &["eager_commit_delay"],
        mi_option_t::mi_option_deprecated_abandoned_page_purge => &["abandoned_page_purge"],
        mi_option_t::mi_option_deprecated_max_segment_reclaim => &["max_segment_reclaim"],
        mi_option_t::mi_option_deprecated_purge_extend_delay => &["purge_extend_delay"],
        _ => &[],
    }
}

#[inline]
fn parse_long_prefix(s: &str) -> Option<(i128, &str)> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    let value = s[start..i].parse::<i128>().ok()?;
    Some((value, &s[i..]))
}

#[inline]
fn option_from_index(i: usize) -> mi_option_t {
    crate::mi_debug_assert!(i < MI_OPTION_COUNT);
    mi_option_t::from_raw(i as i32)
}

fn parse_option_value(raw: &str, size_in_kib: bool) -> Option<c_long> {
    if raw.is_empty()
        || raw == "1"
        || raw.eq_ignore_ascii_case("TRUE")
        || raw.eq_ignore_ascii_case("YES")
        || raw.eq_ignore_ascii_case("ON")
    {
        return Some(1);
    }
    if raw == "0"
        || raw.eq_ignore_ascii_case("FALSE")
        || raw.eq_ignore_ascii_case("NO")
        || raw.eq_ignore_ascii_case("OFF")
    {
        return Some(0);
    }

    let (mut value, mut rest) = parse_long_prefix(raw)?;
    if size_in_kib {
        let mut size = if value < 0 { 0usize } else { value as usize };
        let mut overflow = false;
        let suffix = rest.as_bytes().first().map(|b| b.to_ascii_uppercase());
        if suffix == Some(b'K') {
            rest = &rest[1..];
        } else if suffix == Some(b'M') {
            overflow = size.checked_mul(MI_KiB).is_none();
            if !overflow {
                size *= MI_KiB;
            }
            rest = &rest[1..];
        } else if suffix == Some(b'G') {
            overflow = size.checked_mul(MI_MiB).is_none();
            if !overflow {
                size *= MI_MiB;
            }
            rest = &rest[1..];
        } else if suffix == Some(b'T') {
            overflow = size.checked_mul(MI_GiB).is_none();
            if !overflow {
                size *= MI_GiB;
            }
            rest = &rest[1..];
        } else {
            size = size.div_ceil(MI_KiB);
        }
        if rest.len() >= 2 && rest[..2].eq_ignore_ascii_case("IB") {
            rest = &rest[2..];
        } else if rest
            .as_bytes()
            .first()
            .is_some_and(|b| b.eq_ignore_ascii_case(&b'B'))
        {
            rest = &rest[1..];
        }
        let max_kib = MI_MAX_ALLOC_SIZE / MI_KiB;
        if overflow || size > max_kib {
            size = max_kib;
        }
        value = size as i128;
    }

    if rest.is_empty() {
        Some(clamp_c_long(value))
    } else {
        None
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_version() -> i32 {
    MI_MALLOC_VERSION
}

pub(crate) fn mi_error_default(err: Errno) {
    if err == Errno::EFAULT && (MI_DEBUG > 0 || cfg!(feature = "secure")) {
        unsafe { c_abort() };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void) {
    let callback = OutputCallback::new(Some(out.unwrap_or(mi_out_stderr)));
    let arg = OutputCallbackArg::from_raw(arg);
    mi_out_default.store(callback.storage_ptr(), Ordering::Release);
    mi_out_arg.store(arg.as_ptr(), Ordering::Release);
    if let Some(out) = out {
        mi_out_buf_flush(OutputCallback::new(Some(out)), true, arg);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void) {
    let callback = ErrorCallback::new(fun);
    let arg = ErrorCallbackArg::from_raw(arg);
    error_handler_store(callback);
    mi_error_arg.store(arg.as_ptr(), Ordering::Release);
}

fn mi_option_init(desc: &mut mi_option_desc_t) {
    let name = unsafe { CStr::from_ptr(desc.name) };
    let name_str = name.to_str().unwrap_or("");
    let mut value_buf = [0 as c_char; 65];
    let mut found = get_env_value_stack(name.to_bytes(), &mut value_buf);

    if found.is_none() && !desc.legacy_name.is_null() {
        let legacy = unsafe { CStr::from_ptr(desc.legacy_name) };
        let legacy_str = legacy.to_str().unwrap_or("");
        found = get_env_value_stack(legacy.to_bytes(), &mut value_buf);
        if found.is_some() {
            _mi_warning_message_fmt(format_args!(
                "environment option \"mimalloc_{}\" is deprecated -- use \"mimalloc_{}\" instead.\n",
                legacy_str, name_str
            ));
        }
    }

    if found.is_none() {
        for legacy in option_extra_legacy_names(desc.option) {
            found = get_env_value_stack(legacy.as_bytes(), &mut value_buf);
            if found.is_some() {
                _mi_warning_message_fmt(format_args!(
                    "environment option \"mimalloc_{}\" is deprecated -- use \"mimalloc_{}\" instead.\n",
                    legacy, name_str
                ));
                break;
            }
        }
    }

    if let Some(raw) = found {
        if let Some(value) = parse_option_value(raw, option_has_size_in_kib(desc.option)) {
            desc.value = value;
            desc.init = mi_option_init_t::MI_OPTION_INITIALIZED;
        } else {
            desc.init = mi_option_init_t::MI_OPTION_DEFAULTED;
            if desc.option == mi_option_t::mi_option_verbose && desc.value == 0 {
                desc.value = 1;
                _mi_warning_message_fmt(format_args!(
                    "environment option mimalloc_{} has an invalid value.\n",
                    name_str
                ));
                desc.value = 0;
            } else {
                _mi_warning_message_fmt(format_args!(
                    "environment option mimalloc_{} has an invalid value.\n",
                    name_str
                ));
            }
        }
    } else if !_mi_preloading() {
        desc.init = mi_option_init_t::MI_OPTION_DEFAULTED;
    }
}

fn mi_option_get_fast_impl(option: mi_option_t) -> c_long {
    let idx = option.as_usize();
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return 0;
    }
    option_value_load(idx)
}

#[inline(never)]
pub(crate) extern "C" fn _mi_option_get_fast(option: mi_option_t) -> c_long {
    #[cfg(feature = "bench-diagnostics")]
    crate::bench_diagnostics::option_read(option);
    mi_option_get_fast_impl(option)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get(option: mi_option_t) -> c_long {
    let idx = option.as_usize();
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return 0;
    }

    if option_init_load(idx) == mi_option_init_t::MI_OPTION_UNINIT {
        let desc = with_options_table(|table| {
            if table[idx].init == mi_option_init_t::MI_OPTION_UNINIT {
                table[idx].init = mi_option_init_t::MI_OPTION_DEFAULTED;
                option_init_store(idx, mi_option_init_t::MI_OPTION_DEFAULTED);
                Some(table[idx])
            } else {
                None
            }
        });

        if let Some(mut desc) = desc {
            mi_option_init(&mut desc);
            if desc.init == mi_option_init_t::MI_OPTION_INITIALIZED {
                mi_option_set(option, desc.value);
            }
        }
    }

    option_value_load(idx)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get_clamp(option: mi_option_t, min: c_long, max: c_long) -> c_long {
    let x = mi_option_get(option);
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get_size(option: mi_option_t) -> usize {
    let x = mi_option_get(option);
    let mut size = if x < 0 { 0 } else { x as usize };
    if option_has_size_in_kib(option) {
        match size.checked_mul(MI_KiB) {
            Some(bytes) if bytes <= crate::page::MI_MAX_ALLOC_SIZE => {
                size = bytes;
            }
            _ => size = crate::page::MI_MAX_ALLOC_SIZE,
        }
    }
    size
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set(option: mi_option_t, value: c_long) {
    let idx = option.as_usize();
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return;
    }

    let recurse_adjust = with_options_table(|table| {
        let option_kind = table[idx].option;
        table[idx].value = value;
        table[idx].init = mi_option_init_t::MI_OPTION_INITIALIZED;
        option_value_store(idx, value);
        option_init_store(idx, mi_option_init_t::MI_OPTION_INITIALIZED);

        let guarded_min_idx = mi_option_t::mi_option_guarded_min.as_usize();
        let guarded_max_idx = mi_option_t::mi_option_guarded_max.as_usize();
        if option_kind == mi_option_t::mi_option_guarded_min && table[guarded_max_idx].value < value
        {
            Some((mi_option_t::mi_option_guarded_max, value))
        } else if option_kind == mi_option_t::mi_option_guarded_max
            && table[guarded_min_idx].value > value
        {
            Some((mi_option_t::mi_option_guarded_min, value))
        } else {
            None
        }
    });

    if let Some((option, value)) = recurse_adjust {
        mi_option_set(option, value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_default(option: mi_option_t, value: c_long) {
    let idx = option.as_usize();
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return;
    }
    with_options_table(|table| {
        let desc = &mut table[idx];
        if desc.init != mi_option_init_t::MI_OPTION_INITIALIZED {
            desc.value = value;
            option_value_store(idx, value);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_is_enabled(option: mi_option_t) -> bool {
    mi_option_get(option) != 0
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_enabled(option: mi_option_t, enable: bool) {
    mi_option_set(option, if enable { 1 } else { 0 });
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_enabled_default(option: mi_option_t, enable: bool) {
    mi_option_set_default(option, if enable { 1 } else { 0 });
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_enable(option: mi_option_t) {
    mi_option_set_enabled(option, true);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_disable(option: mi_option_t) {
    mi_option_set_enabled(option, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_options_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    let (out, arg) = if let Some(out) = out {
        (out, arg)
    } else {
        let mut default_arg = OutputCallbackArg::null();
        let default_out = mi_out_get_default(Some(&mut default_arg));
        (
            default_out.raw().unwrap_or(mi_out_buf),
            default_arg.as_ptr(),
        )
    };

    let vermajor = MI_MALLOC_VERSION / 10000;
    let verminor = (MI_MALLOC_VERSION % 10000) / 100;
    let verpatch = MI_MALLOC_VERSION % 100;
    _mi_fprintf(
        Some(out),
        arg,
        format_args!("v{vermajor}.{verminor}.{verpatch} (Rust translation)\n"),
    );

    for i in 0..MI_OPTION_COUNT {
        let option = option_from_index(i);
        let value = mi_option_get(option);
        let desc = with_options_table(|table| table[i]);
        // SAFETY: option names are static ASCII strings from the translated option table.
        let name = unsafe { core::str::from_utf8_unchecked(CStr::from_ptr(desc.name).to_bytes()) };
        _mi_fprintf(
            Some(out),
            arg,
            format_args!(
                "option '{name}': {value} {}\n",
                if option_has_size_in_kib(option) {
                    "KiB"
                } else {
                    ""
                }
            ),
        );
    }

    _mi_fprintf(
        Some(out),
        arg,
        format_args!(
            "debug level : {}\n",
            if cfg!(debug_assertions) { 2 } else { 0 }
        ),
    );
    _mi_fprintf(Some(out), arg, format_args!("secure level: 0\n"));
    _mi_fprintf(Some(out), arg, format_args!("mem tracking: false\n"));
    if mi_option_get(mi_option_t::mi_option_guarded_sample_rate) != 0 {
        _mi_fprintf(Some(out), arg, format_args!("guarded build: enabled\n"));
    } else {
        _mi_fprintf(Some(out), arg, format_args!("guarded build: disabled\n"));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_options_print() {
    mi_options_print_out(None, ptr::null_mut());
}

/// # Safety
///
/// `assertion`, `fname`, and `func`, when non-null, must point to valid
/// nul-terminated C strings. This function aborts and does not return.
#[cold]
pub unsafe extern "C" fn _mi_assert_fail(
    assertion: *const c_char,
    fname: *const c_char,
    line: u32,
    func: *const c_char,
) -> ! {
    let assertion = if assertion.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(assertion).to_str().unwrap_or("") }
    };
    let fname = if fname.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(fname).to_str().unwrap_or("") }
    };
    let func = if func.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(func).to_str().unwrap_or("") }
    };
    unsafe {
        unsafe extern "C" {
            fn dprintf(
                fd: core::ffi::c_int,
                format: *const core::ffi::c_char,
                ...
            ) -> core::ffi::c_int;
        }
        dprintf(
            2,
            c"mimalloc PANIC or ASSERT FAILED: at %s:%d, %s\n  assertion: %s\n".as_ptr(),
            if fname.is_empty() {
                c"".as_ptr()
            } else {
                fname.as_ptr() as *const _
            },
            line,
            if func.is_empty() {
                c"".as_ptr()
            } else {
                func.as_ptr() as *const _
            },
            if assertion.is_empty() {
                c"".as_ptr()
            } else {
                assertion.as_ptr() as *const _
            },
        );
    }
    _mi_fprintf(
        None,
        ptr::null_mut(),
        format_args!(
            "mimalloc: assertion failed: at \"{}\":{}, {}\n  assertion: \"{}\"\n",
            fname, line, func, assertion
        ),
    );
    unsafe { c_abort() };
}

#[inline(never)]
pub(crate) extern "C" fn _mi_options_init() {
    for i in 0..MI_OPTION_COUNT {
        let option = option_from_index(i);
        let _ = mi_option_get(option);
    }
    let max_errors = mi_option_get(mi_option_t::mi_option_max_errors);
    let max_warnings = mi_option_get(mi_option_t::mi_option_max_warnings);
    mi_max_error_count.store(max_errors as isize, Ordering::Release);
    mi_max_warning_count.store(max_warnings as isize, Ordering::Release);
    error_count.store(0, Ordering::Release);
    warning_count.store(0, Ordering::Release);

    if mi_option_get(mi_option_t::mi_option_guarded_sample_rate) > 0
        && mi_option_is_enabled(mi_option_t::mi_option_allow_large_os_pages)
    {
        mi_option_disable(mi_option_t::mi_option_allow_large_os_pages);
        _mi_warning_message_fmt(format_args!(
            "option 'allow_large_os_pages' is disabled to allow for guarded objects\n"
        ));
    }
}

#[inline(never)]
pub(crate) extern "C" fn _mi_options_post_init() {
    mi_add_stderr_output();
    if mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        mi_options_print();
    }
}
