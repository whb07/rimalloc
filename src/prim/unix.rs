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
use core::ffi::{c_uint, c_void};
use std::ffi::CStr;
use std::ffi::CString;
use std::io::{self, Write};

use crate::bits::MI_MiB;
use crate::prim::mi_process_info_t;
use crate::types::{mi_msecs_t, mi_theap_t};

pub const MI_UNIX_PRIM_C: () = ();
pub const MAC_OS_X_VERSION_10_7: c_int = 1070;
pub const MI_HAS_SYSCALL_H: bool = cfg!(any(target_os = "linux", target_os = "freebsd"));
pub const MI_UNIX_LARGE_PAGE_SIZE: usize = 2 * MI_MiB;
pub const GRND_NONBLOCK: c_int = 1;
pub const MPOL_PREFERRED: c_ulong = 1;
pub const R_OK: c_int = 4;
pub const RUSAGE_SELF: c_int = 0;
pub const PTHREAD_KEY_INVALID: pthread_key_t = u32::MAX;

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

#[cfg(any(target_os = "linux", target_os = "android"))]
pub const MAP_ANONYMOUS: c_int = 0x20;
#[cfg(target_os = "macos")]
pub const MAP_ANONYMOUS: c_int = 0x1000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub const MAP_ANONYMOUS: c_int = 0x1000;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
pub const MAP_ANONYMOUS: c_int = 0;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub const MAP_NORESERVE: c_int = 0x4000;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub const MAP_NORESERVE: c_int = 0;

type c_ulong = u64;
type pthread_key_t = c_uint;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct mi_timeval {
    pub tv_sec: c_long,
    pub tv_usec: c_long,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct rusage {
    pub ru_utime: mi_timeval,
    pub ru_stime: mi_timeval,
    pub ru_maxrss: c_long,
    pub ru_ixrss: c_long,
    pub ru_idrss: c_long,
    pub ru_isrss: c_long,
    pub ru_minflt: c_long,
    pub ru_majflt: c_long,
    pub ru_nswap: c_long,
    pub ru_inblock: c_long,
    pub ru_oublock: c_long,
    pub ru_msgsnd: c_long,
    pub ru_msgrcv: c_long,
    pub ru_nsignals: c_long,
    pub ru_nvcsw: c_long,
    pub ru_nivcsw: c_long,
}

#[cfg(unix)]
unsafe extern "C" {
    fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn access(path: *const c_char, amode: c_int) -> c_int;
    fn getrusage(who: c_int, usage: *mut rusage) -> c_int;
    fn pthread_key_create(
        key: *mut pthread_key_t,
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn _NSGetEnviron() -> *mut *mut *mut c_char;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    static mut environ: *mut *mut c_char;
}

static mut _mi_heap_default_key: pthread_key_t = PTHREAD_KEY_INVALID;

#[inline]
pub unsafe extern "C" fn mi_prim_open(fpath: *const c_char, open_flags: c_int) -> c_int {
    open(fpath, open_flags)
}

#[inline]
pub unsafe fn timeval_secs(tv: *const mi_timeval) -> mi_msecs_t {
    if tv.is_null() {
        return 0;
    }
    ((*tv).tv_sec as mi_msecs_t * 1000) + ((*tv).tv_usec as mi_msecs_t / 1000)
}

#[inline]
pub unsafe extern "C" fn _mi_prim_free(addr: *mut c_void, size: usize) -> c_int {
    if size == 0 {
        return 0;
    }
    let err = munmap(addr, size) == -1;
    if err {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    } else {
        0
    }
}

#[inline]
pub unsafe extern "C" fn mi_prim_mbind(
    start: *mut c_void,
    len: c_ulong,
    mode: c_ulong,
    nmask: *const c_ulong,
    maxnode: c_ulong,
    flags: c_uint,
) -> c_long {
    let _ = start;
    let _ = len;
    let _ = mode;
    let _ = nmask;
    let _ = maxnode;
    let _ = flags;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node_count() -> usize {
    #[cfg(target_os = "linux")]
    {
        let mut node: usize = 0;
        while node < 256 {
            let path = format!("/sys/devices/system/node/node{}", node + 1);
            let Ok(path) = CString::new(path) else {
                break;
            };
            if access(path.as_ptr(), R_OK) != 0 {
                break;
            }
            node += 1;
        }
        node + 1
    }
    #[cfg(not(target_os = "linux"))]
    {
        1
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    if pinfo.is_null() {
        return;
    }
    let mut usage = rusage::default();
    if getrusage(RUSAGE_SELF, &mut usage) != 0 {
        return;
    }
    (*pinfo).utime = timeval_secs(&usage.ru_utime);
    (*pinfo).stime = timeval_secs(&usage.ru_stime);
    (*pinfo).page_faults = usage.ru_majflt as usize;
    (*pinfo).peak_rss = (usage.ru_maxrss as usize).saturating_mul(1024);
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

#[inline]
pub unsafe extern "C" fn mi_pthread_done(value: *mut c_void) {
    if !value.is_null() {
        crate::internal::_mi_thread_done(value.cast::<mi_theap_t>());
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {
    crate::mi_assert_internal!(_mi_heap_default_key == PTHREAD_KEY_INVALID);
    let _ = pthread_key_create(&raw mut _mi_heap_default_key, Some(mi_pthread_done));
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    if _mi_heap_default_key != PTHREAD_KEY_INVALID {
        let _ = pthread_setspecific(_mi_heap_default_key, theap.cast::<c_void>());
    }
}
