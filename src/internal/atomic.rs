//! Atomic and bit primitives

// Retains the upstream C atomic facade and memory-order helpers; not every
// translated intrinsic is used on every Rust target or feature set.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

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
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: declares Linux pthread mutex imports
// used only by the release lock facade.
unsafe extern "C" {
    fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> c_int;
    fn pthread_mutex_lock(mutex: *mut pthread_mutex_t) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t) -> c_int;
    fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t) -> c_int;
}

pub struct mi_atomic_guard_scope_t {
    _guard: crate::core_types::AtomicGuard<'static>,
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
    _once: crate::core_types::OnceGuard<'static>,
}

/// # Safety
/// `guard` must point to a valid atomic guard value.
pub unsafe fn mi_atomic_guard_enter(
    guard: *mut mi_atomic_guard_t,
) -> Option<mi_atomic_guard_scope_t> {
    let field = crate::core_types::AtomicUsizeField(guard.cast::<usize>());
    // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: callers provide a live atomic guard
    // word and the typed guard wrapper owns release-on-drop.
    unsafe { crate::core_types::AtomicGuard::try_acquire(field) }
        .map(|_guard| mi_atomic_guard_scope_t { _guard })
}

/// # Safety
/// `once` must point to a valid once-state.
pub unsafe fn mi_atomic_do_once_enter(
    once: *mut mi_atomic_once_t,
) -> Option<mi_atomic_once_scope_t> {
    let state = crate::core_types::OnceStatePtr(once.cast::<usize>());
    // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: callers provide a live once-state
    // word and the typed once wrapper owns release-on-drop.
    unsafe { crate::core_types::OnceGuard::try_enter(state) }
        .map(|_once| mi_atomic_once_scope_t { _once })
}

macro_rules! mi_atomic_guard {
    ($guard:expr, $body:block) => {{
        let _mi_guard_ptr: *mut $crate::atomic::mi_atomic_guard_t = $guard;
        if let Some(_mi_guard_scope) =
            // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: macro callers pass the same
            // raw guard pointer required by the guard-enter helper.
            unsafe { $crate::atomic::mi_atomic_guard_enter(_mi_guard_ptr) }
        {
            let _mi_guard_scope = _mi_guard_scope;
            $body
        }
    }};
}
pub(crate) use mi_atomic_guard;

#[inline]
// ORDERING[AUDIT:atomic-memory-order-map]: preserve the C mi_memory_order_e
// mapping in one place so callers cannot invent local ordering translations.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: callers prove `p` is atomic `usize`
    // storage for the duration of the returned shared view.
    unsafe { &*(p as *mut AtomicUsize) }
}

#[inline]
unsafe fn atomic_usize_const<'a>(p: *const usize) -> &'a AtomicUsize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: callers prove `p` is atomic `usize`
    // storage for the duration of the returned shared view.
    unsafe { &*(p as *const AtomicUsize) }
}

#[inline]
unsafe fn atomic_isize<'a>(p: *mut isize) -> &'a AtomicIsize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: callers prove `p` is atomic `isize`
    // storage for the duration of the returned shared view.
    unsafe { &*(p as *mut AtomicIsize) }
}

#[inline]
unsafe fn atomic_i64<'a>(p: *mut i64) -> &'a AtomicI64 {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: callers prove `p` is atomic `i64`
    // storage for the duration of the returned shared view.
    unsafe { &*(p as *mut AtomicI64) }
}

#[inline]
unsafe fn atomic_i64_const<'a>(p: *const i64) -> &'a AtomicI64 {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: callers prove `p` is atomic `i64`
    // storage for the duration of the returned shared view.
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
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI fetch-add preserves the raw
// pointer atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_fetch_add_explicit(
    p: *mut usize,
    add: usize,
    mo: mi_memory_order_e,
) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one fetch-add operation.
    unsafe { atomic_usize(p) }.fetch_add(add, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI fetch-sub preserves the raw
// pointer atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_fetch_sub_explicit(
    p: *mut usize,
    sub: usize,
    mo: mi_memory_order_e,
) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one fetch-sub operation.
    unsafe { atomic_usize(p) }.fetch_sub(sub, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI fetch-and preserves the raw
// pointer atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_fetch_and_explicit(
    p: *mut usize,
    x: usize,
    mo: mi_memory_order_e,
) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one fetch-and operation.
    unsafe { atomic_usize(p) }.fetch_and(x, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically for
/// the duration of this operation.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI fetch-or preserves the raw pointer
// atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_fetch_or_explicit(
    p: *mut usize,
    x: usize,
    mo: mi_memory_order_e,
) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one fetch-or operation.
    unsafe { atomic_usize(p) }.fetch_or(x, rmw_order(mo))
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_increment_acq_rel(p: *mut usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: forwards the caller-proven atomic
    // storage to the C ABI fetch-add helper with acq_rel ordering.
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
    // SAFETY[AUDIT:internal-atomic-pointer-boundary]: pointer CAS is represented as a
    // pointer-sized `usize` CAS while preserving the caller's expected slot.
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
    // SAFETY[AUDIT:internal-atomic-pointer-boundary]: pointer loads use the pointer-sized
    // atomic load helper and restore the raw pointer value unchanged.
    unsafe { mi_atomic_load_acquire(p.cast::<usize>()) as *mut T }
}

