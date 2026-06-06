//! Bounded mechanical translation of the early helper slice from
//! `src/prim/unix/prim.c`.
//!
//! This file is intentionally standalone for now.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    unused_imports,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::{c_char, c_int, c_long};
use std::ffi::CStr;
use std::io::{self, Write};

use crate::bits::MI_MiB;
use crate::types::mi_msecs_t;

pub const MI_UNIX_PRIM_C: () = ();
pub const MAC_OS_X_VERSION_10_7: c_int = 1070;
pub const MI_HAS_SYSCALL_H: bool = cfg!(any(target_os = "linux", target_os = "freebsd"));
pub const MI_UNIX_LARGE_PAGE_SIZE: usize = 2 * MI_MiB;
pub const GRND_NONBLOCK: c_int = 1;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub const MADV_DONTNEED: c_int = 4;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
pub const MADV_DONTNEED: c_int = 0;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub const MADV_FREE: c_int = 8;
#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub const MADV_FREE: c_int = 5;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
pub const MADV_FREE: c_int = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_timeval {
    pub tv_sec: c_long,
    pub tv_usec: c_long,
}

#[cfg(unix)]
unsafe extern "C" {
    fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn _NSGetEnviron() -> *mut *mut *mut c_char;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    static mut environ: *mut *mut c_char;
}

#[inline]
pub unsafe extern "C" fn mi_prim_open(fpath: *const c_char, open_flags: c_int) -> c_int {
    open(fpath, open_flags)
}

#[inline]
pub fn timeval_secs(tv: *const mi_timeval) -> mi_msecs_t {
    if tv.is_null() {
        return 0;
    }
    unsafe { ((*tv).tv_sec as mi_msecs_t * 1000) + ((*tv).tv_usec as mi_msecs_t / 1000) }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let bytes = CStr::from_ptr(msg).to_bytes();
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(bytes);
    let _ = stderr.flush();
}

#[cfg(target_os = "macos")]
#[inline]
pub unsafe extern "C" fn mi_get_environ() -> *mut *mut c_char {
    *_NSGetEnviron()
}

#[cfg(not(target_os = "macos"))]
#[inline]
pub unsafe extern "C" fn mi_get_environ() -> *mut *mut c_char {
    environ
}
