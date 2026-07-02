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
    // SAFETY[AUDIT:typed-callback-test-fixtures]: this callback is a local
    // test fixture with the C ABI shape required by `OutputCallback`.
    unsafe extern "C" fn output(msg: *const core::ffi::c_char, arg: *mut c_void) {
        let _ = (msg, arg);
    }
    // SAFETY[AUDIT:typed-callback-test-fixtures]: this callback is a local
    // test fixture with the C ABI shape required by `ErrorCallback`.
    unsafe extern "C" fn error(err: i32, arg: *mut c_void) {
        let _ = (err, arg);
    }
    // SAFETY[AUDIT:typed-callback-test-fixtures]: this callback is a local
    // test fixture with the C ABI shape required by `DeferredFreeCallback`.
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
    // SAFETY[AUDIT:typed-callback-test-fixtures]: `deferred_storage` came from
    // the live `deferred_callback` value above and is checked non-null.
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
