//! Global / thread initialization

#![allow(non_camel_case_types, non_upper_case_globals)]

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::types::mi_threadid_t;

#[unsafe(no_mangle)]
pub static mut _mi_cpu_movsb_max: usize = 0;

#[unsafe(no_mangle)]
pub static mut _mi_cpu_stosb_max: usize = 0;

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
