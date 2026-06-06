//! Global / thread initialization

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::atomic::mi_lock_t;
use crate::types::{mi_theap_t, mi_threadid_t, mi_tld_t};

pub const MI_TLS_DIRECT_SLOTS: usize = 64;
pub const MI_TLS_INITIAL_SLOT: usize = 5;
#[allow(clippy::manual_dangling_ptr)]
pub const MI_TLD_INVALID: *mut mi_tld_t = 1usize as *mut mi_tld_t;

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
