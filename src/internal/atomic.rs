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
use core::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use crate::allocator::init::_mi_thread_id;

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
    let field = crate::core_types::AtomicUsizeField::from_raw(guard.cast::<usize>());
    unsafe { crate::core_types::AtomicGuard::try_acquire(field) }
        .map(|_guard| mi_atomic_guard_scope_t { _guard })
}

/// # Safety
/// `once` must point to a valid once-state.
pub unsafe fn mi_atomic_do_once_enter(
    once: *mut mi_atomic_once_t,
) -> Option<mi_atomic_once_scope_t> {
    let state = crate::core_types::OnceStatePtr::from_raw(once.cast::<usize>());
    unsafe { crate::core_types::OnceGuard::try_enter(state) }
        .map(|_once| mi_atomic_once_scope_t { _once })
}

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
pub(crate) use mi_atomic_guard;

#[inline]
fn load_order(order: mi_memory_order_e) -> Ordering {
    match order {
        mi_memory_order_e::mi_memory_order_relaxed => Ordering::Relaxed,
        mi_memory_order_e::mi_memory_order_seq_cst => Ordering::SeqCst,
        _ => Ordering::Acquire,
    }
}

#[inline]
unsafe fn atomic_i64<'a>(p: *mut i64) -> &'a AtomicI64 {
    unsafe { &*(p as *mut AtomicI64) }
}

#[inline]
unsafe fn atomic_i64_const<'a>(p: *const i64) -> &'a AtomicI64 {
    unsafe { &*(p as *const AtomicI64) }
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
pub unsafe extern "C" fn mi_atomic_addi64_relaxed(p: *mut i64, add: i64) -> i64 {
    unsafe { atomic_i64(p) }.fetch_add(add, Ordering::Relaxed)
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
        crate::allocator::options::_mi_error_message_fmt(
            crate::core_types::Errno::new(err),
            format_args!("internal error: lock cannot be acquired\n"),
        );
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
