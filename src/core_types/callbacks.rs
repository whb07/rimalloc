use core::ffi::{c_char, c_void};

use super::ptr::{ArenaPtr, HeapPtr};
use super::scalar::Heartbeat;

use crate::types::{
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
            Self::new(Some(unsafe {
                core::mem::transmute::<*mut c_void, mi_deferred_free_fun>(ptr)
            }))
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
            Self::new(Some(unsafe {
                core::mem::transmute::<*mut c_void, mi_output_fun>(ptr)
            }))
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn invoke(self, message: *const c_char, arg: OutputCallbackArg) {
        if let Some(fun) = self.raw() {
            unsafe { fun(message, arg.as_ptr()) };
        }
    }
}

impl ErrorCallback {
    #[inline(always)]
    pub(crate) unsafe fn invoke(self, err: i32, arg: ErrorCallbackArg) {
        if let Some(fun) = self.raw() {
            unsafe { fun(err, arg.as_ptr()) };
        }
    }
}

impl HeapVisitCallback {
    #[inline(always)]
    pub(crate) unsafe fn invoke(self, heap: HeapPtr, arg: HeapVisitCallbackArg) -> bool {
        match self.raw() {
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
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn callback_wrappers_match_raw_layout() {
        assert_eq!(
            size_of::<DeferredFreeCallback>(),
            size_of::<Option<mi_deferred_free_fun>>()
        );
        assert_eq!(
            align_of::<DeferredFreeCallback>(),
            align_of::<Option<mi_deferred_free_fun>>()
        );
        assert_eq!(
            size_of::<OutputCallback>(),
            size_of::<Option<mi_output_fun>>()
        );
        assert_eq!(
            align_of::<OutputCallback>(),
            align_of::<Option<mi_output_fun>>()
        );
        assert_eq!(
            size_of::<ErrorCallback>(),
            size_of::<Option<mi_error_fun>>()
        );
        assert_eq!(
            align_of::<ErrorCallback>(),
            align_of::<Option<mi_error_fun>>()
        );
        assert_eq!(
            size_of::<HeapVisitCallback>(),
            size_of::<Option<mi_heap_visit_fun>>()
        );
        assert_eq!(
            align_of::<HeapVisitCallback>(),
            align_of::<Option<mi_heap_visit_fun>>()
        );
        assert_eq!(
            size_of::<BlockVisitCallback>(),
            size_of::<Option<mi_block_visit_fun>>()
        );
        assert_eq!(
            align_of::<BlockVisitCallback>(),
            align_of::<Option<mi_block_visit_fun>>()
        );
        assert_eq!(
            size_of::<ThreadLocalClaimCallback>(),
            size_of::<mi_claim_fun_t>()
        );
        assert_eq!(
            align_of::<ThreadLocalClaimCallback>(),
            align_of::<mi_claim_fun_t>()
        );
        assert_eq!(size_of::<CallbackArg>(), size_of::<*mut c_void>());
        assert_eq!(align_of::<CallbackArg>(), align_of::<*mut c_void>());
        assert_eq!(
            size_of::<DeferredFreeCallbackArg>(),
            size_of::<*mut c_void>()
        );
        assert_eq!(
            align_of::<DeferredFreeCallbackArg>(),
            align_of::<*mut c_void>()
        );
        assert_eq!(size_of::<OutputCallbackArg>(), size_of::<*mut c_void>());
        assert_eq!(align_of::<OutputCallbackArg>(), align_of::<*mut c_void>());
        assert_eq!(size_of::<ErrorCallbackArg>(), size_of::<*mut c_void>());
        assert_eq!(align_of::<ErrorCallbackArg>(), align_of::<*mut c_void>());
        assert_eq!(size_of::<HeapVisitCallbackArg>(), size_of::<*mut c_void>());
        assert_eq!(
            align_of::<HeapVisitCallbackArg>(),
            align_of::<*mut c_void>()
        );
        assert_eq!(size_of::<BlockVisitCallbackArg>(), size_of::<*mut c_void>());
        assert_eq!(
            align_of::<BlockVisitCallbackArg>(),
            align_of::<*mut c_void>()
        );
        assert_eq!(
            size_of::<ThreadLocalClaimCallbackArg>(),
            size_of::<*mut c_void>()
        );
        assert_eq!(
            align_of::<ThreadLocalClaimCallbackArg>(),
            align_of::<*mut c_void>()
        );
    }

    #[test]
    fn callback_wrappers_preserve_raw_values() {
        unsafe extern "C" fn output(msg: *const core::ffi::c_char, arg: *mut c_void) {
            let _ = (msg, arg);
        }
        unsafe extern "C" fn error(err: i32, arg: *mut c_void) {
            let _ = (err, arg);
        }
        unsafe extern "C" fn deferred(force: bool, heartbeat: u64, arg: *mut c_void) {
            let _ = (force, heartbeat, arg);
        }

        assert!(!OutputCallback::none().is_some());
        assert!(OutputCallback::new(Some(output)).is_some());
        assert!(OutputCallback::new(Some(output)).raw().is_some());
        assert!(ErrorCallback::new(Some(error)).raw().is_some());
        let deferred_callback = DeferredFreeCallback::new(Some(deferred));
        let deferred_storage = deferred_callback.storage_ptr();
        assert!(!deferred_storage.is_null());
        assert!(unsafe { DeferredFreeCallback::from_storage_ptr(deferred_storage) }.is_some());
        assert!(DeferredFreeCallback::none().storage_ptr().is_null());

        let mut value = 0usize;
        let arg = CallbackArg::from_raw(core::ptr::addr_of_mut!(value).cast());
        assert_eq!(arg.as_ptr(), core::ptr::addr_of_mut!(value).cast());
        assert!(CallbackArg::null().as_ptr().is_null());

        let output_arg = OutputCallbackArg::from_raw(arg.as_ptr());
        assert_eq!(output_arg.as_ptr(), arg.as_ptr());
        assert_eq!(output_arg.as_callback_arg().as_ptr(), arg.as_ptr());
        assert!(DeferredFreeCallbackArg::null().as_ptr().is_null());
        assert_eq!(
            ErrorCallbackArg::from_raw(arg.as_ptr()).as_ptr(),
            arg.as_ptr()
        );
        assert_eq!(
            HeapVisitCallbackArg::from_raw(arg.as_ptr()).as_ptr(),
            arg.as_ptr()
        );
        assert_eq!(
            BlockVisitCallbackArg::from_raw(arg.as_ptr()).as_ptr(),
            arg.as_ptr()
        );
        assert_eq!(
            ThreadLocalClaimCallbackArg::from_raw(arg.as_ptr()).as_ptr(),
            arg.as_ptr()
        );
    }
}
