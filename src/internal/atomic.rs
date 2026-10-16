//! Atomic and bit primitives

// Retains the upstream C atomic facade and memory-order helpers; not every
// translated intrinsic is used on every Rust target or feature set.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

#[cfg(all(target_os = "linux", not(debug_assertions)))]
use core::ffi::c_int;
#[cfg(not(any(
    all(target_os = "linux", not(debug_assertions)),
    all(target_vendor = "apple", not(miri))
)))]
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

/// Lock word of the portable lock: an `os_unfair_lock` on Apple targets, the
/// lock libmalloc uses, whose waiters lend their priority to the owner instead
/// of spinning under Darwin's QoS scheduling; a spinlock flag elsewhere.
#[cfg(all(target_vendor = "apple", not(miri)))]
type LockWord = core::sync::atomic::AtomicU32;
#[cfg(not(all(target_vendor = "apple", not(miri))))]
type LockWord = AtomicUsize;

#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
#[repr(C)]
#[derive(Debug)]
pub struct pthread_mutex_t {
    mutex: LockWord,
}

#[cfg(all(target_vendor = "apple", not(miri)))]
unsafe extern "C" {
    fn os_unfair_lock_lock(lock: *mut u32);
    fn os_unfair_lock_trylock(lock: *mut u32) -> bool;
    fn os_unfair_lock_unlock(lock: *mut u32);
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
            mutex: LockWord::new(0),
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
unsafe fn mi_lock_try_acquire_raw(lock: *mut mi_lock_t) -> bool {
    unsafe { pthread_mutex_trylock(ptr::addr_of_mut!((*lock).mutex)) == 0 }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
unsafe fn mi_lock_try_acquire_raw(lock: *mut mi_lock_t) -> bool {
    #[cfg(all(target_vendor = "apple", not(miri)))]
    // SAFETY: the caller supplies a valid lock, whose word is an `os_unfair_lock`.
    unsafe {
        os_unfair_lock_trylock((*lock).mutex.mutex.as_ptr())
    }
    #[cfg(not(all(target_vendor = "apple", not(miri))))]
    unsafe {
        (*lock)
            .mutex
            .mutex
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
unsafe fn mi_lock_acquire_raw(lock: *mut mi_lock_t) {
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
unsafe fn mi_lock_acquire_raw(lock: *mut mi_lock_t) {
    #[cfg(all(target_vendor = "apple", not(miri)))]
    // SAFETY: the caller supplies a valid lock, whose word is an `os_unfair_lock`.
    unsafe {
        os_unfair_lock_lock((*lock).mutex.mutex.as_ptr());
    }
    #[cfg(not(all(target_vendor = "apple", not(miri))))]
    while unsafe { !mi_lock_try_acquire_raw(lock) } {
        spin_loop();
    }
}

/// # Safety
///
/// `lock` must point to a valid acquired allocator lock object.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
unsafe fn mi_lock_release_raw(lock: *mut mi_lock_t) {
    unsafe { pthread_mutex_unlock(ptr::addr_of_mut!((*lock).mutex)) };
}

/// # Safety
///
/// `lock` must point to a valid acquired allocator lock object.
#[cfg(not(all(target_os = "linux", not(debug_assertions))))]
unsafe fn mi_lock_release_raw(lock: *mut mi_lock_t) {
    #[cfg(all(target_vendor = "apple", not(miri)))]
    // SAFETY: the caller acquired this `os_unfair_lock` on the current thread.
    unsafe {
        os_unfair_lock_unlock((*lock).mutex.mutex.as_ptr());
    }
    #[cfg(not(all(target_vendor = "apple", not(miri))))]
    unsafe {
        (*lock).mutex.mutex.store(0, Ordering::Release);
    }
}

// Fork gate. Every held allocator lock holds an entry in `FORK_GATE`. Before
// `fork`, `_mi_fork_prepare` waits until no thread holds an allocator lock and
// closes the gate, so the child never inherits a lock owned by a thread that
// does not exist there. The gate closes only at zero, so a thread already
// holding a lock can always take nested ones. The forking thread may still
// take locks while the gate is closed: fork handlers run on it and allocate.
static FORK_GATE: AtomicUsize = AtomicUsize::new(0);
static FORK_OWNER: AtomicUsize = AtomicUsize::new(0);
const FORK_GATE_CLOSED: usize = 1 << (usize::BITS - 1);

#[inline]
fn fork_gate_try_enter() -> bool {
    let mut state = FORK_GATE.load(Ordering::Relaxed);
    loop {
        if state & FORK_GATE_CLOSED != 0 && FORK_OWNER.load(Ordering::Relaxed) != _mi_thread_id() {
            return false;
        }
        match FORK_GATE.compare_exchange_weak(
            state,
            state + 1,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => return true,
            Err(current) => state = current,
        }
    }
}

#[inline]
fn fork_gate_exit() {
    FORK_GATE.fetch_sub(1, Ordering::Release);
}

/// `pthread_atfork` prepare handler: waits until no thread holds an allocator
/// lock, then keeps other threads from taking one until the fork completes.
pub(crate) extern "C" fn _mi_fork_prepare() {
    FORK_OWNER.store(_mi_thread_id(), Ordering::Relaxed);
    while FORK_GATE
        .compare_exchange_weak(0, FORK_GATE_CLOSED, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        // SAFETY: yielding has no preconditions.
        unsafe { crate::prim::_mi_prim_thread_yield() };
    }
}

/// `pthread_atfork` parent and child handler: reopens the gate.
pub(crate) extern "C" fn _mi_fork_reopen() {
    FORK_OWNER.store(0, Ordering::Relaxed);
    FORK_GATE.store(0, Ordering::Release);
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
pub unsafe extern "C" fn mi_lock_try_acquire(lock: *mut mi_lock_t) -> bool {
    if !fork_gate_try_enter() {
        return false;
    }
    let acquired = unsafe { mi_lock_try_acquire_raw(lock) };
    if !acquired {
        fork_gate_exit();
    }
    acquired
}

/// # Safety
///
/// `lock` must point to a valid allocator lock object.
pub unsafe extern "C" fn mi_lock_acquire(lock: *mut mi_lock_t) {
    while !fork_gate_try_enter() {
        // SAFETY: yielding has no preconditions.
        unsafe { crate::prim::_mi_prim_thread_yield() };
    }
    unsafe { mi_lock_acquire_raw(lock) };
}

/// # Safety
///
/// `lock` must point to a valid lock acquired by the current thread.
pub unsafe extern "C" fn mi_lock_release(lock: *mut mi_lock_t) {
    unsafe { mi_lock_release_raw(lock) };
    fork_gate_exit();
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
    // Store the unlocked state directly: unlocking an `os_unfair_lock` that is
    // not held aborts.
    unsafe { (*lock).mutex = pthread_mutex_t::new() };
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
