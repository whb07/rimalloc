use super::*;

#[test]
fn interpose_linux_malloc_override_uses_c_tls_fast_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let tls_shim = fs::read_to_string(root.join("src/tls_shim.c")).expect("read src/tls_shim.c");
    let tls_recurse_shim = fs::read_to_string(root.join("src/tls_recurse_shim.c"))
        .expect("read src/tls_recurse_shim.c");
    let build_rs = fs::read_to_string(root.join("build.rs")).expect("read build.rs");
    let aligned_rs = fs::read_to_string(root.join("src/allocator/aligned.rs"))
        .expect("read src/allocator/aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let override_rs =
        fs::read_to_string(root.join("src/raw/macros.rs")).expect("read src/raw/macros.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");

    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/raw/macros.rs\t"))
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-allocation-boundary]")
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-bootstrap-boundary]")
                .count()
                >= 15
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-override-entry-boundary]")
                .count()
                >= 35
            && !override_rs.contains("SAFETY[AUDIT:raw-macro-libc-forwarding-boundary]")
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-core-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-string-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-realloc-boundary]")
                .count()
                >= 2
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]")
                .count()
                >= 20
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]")
                .count()
                >= 4
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-wasi-libc-boundary]")
                .count()
                >= 11
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-linux-libc-boundary]")
                .count()
                >= 20
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-allocation-query-boundary]")
                .count()
                >= 10
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-cxx-boundary]")
                .count()
                >= 45
            && override_rs
                .matches("SAFETY[AUDIT:raw-macro-header-forwarding-boundary]")
                .count()
                >= 6
            && override_rs.contains("core::arch::global_asm!(")
            && override_rs.contains("mod unix_bootstrap")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && override_rs.contains("pub unsafe fn mi_forward1")
            && override_rs.contains("pub unsafe fn mi_malloc_size_forward")
            && override_rs.contains("pub unsafe fn mi_unix_malloc(")
            && override_rs.contains("pub unsafe fn mi_unix_posix_memalign")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_unix_malloc_export")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_wasi_libc_malloc_export")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_linux_libc_malloc_export"),
        "raw macro allocation facade unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        cargo_toml.contains("crate-type = [\"rlib\"]")
            && cargo_toml.contains("c-abi = []")
            && cargo_toml.contains("cxx-overrides = [\"c-abi\"]")
            && cargo_toml.contains("tls-shim = [\"c-abi\"]")
            && cargo_toml.contains("interpose = [\"c-abi\", \"cxx-overrides\", \"tls-shim\"]")
            && cargo_toml.contains("lto = \"fat\"")
            && cargo_toml.contains("codegen-units = 1"),
        "ordinary builds should stay rlib-first while the explicit interpose feature restores the optimized cdylib artifact mode"
    );
    assert!(
        lib_rs.contains("not(feature = \"std\")")
            && lib_rs.contains("no_std")
            && lib_rs.contains("#[panic_handler]")
            && lib_rs.contains("#[link_name = \"abort\"]")
            && lib_rs.contains("extern \"C\" fn rust_eh_personality()"),
        "Linux release cdylibs should avoid linking Rust std panic/backtrace machinery unless the explicit std feature is enabled for tooling"
    );
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
    assert!(
        !unsafe_allowlist.contains("src/allocator/threadlocal.rs")
            && threadlocal_rs
                .matches("SAFETY[AUDIT:threadlocal-key-manager]")
                .count()
                >= 40,
        "dynamic thread-local unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
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
    assert!(
        override_rs.contains("#[unsafe(naked)]")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && !override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_array_fast_64")
            && override_rs.contains("core::arch::naked_asm!")
            && override_rs.contains(".set _Znam, _Znwm")
            && override_rs.contains("try_new = sym crate::alloc::mi_theap_try_new")
            && override_rs.contains("\"xor edx, edx\"")
            && !override_rs.contains("\"jmp mi_new\"")
            && override_rs.matches("\"endbr64\"").count() >= 4
            && override_rs.contains("__mi_theap_default@GOTTPOFF")
            && override_rs.contains("[rdi + rax*8 + 0x110]")
            && override_rs.contains("malloc_generic = sym crate::page::_mi_malloc_generic")
            && override_rs.contains("stosb_max = sym crate::init::_mi_cpu_stosb_max")
            && override_rs.contains("\"mul rsi\"")
            && override_rs.contains("\"jo .Lmi_calloc_overflow\"")
            && override_rs.contains("\"mov edx, 1\"")
            && override_rs.contains("\"rep stosb\"")
            && override_rs.contains("\"call memset\"")
            && override_rs.contains("core::arch::global_asm!")
            && override_rs.contains(".globl free")
            && override_rs.contains("\"malloc:\"")
            && override_rs.contains("\"calloc:\"")
            && override_rs.contains(".set mi_malloc, malloc")
            && override_rs.contains(".set __libc_malloc, malloc")
            && override_rs.contains(".globl mi_calloc")
            && override_rs.contains(".set mi_calloc, calloc")
            && override_rs.contains(".globl __libc_calloc")
            && override_rs.contains(".set __libc_calloc, calloc")
            && override_rs.contains(".p2align 4")
            && override_rs.contains(".size free, .-free")
            && override_rs.contains(".set mi_free, free")
            && override_rs.contains(".set __libc_free, free")
            && override_rs.contains(".set _ZdlPv, free")
            && override_rs.contains(".set _ZdaPv, free")
            && override_rs.contains(".fill 0x30,1,0x90")
            && override_rs.contains(".fill 0x520,1,0x90")
            && override_rs.contains("page_map = sym crate::page_map::_mi_page_map")
            && override_rs.contains("retire = sym crate::page::_mi_page_retire")
            && override_rs.contains("collect_mt = sym crate::free::mi_free_try_collect_mt")
            && override_rs.contains("generic_local = sym crate::free::mi_free_generic_local")
            && override_rs.contains("generic_mt = sym crate::free::mi_free_generic_mt")
            && override_rs.contains("\"cmp rdx, 3\"")
            && override_rs.contains("\"lock cmpxchg qword ptr [rdx], rcx\"")
            && override_rs.contains("\"jmp {try_new}\""),
        "default Linux malloc/new/free overrides should use direct naked-assembly fast paths"
    );
    assert!(
        alloc_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_malloc"),
        "Rust mi_malloc export gate must stay paired with the assembly fast override"
    );
    assert!(
        alloc_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_calloc")
            && override_rs.contains("cfg(all(not(windows), not(mi_c_malloc_fast)))")
            && override_rs.contains("export_name = \"calloc\""),
        "Rust calloc/mi_calloc export gates must stay paired with the assembly fast override"
    );
    assert!(
        override_rs.contains("cfg(all(not(windows), not(mi_c_malloc_fast)))")
            && override_rs.contains("export_name = \"malloc\""),
        "Rust malloc override gate must stay paired with the assembly fast override"
    );
    for export_name in [
        "reallocf",
        "strdup",
        "strndup",
        "malloc_good_size",
        "vfree",
        "_aligned_malloc",
        "__libc_realloc",
        "__libc_cfree",
        "__libc_valloc",
        "__libc_pvalloc",
        "__libc_memalign",
        "__posix_memalign",
    ] {
        assert!(
            override_rs.contains(&format!("export_name = \"{export_name}\"")),
            "Rust Unix/Linux override surface should export {export_name} like C mimalloc"
        );
    }
    assert!(
        override_rs.contains(".weak reallocarr")
            && override_rs.contains(".set reallocarr, mi_reallocarr")
            && override_rs.contains("export_name = \"__libc_malloc\"")
            && override_rs.contains("export_name = \"__libc_calloc\"")
            && override_rs.contains("export_name = \"__libc_free\""),
        "Rust override surface should keep C's libc aliases and weak reallocarr compatibility"
    );
    assert!(
        aligned_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[unsafe(no_mangle)]")
            && aligned_rs.contains("#[cfg(mi_c_malloc_fast)]\n#[unsafe(naked)]")
            && aligned_rs.contains("pub unsafe extern \"C\" fn mi_malloc_aligned")
            && aligned_rs.contains("\"endbr64\"")
            && aligned_rs.contains("__mi_theap_default@GOTTPOFF")
            && aligned_rs.contains("target = sym mi_theap_malloc_aligned"),
        "release mi_malloc_aligned should load the default heap from direct TLS like C"
    );
    assert!(
        override_rs.contains("\"free:\"")
            && override_rs.contains("\"sub word ptr [rdi + 0x10], 1\"")
            && override_rs.contains("\"mov qword ptr [rdi + 0x18], rsi\"")
            && override_rs.contains("\"je 7f\"")
            && override_rs.contains("\"3:\"")
            && override_rs.contains("\"ret\"")
            && override_rs.contains("\"cmp byte ptr [rdi + 0x16], 0\"")
            && override_rs.contains("\"jne 3b\"")
            && override_rs.contains(".set mi_free, free")
            && free_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))"),
        "Unix free/mi_free exports must keep the C-like aligned assembly fast override and hot local-free branch layout"
    );
    assert!(
        free_rs.contains("#[cfg(mi_c_malloc_fast)]\ncore::arch::global_asm!(")
            && free_rs.contains("\".globl mi_free_size\"")
            && free_rs.contains("\"mi_free_size:\"")
            && free_rs.contains("\"jmp mi_free\"")
            && free_rs.contains("\".globl mi_free_size_aligned\"")
            && free_rs.contains("\"mi_free_size_aligned:\"")
            && free_rs.contains("\"jmp mi_free_size\"")
            && free_rs.contains("\".globl mi_free_aligned\"")
            && free_rs.contains("\"mi_free_aligned:\"")
            && free_rs
                .matches("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
                .count()
                >= 4,
        "release free-size/aligned public variants should remain C-shaped forwarding stubs in the fast build"
    );
}

