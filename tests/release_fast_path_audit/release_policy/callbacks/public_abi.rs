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
