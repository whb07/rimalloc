//! Deferred-free callback registration and dispatch.
//!
//! This module owns the process-global deferred-free callback slots and the
//! local thread-heap reentry guard used when invoking the callback.

use core::ffi::c_void;
use core::ptr;

use crate::types::{mi_deferred_free_fun, mi_theap_t};

static mut deferred_free: *mut c_void = ptr::null_mut();
static mut deferred_arg: *mut c_void = ptr::null_mut();

/// # Safety
/// `theap` must point to writable thread-heap metadata.
#[inline(always)]
unsafe fn mi_theap_increment_heartbeat(theap: *mut mi_theap_t) -> crate::core_types::Heartbeat {
    // SAFETY: callers pass the live local
    // thread heap used by the page allocator before dispatching deferred-free callbacks.
    unsafe {
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_theap_increment_heartbeat_typed(theap)
    }
}

/// # Safety
/// `theap` must identify writable local thread-heap metadata.
#[inline(always)]
unsafe fn mi_theap_increment_heartbeat_typed(
    theap: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::Heartbeat {
    // SAFETY: the typed heap witness points
    // at writable thread-local heap metadata owned by the caller's allocation path.
    unsafe {
        let heartbeat = theap.ptr().heartbeat().increment_wrapping();
        theap.ptr().set_heartbeat(heartbeat);
        heartbeat
    }
}

// SAFETY: this C-shaped internal helper is
// called with the live local thread heap and preserves C mimalloc's callback
// dispatch and reentry contract.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_deferred_free(theap: *mut mi_theap_t, force: bool) {
    // SAFETY: the callback slots are
    // process-global atomics; `theap` is converted once into a local heap witness.
    unsafe {
        let heartbeat = mi_theap_increment_heartbeat(theap);
        let fun_ptr = crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(deferred_free))
            .load(crate::core_types::MemoryOrder::Acquire);
        let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);
        let theap = crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(theap),
        );
        mi_deferred_free_typed(theap, force, heartbeat, callback);
    }
}

/// # Safety
/// `theap` must identify the live local thread heap whose deferred-free callback
/// is being dispatched.
#[inline(always)]
unsafe fn mi_deferred_free_typed(
    theap: crate::core_types::ThreadHeapLocal,
    force: bool,
    heartbeat: crate::core_types::Heartbeat,
    callback: crate::core_types::DeferredFreeCallback,
) {
    // SAFETY: callback invocation is gated
    // by the C-compatible per-thread-heap recurse flag and uses the atomically
    // published process-global argument.
    unsafe {
        let theap_raw = theap.ptr().as_ptr();
        if callback.is_some() && !(*(*theap_raw).tld).recurse {
            (*(*theap_raw).tld).recurse = true;
            let arg = crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(deferred_arg))
                .load(crate::core_types::MemoryOrder::Acquire);
            let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);
            callback.invoke(force, heartbeat, arg);
            (*(*theap_raw).tld).recurse = false;
        }
    }
}

/// # Safety
/// `fn_`, when present, must remain callable for the lifetime of its registration.
/// `arg` is stored opaquely and passed back to that callback.
// SAFETY: this exported C ABI function
// stores the nullable callback and opaque argument using release stores, matching
// C mimalloc's process-global registration contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_register_deferred_free(
    fn_: Option<mi_deferred_free_fun>,
    arg: *mut c_void,
) {
    // SAFETY: nullable C callback and opaque
    // argument are converted into storage wrappers without taking ownership.
    unsafe {
        let callback = crate::core_types::DeferredFreeCallback::new(fn_);
        let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);
        crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(deferred_arg))
            .store(arg.as_ptr(), crate::core_types::MemoryOrder::Release);
        let f = callback.storage_ptr();
        crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(deferred_free))
            .store(f, crate::core_types::MemoryOrder::Release);
    }
}

/// # Safety
/// The returned callback snapshot is process-global state and must only be used
/// by tests or guards that restore it with compatible serialization.
pub(crate) unsafe fn registered_deferred_free_callback()
-> (Option<mi_deferred_free_fun>, *mut c_void) {
    // SAFETY: the snapshot observes the same
    // atomic callback slots used by registration and dispatch.
    unsafe {
        let fun_ptr = crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(deferred_free))
            .load(crate::core_types::MemoryOrder::Acquire);
        let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);
        let arg = crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!(deferred_arg))
            .load(crate::core_types::MemoryOrder::Acquire);
        (callback.raw(), arg)
    }
}

/// # Safety
/// `snapshot` must have come from `registered_deferred_free_callback` or an
/// equivalent process-global callback registration snapshot.
pub(crate) unsafe fn restore_deferred_free_callback(
    snapshot: (Option<mi_deferred_free_fun>, *mut c_void),
) {
    let (fun, arg) = snapshot;
    // SAFETY: restoration republishes a
    // previously captured process-global callback snapshot through the same C ABI.
    unsafe { mi_register_deferred_free(fun, arg) };
}
