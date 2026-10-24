//! Allocator option ABI and policy/reporting orchestration.

// Retains upstream option aliases for C header compatibility.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

mod diagnostics;
mod storage;

use core::ffi::{c_char, c_int, c_long, c_void};

use crate::types::{MI_MALLOC_VERSION, mi_error_fun, mi_output_fun};

pub(crate) use diagnostics::{
    _mi_error_message_fmt, _mi_fprintf, _mi_fputs, _mi_raw_message, _mi_verbose_message,
    _mi_warning_message_fmt, registered_error_callback, restore_error_callback,
};

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

#[unsafe(no_mangle)]
pub extern "C" fn mi_version() -> i32 {
    MI_MALLOC_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void) {
    diagnostics::mi_register_output(out, arg);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void) {
    diagnostics::mi_register_error(fun, arg);
}

/// # Safety
/// Matches the internal C helper surface for static C probes. Extra varargs are ignored here.
#[inline(never)]
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
pub(crate) unsafe extern "C" fn _mi_warning_message(fmt: *const c_char) {
    // SAFETY: the caller provides the same C string required by the diagnostic implementation.
    unsafe { diagnostics::warning_message(fmt) };
}

/// # Safety
/// Matches the internal C helper surface for static C probes. Extra varargs are ignored here.
#[inline(never)]
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
pub(crate) unsafe extern "C" fn _mi_error_message(err: c_int, fmt: *const c_char) {
    // SAFETY: the caller provides the same C string required by the diagnostic implementation.
    unsafe { diagnostics::error_message(err, fmt) };
}

#[inline]
pub(crate) extern "C" fn _mi_option_get_fast(option: mi_option_t) -> c_long {
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::option_read(option);
    storage::option_get_fast(option)
}

/// Returns the compile-time default value of `option`.
#[inline]
pub(crate) const fn option_default(option: mi_option_t) -> c_long {
    storage::option_default(option)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get(option: mi_option_t) -> c_long {
    storage::option_get(option)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get_clamp(option: mi_option_t, min: c_long, max: c_long) -> c_long {
    storage::option_get_clamp(option, min, max)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_get_size(option: mi_option_t) -> usize {
    storage::option_get_size(option)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set(option: mi_option_t, value: c_long) {
    storage::option_set(option, value);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_default(option: mi_option_t, value: c_long) {
    storage::option_set_default(option, value);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_is_enabled(option: mi_option_t) -> bool {
    storage::option_is_enabled(option)
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_enabled(option: mi_option_t, enable: bool) {
    storage::option_set_enabled(option, enable);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_set_enabled_default(option: mi_option_t, enable: bool) {
    storage::option_set_enabled_default(option, enable);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_enable(option: mi_option_t) {
    storage::option_enable(option);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_option_disable(option: mi_option_t) {
    storage::option_disable(option);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_options_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    diagnostics::mi_options_print_out(out, arg);
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_options_print() {
    diagnostics::mi_options_print();
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
    // SAFETY: the caller guarantees that every non-null argument is a valid C string.
    unsafe { diagnostics::mi_assert_fail(assertion, fname, line, func) }
}

#[inline(never)]
pub(crate) extern "C" fn _mi_options_init() {
    for index in 0..storage::option_count() {
        let _ = mi_option_get(storage::option_at(index));
    }

    diagnostics::configure_error_limits(
        mi_option_get(mi_option_t::mi_option_max_errors),
        mi_option_get(mi_option_t::mi_option_max_warnings),
    );

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
    diagnostics::add_stderr_output();
    if mi_option_is_enabled(mi_option_t::mi_option_verbose) {
        mi_options_print();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    #[test]
    #[cfg_attr(
        miri,
        ignore = "spawns a native subprocess; unavailable with Miri isolation"
    )]
    fn mimalloc_assert_fail_aborts_instead_of_unwinding() {
        const CHILD_ENV: &str = "MI_FFI_PANIC_AUDIT_ASSERT_CHILD";
        const TEST_NAME: &str =
            "allocator::options::tests::mimalloc_assert_fail_aborts_instead_of_unwinding";

        if std::env::var_os(CHILD_ENV).is_some() {
            unsafe {
                _mi_assert_fail(
                    c"false".as_ptr(),
                    c"options.rs".as_ptr(),
                    line!(),
                    c"mimalloc_assert_fail_aborts_instead_of_unwinding".as_ptr(),
                )
            };
        }

        let current_exe = std::env::current_exe().expect("current test executable path");
        let status = Command::new(current_exe)
            .arg("--exact")
            .arg(TEST_NAME)
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run assertion-abort child process");

        assert!(
            !status.success(),
            "assert failure child unexpectedly returned"
        );
        #[cfg(unix)]
        assert_eq!(status.signal(), Some(6), "expected SIGABRT from child");
    }
}
