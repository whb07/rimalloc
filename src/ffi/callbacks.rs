//! Unsafe-adjacent Rust callback guards.
//!
//! These guards provide scoped registration for mimalloc callback slots while
//! borrowing Rust `FnMut` closures. They are not part of the Tier S safe facade:
//! constructors are unsafe because callers must uphold process-global callback
//! synchronization, lifetime, reentrancy, and allocator-entry obligations.

#![deny(clippy::must_use_candidate)]

use core::ffi::c_void;
use core::ptr;

/// RAII guard for a temporary error callback registration.
///
/// The guard borrows the callback closure, cannot outlive captured state, and
/// restores the previous global registration on drop. The trampoline catches
/// Rust panics before returning through the C callback ABI in `std` and test
/// builds; `no_std` builds must not panic from the callback.
///
/// ```rust,compile_fail
/// use mechanical_mimalloc::unsafe_callbacks::ErrorCallbackGuard;
///
/// let guard;
/// {
///     let mut hits = 0usize;
///     let mut callback = |_err| hits += 1;
///     guard = unsafe { ErrorCallbackGuard::new(&mut callback) };
/// }
/// drop(guard);
/// ```
#[must_use]
pub struct ErrorCallbackGuard<'a, F: FnMut(i32)> {
    previous: (Option<crate::types::mi_error_fun>, *mut c_void),
    _callback: &'a mut F,
}

impl<'a, F: FnMut(i32)> ErrorCallbackGuard<'a, F> {
    /// Registers `callback` as the process-global mimalloc error callback.
    ///
    /// # Safety
    ///
    /// The error callback slot is process-global. The caller must synchronize
    /// registration, restoration, and callback entry with every thread and all
    /// allocator activity that could invoke or replace the callback.
    ///
    /// Mimalloc may invoke the callback from another thread unless upstream
    /// evidence proves otherwise, and callback invocation may be reentrant. The
    /// callback must not reenter allocator paths that can invoke the same error
    /// callback before the current invocation returns.
    ///
    /// The borrowed closure must outlive every possible callback invocation.
    /// Callers must not leak the guard, call [`core::mem::forget`] on it, or
    /// wrap it in [`core::mem::ManuallyDrop`] unless they manually restore the
    /// previous callback state before borrowed state can be invalidated.
    /// Preventing [`Drop`] can leave a process-global C callback pointing at
    /// borrowed Rust state.
    ///
    /// In `std` and test builds, the trampoline catches Rust panics before C ABI
    /// return. In `no_std` builds, callbacks must not panic. Callback bodies must
    /// also avoid allocator reentry unless the caller has proven the reentry path
    /// cannot trigger the same callback or violate the required synchronization.
    #[inline]
    pub unsafe fn new(callback: &'a mut F) -> Self {
        let previous = crate::allocator::options::registered_error_callback();
        crate::allocator::options::mi_register_error(
            Some(error_callback_trampoline::<F>),
            ptr::from_mut(callback).cast::<c_void>(),
        );
        Self {
            previous,
            _callback: callback,
        }
    }
}

impl<F: FnMut(i32)> Drop for ErrorCallbackGuard<'_, F> {
    fn drop(&mut self) {
        crate::allocator::options::restore_error_callback(self.previous);
    }
}

/// RAII guard for a temporary deferred-free callback registration.
///
/// The guard borrows the callback closure, cannot outlive captured state, and
/// restores the previous global registration on drop. The trampoline catches
/// Rust panics before returning through the C callback ABI in `std` and test
/// builds; `no_std` builds must not panic from the callback.
///
/// ```rust,compile_fail
/// use mechanical_mimalloc::unsafe_callbacks::DeferredFreeCallbackGuard;
///
/// let guard;
/// {
///     let mut heartbeats = Vec::new();
///     let mut callback = |_force, heartbeat| heartbeats.push(heartbeat);
///     guard = unsafe { DeferredFreeCallbackGuard::new(&mut callback) };
/// }
/// drop(guard);
/// ```
#[must_use]
pub struct DeferredFreeCallbackGuard<'a, F: FnMut(bool, u64)> {
    previous: (Option<crate::types::mi_deferred_free_fun>, *mut c_void),
    _callback: &'a mut F,
}

