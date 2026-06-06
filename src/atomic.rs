//! Atomic and bit primitives

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::hint::spin_loop;
use core::ptr::read_volatile;
use core::sync::atomic::{AtomicI64, AtomicIsize, AtomicUsize, Ordering, fence};

use crate::init::_mi_thread_id;

pub const MI_ATOMIC_H: () = ();
pub const WIN32_LEAN_AND_MEAN: () = ();
pub const MI_USE_PTHREADS: () = ();
pub const _Atomic: () = ();
pub const mi_atomic: () = ();
pub const mi_memory_order: () = ();
pub const MI_ATOMIC_VAR_INIT: () = ();
pub const MI_MSC_64: () = ();
pub const mi_lock: () = ();
pub const EFAULT: i32 = 14;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_memory_order_e {
    mi_memory_order_relaxed = 0,
    mi_memory_order_consume = 1,
    mi_memory_order_acquire = 2,
    mi_memory_order_release = 3,
    mi_memory_order_acq_rel = 4,
    mi_memory_order_seq_cst = 5,
}

pub type mi_memory_order = mi_memory_order_e;

#[repr(C)]
#[derive(Debug)]
pub struct mi_lock_s {
    pub mutex: AtomicUsize,
}

pub type mi_lock_t = mi_lock_s;

impl mi_lock_s {
    pub const fn new() -> Self {
        Self {
            mutex: AtomicUsize::new(0),
        }
    }
}

impl Default for mi_lock_s {
    fn default() -> Self {
        Self::new()
    }
}

pub static MI_LOCK_INITIALIZER: mi_lock_t = mi_lock_s::new();

pub type mi_atomic_guard_t = AtomicUsize;

#[repr(C)]
#[derive(Debug)]
pub struct mi_atomic_once_s {
    pub tid: AtomicUsize,
    pub lock: mi_lock_t,
}

pub type mi_atomic_once_t = mi_atomic_once_s;

impl mi_atomic_once_s {
    pub const fn new() -> Self {
        Self {
            tid: AtomicUsize::new(0),
            lock: mi_lock_s::new(),
        }
    }
}

impl Default for mi_atomic_once_s {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn load_order(order: mi_memory_order_e) -> Ordering {
    match order {
        mi_memory_order_e::mi_memory_order_relaxed => Ordering::Relaxed,
        mi_memory_order_e::mi_memory_order_seq_cst => Ordering::SeqCst,
        _ => Ordering::Acquire,
    }
}

#[inline]
fn store_order(order: mi_memory_order_e) -> Ordering {
    match order {
        mi_memory_order_e::mi_memory_order_relaxed => Ordering::Relaxed,
        mi_memory_order_e::mi_memory_order_seq_cst => Ordering::SeqCst,
        _ => Ordering::Release,
    }
}

#[inline]
fn rmw_order(order: mi_memory_order_e) -> Ordering {
    match order {
        mi_memory_order_e::mi_memory_order_relaxed => Ordering::Relaxed,
        mi_memory_order_e::mi_memory_order_consume | mi_memory_order_e::mi_memory_order_acquire => {
            Ordering::Acquire
        }
        mi_memory_order_e::mi_memory_order_release => Ordering::Release,
        mi_memory_order_e::mi_memory_order_acq_rel => Ordering::AcqRel,
        mi_memory_order_e::mi_memory_order_seq_cst => Ordering::SeqCst,
    }
}

#[inline]
fn cas_failure_order(order: mi_memory_order_e) -> Ordering {
    match order {
        mi_memory_order_e::mi_memory_order_relaxed => Ordering::Relaxed,
        mi_memory_order_e::mi_memory_order_seq_cst => Ordering::SeqCst,
        _ => Ordering::Acquire,
    }
}

#[inline]
unsafe fn atomic_usize<'a>(p: *mut usize) -> &'a AtomicUsize {
    &*(p as *mut AtomicUsize)
}

#[inline]
unsafe fn atomic_usize_const<'a>(p: *const usize) -> &'a AtomicUsize {
    &*(p as *const AtomicUsize)
}

#[inline]
unsafe fn atomic_isize<'a>(p: *mut isize) -> &'a AtomicIsize {
    &*(p as *mut AtomicIsize)
}

#[inline]
unsafe fn atomic_i64<'a>(p: *mut i64) -> &'a AtomicI64 {
    &*(p as *mut AtomicI64)
}

#[inline]
unsafe fn atomic_i64_const<'a>(p: *const i64) -> &'a AtomicI64 {
    &*(p as *const AtomicI64)
}

pub extern "C" fn mi_atomic_thread_fence(mo: mi_memory_order_e) {
    match mo {
        mi_memory_order_e::mi_memory_order_relaxed => {}
        mi_memory_order_e::mi_memory_order_consume | mi_memory_order_e::mi_memory_order_acquire => {
            fence(Ordering::Acquire)
        }
        mi_memory_order_e::mi_memory_order_release => fence(Ordering::Release),
        mi_memory_order_e::mi_memory_order_acq_rel => fence(Ordering::AcqRel),
        mi_memory_order_e::mi_memory_order_seq_cst => fence(Ordering::SeqCst),
    }
}

pub unsafe extern "C" fn mi_atomic_fetch_add_explicit(
    p: *mut usize,
    add: usize,
    mo: mi_memory_order_e,
) -> usize {
    atomic_usize(p).fetch_add(add, rmw_order(mo))
}

pub unsafe extern "C" fn mi_atomic_fetch_sub_explicit(
    p: *mut usize,
    sub: usize,
    mo: mi_memory_order_e,
) -> usize {
    atomic_usize(p).fetch_sub(sub, rmw_order(mo))
}

pub unsafe extern "C" fn mi_atomic_fetch_and_explicit(
    p: *mut usize,
    x: usize,
    mo: mi_memory_order_e,
) -> usize {
    atomic_usize(p).fetch_and(x, rmw_order(mo))
}

pub unsafe extern "C" fn mi_atomic_fetch_or_explicit(
    p: *mut usize,
    x: usize,
    mo: mi_memory_order_e,
) -> usize {
    atomic_usize(p).fetch_or(x, rmw_order(mo))
}

pub unsafe extern "C" fn mi_atomic_compare_exchange_strong_explicit(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mo1: mi_memory_order_e,
    mo2: mi_memory_order_e,
) -> bool {
    let current = *expected;
    match atomic_usize(p).compare_exchange(current, desired, rmw_order(mo1), cas_failure_order(mo2))
    {
        Ok(_) => true,
        Err(actual) => {
            *expected = actual;
            false
        }
    }
}

pub unsafe extern "C" fn mi_atomic_compare_exchange_weak_explicit(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mo1: mi_memory_order_e,
    mo2: mi_memory_order_e,
) -> bool {
    mi_atomic_compare_exchange_strong_explicit(p, expected, desired, mo1, mo2)
}

pub unsafe extern "C" fn mi_atomic_exchange_explicit(
    p: *mut usize,
    exchange: usize,
    mo: mi_memory_order_e,
) -> usize {
    atomic_usize(p).swap(exchange, rmw_order(mo))
}

pub unsafe extern "C" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize {
    atomic_usize_const(p).load(load_order(mo))
}

pub unsafe extern "C" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e) {
    atomic_usize(p).store(x, store_order(mo));
}

