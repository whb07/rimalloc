use core::ffi::{c_char, c_void};

use super::ptr::{ArenaPtr, HeapPtr};
use super::scalar::Heartbeat;

use super::constants::{
    mi_block_visit_fun, mi_claim_fun_t, mi_deferred_free_fun, mi_error_fun, mi_heap_area_t,
    mi_heap_visit_fun, mi_output_fun,
};

/// Opaque callback argument passed back to C callback trampolines.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct CallbackArg(*mut c_void);

impl CallbackArg {
    #[inline(always)]
    pub(crate) const fn null() -> Self {
        Self(core::ptr::null_mut())
    }

    #[inline(always)]
    pub(crate) const fn from_raw(arg: *mut c_void) -> Self {
        Self(arg)
    }

    #[inline(always)]
    pub(crate) const fn as_ptr(self) -> *mut c_void {
        self.0
    }
}

macro_rules! callback_arg_wrapper {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub(crate) struct $name(CallbackArg);

        impl $name {
            #[inline(always)]
            pub(crate) const fn null() -> Self {
                Self(CallbackArg::null())
            }

            #[inline(always)]
            pub(crate) const fn from_raw(arg: *mut c_void) -> Self {
                Self(CallbackArg::from_raw(arg))
            }

            #[inline(always)]
            pub(crate) const fn as_ptr(self) -> *mut c_void {
                self.0.as_ptr()
            }

            #[inline(always)]
            pub(crate) const fn as_callback_arg(self) -> CallbackArg {
                self.0
            }
        }
    };
}

callback_arg_wrapper!(DeferredFreeCallbackArg);
callback_arg_wrapper!(OutputCallbackArg);
callback_arg_wrapper!(ErrorCallbackArg);
callback_arg_wrapper!(HeapVisitCallbackArg);
callback_arg_wrapper!(BlockVisitCallbackArg);
callback_arg_wrapper!(ThreadLocalClaimCallbackArg);

macro_rules! optional_callback_wrapper {
    ($name:ident, $fun:ty) => {
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub(crate) struct $name(Option<$fun>);

        impl $name {
            #[inline(always)]
            pub(crate) const fn none() -> Self {
                Self(None)
            }

            #[inline(always)]
            pub(crate) const fn new(fun: Option<$fun>) -> Self {
                Self(fun)
            }

            #[inline(always)]
            pub(crate) const fn raw(self) -> Option<$fun> {
                self.0
            }

            #[inline(always)]
            pub(crate) const fn is_some(self) -> bool {
                self.0.is_some()
            }
        }
    };
}

optional_callback_wrapper!(DeferredFreeCallback, mi_deferred_free_fun);
optional_callback_wrapper!(OutputCallback, mi_output_fun);
optional_callback_wrapper!(ErrorCallback, mi_error_fun);
optional_callback_wrapper!(HeapVisitCallback, mi_heap_visit_fun);
optional_callback_wrapper!(BlockVisitCallback, mi_block_visit_fun);

