//! Atomic and bit primitives

#![allow(non_camel_case_types, non_upper_case_globals)]

#[cfg(all(target_os = "linux", not(debug_assertions)))]
use core::ffi::c_int;
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
use core::hint::spin_loop;
#[cfg(all(target_os = "linux", not(debug_assertions)))]
use core::ptr;
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
    pub mutex: pthread_mutex_t,
}

pub type mi_lock_t = mi_lock_s;

#[cfg(all(target_os = "linux", not(debug_assertions)))]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct pthread_mutex_t {
    __data: [usize; 5],
}

#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
#[repr(C)]
#[derive(Debug)]
pub struct pthread_mutex_t {
    mutex: AtomicUsize,
}

impl mi_lock_s {
    pub const fn new() -> Self {
        Self {
            mutex: pthread_mutex_t::new(),
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

#[cfg(all(target_os = "linux", not(debug_assertions)))]
impl pthread_mutex_t {
    pub const fn new() -> Self {
        Self { __data: [0; 5] }
    }
}

#[cfg(all(target_os = "linux", not(debug_assertions)))]
impl Default for pthread_mutex_t {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
impl pthread_mutex_t {
    pub const fn new() -> Self {
        Self {
            mutex: AtomicUsize::new(0),
        }
    }
}

#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
impl Default for pthread_mutex_t {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(target_os = "linux", not(debug_assertions)))]
#[link(name = "pthread")]
unsafe extern "C" {
    fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> c_int;
    fn pthread_mutex_lock(mutex: *mut pthread_mutex_t) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t) -> c_int;
    fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t) -> c_int;
}

pub struct mi_atomic_guard_scope_t {
    guard: *mut mi_atomic_guard_t,
}

impl Drop for mi_atomic_guard_scope_t {
    fn drop(&mut self) {
        // SAFETY: the scope is created only after successfully acquiring the guard and
        // releases that same guard exactly once on scope exit.
        unsafe {
            mi_atomic_store_release(self.guard.cast::<usize>(), 0);
        }
    }
}

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

pub struct mi_atomic_once_scope_t {
    once: *mut mi_atomic_once_t,
}

impl Drop for mi_atomic_once_scope_t {
    fn drop(&mut self) {
        // SAFETY: the scope is created only after `_mi_atomic_once_enter` succeeds and
        // must release the same once-state exactly once on scope exit.
        unsafe {
            _mi_atomic_once_release(self.once);
        }
    }
}

/// # Safety
/// `guard` must point to a valid atomic guard value.
pub unsafe fn mi_atomic_guard_enter(
    guard: *mut mi_atomic_guard_t,
) -> Option<mi_atomic_guard_scope_t> {
    let mut expected = 0usize;
    if unsafe { mi_atomic_cas_strong_acq_rel(guard.cast::<usize>(), &mut expected, 1) } {
        Some(mi_atomic_guard_scope_t { guard })
    } else {
        None
    }
}

/// # Safety
/// `once` must point to a valid once-state.
pub unsafe fn mi_atomic_do_once_enter(
    once: *mut mi_atomic_once_t,
) -> Option<mi_atomic_once_scope_t> {
    if unsafe { _mi_atomic_once_enter(once) } {
        Some(mi_atomic_once_scope_t { once })
    } else {
        None
    }
}

#[macro_export]
macro_rules! mi_atomic_guard {
    ($guard:expr, $body:block) => {{
        let _mi_guard_ptr: *mut $crate::atomic::mi_atomic_guard_t = $guard;
        if let Some(_mi_guard_scope) =
            unsafe { $crate::atomic::mi_atomic_guard_enter(_mi_guard_ptr) }
        {
            let _mi_guard_scope = _mi_guard_scope;
            $body
        }
    }};
}

#[macro_export]
macro_rules! mi_atomic_do_once {
    ($body:block) => {{
        static _MI_ONCE: $crate::atomic::mi_atomic_once_t = $crate::atomic::mi_atomic_once_s::new();
        let _mi_once_ptr = core::ptr::addr_of!(_MI_ONCE).cast_mut();
        if let Some(_mi_once_scope) =
            unsafe { $crate::atomic::mi_atomic_do_once_enter(_mi_once_ptr) }
        {
            let _mi_once_scope = _mi_once_scope;
            $body
        }
    }};
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
    unsafe { &*(p as *mut AtomicUsize) }
}

#[inline]
unsafe fn atomic_usize_const<'a>(p: *const usize) -> &'a AtomicUsize {
    unsafe { &*(p as *const AtomicUsize) }
}

#[inline]
unsafe fn atomic_isize<'a>(p: *mut isize) -> &'a AtomicIsize {
    unsafe { &*(p as *mut AtomicIsize) }
}

#[inline]
unsafe fn atomic_i64<'a>(p: *mut i64) -> &'a AtomicI64 {
    unsafe { &*(p as *mut AtomicI64) }
}

#[inline]
unsafe fn atomic_i64_const<'a>(p: *const i64) -> &'a AtomicI64 {
    unsafe { &*(p as *const AtomicI64) }
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

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
pub unsafe extern "C" fn mi_atomic_fetch_add_explicit(
    p: *mut usize,
    add: usize,
    mo: mi_memory_order_e,
) -> usize {
    unsafe { atomic_usize(p) }.fetch_add(add, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
pub unsafe extern "C" fn mi_atomic_fetch_sub_explicit(
    p: *mut usize,
    sub: usize,
    mo: mi_memory_order_e,
) -> usize {
    unsafe { atomic_usize(p) }.fetch_sub(sub, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
pub unsafe extern "C" fn mi_atomic_fetch_and_explicit(
    p: *mut usize,
    x: usize,
    mo: mi_memory_order_e,
) -> usize {
    unsafe { atomic_usize(p) }.fetch_and(x, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
pub unsafe extern "C" fn mi_atomic_fetch_or_explicit(
    p: *mut usize,
    x: usize,
    mo: mi_memory_order_e,
) -> usize {
    unsafe { atomic_usize(p) }.fetch_or(x, rmw_order(mo))
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_increment_acq_rel(p: *mut usize) -> usize {
    unsafe { mi_atomic_fetch_add_explicit(p, 1, mi_memory_order_e::mi_memory_order_acq_rel) }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid pointer-sized storage that is only
/// accessed atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_ptr_weak_release<T>(
    p: *mut *mut T,
    expected: *mut *mut T,
    desired: *mut T,
) -> bool {
    unsafe {
        mi_atomic_compare_exchange_weak_explicit(
            p.cast::<usize>(),
            expected.cast::<usize>(),
            desired as usize,
            mi_memory_order_e::mi_memory_order_release,
            mi_memory_order_e::mi_memory_order_relaxed,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` must point to valid pointer-sized storage that is only accessed
/// atomically.
pub unsafe fn mi_atomic_load_ptr_acquire<T>(p: *const *mut T) -> *mut T {
    unsafe { mi_atomic_load_acquire(p.cast::<usize>()) as *mut T }
}

#[inline]
/// # Safety
///
/// `p` must point to valid pointer-sized storage that is only accessed
/// atomically.
pub unsafe fn mi_atomic_store_ptr_release<T>(p: *mut *mut T, x: *mut T) {
    unsafe { mi_atomic_store_release(p.cast::<usize>(), x as usize) };
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid pointer-sized storage that is only
/// accessed atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_ptr_strong_release<T>(
    p: *mut *mut T,
    expected: *mut *mut T,
    desired: *mut T,
) -> bool {
    unsafe {
        mi_atomic_cas_strong_release(
            p.cast::<usize>(),
            expected.cast::<usize>(),
            desired as usize,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` must point to valid pointer-sized storage that is only accessed
/// atomically.
pub unsafe fn mi_atomic_exchange_ptr_relaxed<T>(p: *mut *mut T, x: *mut T) -> *mut T {
    unsafe { mi_atomic_exchange_relaxed(p.cast::<usize>(), x as usize) as *mut T }
}

#[inline]
/// # Safety
///
/// `p` must point to valid pointer-sized storage that is only accessed
/// atomically.
pub unsafe fn mi_atomic_exchange_ptr_acq_rel<T>(p: *mut *mut T, x: *mut T) -> *mut T {
    unsafe { mi_atomic_exchange_acq_rel(p.cast::<usize>(), x as usize) as *mut T }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_weak(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mem_success: mi_memory_order_e,
    mem_fail: mi_memory_order_e,
) -> bool {
    unsafe { mi_atomic_compare_exchange_weak_explicit(p, expected, desired, mem_success, mem_fail) }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_strong(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mem_success: mi_memory_order_e,
    mem_fail: mi_memory_order_e,
) -> bool {
    unsafe {
        mi_atomic_compare_exchange_strong_explicit(p, expected, desired, mem_success, mem_fail)
    }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_load_acquire(p: *const usize) -> usize {
    unsafe { mi_atomic_load_explicit(p, mi_memory_order_e::mi_memory_order_acquire) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_store_release(p: *mut usize, x: usize) {
    unsafe { mi_atomic_store_explicit(p, x, mi_memory_order_e::mi_memory_order_release) };
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_exchange_relaxed(p: *mut usize, x: usize) -> usize {
    unsafe { mi_atomic_exchange_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_exchange_acq_rel(p: *mut usize, x: usize) -> usize {
    unsafe { mi_atomic_exchange_explicit(p, x, mi_memory_order_e::mi_memory_order_acq_rel) }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_weak_relaxed(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
) -> bool {
    unsafe {
        mi_atomic_cas_weak(
            p,
            expected,
            desired,
            mi_memory_order_e::mi_memory_order_relaxed,
            mi_memory_order_e::mi_memory_order_relaxed,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_weak_release(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
) -> bool {
    unsafe {
        mi_atomic_cas_weak(
            p,
            expected,
            desired,
            mi_memory_order_e::mi_memory_order_release,
            mi_memory_order_e::mi_memory_order_relaxed,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_weak_acq_rel(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
) -> bool {
    unsafe {
        mi_atomic_cas_weak(
            p,
            expected,
            desired,
            mi_memory_order_e::mi_memory_order_acq_rel,
            mi_memory_order_e::mi_memory_order_acquire,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_strong_release(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
) -> bool {
    unsafe {
        mi_atomic_cas_strong(
            p,
            expected,
            desired,
            mi_memory_order_e::mi_memory_order_release,
            mi_memory_order_e::mi_memory_order_relaxed,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe fn mi_atomic_cas_strong_acq_rel(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
) -> bool {
    unsafe {
        mi_atomic_cas_strong(
            p,
            expected,
            desired,
            mi_memory_order_e::mi_memory_order_acq_rel,
            mi_memory_order_e::mi_memory_order_acquire,
        )
    }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_add_relaxed(p: *mut usize, x: usize) -> usize {
    unsafe { mi_atomic_fetch_add_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_sub_relaxed(p: *mut usize, x: usize) -> usize {
    unsafe { mi_atomic_fetch_sub_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_and_relaxed(p: *mut usize, x: usize) -> usize {
    unsafe { mi_atomic_fetch_and_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_or_relaxed(p: *mut usize, x: usize) -> usize {
    unsafe { mi_atomic_fetch_or_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_increment_relaxed(p: *mut usize) -> usize {
    unsafe { mi_atomic_add_relaxed(p, 1) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe fn mi_atomic_loadi64_acquire(p: *const i64) -> i64 {
    unsafe { mi_atomic_loadi64_explicit(p, mi_memory_order_e::mi_memory_order_acquire) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe fn mi_atomic_storei64_release(p: *mut i64, x: i64) {
    unsafe { mi_atomic_storei64_explicit(p, x, mi_memory_order_e::mi_memory_order_release) };
}

/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe extern "C" fn mi_atomic_compare_exchange_strong_explicit(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mo1: mi_memory_order_e,
    mo2: mi_memory_order_e,
) -> bool {
    let current = unsafe { *expected };
    match unsafe { atomic_usize(p) }.compare_exchange(
        current,
        desired,
        rmw_order(mo1),
        cas_failure_order(mo2),
    ) {
        Ok(_) => true,
        Err(actual) => {
            unsafe { *expected = actual };
            false
        }
    }
}

/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe extern "C" fn mi_atomic_compare_exchange_weak_explicit(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mo1: mi_memory_order_e,
    mo2: mi_memory_order_e,
) -> bool {
    unsafe { mi_atomic_compare_exchange_strong_explicit(p, expected, desired, mo1, mo2) }
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_exchange_explicit(
    p: *mut usize,
    exchange: usize,
    mo: mi_memory_order_e,
) -> usize {
    unsafe { atomic_usize(p) }.swap(exchange, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize {
    unsafe { atomic_usize_const(p) }.load(load_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e) {
    unsafe { atomic_usize(p) }.store(x, store_order(mo));
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_loadi64_explicit(p: *const i64, mo: mi_memory_order_e) -> i64 {
    unsafe { atomic_i64_const(p) }.load(load_order(mo))
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_storei64_explicit(p: *mut i64, x: i64, mo: mi_memory_order_e) {
    unsafe { atomic_i64(p) }.store(x, store_order(mo));
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_addi64_relaxed(p: *mut i64, add: i64) -> i64 {
    unsafe { atomic_i64(p) }.fetch_add(add, Ordering::Relaxed)
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
/// `padd` must point to readable `i64` storage.
pub unsafe extern "C" fn mi_atomic_void_addi64_relaxed(p: *mut i64, padd: *const i64) {
    let add = unsafe { read_volatile(padd) };
    if add != 0 {
        unsafe { atomic_i64(p) }.fetch_add(add, Ordering::Relaxed);
    }
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_maxi64_relaxed(p: *mut i64, x: i64) {
    let atomic = unsafe { atomic_i64(p) };
    let mut current = atomic.load(Ordering::Relaxed);
    while current < x {
        match atomic.compare_exchange_weak(current, x, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_addi64_acq_rel(p: *mut i64, i: i64) {
    unsafe { atomic_i64(p) }.fetch_add(i, Ordering::AcqRel);
}

/// # Safety
///
/// `p` and `exp` must point to valid `i64` storage that is only accessed
/// atomically for this compare-exchange operation.
pub unsafe extern "C" fn mi_atomic_casi64_strong_acq_rel(
    p: *mut i64,
    exp: *mut i64,
    des: i64,
) -> bool {
    let current = unsafe { *exp };
    match unsafe { atomic_i64(p) }.compare_exchange(
        current,
        des,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => true,
        Err(actual) => {
            unsafe { *exp = actual };
            false
        }
    }
}

/// # Safety
///
/// `p` must point to valid `isize` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_addi(p: *mut isize, add: isize) -> isize {
    unsafe { atomic_isize(p) }.fetch_add(add, Ordering::AcqRel)
}

/// # Safety
///
/// `p` must point to valid `isize` storage that is only accessed atomically.
pub unsafe extern "C" fn mi_atomic_subi(p: *mut isize, sub: isize) -> isize {
    unsafe { atomic_isize(p) }.fetch_sub(sub, Ordering::AcqRel)
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
pub unsafe extern "C" fn mi_lock_try_acquire(lock: *mut mi_lock_t) -> bool {
    unsafe { pthread_mutex_trylock(ptr::addr_of_mut!((*lock).mutex)) == 0 }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
pub unsafe extern "C" fn mi_lock_try_acquire(lock: *mut mi_lock_t) -> bool {
    let expected = 0;
    unsafe {
        (*lock)
            .mutex
            .mutex
            .compare_exchange(expected, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
pub unsafe extern "C" fn mi_lock_acquire(lock: *mut mi_lock_t) {
    let err = unsafe { pthread_mutex_lock(ptr::addr_of_mut!((*lock).mutex)) };
    if err != 0 {
        unsafe {
            crate::options::_mi_error_message(
                err,
                c"internal error: lock cannot be acquired\n".as_ptr(),
            )
        };
    }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
pub unsafe extern "C" fn mi_lock_acquire(lock: *mut mi_lock_t) {
    while unsafe { !mi_lock_try_acquire(lock) } {
        spin_loop();
    }
}

/// # Safety
///
/// `lock` must point to a valid acquired allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
pub unsafe extern "C" fn mi_lock_release(lock: *mut mi_lock_t) {
    unsafe { pthread_mutex_unlock(ptr::addr_of_mut!((*lock).mutex)) };
}

/// # Safety
///
/// `lock` must point to a valid acquired allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
pub unsafe extern "C" fn mi_lock_release(lock: *mut mi_lock_t) {
    unsafe { (*lock).mutex.mutex.store(0, Ordering::Release) };
}

/// # Safety
///
/// `lock`, when non-null, must point to writable allocator lock storage.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
pub unsafe extern "C" fn mi_lock_init(lock: *mut mi_lock_t) {
    if !lock.is_null() {
        unsafe { (*lock).mutex = pthread_mutex_t::new() };
    }
}

/// # Safety
///
/// `lock` must point to writable allocator lock storage.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
pub unsafe extern "C" fn mi_lock_init(lock: *mut mi_lock_t) {
    unsafe { mi_lock_release(lock) };
}

/// # Safety
///
/// `lock` must point to an initialized allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
pub unsafe extern "C" fn mi_lock_done(lock: *mut mi_lock_t) {
    unsafe { pthread_mutex_destroy(ptr::addr_of_mut!((*lock).mutex)) };
}

/// # Safety
///
/// `lock` must point to an initialized allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
pub unsafe extern "C" fn mi_lock_done(lock: *mut mi_lock_t) {
    let _ = lock;
}

/// # Safety
///
/// `once` must point to a valid allocator once-state object.
#[inline(never)]
pub unsafe extern "C" fn _mi_atomic_once_enter(once: *mut mi_atomic_once_t) -> bool {
    let once_tid = unsafe { (*once).tid.load(Ordering::Acquire) };
    if once_tid == 1 {
        return false;
    }
    let current_tid = _mi_thread_id();
    if once_tid == current_tid {
        return false;
    }

    unsafe { mi_lock_acquire(&mut (*once).lock) };
    let expected = 0usize;
    if unsafe {
        (*once)
            .tid
            .compare_exchange(expected, current_tid, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    } {
        true
    } else {
        unsafe { mi_lock_release(&mut (*once).lock) };
        false
    }
}

/// # Safety
///
/// `once` must point to a once-state object previously entered by the current
/// thread.
#[inline(never)]
pub unsafe extern "C" fn _mi_atomic_once_release(once: *mut mi_atomic_once_t) {
    if unsafe { (*once).tid.load(Ordering::Acquire) } > 1 {
        unsafe { (*once).tid.store(1, Ordering::Release) };
        unsafe { mi_lock_release(&mut (*once).lock) };
    }
}
