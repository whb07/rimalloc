use super::*;

#[test]
fn public_callback_abi_inventory_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let deferred_rs = fs::read_to_string(root.join("src/allocator/page/deferred.rs"))
        .expect("read src/allocator/page/deferred.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");

    for callback in [
        "mi_output_fun",
        "mi_error_fun",
        "mi_deferred_free_fun",
        "mi_heap_visit_fun",
        "mi_block_visit_fun",
        "mi_commit_fun_t",
    ] {
        assert!(
            abi_doc.contains(callback) && types_rs.contains(&format!("pub type {callback}")),
            "{callback} should be listed in docs/abi.md and declared in src/types/constants.rs"
        );
    }

    for contract_word in [
        "Nullability",
        "Lifetime",
        "Threading",
        "Reentrancy",
        "Unwind",
    ] {
        assert!(
            types_rs.matches(contract_word).count() >= 6,
            "each public callback type should document {contract_word}"
        );
    }
    assert!(
        types_rs.matches("panic").count() >= 6 && types_rs.matches("unwind").count() >= 6,
        "each public callback type should document panic/unwind behavior"
    );

    for family in [
        "Output callback",
        "Error callback",
        "Deferred free callback",
        "Heap visitor callback",
        "Block visitor callback",
        "Managed-memory commit callback",
    ] {
        assert!(
            abi_doc.contains(family),
            "docs/abi.md should list the {family} callback family"
        );
    }

    assert!(
        abi_rs.contains(
            "pub fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void);"
        ) && abi_rs
            .contains("pub fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void);")
            && abi_rs.contains(
                "pub fn mi_register_deferred_free(deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void);"
            )
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && abi_rs.contains("commit_fun: Option<mi_commit_fun_t>"),
        "src/ffi/abi.rs should expose callback-accepting C declarations with nullable Option callbacks"
    );

    assert!(
        !unsafe_allowlist.contains("src/types/constants.rs")
            && types_rs
                .matches("SAFETY[AUDIT:raw-layout-abi-typedefs]")
                .count()
                >= 8
            && types_rs.matches("SAFETY[AUDIT:raw-auto-trait]").count() == 2
            && types_rs.matches("unsafe impl Sync").count() == 2,
        "raw layout callback typedefs and sentinel Sync impls should stay locally audited without a migration allowlist entry"
    );

    for forbidden in [
        "OutputCallback",
        "ErrorCallback",
        "DeferredFreeCallback",
        "HeapVisitCallback",
        "BlockVisitCallback",
        "ThreadLocalClaimCallback",
        "CallbackArg",
        "dyn Fn",
        "impl Fn",
    ] {
        assert!(
            !types_rs.contains(forbidden) && !abi_rs.contains(forbidden),
            "raw callback typedefs and ABI declarations must not use {forbidden}"
        );
    }

    assert!(
        options_rs.contains("pub extern \"C\" fn mi_register_output")
            && options_rs.contains("pub extern \"C\" fn mi_register_error")
            && deferred_rs.contains("pub unsafe extern \"C\" fn mi_register_deferred_free")
            && arena_rs.contains("pub unsafe extern \"C\" fn mi_manage_memory")
            && abi_doc.contains("mi_claim_fun_t")
            && abi_doc.contains("mi_forall_set_fun_t")
            && abi_doc.contains("crate-private implementation modules"),
        "callback registrations and internal callback families should be inventoried at their public boundary"
    );
}

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

#[test]
fn safe_callback_guards_document_lifetime_restore_and_unwind_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let safe_doc =
        fs::read_to_string(root.join("docs/safe-api.md")).expect("read docs/safe-api.md");
    let unsafe_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read docs/unsafe-audit.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let unsafe_callbacks_rs =
        fs::read_to_string(root.join("src/ffi/callbacks.rs")).expect("read unsafe callbacks");

    assert!(
        abi_doc.contains("Raw callback registration remains unsafe.")
            && abi_doc.contains("`unsafe_callbacks::ErrorCallbackGuard` and")
            && abi_doc.contains("restore the previous registration on drop")
            && abi_doc.contains(
                "catch panic or unwind before returning across the\ncallback C ABI boundary"
            )
            && abi_doc.contains("Nested borrowed guards must be dropped in LIFO order")
            && abi_doc.contains("Output callbacks remain raw"),
        "docs/abi.md should document the safe callback guard proof and remaining raw output callback gap"
    );
    assert!(
        safe_doc.contains("`unsafe_callbacks`")
            && safe_doc.contains("`FnMut` callback guards")
            && safe_doc.contains("Tier U unsafe Rust convenience APIs")
            && safe_doc.contains("catch Rust panics")
            && safe_doc.contains("Nested guards must be dropped in LIFO order")
            && unsafe_doc.contains("SAFETY[AUDIT:unsafe-callback-guard-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:unsafe-callback-test-fixtures]")
            && unsafe_doc.contains("restores that previous registration on drop")
            && unsafe_doc.contains("restores the process-global snapshot")
            && unsafe_doc.contains("catches Rust\n  panics before callback ABI return")
            && unsafe_doc.contains("`error_callback_trampoline::<F>`")
            && unsafe_doc.contains("`deferred_free_callback_trampoline::<F>`")
            && unsafe_doc.contains(
                "no concurrent or reentrant entry may access the same\n  borrowed callback"
            ),
        "safe API and unsafe audit docs should prove safe callback guard lifetime, restore, and unwind behavior"
    );

    assert!(
        lib_rs.contains("pub use ffi::callbacks as unsafe_callbacks;")
            && unsafe_callbacks_rs.contains("std::panic::catch_unwind")
            && unsafe_callbacks_rs.contains("mi_register_error")
            && unsafe_callbacks_rs.contains("mi_register_deferred_free")
            && !unsafe_callbacks_rs.contains("mi_register_output"),
        "unsafe callback namespace should expose only callback guards that catch panics and restore previous registrations"
    );
}