impl DeferredFreeCallback {
    #[inline(always)]
    pub(crate) fn storage_ptr(self) -> *mut c_void {
        match self.raw() {
            Some(fun) => fun as *const () as *mut c_void,
            None => core::ptr::null_mut(),
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn from_storage_ptr(ptr: *mut c_void) -> Self {
        if ptr.is_null() {
            Self::none()
        } else {
            // SAFETY[AUDIT:callback-wrappers]: callers pass pointers produced
            // by `DeferredFreeCallback::storage_ptr` for this callback family.
            let fun = unsafe { core::mem::transmute::<*mut c_void, mi_deferred_free_fun>(ptr) };
            Self::new(Some(fun))
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn invoke(
        self,
        force: bool,
        heartbeat: Heartbeat,
        arg: DeferredFreeCallbackArg,
    ) {
        if let Some(fun) = self.raw() {
            // SAFETY[AUDIT:callback-wrappers]: the caller upholds the C
            // callback contract; typed wrappers only forward raw ABI values.
            unsafe { fun(force, heartbeat.get(), arg.as_ptr()) };
        }
    }
}

impl OutputCallback {
    #[inline(always)]
    pub(crate) fn storage_ptr(self) -> *mut c_void {
        match self.raw() {
            Some(fun) => fun as *const () as *mut c_void,
            None => core::ptr::null_mut(),
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn from_storage_ptr(ptr: *mut c_void) -> Self {
        if ptr.is_null() {
            Self::none()
        } else {
            // SAFETY[AUDIT:callback-wrappers]: callers pass pointers produced
            // by `OutputCallback::storage_ptr` for this callback family.
            let fun = unsafe { core::mem::transmute::<*mut c_void, mi_output_fun>(ptr) };
            Self::new(Some(fun))
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn invoke(self, message: *const c_char, arg: OutputCallbackArg) {
        if let Some(fun) = self.raw() {
            // SAFETY[AUDIT:callback-wrappers]: the caller guarantees `message`
            // and `arg` satisfy the registered output callback ABI contract.
            unsafe { fun(message, arg.as_ptr()) };
        }
    }
}

impl ErrorCallback {
    #[inline(always)]
    pub(crate) unsafe fn invoke(self, err: i32, arg: ErrorCallbackArg) {
        if let Some(fun) = self.raw() {
            // SAFETY[AUDIT:callback-wrappers]: the caller guarantees `arg`
            // satisfies the registered error callback ABI contract.
            unsafe { fun(err, arg.as_ptr()) };
        }
    }
}

impl HeapVisitCallback {
    #[inline(always)]
    pub(crate) unsafe fn invoke(self, heap: HeapPtr, arg: HeapVisitCallbackArg) -> bool {
        match self.raw() {
            // SAFETY[AUDIT:callback-wrappers]: the caller supplies a live heap
            // witness and callback argument for the traversal callback.
            Some(fun) => unsafe { fun(heap.as_ptr(), arg.as_ptr()) },
            None => false,
        }
    }
}

impl BlockVisitCallback {
    #[inline(always)]
    pub(crate) unsafe fn invoke(
        self,
        heap: HeapPtr,
        area: *const mi_heap_area_t,
        block: *mut c_void,
        block_size: usize,
        arg: BlockVisitCallbackArg,
    ) -> bool {
        match self.raw() {
            // SAFETY[AUDIT:callback-wrappers]: the caller supplies live
            // traversal pointers and an opaque callback argument.
            Some(fun) => unsafe { fun(heap.as_ptr(), area, block, block_size, arg.as_ptr()) },
            None => false,
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct ThreadLocalClaimCallback(mi_claim_fun_t);

impl ThreadLocalClaimCallback {
    #[inline(always)]
    pub(crate) const fn new(fun: mi_claim_fun_t) -> Self {
        Self(fun)
    }

    #[inline(always)]
    pub(crate) const fn raw(self) -> mi_claim_fun_t {
        self.0
    }

    #[inline(always)]
    pub(crate) unsafe fn invoke(
        self,
        slice_index: usize,
        arena: Option<ArenaPtr>,
        keep_set: *mut bool,
    ) -> bool {
        // SAFETY[AUDIT:callback-wrappers]: the caller supplies the typed arena
        // witness and writable keep-set pointer required by the claim callback.
        unsafe {
            (self.raw())(
                slice_index,
                arena.map(ArenaPtr::as_ptr).unwrap_or(core::ptr::null_mut()),
                keep_set,
            )
        }
    }
}

pub(super) fn __lint_touch() {
    let _ = DeferredFreeCallbackArg::null();
    let _ = DeferredFreeCallbackArg::null().as_callback_arg();
    let _ = OutputCallbackArg::null();
    let _ = OutputCallbackArg::null().as_callback_arg();
    let _ = ErrorCallbackArg::null();
    let _ = ErrorCallbackArg::null().as_callback_arg();
    let _ = HeapVisitCallbackArg::null();
    let _ = HeapVisitCallbackArg::null().as_callback_arg();
    let _ = BlockVisitCallbackArg::null();
    let _ = BlockVisitCallbackArg::null().as_callback_arg();
    let _ = ThreadLocalClaimCallbackArg::null();
    let _ = ThreadLocalClaimCallbackArg::from_raw(core::ptr::null_mut()).as_ptr();
    let _ = ThreadLocalClaimCallbackArg::null().as_callback_arg();
    let _ = OutputCallback::new(None).is_some();
    let _ = HeapVisitCallback::none();
    let _ = BlockVisitCallback::none().is_some();
}

#[cfg(test)]
mod tests;
