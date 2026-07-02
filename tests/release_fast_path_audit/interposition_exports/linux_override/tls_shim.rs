use super::*;

#[test]
fn linux_tls_shims_use_initial_exec_fast_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let tls_shim = fs::read_to_string(root.join("src/tls_shim.c")).expect("read src/tls_shim.c");
    let tls_recurse_shim = fs::read_to_string(root.join("src/tls_recurse_shim.c"))
        .expect("read src/tls_recurse_shim.c");
    let build_rs = fs::read_to_string(root.join("build.rs")).expect("read build.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");

    assert!(
        tls_shim.contains("__mi_theap_default")
            && tls_shim.contains("__mi_theap_cached")
            && tls_shim.contains("tls_model(\"initial-exec\")")
            && tls_shim.contains("visibility(\"hidden\")")
            && tls_shim.contains("= &_mi_theap_empty")
            && tls_shim.contains("mi_thread_locals_empty")
            && tls_shim.contains("mi_thread_locals")
            && tls_shim.contains("= &mi_thread_locals_empty")
            && tls_shim.contains("_mi_tls_thread_locals_get")
            && !tls_shim.contains("mi_tls_recurse")
            && !tls_shim.contains("static __thread unsigned char recurse"),
        "default Linux allocator fast paths should read hidden initial-exec TLS cells like C"
    );
    assert!(
        tls_recurse_shim.contains("static __thread unsigned char recurse")
            && tls_recurse_shim.contains("tls_model(\"initial-exec\")")
            && tls_recurse_shim.contains("_mi_tls_recurse_get")
            && tls_recurse_shim.contains("return recurse != 0;")
            && tls_recurse_shim.contains("_mi_tls_recurse_set")
            && tls_recurse_shim.contains("recurse = (value != 0);")
            && !tls_recurse_shim.contains("int mi_tls_recurse"),
        "Linux recursion guard TLS should stay byte-sized and isolated after heap TLS like C"
    );
    assert!(
        prim_rs.contains("fn mi_linux_x64_theap_default_get()")
            && prim_rs.contains("fn mi_linux_x64_theap_default_set(theap: Option<ThreadHeapPtr>)")
            && prim_rs.contains("fn mi_linux_x64_theap_cached_get()")
            && prim_rs.contains("fn mi_linux_x64_theap_cached_set(theap: Option<ThreadHeapPtr>)")
            && prim_rs.contains("__mi_theap_default@GOTTPOFF")
            && prim_rs.contains("__mi_theap_cached@GOTTPOFF")
            && init_rs.contains("fn mi_linux_x64_theap_main_get()")
            && init_rs.contains("fn mi_linux_x64_theap_main_set(theap: Option<ThreadHeapPtr>)")
            && init_rs.contains("__mi_theap_main@GOTTPOFF"),
        "Linux x86_64 heap TLS accessors should inline C-style initial-exec loads and stores"
    );
    assert!(
        prim_rs.matches("tls_offset = out(reg) _").count() >= 2
            && init_rs.contains("tls_offset = out(reg) _")
            && threadlocal_rs.contains("tls_offset = out(reg) _"),
        "Linux x86_64 TLS setters should not let the offset scratch register overlap the stored pointer"
    );
    assert!(
        prim_rs.contains("pub unsafe fn __mi_theap_empty() -> *mut mi_theap_t")
            && prim_rs.contains("\"lea {empty}, [rip + _mi_theap_empty]\"")
            && init_rs.contains("core::arch::global_asm!(\".hidden _mi_theap_empty\")"),
        "Linux x86_64 empty-theap address loads should use the hidden local symbol directly like C"
    );
    assert!(
        threadlocal_rs.contains("fn mi_linux_x64_thread_locals_get()")
            && threadlocal_rs
                .contains("fn mi_linux_x64_thread_locals_set(tls: *mut mi_thread_locals_t)")
            && threadlocal_rs.contains("mi_thread_locals@GOTTPOFF")
            && threadlocal_rs.contains("static mi_thread_locals_empty: mi_thread_locals_t;")
            && !tls_shim.contains("static __thread void* mi_tls_thread_locals"),
        "Linux x86_64 dynamic thread-local storage should use C's mi_thread_locals TLS cell directly"
    );
    let dynamic_tls_marker_total = [
        "SAFETY[AUDIT:threadlocal-empty-sentinel-boundary]",
        "SAFETY[AUDIT:threadlocal-tls-shim-boundary]",
        "SAFETY[AUDIT:threadlocal-slot-block-boundary]",
        "SAFETY[AUDIT:threadlocal-claim-callback-boundary]",
        "SAFETY[AUDIT:threadlocal-key-claim-boundary]",
        "SAFETY[AUDIT:threadlocal-free-bitmap-boundary]",
        "SAFETY[AUDIT:threadlocal-global-manager-boundary]",
        "SAFETY[AUDIT:threadlocal-key-access-boundary]",
        "SAFETY[AUDIT:threadlocal-thread-done-boundary]",
        "SAFETY[AUDIT:threadlocal-key-create-boundary]",
        "SAFETY[AUDIT:threadlocal-key-free-boundary]",
    ]
    .into_iter()
    .map(|marker| threadlocal_rs.matches(marker).count())
    .sum::<usize>();
    assert!(
        !unsafe_allowlist.contains("src/allocator/threadlocal.rs")
            && !threadlocal_rs.contains("SAFETY[AUDIT:threadlocal-key-manager]")
            && dynamic_tls_marker_total >= 50,
        "dynamic thread-local unsafe boundaries should stay split into focused operation markers without a migration allowlist entry"
    );
    assert!(
        threadlocal_rs.contains(
            "if mi_thread_locals_free.load(Ordering::Acquire).is_null() {\n        return;\n    }"
        ),
        "dynamic thread-local cleanup should skip the lock path when no dynamic TLS slots were allocated"
    );
    assert!(
        build_rs.contains("let interpose = feature_enabled(\"INTERPOSE\")")
            && build_rs.contains("let cxx_overrides = feature_enabled(\"CXX_OVERRIDES\")")
            && build_rs.contains("let tls_shim = feature_enabled(\"TLS_SHIM\")")
            && build_rs.contains("if cxx_overrides")
            && build_rs.contains("let needs_c_tls_shim = tls_shim || !std")
            && build_rs.contains("if needs_c_tls_shim")
            && build_rs.contains("target_arch == \"x86_64\"")
            && build_rs.contains("cargo:rustc-cfg=mi_c_malloc_fast")
            && build_rs.contains("src/tls_shim.c")
            && build_rs.contains("src/tls_recurse_shim.c")
            && build_rs.contains("ar.arg(\"crs\").arg(&lib).arg(&tls_obj).arg(&recurse_obj);"),
        "assembly malloc fast override must be limited to the explicit interpose build mode and supported target"
    );
}
