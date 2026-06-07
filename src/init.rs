//! Global / thread initialization

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::atomic::mi_lock_t;
use crate::bits::MI_SIZE_SIZE;
use crate::stats::mi_stat_count_t;
use crate::types::{mi_theap_t, mi_thread_local_t, mi_threadid_t, mi_tld_t};

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