impl<'a, F: FnMut(bool, u64)> DeferredFreeCallbackGuard<'a, F> {
    /// Registers `callback` as the process-global mimalloc deferred-free
    /// callback.
    ///
    /// # Safety
    ///
    /// The deferred-free callback slot is process-global. The caller must
    /// synchronize registration, restoration, and callback entry with every
    /// thread and all allocator activity that could invoke or replace the
    /// callback.
    ///
    /// Mimalloc may invoke the callback from another thread unless upstream
    /// evidence proves otherwise, and callback invocation may be reentrant. The
    /// callback must not reenter allocator paths that can invoke the same
    /// deferred-free callback before the current invocation returns.
    ///
    /// The borrowed closure must outlive every possible callback invocation.
    /// Callers must not leak the guard, call [`core::mem::forget`] on it, or
    /// wrap it in [`core::mem::ManuallyDrop`] unless they manually restore the
    /// previous callback state before borrowed state can be invalidated.
    /// Preventing [`Drop`] can leave a process-global C callback pointing at
    /// borrowed Rust state.
    ///
    /// In `std` and test builds, the trampoline catches Rust panics before C ABI
    /// return. In `no_std` builds, callbacks must not panic. Callback bodies must
    /// also avoid allocator reentry unless the caller has proven the reentry path
    /// cannot trigger the same callback or violate the required synchronization.
    #[inline]
    pub unsafe fn new(callback: &'a mut F) -> Self {
        // SAFETY: this snapshots global callback state
        // before replacing it with a trampoline tied to `callback`.
        let previous = unsafe { crate::allocator::page::registered_deferred_free_callback() };
        // SAFETY: the guard stores `callback` for at
        // least as long as the registration is active and restores the previous
        // callback in `Drop`.
        unsafe {
            crate::allocator::page::mi_register_deferred_free(
                Some(deferred_free_callback_trampoline::<F>),
                ptr::from_mut(callback).cast::<c_void>(),
            );
        }
        Self {
            previous,
            _callback: callback,
        }
    }
}

impl<F: FnMut(bool, u64)> Drop for DeferredFreeCallbackGuard<'_, F> {
    fn drop(&mut self) {
        // SAFETY: restoring the exact callback snapshot
        // captured during guard construction.
        unsafe { crate::allocator::page::restore_deferred_free_callback(self.previous) };
    }
}

// SAFETY: registered only with callback arguments
// created by `ErrorCallbackGuard`, and it catches Rust panics before ABI return.
unsafe extern "C" fn error_callback_trampoline<F: FnMut(i32)>(err: i32, arg: *mut c_void) {
    if arg.is_null() {
        return;
    }
    // SAFETY: guard construction stores a valid `&mut F`
    // as the callback argument for the active registration lifetime.
    let Some(callback) = (unsafe { arg.cast::<F>().as_mut() }) else {
        return;
    };
    #[cfg(any(feature = "std", test))]
    {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(err)));
    }
    #[cfg(not(any(feature = "std", test)))]
    {
        callback(err);
    }
}

// SAFETY: registered only with callback arguments
// created by `DeferredFreeCallbackGuard`, and it catches Rust panics before ABI
// return.
unsafe extern "C" fn deferred_free_callback_trampoline<F: FnMut(bool, u64)>(
    force: bool,
    heartbeat: u64,
    arg: *mut c_void,
) {
    if arg.is_null() {
        return;
    }
    // SAFETY: guard construction stores a valid `&mut F`
    // as the callback argument for the active registration lifetime.
    let Some(callback) = (unsafe { arg.cast::<F>().as_mut() }) else {
        return;
    };
    #[cfg(any(feature = "std", test))]
    {
        let _ =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(force, heartbeat)));
    }
    #[cfg(not(any(feature = "std", test)))]
    {
        callback(force, heartbeat);
    }
}
