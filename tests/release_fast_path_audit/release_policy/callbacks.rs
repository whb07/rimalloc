use super::*;

#[path = "callbacks/public_abi.rs"]
mod public_abi;
#[path = "callbacks/safe_guards.rs"]
mod safe_guards;

#[test]
fn callback_trampolines_remain_no_alloc_no_unwind_raw_abi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let deferred_rs = fs::read_to_string(root.join("src/allocator/page/deferred.rs"))
        .expect("read src/allocator/page/deferred.rs");
    let arena_heap_pages_rs = fs::read_to_string(root.join("src/allocator/arena/heap_pages.rs"))
        .expect("read src/allocator/arena/heap_pages.rs");
    let theap_rs = fs::read_to_string(root.join("src/allocator/thread_heap.rs"))
        .expect("read src/allocator/thread_heap.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");

    for (source_name, source, marker) in [
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        force: bool,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, message: *const c_char,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, err: i32,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, heap: HeapPtr,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        heap: HeapPtr,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        slice_index: usize,",
        ),
        (
            "src/allocator/options.rs",
            options_rs.as_str(),
            "pub extern \"C\" fn mi_register_output",
        ),
        (
            "src/allocator/options.rs",
            options_rs.as_str(),
            "pub extern \"C\" fn mi_register_error",
        ),
        (
            "src/allocator/page/deferred.rs",
            deferred_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn _mi_deferred_free",
        ),
        (
            "src/allocator/page/deferred.rs",
            deferred_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_register_deferred_free",
        ),
        (
            "src/allocator/arena/heap_pages.rs",
            arena_heap_pages_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_heap_visit_blocks",
        ),
        (
            "src/allocator/arena/heap_pages.rs",
            arena_heap_pages_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_heap_visit_abandoned_blocks",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_theap_visit_blocks",
        ),
        (
            "src/allocator/init.rs",
            init_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_subproc_visit_heaps",
        ),
    ] {
        let item = source_item(source_name, source, marker);
        assert_no_callback_alloc_or_unwind(source_name, marker, item);
    }

    assert!(
        callbacks_rs.contains("SAFETY[AUDIT:callback-wrappers]")
            && callbacks_rs.contains("core::mem::transmute::<*mut c_void, mi_deferred_free_fun>")
            && callbacks_rs.contains("core::mem::transmute::<*mut c_void, mi_output_fun>")
            && callbacks_rs
                .matches("SAFETY[AUDIT:callback-wrappers]")
                .count()
                >= 8,
        "callback wrapper storage and invocation boundaries should have operation-level audit markers"
    );

    assert!(
        types_rs.contains("pub type mi_output_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_error_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_deferred_free_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_heap_visit_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_block_visit_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_commit_fun_t = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_claim_fun_t =\n    unsafe extern \"C\" fn"),
        "src/types/constants.rs should keep public and unstable callback typedefs raw C ABI function pointers"
    );
    assert!(
        types_rs
            .matches("SAFETY[AUDIT:raw-layout-abi-typedefs]")
            .count()
            >= 8,
        "raw callback typedef declarations should retain operation-level audit markers"
    );
    assert!(
        abi_rs.contains("out: Option<mi_output_fun>")
            && abi_rs.contains("fun: Option<mi_error_fun>")
            && abi_rs.contains("deferred_free: Option<mi_deferred_free_fun>")
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && abi_rs.contains("commit_fun: Option<mi_commit_fun_t>")
            && !types_rs.contains("CallbackArg")
            && !abi_rs.contains("CallbackArg"),
        "src/ffi/abi.rs should keep nullable raw callback typedefs at the C ABI boundary"
    );
}
