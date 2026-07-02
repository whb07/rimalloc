use super::DatatypeAcceptanceSources;

pub(super) fn assert_callback_acceptance(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let core_mod = audit_sources.source("src/types/mod.rs");

    let callbacks_rs = audit_sources.source("src/types/callbacks.rs");
    let types_rs = audit_sources.source("src/types/constants.rs");
    let abi_rs = audit_sources.source("src/ffi/abi.rs");
    let options_rs = audit_sources.source("src/allocator/options.rs");
    let deferred_rs = audit_sources.source("src/allocator/page/deferred.rs");
    let arena_heap_pages_rs = audit_sources.source("src/allocator/arena/heap_pages.rs");
    let init_rs = audit_sources.source("src/allocator/init.rs");
    let threadlocal_rs = audit_sources.source("src/allocator/threadlocal.rs");
    assert!(
        callbacks_rs.contains("pub(crate) struct CallbackArg(*mut c_void);")
            && callbacks_rs.contains("callback_arg_wrapper!(DeferredFreeCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(OutputCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(ErrorCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(HeapVisitCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(BlockVisitCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(ThreadLocalClaimCallbackArg);")
            && callbacks_rs.contains("SAFETY[AUDIT:callback-wrappers]")
            && callbacks_rs.contains(
                "optional_callback_wrapper!(DeferredFreeCallback, mi_deferred_free_fun);"
            )
            && callbacks_rs.contains("optional_callback_wrapper!(OutputCallback, mi_output_fun);")
            && callbacks_rs.contains("optional_callback_wrapper!(ErrorCallback, mi_error_fun);")
            && callbacks_rs
                .contains("optional_callback_wrapper!(HeapVisitCallback, mi_heap_visit_fun);")
            && callbacks_rs
                .contains("optional_callback_wrapper!(BlockVisitCallback, mi_block_visit_fun);")
            && callbacks_rs.contains("pub(crate) struct ThreadLocalClaimCallback(mi_claim_fun_t);")
            && core_mod.contains("DeferredFreeCallback")
            && core_mod.contains("OutputCallback")
            && core_mod.contains("ErrorCallback")
            && core_mod.contains("HeapVisitCallback")
            && core_mod.contains("BlockVisitCallback")
            && core_mod.contains("ThreadLocalClaimCallback")
            && core_mod.contains("DeferredFreeCallbackArg")
            && core_mod.contains("OutputCallbackArg")
            && core_mod.contains("ErrorCallbackArg")
            && core_mod.contains("HeapVisitCallbackArg")
            && core_mod.contains("BlockVisitCallbackArg")
            && core_mod.contains("ThreadLocalClaimCallbackArg")
            && types_rs.contains("pub type mi_output_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_error_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_deferred_free_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_heap_visit_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_block_visit_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_commit_fun_t = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_claim_fun_t =\n    unsafe extern \"C\" fn(")
            && abi_rs.contains("out: Option<mi_output_fun>, arg: *mut c_void")
            && abi_rs.contains("fun: Option<mi_error_fun>, arg: *mut c_void")
            && abi_rs.contains("deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void")
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && !types_rs.contains("CallbackArg")
            && !abi_rs.contains("CallbackArg")
            && deferred_rs.contains(
                "let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);"
            )
            && deferred_rs
                .contains("let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);")
            && options_rs.contains(
                "fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback"
            )
            && options_rs.contains("let arg = OutputCallbackArg::from_raw(arg);")
            && options_rs.contains(
                "let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));"
            )
            && arena_heap_pages_rs.contains("visitor: BlockVisitCallback,")
            && arena_heap_pages_rs.contains("arg: BlockVisitCallbackArg,")
            && arena_heap_pages_rs.contains("BlockVisitCallback::new(Some(visitor))")
            && arena_heap_pages_rs.contains("BlockVisitCallbackArg::from_raw(arg)")
            && init_rs.contains("let visitor = HeapVisitCallback::new(visitor);")
            && init_rs.contains("let arg = HeapVisitCallbackArg::from_raw(arg);")
            && threadlocal_rs.contains("ThreadLocalClaimCallback::new(mi_thread_local_claim_fun)"),
        "callback function and context pointers should be typed inside allocator internals while raw callback typedefs stay at C ABI boundaries"
    );
    assert!(
        callbacks_rs
            .matches("SAFETY[AUDIT:callback-wrappers]")
            .count()
            >= 8,
        "callback wrapper unsafe operations should retain operation-level audit markers"
    );
}
