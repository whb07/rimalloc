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
use core::ptr;
use core::sync::atomic::{AtomicI32, Ordering};
use std::ffi::CStr;
use std::ffi::CString;
use std::io::{self, Write};

use crate::bits::MI_MiB;
use crate::internal::MI_DEBUG;
use crate::options::{mi_option_is_enabled, mi_option_t};
use crate::os::{
    _mi_os_canuse_large_page, _mi_os_get_aligned_hint, _mi_os_has_overcommit, _mi_os_page_size,
};
use crate::prim::{mi_os_mem_config_t, mi_process_info_t};
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
pub const MAP_PRIVATE: c_int = 0x02;
pub const PROT_NONE: c_int = 0x00;
pub const PROT_READ: c_int = 0x01;
pub const PROT_WRITE: c_int = 0x02;
pub const O_RDONLY: c_int = 0;
pub const EAGAIN: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EINVAL: c_int = 22;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub const MADV_HUGEPAGE: c_int = 14;

type c_ulong = u64;
type pthread_key_t = c_uint;
type ssize_t = isize;

#[cfg(any(target_os = "linux", target_os = "android"))]
const _SC_PAGESIZE: c_int = 30;
#[cfg(any(target_os = "linux", target_os = "android"))]
const _SC_PHYS_PAGES: c_int = 85;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const _SC_PAGESIZE: c_int = 29;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const _SC_PHYS_PAGES: c_int = 121;