#[inline]
/// # Safety
///
/// `p` must point to valid pointer-sized storage that is only accessed
/// atomically.
pub unsafe fn mi_atomic_store_ptr_release<T>(p: *mut *mut T, x: *mut T) {
    // SAFETY[AUDIT:internal-atomic-pointer-boundary]: pointer stores use the pointer-sized
    // atomic store helper and preserve the raw pointer bits unchanged.
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
    // SAFETY[AUDIT:internal-atomic-pointer-boundary]: pointer CAS is represented as a
    // pointer-sized `usize` CAS while preserving the caller's expected slot.
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
    // SAFETY[AUDIT:internal-atomic-pointer-boundary]: pointer exchange uses the
    // pointer-sized atomic exchange helper and restores the old pointer value.
    unsafe { mi_atomic_exchange_relaxed(p.cast::<usize>(), x as usize) as *mut T }
}

#[inline]
/// # Safety
///
/// `p` must point to valid pointer-sized storage that is only accessed
/// atomically.
pub unsafe fn mi_atomic_exchange_ptr_acq_rel<T>(p: *mut *mut T, x: *mut T) -> *mut T {
    // SAFETY[AUDIT:internal-atomic-pointer-boundary]: pointer exchange uses the
    // pointer-sized atomic exchange helper and restores the old pointer value.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: weak CAS forwards caller-proven raw
    // atomic storage and expected slot to the C ABI CAS helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: strong CAS forwards caller-proven raw
    // atomic storage and expected slot to the C ABI CAS helper.
    unsafe {
        mi_atomic_compare_exchange_strong_explicit(p, expected, desired, mem_success, mem_fail)
    }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_load_acquire(p: *const usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: acquire load forwards caller-proven
    // atomic storage to the explicit load helper.
    unsafe { mi_atomic_load_explicit(p, mi_memory_order_e::mi_memory_order_acquire) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_store_release(p: *mut usize, x: usize) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: release store forwards caller-proven
    // atomic storage to the explicit store helper.
    unsafe { mi_atomic_store_explicit(p, x, mi_memory_order_e::mi_memory_order_release) };
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_exchange_relaxed(p: *mut usize, x: usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed exchange forwards
    // caller-proven atomic storage to the explicit exchange helper.
    unsafe { mi_atomic_exchange_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_exchange_acq_rel(p: *mut usize, x: usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: acq_rel exchange forwards
    // caller-proven atomic storage to the explicit exchange helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed weak CAS forwards
    // caller-proven raw storage and expected slot to the CAS helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: release weak CAS forwards
    // caller-proven raw storage and expected slot to the CAS helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: acq_rel weak CAS forwards
    // caller-proven raw storage and expected slot to the CAS helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: release strong CAS forwards
    // caller-proven raw storage and expected slot to the CAS helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: acq_rel strong CAS forwards
    // caller-proven raw storage and expected slot to the CAS helper.
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
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed add forwards caller-proven
    // atomic storage to the explicit fetch-add helper.
    unsafe { mi_atomic_fetch_add_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_sub_relaxed(p: *mut usize, x: usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed sub forwards caller-proven
    // atomic storage to the explicit fetch-sub helper.
    unsafe { mi_atomic_fetch_sub_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_and_relaxed(p: *mut usize, x: usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed and forwards caller-proven
    // atomic storage to the explicit fetch-and helper.
    unsafe { mi_atomic_fetch_and_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_or_relaxed(p: *mut usize, x: usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed or forwards caller-proven
    // atomic storage to the explicit fetch-or helper.
    unsafe { mi_atomic_fetch_or_explicit(p, x, mi_memory_order_e::mi_memory_order_relaxed) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
pub unsafe fn mi_atomic_increment_relaxed(p: *mut usize) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: relaxed increment forwards
    // caller-proven atomic storage to the relaxed add helper.
    unsafe { mi_atomic_add_relaxed(p, 1) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe fn mi_atomic_loadi64_acquire(p: *const i64) -> i64 {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: acquire i64 load forwards
    // caller-proven atomic storage to the explicit load helper.
    unsafe { mi_atomic_loadi64_explicit(p, mi_memory_order_e::mi_memory_order_acquire) }
}

#[inline]
/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
pub unsafe fn mi_atomic_storei64_release(p: *mut i64, x: i64) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: release i64 store forwards
    // caller-proven atomic storage to the explicit store helper.
    unsafe { mi_atomic_storei64_explicit(p, x, mi_memory_order_e::mi_memory_order_release) };
}

/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI strong CAS reads and updates the
// caller-provided expected slot according to the C atomic contract.
pub unsafe extern "C" fn mi_atomic_compare_exchange_strong_explicit(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mo1: mi_memory_order_e,
    mo2: mi_memory_order_e,
) -> bool {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-provided expected storage is
    // readable for the current comparison value.
    let current = unsafe { *expected };
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one compare-exchange operation.
    match unsafe { atomic_usize(p) }.compare_exchange(
        current,
        desired,
        rmw_order(mo1),
        cas_failure_order(mo2),
    ) {
        Ok(_) => true,
        Err(actual) => {
            // SAFETY[AUDIT:internal-atomic-field-boundary]: failed CAS writes the actual
            // value back to the caller-provided expected slot.
            unsafe { *expected = actual };
            false
        }
    }
}

/// # Safety
///
/// `p` and `expected` must point to valid `usize` storage that is only accessed
/// atomically for this compare-exchange operation.
// SAFETY[AUDIT:internal-atomic-field-boundary]: weak CAS currently delegates to the
// strong helper while preserving expected-slot update semantics.
pub unsafe extern "C" fn mi_atomic_compare_exchange_weak_explicit(
    p: *mut usize,
    expected: *mut usize,
    desired: usize,
    mo1: mi_memory_order_e,
    mo2: mi_memory_order_e,
) -> bool {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage and
    // expected slot are forwarded unchanged to the strong CAS helper.
    unsafe { mi_atomic_compare_exchange_strong_explicit(p, expected, desired, mo1, mo2) }
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI exchange preserves the raw pointer
// atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_exchange_explicit(
    p: *mut usize,
    exchange: usize,
    mo: mi_memory_order_e,
) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one swap operation.
    unsafe { atomic_usize(p) }.swap(exchange, rmw_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI load preserves the raw pointer
// atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one load operation.
    unsafe { atomic_usize_const(p) }.load(load_order(mo))
}

/// # Safety
///
/// `p` must point to valid `usize` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI store preserves the raw pointer
// atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicUsize` for one store operation.
    unsafe { atomic_usize(p) }.store(x, store_order(mo));
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI i64 load preserves the raw pointer
// atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_loadi64_explicit(p: *const i64, mo: mi_memory_order_e) -> i64 {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicI64` for one load operation.
    unsafe { atomic_i64_const(p) }.load(load_order(mo))
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI i64 store preserves the raw pointer
// atomic contract and central memory-order mapping.
pub unsafe extern "C" fn mi_atomic_storei64_explicit(p: *mut i64, x: i64, mo: mi_memory_order_e) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicI64` for one store operation.
    unsafe { atomic_i64(p) }.store(x, store_order(mo));
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI relaxed i64 add preserves the raw
// pointer atomic contract.
pub unsafe extern "C" fn mi_atomic_addi64_relaxed(p: *mut i64, add: i64) -> i64 {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicI64` for one fetch-add operation.
    unsafe { atomic_i64(p) }.fetch_add(add, Ordering::Relaxed)
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
/// `padd` must point to readable `i64` storage.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI void add reads the volatile delta
// and applies it to caller-proven i64 atomic storage when nonzero.
pub unsafe extern "C" fn mi_atomic_void_addi64_relaxed(p: *mut i64, padd: *const i64) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: `padd` is caller-provided readable
    // storage for the volatile addend.
    let add = unsafe { read_volatile(padd) };
    if add != 0 {
        // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is
        // viewed as `AtomicI64` for one relaxed fetch-add operation.
        unsafe { atomic_i64(p) }.fetch_add(add, Ordering::Relaxed);
    }
}

/// # Safety
///
/// `p` must point to valid `i64` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI max helper loops over
// caller-proven i64 atomic storage until the relaxed maximum is published.
pub unsafe extern "C" fn mi_atomic_maxi64_relaxed(p: *mut i64, x: i64) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicI64` for relaxed load and weak CAS operations.
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
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI acq_rel i64 add preserves the raw
// pointer atomic contract.
pub unsafe extern "C" fn mi_atomic_addi64_acq_rel(p: *mut i64, i: i64) {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicI64` for one acq_rel fetch-add operation.
    unsafe { atomic_i64(p) }.fetch_add(i, Ordering::AcqRel);
}

/// # Safety
///
/// `p` and `exp` must point to valid `i64` storage that is only accessed
/// atomically for this compare-exchange operation.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI i64 strong CAS reads and updates
// the caller-provided expected slot according to the C atomic contract.
pub unsafe extern "C" fn mi_atomic_casi64_strong_acq_rel(
    p: *mut i64,
    exp: *mut i64,
    des: i64,
) -> bool {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-provided expected storage is
    // readable for the current comparison value.
    let current = unsafe { *exp };
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicI64` for one compare-exchange operation.
    match unsafe { atomic_i64(p) }.compare_exchange(
        current,
        des,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => true,
        Err(actual) => {
            // SAFETY[AUDIT:internal-atomic-field-boundary]: failed CAS writes the actual
            // value back to the caller-provided expected slot.
            unsafe { *exp = actual };
            false
        }
    }
}

/// # Safety
///
/// `p` must point to valid `isize` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI isize add preserves the raw
// pointer atomic contract.
pub unsafe extern "C" fn mi_atomic_addi(p: *mut isize, add: isize) -> isize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicIsize` for one acq_rel fetch-add operation.
    unsafe { atomic_isize(p) }.fetch_add(add, Ordering::AcqRel)
}

/// # Safety
///
/// `p` must point to valid `isize` storage that is only accessed atomically.
// SAFETY[AUDIT:internal-atomic-field-boundary]: C ABI isize sub preserves the raw
// pointer atomic contract.
pub unsafe extern "C" fn mi_atomic_subi(p: *mut isize, sub: isize) -> isize {
    // SAFETY[AUDIT:internal-atomic-field-boundary]: caller-proven raw storage is viewed
    // as `AtomicIsize` for one acq_rel fetch-sub operation.
    unsafe { atomic_isize(p) }.fetch_sub(sub, Ordering::AcqRel)
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: Linux release lock acquisition delegates
// to pthread over the caller-provided lock object's mutex field.
pub unsafe extern "C" fn mi_lock_try_acquire(lock: *mut mi_lock_t) -> bool {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage contains
    // an initialized pthread mutex field.
    unsafe { pthread_mutex_trylock(ptr::addr_of_mut!((*lock).mutex)) == 0 }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: fallback lock acquisition uses the
// in-struct atomic mutex word with acquire/release semantics.
pub unsafe extern "C" fn mi_lock_try_acquire(lock: *mut mi_lock_t) -> bool {
    let expected = 0;
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage contains
    // the fallback atomic mutex word.
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
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: Linux release lock acquisition delegates
// to pthread and reports nonzero errors through the allocator error path.
pub unsafe extern "C" fn mi_lock_acquire(lock: *mut mi_lock_t) {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage contains
    // an initialized pthread mutex field.
    let err = unsafe { pthread_mutex_lock(ptr::addr_of_mut!((*lock).mutex)) };
    if err != 0 {
        crate::options::_mi_error_message_fmt(
            crate::core_types::Errno::new(err),
            format_args!("internal error: lock cannot be acquired\n"),
        );
    }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: fallback lock acquisition spins over the
// same in-struct atomic mutex word used by try-acquire.
pub unsafe extern "C" fn mi_lock_acquire(lock: *mut mi_lock_t) {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage is
    // repeatedly passed to the fallback try-acquire helper.
    while unsafe { !mi_lock_try_acquire(lock) } {
        spin_loop();
    }
}

/// # Safety
///
/// `lock` must point to a valid acquired allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: Linux release lock release delegates to
// pthread over the caller-provided lock object's mutex field.
pub unsafe extern "C" fn mi_lock_release(lock: *mut mi_lock_t) {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage contains
    // an initialized acquired pthread mutex field.
    unsafe { pthread_mutex_unlock(ptr::addr_of_mut!((*lock).mutex)) };
}

/// # Safety
///
/// `lock` must point to a valid acquired allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: fallback lock release publishes zero to
// the in-struct atomic mutex word with release ordering.
pub unsafe extern "C" fn mi_lock_release(lock: *mut mi_lock_t) {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage contains
    // the fallback atomic mutex word.
    unsafe { (*lock).mutex.mutex.store(0, Ordering::Release) };
}

/// # Safety
///
/// `lock`, when non-null, must point to writable allocator lock storage.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: Linux lock init overwrites only non-null
// caller-provided lock storage with the pthread initializer layout.
pub unsafe extern "C" fn mi_lock_init(lock: *mut mi_lock_t) {
    if !lock.is_null() {
        // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: non-null lock storage is writable
        // for one pthread mutex initializer value.
        unsafe { (*lock).mutex = pthread_mutex_t::new() };
    }
}

/// # Safety
///
/// `lock` must point to writable allocator lock storage.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: fallback lock init resets the in-struct
// atomic mutex word through the release helper.
pub unsafe extern "C" fn mi_lock_init(lock: *mut mi_lock_t) {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage is passed
    // to the fallback release helper for zero initialization.
    unsafe { mi_lock_release(lock) };
}

/// # Safety
///
/// `lock` must point to an initialized allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: Linux lock teardown delegates to pthread
// over the caller-provided initialized mutex field.
pub unsafe extern "C" fn mi_lock_done(lock: *mut mi_lock_t) {
    // SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: caller-proven lock storage contains
    // an initialized pthread mutex field.
    unsafe { pthread_mutex_destroy(ptr::addr_of_mut!((*lock).mutex)) };
}

/// # Safety
///
/// `lock` must point to an initialized allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
// SAFETY[AUDIT:internal-atomic-platform-lock-boundary]: fallback lock teardown is a compatibility
// no-op because the atomic mutex word owns no external resources.
pub unsafe extern "C" fn mi_lock_done(lock: *mut mi_lock_t) {
    let _ = lock;
}

/// # Safety
///
/// `once` must point to a valid allocator once-state object.
#[inline(never)]
// SAFETY[AUDIT:internal-atomic-guard-once-boundary]: once enter coordinates a caller-provided
// once state through acquire/release tid publication and the local lock.
pub unsafe extern "C" fn _mi_atomic_once_enter(once: *mut mi_atomic_once_t) -> bool {
    // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: caller-proven once storage contains
    // an initialized atomic tid field.
    let once_tid = unsafe { (*once).tid.load(Ordering::Acquire) };
    if once_tid == 1 {
        return false;
    }
    let current_tid = _mi_thread_id();
    if once_tid == current_tid {
        return false;
    }

    // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: caller-proven once storage contains
    // the lock used to serialize first entry.
    unsafe { mi_lock_acquire(&mut (*once).lock) };
    let expected = 0usize;
    // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: caller-proven once storage contains
    // an initialized atomic tid field for entry publication.
    if unsafe {
        (*once)
            .tid
            .compare_exchange(expected, current_tid, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    } {
        true
    } else {
        // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: failed first-entry publication
        // releases the lock acquired for this once state.
        unsafe { mi_lock_release(&mut (*once).lock) };
        false
    }
}

/// # Safety
///
/// `once` must point to a once-state object previously entered by the current
/// thread.
#[inline(never)]
// SAFETY[AUDIT:internal-atomic-guard-once-boundary]: once release publishes completion and
// releases the lock only for an entered caller-provided once state.
pub unsafe extern "C" fn _mi_atomic_once_release(once: *mut mi_atomic_once_t) {
    // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: caller-proven once storage contains
    // an initialized atomic tid field.
    if unsafe { (*once).tid.load(Ordering::Acquire) } > 1 {
        // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: completion is published with
        // release ordering to the caller-provided once tid field.
        unsafe { (*once).tid.store(1, Ordering::Release) };
        // SAFETY[AUDIT:internal-atomic-guard-once-boundary]: the once lock was acquired by
        // the matching enter path for this caller-provided once state.
        unsafe { mi_lock_release(&mut (*once).lock) };
    }
}
