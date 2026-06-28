use core::ffi::c_void;
use core::fmt;
use core::ptr;

/// Error returned by safe callback guard registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackError {
    Register,
    Unsupported,
}

impl fmt::Display for CallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Register => f.write_str("mimalloc callback registration failed"),
            Self::Unsupported => f.write_str("mimalloc callback is unsupported"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CallbackError {}

/// RAII guard for a temporary error callback registration.
///
/// The guard borrows the callback closure, cannot outlive captured state, and
/// restores the previous global registration on drop. The trampoline catches
/// Rust panics before returning through the C callback ABI in `std` and test
/// builds; `no_std` builds must not panic from the callback.
///
/// ```rust,compile_fail
/// use mechanical_mimalloc::ErrorCallbackGuard;
///
/// let guard;
/// {
///     let mut hits = 0usize;
///     let mut callback = |_err| hits += 1;
///     guard = unsafe { ErrorCallbackGuard::new(&mut callback).unwrap() };
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
    /// The caller must ensure no other thread can enter, replace, or restore
    /// the error callback while this guard is active, and the callback must not
    /// reenter allocator paths that can invoke the same error callback before
    /// the current invocation returns. These obligations prevent simultaneous
    /// mutable access to the borrowed `FnMut` and keep the global registration
    /// stack well formed.
    #[inline]
    pub unsafe fn new(callback: &'a mut F) -> Result<Self, CallbackError> {
        let previous = crate::options::registered_error_callback();
        crate::options::mi_register_error(
            Some(error_callback_trampoline::<F>),
            ptr::from_mut(callback).cast::<c_void>(),
        );
        Ok(Self {
            previous,
            _callback: callback,
        })
    }
}

impl<F: FnMut(i32)> Drop for ErrorCallbackGuard<'_, F> {
    fn drop(&mut self) {
        crate::options::restore_error_callback(self.previous);
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
/// use mechanical_mimalloc::DeferredFreeCallbackGuard;
///
/// let guard;
/// {
///     let mut heartbeats = Vec::new();
///     let mut callback = |_force, heartbeat| heartbeats.push(heartbeat);
///     guard = unsafe { DeferredFreeCallbackGuard::new(&mut callback).unwrap() };
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
    /// The caller must install the guard before allocator concurrency begins or
    /// otherwise ensure no other thread can enter, replace, or restore the
    /// deferred-free callback while this guard is active. The callback must not
    /// reenter allocator paths that can invoke the same deferred-free callback
    /// before the current invocation returns. These obligations prevent
    /// simultaneous mutable access to the borrowed `FnMut` and keep the global
    /// registration stack well formed.
    #[inline]
    pub unsafe fn new(callback: &'a mut F) -> Result<Self, CallbackError> {
        // SAFETY[AUDIT:safe-callback]: this snapshots global callback state
        // before replacing it with a trampoline tied to `callback`.
        let previous = unsafe { crate::page::registered_deferred_free_callback() };
        // SAFETY[AUDIT:safe-callback]: the guard stores `callback` for at
        // least as long as the registration is active and restores the previous
        // callback in `Drop`.
        unsafe {
            crate::page::mi_register_deferred_free(
                Some(deferred_free_callback_trampoline::<F>),
                ptr::from_mut(callback).cast::<c_void>(),
            );
        }
        Ok(Self {
            previous,
            _callback: callback,
        })
    }
}

impl<F: FnMut(bool, u64)> Drop for DeferredFreeCallbackGuard<'_, F> {
    fn drop(&mut self) {
        // SAFETY[AUDIT:safe-callback]: restoring the exact callback snapshot
        // captured during guard construction.
        unsafe { crate::page::restore_deferred_free_callback(self.previous) };
    }
}

// SAFETY[AUDIT:safe-callback]: registered only with callback arguments created
// by `ErrorCallbackGuard`, and it catches Rust panics before ABI return.
unsafe extern "C" fn error_callback_trampoline<F: FnMut(i32)>(err: i32, arg: *mut c_void) {
    if arg.is_null() {
        return;
    }
    // SAFETY[AUDIT:safe-callback]: guard construction stores a valid `&mut F`
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

// SAFETY[AUDIT:safe-callback]: registered only with callback arguments created
// by `DeferredFreeCallbackGuard`, and it catches Rust panics before ABI return.
unsafe extern "C" fn deferred_free_callback_trampoline<F: FnMut(bool, u64)>(
    force: bool,
    heartbeat: u64,
    arg: *mut c_void,
) {
    if arg.is_null() {
        return;
    }
    // SAFETY[AUDIT:safe-callback]: guard construction stores a valid `&mut F`
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