pub unsafe extern "C" fn mi_atomic_loadi64_explicit(p: *const i64, mo: mi_memory_order_e) -> i64 {
    atomic_i64_const(p).load(load_order(mo))
}

pub unsafe extern "C" fn mi_atomic_storei64_explicit(p: *mut i64, x: i64, mo: mi_memory_order_e) {
    atomic_i64(p).store(x, store_order(mo));
}

pub unsafe extern "C" fn mi_atomic_addi64_relaxed(p: *mut i64, add: i64) -> i64 {
    atomic_i64(p).fetch_add(add, Ordering::Relaxed)
}

pub unsafe extern "C" fn mi_atomic_void_addi64_relaxed(p: *mut i64, padd: *const i64) {
    let add = read_volatile(padd);
    if add != 0 {
        atomic_i64(p).fetch_add(add, Ordering::Relaxed);
    }
}

pub unsafe extern "C" fn mi_atomic_maxi64_relaxed(p: *mut i64, x: i64) {
    let atomic = atomic_i64(p);
    let mut current = atomic.load(Ordering::Relaxed);
    while current < x {
        match atomic.compare_exchange_weak(current, x, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

pub unsafe extern "C" fn mi_atomic_addi64_acq_rel(p: *mut i64, i: i64) {
    atomic_i64(p).fetch_add(i, Ordering::AcqRel);
}

pub unsafe extern "C" fn mi_atomic_casi64_strong_acq_rel(
    p: *mut i64,
    exp: *mut i64,
    des: i64,
) -> bool {
    let current = *exp;
    match atomic_i64(p).compare_exchange(current, des, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => true,
        Err(actual) => {
            *exp = actual;
            false
        }
    }
}

pub unsafe extern "C" fn mi_atomic_addi(p: *mut isize, add: isize) -> isize {
    atomic_isize(p).fetch_add(add, Ordering::AcqRel)
}

pub unsafe extern "C" fn mi_atomic_subi(p: *mut isize, sub: isize) -> isize {
    atomic_isize(p).fetch_sub(sub, Ordering::AcqRel)
}

pub unsafe extern "C" fn mi_lock_try_acquire(lock: *mut mi_lock_t) -> bool {
    let expected = 0;
    (*lock)
        .mutex
        .compare_exchange(expected, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

pub unsafe extern "C" fn mi_lock_acquire(lock: *mut mi_lock_t) {
    while !mi_lock_try_acquire(lock) {
        spin_loop();
    }
}

pub unsafe extern "C" fn mi_lock_release(lock: *mut mi_lock_t) {
    (*lock).mutex.store(0, Ordering::Release);
}

pub unsafe extern "C" fn mi_lock_init(lock: *mut mi_lock_t) {
    mi_lock_release(lock);
}

pub unsafe extern "C" fn mi_lock_done(lock: *mut mi_lock_t) {
    let _ = lock;
}

pub unsafe extern "C" fn _mi_atomic_once_enter(once: *mut mi_atomic_once_t) -> bool {
    let once_tid = (*once).tid.load(Ordering::Acquire);
    if once_tid == 1 {
        return false;
    }
    let current_tid = _mi_thread_id();
    if once_tid == current_tid {
        return false;
    }

    mi_lock_acquire(&mut (*once).lock);
    let expected = 0usize;
    if (*once)
        .tid
        .compare_exchange(expected, current_tid, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        true
    } else {
        mi_lock_release(&mut (*once).lock);
        false
    }
}

pub unsafe extern "C" fn _mi_atomic_once_release(once: *mut mi_atomic_once_t) {
    if (*once).tid.load(Ordering::Acquire) > 1 {
        (*once).tid.store(1, Ordering::Release);
        mi_lock_release(&mut (*once).lock);
    }
}
