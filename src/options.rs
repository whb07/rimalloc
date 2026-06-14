//! Options, stats, random, thread, init helpers

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::cell::Cell;
use core::cell::UnsafeCell;
use core::ffi::c_char;
use core::ptr;
use core::ptr::addr_of_mut;
use std::ffi::CStr;
use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::os::raw::{c_int, c_long, c_void};
use std::process::abort;
use std::sync::atomic::{AtomicIsize, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::arena::{MI_ARENA_BCHUNK_BITS, MI_ARENA_SLICE_SIZE};
use crate::atomic::{mi_lock_acquire, mi_lock_release, mi_lock_s, mi_lock_t};
use crate::bits::{MI_GiB, MI_KiB, MI_MiB, MI_SIZE_BITS};
use crate::init::{_mi_is_main_thread, _mi_preloading, _mi_thread_id, mi_mark_main_thread};
use crate::internal::{MI_DEBUG, MI_GUARDED};
use crate::os::_mi_prim_out_stderr;
use crate::page::MI_MAX_ALLOC_SIZE;
use crate::types::{
    EFAULT, MI_MALLOC_VERSION, mi_error_fun, mi_option_desc_t, mi_option_init_t, mi_output_fun,
};

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_option_e {
    mi_option_show_errors = 0,
    mi_option_show_stats = 1,
    mi_option_verbose = 2,
    mi_option_deprecated_eager_commit = 3,
    mi_option_arena_eager_commit = 4,
    mi_option_purge_decommits = 5,
    mi_option_allow_large_os_pages = 6,
    mi_option_reserve_huge_os_pages = 7,
    mi_option_reserve_huge_os_pages_at = 8,
    mi_option_reserve_os_memory = 9,
    mi_option_deprecated_segment_cache = 10,
    mi_option_deprecated_page_reset = 11,
    mi_option_deprecated_abandoned_page_purge = 12,
    mi_option_deprecated_segment_reset = 13,
    mi_option_deprecated_eager_commit_delay = 14,
    mi_option_purge_delay = 15,
    mi_option_use_numa_nodes = 16,
    mi_option_disallow_os_alloc = 17,
    mi_option_os_tag = 18,
    mi_option_max_errors = 19,
    mi_option_max_warnings = 20,
    mi_option_deprecated_max_segment_reclaim = 21,
    mi_option_destroy_on_exit = 22,
    mi_option_arena_reserve = 23,
    mi_option_arena_purge_mult = 24,
    mi_option_deprecated_purge_extend_delay = 25,
    mi_option_disallow_arena_alloc = 26,
    mi_option_retry_on_oom = 27,
    mi_option_visit_abandoned = 28,
    mi_option_guarded_min = 29,
    mi_option_guarded_max = 30,
    mi_option_guarded_precise = 31,
    mi_option_guarded_sample_rate = 32,
    mi_option_guarded_sample_seed = 33,
    mi_option_generic_collect = 34,
    mi_option_page_reclaim_on_free = 35,
    mi_option_page_full_retain = 36,
    mi_option_page_max_candidates = 37,
    mi_option_max_vabits = 38,
    mi_option_pagemap_commit = 39,
    mi_option_page_commit_on_demand = 40,
    mi_option_page_max_reclaim = 41,
    mi_option_page_cross_thread_max_reclaim = 42,
    mi_option_allow_thp = 43,
    mi_option_minimal_purge_size = 44,
    mi_option_arena_max_object_size = 45,
    mi_option_arena_is_numa_local = 46,
    _mi_option_last = 47,
}

pub type mi_option_t = mi_option_e;

pub const mi_option_eager_commit: mi_option_t = mi_option_e::mi_option_deprecated_eager_commit;
pub const mi_option_eager_commit_delay: mi_option_t =
    mi_option_e::mi_option_deprecated_eager_commit_delay;
pub const mi_option_abandoned_page_purge: mi_option_t =
    mi_option_e::mi_option_deprecated_abandoned_page_purge;
pub const mi_option_abandoned_reclaim_on_free: mi_option_t =
    mi_option_e::mi_option_page_reclaim_on_free;
pub const mi_option_max_segment_reclaim: mi_option_t =
    mi_option_e::mi_option_deprecated_max_segment_reclaim;
pub const mi_option_purge_extend_delay: mi_option_t =
    mi_option_e::mi_option_deprecated_purge_extend_delay;
pub const mi_option_decommit_extend_delay: mi_option_t =
    mi_option_e::mi_option_deprecated_purge_extend_delay;
pub const mi_option_large_os_pages: mi_option_t = mi_option_e::mi_option_allow_large_os_pages;
pub const mi_option_eager_region_commit: mi_option_t = mi_option_e::mi_option_arena_eager_commit;
pub const mi_option_reset_decommits: mi_option_t = mi_option_e::mi_option_purge_decommits;
pub const mi_option_reset_delay: mi_option_t = mi_option_e::mi_option_purge_delay;
pub const mi_option_limit_os_alloc: mi_option_t = mi_option_e::mi_option_disallow_os_alloc;

const MI_OPTION_COUNT: usize = mi_option_e::_mi_option_last as usize;

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
            option: mi_option_e::$option,
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
            option: mi_option_e::$option,
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

static OPTIONS: Mutex<[mi_option_desc_t; MI_OPTION_COUNT]> = Mutex::new([
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
]);

thread_local! {
    static recurse: Cell<bool> = const { Cell::new(false) };
}

struct DelayedOutputBuffer(UnsafeCell<[u8; MI_MAX_DELAY_OUTPUT + 1]>);

unsafe impl Sync for DelayedOutputBuffer {}

static out_buf: DelayedOutputBuffer =
    DelayedOutputBuffer(UnsafeCell::new([0; MI_MAX_DELAY_OUTPUT + 1]));
static out_len: AtomicUsize = AtomicUsize::new(0);
static mut out_buf_lock: mi_lock_t = mi_lock_s::new();
static mi_out_default: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static mi_out_arg: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
#[allow(dead_code)]
static mi_max_error_count: AtomicIsize = AtomicIsize::new(16);
#[allow(dead_code)]
static mi_max_warning_count: AtomicIsize = AtomicIsize::new(16);
#[allow(dead_code)]
static error_count: AtomicUsize = AtomicUsize::new(0);
#[allow(dead_code)]
static warning_count: AtomicUsize = AtomicUsize::new(0);
static mi_error_handler: Mutex<Option<mi_error_fun>> = Mutex::new(None);
static mi_error_arg: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

#[inline]
fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
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
fn mi_output_fun_to_ptr(out: mi_output_fun) -> *mut c_void {
    out as *const () as *mut c_void
}

#[inline]
fn ptr_to_mi_output_fun(p: *mut c_void) -> Option<mi_output_fun> {
    if p.is_null() {
        None
    } else {
        Some(unsafe { core::mem::transmute::<*mut c_void, mi_output_fun>(p) })
    }
}

#[inline]
fn emit_text(out: Option<mi_output_fun>, arg: *mut c_void, text: &str) {
    let mut ctext = StackMessage::new();
    let _ = ctext.write_str(text);
    unsafe {
        match out {
            Some(out) => out(ctext.as_c_ptr(), arg),
            None => _mi_prim_out_stderr(ctext.as_c_ptr()),
        }
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

fn mi_out_buf_flush(out: mi_output_fun, no_more_buf: bool, arg: *mut c_void) {
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
    unsafe { out(buf.as_ptr().cast::<c_char>(), arg) };
    if !no_more_buf {
        buf[count] = b'\n';
    }
    unsafe { mi_lock_release(addr_of_mut!(out_buf_lock)) };
}

#[allow(dead_code)]
unsafe extern "C" fn mi_out_buf_stderr(msg: *const c_char, arg: *mut c_void) {
    unsafe { mi_out_stderr(msg, arg) };
    unsafe { mi_out_buf(msg, arg) };
}

fn mi_out_get_default(parg: Option<&mut *mut c_void>) -> mi_output_fun {
    if let Some(arg) = parg {
        *arg = mi_out_arg.load(Ordering::Acquire);
    }
    ptr_to_mi_output_fun(mi_out_default.load(Ordering::Acquire)).unwrap_or(mi_out_buf)
}

#[allow(dead_code)]
fn mi_add_stderr_output() {
    if !mi_out_default.load(Ordering::Acquire).is_null() {
        return;
    }
    mi_out_buf_flush(mi_out_stderr, false, ptr::null_mut());
    mi_out_default.store(mi_output_fun_to_ptr(mi_out_buf_stderr), Ordering::Release);
    mi_out_arg.store(ptr::null_mut(), Ordering::Release);
}

#[inline]
fn mi_recurse_enter_prim() -> bool {
    recurse.with(|slot| {
        if slot.get() {
            false
        } else {
            slot.set(true);
            true
        }
    })
}

#[inline]
fn mi_recurse_enter() -> bool {
    if cfg!(any(target_vendor = "apple", target_os = "android")) && _mi_preloading() {
        return false;
    }
    mi_recurse_enter_prim()
}

#[inline]
fn mi_recurse_exit_prim() {
    recurse.with(|slot| slot.set(false));
}

#[inline]
fn mi_recurse_exit() {
    if cfg!(any(target_vendor = "apple", target_os = "android")) && _mi_preloading() {
        return;
    }
    mi_recurse_exit_prim();
}

#[allow(dead_code)]
pub(crate) unsafe fn _mi_fputs(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    message: *const c_char,
) {
    if let Some(out) = out {
        if !prefix.is_null() {
            unsafe { out(prefix, arg) };
        }
        if !message.is_null() {
            unsafe { out(message, arg) };
        }
        return;
    }

    if !mi_recurse_enter() {
        return;
    }
    let mut default_arg = ptr::null_mut();
    let default_out = mi_out_get_default(Some(&mut default_arg));
    if !prefix.is_null() {
        unsafe { default_out(prefix, default_arg) };
    }
    if !message.is_null() {
        unsafe { default_out(message, default_arg) };
    }
    mi_recurse_exit();
}

#[allow(dead_code)]
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
}

impl fmt::Write for StackMessage {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let capacity = self.bytes.len() - 1;
        if self.len >= capacity {
            return Ok(());
        }
        let available = capacity - self.len;
        let copy = s.len().min(available);
        self.bytes[self.len..self.len + copy].copy_from_slice(&s.as_bytes()[..copy]);
        self.len += copy;
        Ok(())
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn mi_vfprintf_thread(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    args: fmt::Arguments<'_>,
) {
    if !prefix.is_null() {
        let prefix_len = unsafe { CStr::from_ptr(prefix).to_bytes().len() };
        if prefix_len <= 32 && !_mi_is_main_thread() {
            let prefix_text = unsafe { CStr::from_ptr(prefix).to_string_lossy() };
            let mut tprefix = StackMessage::new();
            let _ = write!(
                tprefix,
                "{prefix_text}thread 0x{:x}: ",
                _mi_thread_id() as usize
            );
            mi_vfprintf(out, arg, tprefix.as_c_ptr(), args);
            return;
        }
    }
    mi_vfprintf(out, arg, prefix, args);
}

pub(crate) fn _mi_raw_message(args: fmt::Arguments<'_>) {
    mi_vfprintf(None, ptr::null_mut(), ptr::null(), args);
}

pub(crate) fn _mi_message(args: fmt::Arguments<'_>) {
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

pub(crate) fn _mi_trace_message(args: fmt::Arguments<'_>) {
    if mi_option_get(mi_option_t::mi_option_verbose) <= 1 {
        return;
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

pub(crate) fn _mi_verbose_message(args: fmt::Arguments<'_>) {
    if !mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        return;
    }
    mi_vfprintf(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[allow(dead_code)]
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
/// Matches the exported C helper surface. Extra varargs from C-style callers are ignored here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_warning_message(fmt: *const c_char) {
    let args = if fmt.is_null() {
        format_args!("")
    } else {
        format_args!("{}", CStr::from_ptr(fmt).to_string_lossy())
    };
    _mi_warning_message_fmt(args);
}

pub(crate) fn _mi_error_message_fmt(err: c_int, args: fmt::Arguments<'_>) {
    mi_show_error_message(args);
    let handler = *lock_unpoisoned(&mi_error_handler);
    let arg = mi_error_arg.load(Ordering::Acquire);
    if let Some(handler) = handler {
        unsafe { handler(err, arg) };
    } else {
        mi_error_default(err);
    }
}

/// # Safety
/// Matches the exported C helper surface. Extra varargs from C-style callers are ignored here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_error_message(err: c_int, fmt: *const c_char) {
    let args = if fmt.is_null() {
        format_args!("")
    } else {
        format_args!("{}", CStr::from_ptr(fmt).to_string_lossy())
    };
    _mi_error_message_fmt(err, args);
}

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
    unsafe { core::mem::transmute::<i32, mi_option_t>(i as i32) }
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

#[allow(dead_code)]
pub(crate) fn mi_error_default(err: i32) {
    if err == EFAULT && (MI_DEBUG > 0 || cfg!(feature = "secure")) {
        abort();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void) {
    mi_out_default.store(
        mi_output_fun_to_ptr(out.unwrap_or(mi_out_stderr)),
        Ordering::Release,
    );
    mi_out_arg.store(arg, Ordering::Release);
    if let Some(out) = out {
        mi_out_buf_flush(out, true, arg);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void) {
    *lock_unpoisoned(&mi_error_handler) = fun;
    mi_error_arg.store(arg, Ordering::Release);
}

#[allow(dead_code)]
pub(crate) fn mi_option_has_size_in_kib(option: mi_option_t) -> bool {
    option_has_size_in_kib(option)
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
    let idx = option as usize;
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    let table = lock_unpoisoned(&OPTIONS);
    let desc = &table[idx];
    crate::mi_debug_assert!(desc.option as usize == idx);
    desc.value
}

#[unsafe(no_mangle)]
pub extern "C" fn _mi_option_get_fast(option: mi_option_t) -> c_long {
    mi_option_get_fast_impl(option)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get(option: mi_option_t) -> c_long {
    let idx = option as usize;
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return 0;
    }

    let needs_init = {
        let table = lock_unpoisoned(&OPTIONS);
        table[idx].init == mi_option_init_t::MI_OPTION_UNINIT
    };

    if needs_init {
        let mut desc = {
            let mut table = lock_unpoisoned(&OPTIONS);
            if table[idx].init == mi_option_init_t::MI_OPTION_UNINIT {
                table[idx].init = mi_option_init_t::MI_OPTION_DEFAULTED;
            }
            table[idx]
        };
        mi_option_init(&mut desc);
        if desc.init == mi_option_init_t::MI_OPTION_INITIALIZED {
            mi_option_set(option, desc.value);
        } else {
            let mut table = lock_unpoisoned(&OPTIONS);
            if table[idx].init == mi_option_init_t::MI_OPTION_UNINIT {
                table[idx].init = mi_option_init_t::MI_OPTION_DEFAULTED;
            }
        }
    }

    let table = lock_unpoisoned(&OPTIONS);
    table[idx].value
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
    let idx = option as usize;
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return;
    }

    let recurse_adjust = {
        let mut table = lock_unpoisoned(&OPTIONS);
        let option_kind = table[idx].option;
        table[idx].value = value;
        table[idx].init = mi_option_init_t::MI_OPTION_INITIALIZED;

        let guarded_min_idx = mi_option_t::mi_option_guarded_min as usize;
        let guarded_max_idx = mi_option_t::mi_option_guarded_max as usize;
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
    };

    if let Some((option, value)) = recurse_adjust {
        mi_option_set(option, value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_default(option: mi_option_t, value: c_long) {
    let idx = option as usize;
    crate::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return;
    }
    let mut table = lock_unpoisoned(&OPTIONS);
    let desc = &mut table[idx];
    if desc.init != mi_option_init_t::MI_OPTION_INITIALIZED {
        desc.value = value;
    }
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
        let mut default_arg = ptr::null_mut();
        (mi_out_get_default(Some(&mut default_arg)), default_arg)
    };

    let vermajor = MI_MALLOC_VERSION / 10000;
    let verminor = (MI_MALLOC_VERSION % 10000) / 100;
    let verpatch = MI_MALLOC_VERSION % 100;
    emit_text(
        Some(out),
        arg,
        &format!("v{vermajor}.{verminor}.{verpatch} (Rust translation)\n"),
    );

    for i in 0..MI_OPTION_COUNT {
        let option = option_from_index(i);
        let value = mi_option_get(option);
        let desc = {
            let table = lock_unpoisoned(&OPTIONS);
            table[i]
        };
        let name = unsafe { CStr::from_ptr(desc.name).to_string_lossy().into_owned() };
        emit_text(
            Some(out),
            arg,
            &format!(
                "option '{name}': {value} {}\n",
                if option_has_size_in_kib(option) {
                    "KiB"
                } else {
                    ""
                }
            ),
        );
    }

    emit_text(
        Some(out),
        arg,
        &format!(
            "debug level : {}\n",
            if cfg!(debug_assertions) { 2 } else { 0 }
        ),
    );
    emit_text(Some(out), arg, "secure level: 0\n");
    emit_text(Some(out), arg, "mem tracking: false\n");
    if mi_option_get(mi_option_t::mi_option_guarded_sample_rate) != 0 {
        emit_text(Some(out), arg, "guarded build: enabled\n");
    } else {
        emit_text(Some(out), arg, "guarded build: disabled\n");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_options_print() {
    mi_options_print_out(None, ptr::null_mut());
}

#[cold]
#[unsafe(no_mangle)]
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
    _mi_fprintf(
        None,
        ptr::null_mut(),
        format_args!(
            "mimalloc: assertion failed: at \"{}\":{}, {}\n  assertion: \"{}\"\n",
            fname, line, func, assertion
        ),
    );
    abort();
}

pub(crate) fn _mi_options_init() {
    mi_mark_main_thread();
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
        eprintln!("option 'allow_large_os_pages' is disabled to allow for guarded objects");
    }
}

pub(crate) fn _mi_options_post_init() {
    mi_add_stderr_output();
    if mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        mi_options_print();
    }
}
