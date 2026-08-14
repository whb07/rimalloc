//! Option descriptors, atomic values, environment parsing, and mutation.

use core::cell::UnsafeCell;
use core::ffi::{CStr, c_char, c_long};
use core::fmt;
use core::mem::size_of;
use core::ptr;
use core::ptr::{NonNull, addr_of_mut};
use core::sync::atomic::{AtomicIsize, AtomicU8, Ordering};

use crate::allocator::arena::{MI_ARENA_BCHUNK_BITS, MI_ARENA_SLICE_SIZE};
use crate::allocator::init::_mi_preloading;
use crate::allocator::page::MI_MAX_ALLOC_SIZE;
use crate::ffi::libc;
use crate::internal::MI_GUARDED;
use crate::internal::atomic::{mi_lock_s, mi_lock_t};
use crate::internal::bits::{MI_GiB, MI_KiB, MI_MiB, MI_SIZE_BITS};
use crate::types::{MiLockGuard, MiLockPtr};
use crate::types::{mi_option_desc_t, mi_option_init_t};

use super::{MI_OPTION_COUNT, mi_option_t};

type WarningSink = for<'a> fn(fmt::Arguments<'a>);

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

// SAFETY: every mutable table access is serialized by `options_lock`.
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

#[inline]
fn with_options_table<R>(f: impl FnOnce(&mut [mi_option_desc_t; MI_OPTION_COUNT]) -> R) -> R {
    unsafe {
        let lock = MiLockPtr::from_nonnull(NonNull::new_unchecked(addr_of_mut!(options_lock)));
        let _lock = MiLockGuard::acquire(lock);
        f(&mut *OPTIONS.0.get())
    }
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
        libc::_mi_getenv(
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
    crate::internal::mi_debug_assert!(i < MI_OPTION_COUNT);
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

fn mi_option_init(desc: &mut mi_option_desc_t, warn: WarningSink) {
    let name = unsafe { CStr::from_ptr(desc.name) };
    let name_str = name.to_str().unwrap_or("");
    let mut value_buf = [0 as c_char; 65];
    let mut found = get_env_value_stack(name.to_bytes(), &mut value_buf);

    if found.is_none() && !desc.legacy_name.is_null() {
        let legacy = unsafe { CStr::from_ptr(desc.legacy_name) };
        let legacy_str = legacy.to_str().unwrap_or("");
        found = get_env_value_stack(legacy.to_bytes(), &mut value_buf);
        if found.is_some() {
            warn(format_args!(
                "environment option \"mimalloc_{}\" is deprecated -- use \"mimalloc_{}\" instead.\n",
                legacy_str, name_str
            ));
        }
    }

    if found.is_none() {
        for legacy in option_extra_legacy_names(desc.option) {
            found = get_env_value_stack(legacy.as_bytes(), &mut value_buf);
            if found.is_some() {
                warn(format_args!(
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
                warn(format_args!(
                    "environment option mimalloc_{} has an invalid value.\n",
                    name_str
                ));
                desc.value = 0;
            } else {
                warn(format_args!(
                    "environment option mimalloc_{} has an invalid value.\n",
                    name_str
                ));
            }
        }
    } else if !_mi_preloading() {
        desc.init = mi_option_init_t::MI_OPTION_DEFAULTED;
    }
}

pub(super) fn option_get_fast(option: mi_option_t) -> c_long {
    let idx = option.as_usize();
    crate::internal::mi_assert!(idx < MI_OPTION_COUNT);
    if idx >= MI_OPTION_COUNT {
        return 0;
    }
    option_value_load(idx)
}

pub(super) fn option_get(option: mi_option_t, warn: WarningSink) -> c_long {
    let idx = option.as_usize();
    crate::internal::mi_assert!(idx < MI_OPTION_COUNT);
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
            mi_option_init(&mut desc, warn);
            if desc.init == mi_option_init_t::MI_OPTION_INITIALIZED {
                option_set(option, desc.value);
            }
        }
    }

    option_value_load(idx)
}

pub(super) fn option_get_clamp(
    option: mi_option_t,
    min: c_long,
    max: c_long,
    warn: WarningSink,
) -> c_long {
    let x = option_get(option, warn);
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

pub(super) fn option_get_size(option: mi_option_t, warn: WarningSink) -> usize {
    let x = option_get(option, warn);
    let mut size = if x < 0 { 0 } else { x as usize };
    if option_has_size_in_kib(option) {
        match size.checked_mul(MI_KiB) {
            Some(bytes) if bytes <= MI_MAX_ALLOC_SIZE => {
                size = bytes;
            }
            _ => size = MI_MAX_ALLOC_SIZE,
        }
    }
    size
}

pub(super) fn option_set(option: mi_option_t, value: c_long) {
    let idx = option.as_usize();
    crate::internal::mi_assert!(idx < MI_OPTION_COUNT);
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
        option_set(option, value);
    }
}

pub(super) fn option_set_default(option: mi_option_t, value: c_long) {
    let idx = option.as_usize();
    crate::internal::mi_assert!(idx < MI_OPTION_COUNT);
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

pub(super) fn option_is_enabled(option: mi_option_t, warn: WarningSink) -> bool {
    option_get(option, warn) != 0
}

pub(super) fn option_set_enabled(option: mi_option_t, enable: bool) {
    option_set(option, if enable { 1 } else { 0 });
}

pub(super) fn option_set_enabled_default(option: mi_option_t, enable: bool) {
    option_set_default(option, if enable { 1 } else { 0 });
}

pub(super) fn option_enable(option: mi_option_t) {
    option_set_enabled(option, true);
}

pub(super) fn option_disable(option: mi_option_t) {
    option_set_enabled(option, false);
}

pub(super) fn option_count() -> usize {
    MI_OPTION_COUNT
}

pub(super) fn option_at(index: usize) -> mi_option_t {
    option_from_index(index)
}

pub(super) fn option_descriptor(index: usize) -> mi_option_desc_t {
    with_options_table(|table| table[index])
}

pub(super) fn option_uses_kib(option: mi_option_t) -> bool {
    option_has_size_in_kib(option)
}