#[test]
fn aligned_os_hints_are_randomized_like_c_release_builds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let os_rs =
        fs::read_to_string(root.join("src/platform/os.rs")).expect("read src/platform/os.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");

    assert!(
        os_rs.contains("let mut init = MI_HINT_BASE;")
            && os_rs.contains("if cfg!(feature = \"secure\") || cfg!(not(debug_assertions))")
            && os_rs.contains("let theap = unsafe { _mi_theap_default() };")
            && os_rs.contains("if unsafe { !mi_theap_is_initialized(theap) }")
            && os_rs.contains("return core::ptr::null_mut();")
            && os_rs.contains("let r = unsafe { crate::theap::_mi_theap_random_next(theap) };")
            && os_rs.contains("init += (MI_HINT_ALIGN * ((r >> 17) & 0xFFFFF)) % MI_HINT_AREA;")
            && os_rs.contains("hint.saturating_add(size),\n            init,"),
        "aligned OS allocation hints should randomize the base from the thread heap like C mimalloc"
    );
    assert!(
        init_rs.contains(
            "crate::random::_mi_random_reinit_if_weak(ptr::addr_of_mut!(theap_main.random))"
        ),
        "process initialization should reinitialize a weak main random context like C mimalloc"
    );
}

#[test]
fn cxx_new_wrappers_export_itanium_abi_symbols() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let override_rs =
        fs::read_to_string(root.join("src/raw/macros.rs")).expect("read src/raw/macros.rs");

    assert!(
        override_rs.contains("export_name = \"_Znwm\"")
            && override_rs.contains("export_name = \"_Znam\"")
            && override_rs.contains("not(mi_c_malloc_fast)")
            && override_rs.contains("not(mi_cxx_shim_linked)")
            && override_rs.contains("export_name = \"_ZnwmRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamRKSt9nothrow_t\""),
        "64-bit Rust C++ operator new fallbacks must export Itanium ABI symbols only when the C++ shim is not linked"
    );
    assert!(
        override_rs.contains("export_name = \"_ZnwmSt11align_val_t\"")
            && override_rs.contains("export_name = \"_ZnamSt11align_val_t\"")
            && override_rs.contains("export_name = \"_ZnwmSt11align_val_tRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamSt11align_val_tRKSt9nothrow_t\""),
        "aligned C++ operator new wrappers must export Itanium ABI symbols"
    );
}