const MAP_FAILED: *mut c_void = usize::MAX as *mut c_void;

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
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> ssize_t;
    fn close(fd: c_int) -> c_int;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn access(path: *const c_char, amode: c_int) -> c_int;
    fn sysconf(name: c_int) -> c_long;
    fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn madvise(addr: *mut c_void, len: usize, advice: c_int) -> c_int;
    fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
    fn getrusage(who: c_int, usage: *mut rusage) -> c_int;
    fn pthread_key_create(
        key: *mut pthread_key_t,
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn pthread_key_delete(key: pthread_key_t) -> c_int;
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
fn mi_last_errno() -> c_int {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[inline]
pub unsafe extern "C" fn mi_prim_open(fpath: *const c_char, open_flags: c_int) -> c_int {
    open(fpath, open_flags)
}

#[inline]
pub unsafe extern "C" fn mi_prim_read(fd: c_int, buf: *mut c_void, bufsize: usize) -> ssize_t {
    read(fd, buf, bufsize)
}

#[inline]
pub unsafe extern "C" fn mi_prim_close(fd: c_int) -> c_int {
    close(fd)
}

#[inline]
pub unsafe extern "C" fn mi_prim_access(fpath: *const c_char, mode: c_int) -> c_int {
    access(fpath, mode)
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
unsafe fn unix_detect_overcommit() -> bool {
    #[cfg(target_os = "linux")]
    {
        let fd = mi_prim_open(c"/proc/sys/vm/overcommit_memory".as_ptr(), O_RDONLY);
        if fd >= 0 {
            let mut buf = [0u8; 32];
            let nread = mi_prim_read(fd, buf.as_mut_ptr().cast::<c_void>(), buf.len());
            let _ = mi_prim_close(fd);
            if nread >= 1 {
                return buf[0] == b'0' || buf[0] == b'1';
            }
        }
    }
    true
}

#[inline]
unsafe fn unix_detect_thp() -> bool {
    #[cfg(target_os = "linux")]
    {
        let fd = mi_prim_open(
            c"/sys/kernel/mm/transparent_hugepage/enabled".as_ptr(),
            O_RDONLY,
        );
        if fd >= 0 {
            let mut buf = [0u8; 64];
            let nread = mi_prim_read(fd, buf.as_mut_ptr().cast::<c_void>(), buf.len());
            let _ = mi_prim_close(fd);
            if nread > 0 {
                let nread = nread as usize;
                return !buf[..nread].windows(7).any(|w| w == b"[never]");
            }
        }
    }
    false
}

#[inline]
unsafe fn unix_detect_physical_memory(page_size: usize, physical_memory_in_kib: *mut usize) {
    if physical_memory_in_kib.is_null() {
        return;
    }
    let pphys = sysconf(_SC_PHYS_PAGES);
    let psize_in_kib = page_size / 1024;
    if pphys > 0 && psize_in_kib > 0 {
        let Ok(pphys) = usize::try_from(pphys) else {
            return;
        };
        if let Some(total) = pphys.checked_mul(psize_in_kib) {
            *physical_memory_in_kib = total;
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    if config.is_null() {
        return;
    }
    let psize = sysconf(_SC_PAGESIZE);
    if psize > 0 {
        let Ok(psize) = usize::try_from(psize) else {
            return;
        };
        (*config).page_size = psize;
        (*config).alloc_granularity = psize;
        unix_detect_physical_memory(
            (*config).page_size,
            ptr::addr_of_mut!((*config).physical_memory_in_kib),
        );
    }
    (*config).large_page_size = MI_UNIX_LARGE_PAGE_SIZE;
    (*config).has_overcommit = unix_detect_overcommit();
    (*config).has_partial_free = true;
    (*config).has_virtual_reserve = true;
    (*config).has_transparent_huge_pages =
        unix_detect_thp() && mi_option_is_enabled(mi_option_t::mi_option_allow_thp);
}

#[inline]
unsafe fn unix_madvise(addr: *mut c_void, size: usize, advice: c_int) -> c_int {
    let res = madvise(addr, size, advice);
    if res == 0 { 0 } else { mi_last_errno() }
}

#[inline]
unsafe fn unix_mmap_prim(
    addr: *mut c_void,
    size: usize,
    protect_flags: c_int,
    flags: c_int,
    fd: c_int,
) -> *mut c_void {
    let p = mmap(addr, size, protect_flags, flags, fd, 0);
    if p == MAP_FAILED { ptr::null_mut() } else { p }
}

#[inline]
unsafe fn unix_mmap_prim_aligned(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    protect_flags: c_int,
    flags: c_int,
    fd: c_int,
) -> *mut c_void {
    if addr.is_null() && try_alignment > 1 {
        let hint = _mi_os_get_aligned_hint(try_alignment, size);
        if !hint.is_null() {
            let p = unix_mmap_prim(hint, size, protect_flags, flags, fd);
            if !p.is_null() && (p as usize).is_multiple_of(try_alignment) {
                return p;
            }
        }
    }
    unix_mmap_prim(addr, size, protect_flags, flags, fd)
}

#[inline]
unsafe fn unix_mmap_fd() -> c_int {
    -1
}

#[inline]
unsafe fn unix_mmap(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    protect_flags: c_int,
    large_only: bool,
    allow_large: bool,
    is_large: *mut bool,
) -> *mut c_void {
    if !is_large.is_null() {
        *is_large = false;
    }
    if large_only {
        return ptr::null_mut();
    }
    let mut flags = MAP_PRIVATE | MAP_ANONYMOUS;
    if _mi_os_has_overcommit() {
        flags |= MAP_NORESERVE;
    }
    let p = unix_mmap_prim_aligned(
        addr,
        size,
        try_alignment,
        protect_flags,
        flags,
        unix_mmap_fd(),
    );
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !p.is_null()
        && allow_large
        && mi_option_is_enabled(mi_option_t::mi_option_allow_thp)
        && _mi_os_canuse_large_page(size, try_alignment)
    {
        let _ = unix_madvise(p, size, MADV_HUGEPAGE);
    }
    p
}

#[inline]
pub unsafe extern "C" fn _mi_prim_alloc(
    hint_addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
    addr: *mut *mut c_void,
) -> c_int {
    crate::mi_assert_internal!(size > 0 && size.is_multiple_of(_mi_os_page_size()));
    crate::mi_assert_internal!(commit || !allow_large);
    crate::mi_assert_internal!(try_alignment > 0);
    if !is_zero.is_null() {
        *is_zero = true;
    }
    let protect_flags = if commit {
        PROT_READ | PROT_WRITE
    } else {
        PROT_NONE
    };
    let p = unix_mmap(
        hint_addr,
        size,
        try_alignment,
        protect_flags,
        false,
        allow_large,
        is_large,
    );
    if !addr.is_null() {
        *addr = p;
    }
    if p.is_null() { mi_last_errno() } else { 0 }
}

#[inline]
unsafe fn unix_mprotect_hint(_err: c_int) {}

#[inline]
pub unsafe extern "C" fn _mi_prim_commit(
    start: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> c_int {
    if !is_zero.is_null() {
        *is_zero = false;
    }
    let err = if mprotect(start, size, PROT_READ | PROT_WRITE) == 0 {
        0
    } else {
        mi_last_errno()
    };
    if err != 0 {
        unix_mprotect_hint(err);
    }
    err
}

#[inline]
pub unsafe extern "C" fn _mi_prim_reuse(start: *mut c_void, size: usize) -> c_int {
    let _ = start;
    let _ = size;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_decommit(
    start: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
) -> c_int {
    let err = unix_madvise(start, size, MADV_DONTNEED);
    if !needs_recommit.is_null() {
        if MI_DEBUG == 0 {
            *needs_recommit = false;
        } else {
            *needs_recommit = true;
            let _ = mprotect(start, size, PROT_NONE);
        }
    }
    err
}

#[inline]
pub unsafe extern "C" fn _mi_prim_reset(start: *mut c_void, size: usize) -> c_int {
    static ADVICE: AtomicI32 = AtomicI32::new(MADV_FREE);
    let mut advice = ADVICE.load(Ordering::Relaxed);
    if advice == 0 {
        advice = MADV_DONTNEED;
    }
    let mut err = unix_madvise(start, size, advice);
    while err == EAGAIN {
        err = unix_madvise(start, size, advice);
    }
    if err == EINVAL && advice == MADV_FREE {
        ADVICE.store(MADV_DONTNEED, Ordering::Release);
        err = unix_madvise(start, size, MADV_DONTNEED);
    }
    err
}

#[inline]
pub unsafe extern "C" fn _mi_prim_protect(start: *mut c_void, size: usize, protect: bool) -> c_int {
    let prot = if protect {
        PROT_NONE
    } else {
        PROT_READ | PROT_WRITE
    };
    let err = if mprotect(start, size, prot) == 0 {
        0
    } else {
        mi_last_errno()
    };
    unix_mprotect_hint(err);
    err
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
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {
    if _mi_heap_default_key != PTHREAD_KEY_INVALID {
        let _ = pthread_key_delete(_mi_heap_default_key);
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    if _mi_heap_default_key != PTHREAD_KEY_INVALID {
        let _ = pthread_setspecific(_mi_heap_default_key, theap.cast::<c_void>());
    }
}
