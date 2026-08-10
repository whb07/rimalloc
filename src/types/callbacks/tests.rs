use super::*;
use core::mem::{MaybeUninit, align_of, size_of};

struct DeferredInvocation {
    force: bool,
    heartbeat: u64,
}

struct OutputInvocation {
    msg: *const core::ffi::c_char,
}

struct ErrorInvocation {
    err: i32,
}

struct HeapVisitInvocation {
    heap: *mut crate::types::mi_heap_t,
}

struct BlockVisitInvocation {
    heap: *const crate::types::mi_heap_t,
    area: *const mi_heap_area_t,
    block: *mut c_void,
    block_size: usize,
}

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
    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `OutputCallback`.
    unsafe extern "C" fn output(msg: *const core::ffi::c_char, arg: *mut c_void) {
        let _ = (msg, arg);
    }
    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `ErrorCallback`.
    unsafe extern "C" fn error(err: i32, arg: *mut c_void) {
        let _ = (err, arg);
    }
    // SAFETY: this callback is a local
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
    // SAFETY: `deferred_storage` came from
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

#[test]
fn callback_wrappers_invoke_local_callbacks() {
    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `DeferredFreeCallback`.
    unsafe extern "C" fn deferred(force: bool, heartbeat: u64, arg: *mut c_void) {
        // SAFETY: callers pass a
        // stack-local `DeferredInvocation` through the opaque callback arg.
        unsafe {
            core::ptr::addr_of_mut!((*arg.cast::<DeferredInvocation>()).force).write(force);
            core::ptr::addr_of_mut!((*arg.cast::<DeferredInvocation>()).heartbeat).write(heartbeat);
        }
    }

    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `OutputCallback`.
    unsafe extern "C" fn output(msg: *const core::ffi::c_char, arg: *mut c_void) {
        // SAFETY: callers pass a
        // stack-local `OutputInvocation` through the opaque callback arg.
        unsafe {
            core::ptr::addr_of_mut!((*arg.cast::<OutputInvocation>()).msg).write(msg);
        }
    }

    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `ErrorCallback`.
    unsafe extern "C" fn error(err: i32, arg: *mut c_void) {
        // SAFETY: callers pass a
        // stack-local `ErrorInvocation` through the opaque callback arg.
        unsafe {
            core::ptr::addr_of_mut!((*arg.cast::<ErrorInvocation>()).err).write(err);
        }
    }

    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `HeapVisitCallback`.
    unsafe extern "C" fn heap_visit(heap: *mut crate::types::mi_heap_t, arg: *mut c_void) -> bool {
        // SAFETY: callers pass a
        // stack-local `HeapVisitInvocation` through the opaque callback arg.
        unsafe {
            core::ptr::addr_of_mut!((*arg.cast::<HeapVisitInvocation>()).heap).write(heap);
        }
        true
    }

    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `BlockVisitCallback`.
    unsafe extern "C" fn block_visit(
        heap: *const crate::types::mi_heap_t,
        area: *const mi_heap_area_t,
        block: *mut c_void,
        block_size: usize,
        arg: *mut c_void,
    ) -> bool {
        // SAFETY: callers pass a
        // stack-local `BlockVisitInvocation` through the opaque callback arg.
        unsafe {
            core::ptr::addr_of_mut!((*arg.cast::<BlockVisitInvocation>()).heap).write(heap);
            core::ptr::addr_of_mut!((*arg.cast::<BlockVisitInvocation>()).area).write(area);
            core::ptr::addr_of_mut!((*arg.cast::<BlockVisitInvocation>()).block).write(block);
            core::ptr::addr_of_mut!((*arg.cast::<BlockVisitInvocation>()).block_size)
                .write(block_size);
        }
        true
    }

    // SAFETY: this callback is a local
    // test fixture with the C ABI shape required by `ThreadLocalClaimCallback`.
    unsafe extern "C" fn claim(
        slice_index: usize,
        arena: *mut crate::types::mi_arena_t,
        keep_set: *mut bool,
    ) -> bool {
        // SAFETY: callers pass a
        // stack-local `bool` keep-set cell for this callback invocation.
        unsafe { *keep_set = true };
        slice_index == 99 && !arena.is_null()
    }

    let mut deferred_state = DeferredInvocation {
        force: false,
        heartbeat: 0,
    };
    // SAFETY: the callback and opaque
    // argument are local fixtures valid for the duration of this invocation.
    unsafe {
        DeferredFreeCallback::new(Some(deferred)).invoke(
            true,
            Heartbeat::new(73),
            DeferredFreeCallbackArg::from_raw(core::ptr::addr_of_mut!(deferred_state).cast()),
        );
    }
    assert!(deferred_state.force);
    assert_eq!(deferred_state.heartbeat, 73);

    let msg = c"callback-wrapper".as_ptr();
    let mut output_state = OutputInvocation {
        msg: core::ptr::null(),
    };
    // SAFETY: the message and opaque
    // argument are local fixtures valid for the duration of this invocation.
    unsafe {
        OutputCallback::new(Some(output)).invoke(
            msg,
            OutputCallbackArg::from_raw(core::ptr::addr_of_mut!(output_state).cast()),
        );
    }
    assert_eq!(output_state.msg, msg);

    let mut error_state = ErrorInvocation { err: 0 };
    // SAFETY: the callback and opaque
    // argument are local fixtures valid for the duration of this invocation.
    unsafe {
        ErrorCallback::new(Some(error)).invoke(
            12,
            ErrorCallbackArg::from_raw(core::ptr::addr_of_mut!(error_state).cast()),
        );
    }
    assert_eq!(error_state.err, 12);

    let mut heap = MaybeUninit::<crate::types::mi_heap_t>::zeroed();
    let heap = HeapPtr::new(heap.as_mut_ptr()).unwrap();
    let mut heap_state = HeapVisitInvocation {
        heap: core::ptr::null_mut(),
    };
    // SAFETY: `heap` and the opaque
    // argument are stack-local fixtures valid for this callback invocation.
    let heap_result = unsafe {
        HeapVisitCallback::new(Some(heap_visit)).invoke(
            heap,
            HeapVisitCallbackArg::from_raw(core::ptr::addr_of_mut!(heap_state).cast()),
        )
    };
    assert!(heap_result);
    assert_eq!(heap_state.heap, heap.as_ptr());

    let area = mi_heap_area_t::default();
    let mut block = 0u8;
    let mut block_state = BlockVisitInvocation {
        heap: core::ptr::null(),
        area: core::ptr::null(),
        block: core::ptr::null_mut(),
        block_size: 0,
    };
    // SAFETY: `heap`, `area`, `block`, and
    // the opaque argument are stack-local fixtures valid for this invocation.
    let block_result = unsafe {
        BlockVisitCallback::new(Some(block_visit)).invoke(
            heap,
            core::ptr::addr_of!(area),
            core::ptr::addr_of_mut!(block).cast(),
            64,
            BlockVisitCallbackArg::from_raw(core::ptr::addr_of_mut!(block_state).cast()),
        )
    };
    assert!(block_result);
    assert_eq!(block_state.heap, heap.as_ptr());
    assert_eq!(block_state.area, core::ptr::addr_of!(area));
    assert_eq!(block_state.block, core::ptr::addr_of_mut!(block).cast());
    assert_eq!(block_state.block_size, 64);

    let mut arena = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    let arena = ArenaPtr::new(arena.as_mut_ptr()).unwrap();
    let mut keep_set = false;
    // SAFETY: `arena` and `keep_set` are
    // stack-local fixtures valid for this callback invocation.
    let claim_result = unsafe {
        ThreadLocalClaimCallback::new(claim).invoke(
            99,
            Some(arena),
            core::ptr::addr_of_mut!(keep_set),
        )
    };
    assert!(claim_result);
    assert!(keep_set);
}