#[test]
fn linux_cdylib_export_script_declares_allocator_symbols() {
    let build_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
        .expect("read build.rs");

    assert!(
        build_rs.contains("if c_abi")
            && build_rs.contains("cargo:rustc-link-arg=-Wl,--version-script="),
        "Linux C ABI artifact builds should pass the allocator export script through the cdylib link step"
    );
    assert!(
        build_rs.contains("PUBLIC_HEADERS")
            && build_rs.contains("CXX_EXPORT_PATTERNS")
            && build_rs.contains("MALLOC_EXPORT_PATTERNS")
            && build_rs.contains("public_mi_symbols_from_headers")
            && build_rs.contains("if cxx_shim_linked")
            && build_rs.contains("if interpose")
            && build_rs.contains("exports.push_str(&symbol)")
            && !build_rs.contains("mi_*;")
            && build_rs.contains("local:\n    *;"),
        "version script should export exact public-header mi_* APIs while hiding unmatched internals"
    );
    assert!(
        !build_rs.contains("_Z*;")
            && build_rs.contains("\"_Zda*\"")
            && build_rs.contains("\"_Zdl*\"")
            && build_rs.contains("\"_Zna*\"")
            && build_rs.contains("\"_Znw*\"")
            && build_rs.contains("\"malloc*\"")
            && build_rs.contains("\"free*\"")
            && build_rs.contains("\"realloc*\""),
        "version script must keep allocator and C++ ABI override names available only through mode groups"
    );
}
