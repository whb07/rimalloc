use super::*;

#[test]
fn emscripten_primitive_backend_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emscripten_rs = fs::read_to_string(root.join("src/platform/emscripten.rs"))
        .expect("read src/platform/emscripten.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/emscripten.rs\t")),
        "Emscripten primitive backend should stay off the unsafe migration allowlist"
    );
    assert!(
        !emscripten_rs.contains("SAFETY[AUDIT:emscripten-primitive-boundary]"),
        "Emscripten primitive backend markers should stay split by operation"
    );
    for (audit_id, exact_count) in [
        ("emscripten-import-boundary", 2),
        ("emscripten-allocation-boundary", 10),
        ("emscripten-memory-operation-boundary", 10),
        ("emscripten-clock-io-boundary", 9),
        ("emscripten-pthread-key-boundary", 14),
        ("emscripten-thread-shim-boundary", 3),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            emscripten_rs.matches(&marker).count(),
            exact_count,
            "Emscripten primitive marker {audit_id} should keep exact operation count"
        );
    }
    for import in [
        "fn emmalloc_free(p: *mut c_void);",
        "fn emmalloc_memalign(alignment: usize, size: usize) -> *mut c_void;",
        "fn emscripten_date_now() -> f64;",
        "fn emscripten_console_error(msg: *const c_char);",
        "fn getentropy(buf: *mut c_void, len: usize) -> c_int;",
        "fn pthread_key_create(",
        "fn pthread_key_delete(key: pthread_key_t) -> c_int;",
        "fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;",
        "fn sched_yield() -> c_int;",
    ] {
        assert!(
            emscripten_rs.contains(import),
            "Emscripten primitive import missing: {import}"
        );
    }

    let mem_init = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_mem_init(",
    );
    assert!(
        mem_init.contains("let Some(config) = (unsafe { config.as_mut() }) else")
            && mem_init.contains("config.page_size = 64 * MI_KiB;")
            && mem_init.contains("config.alloc_granularity = 16")
            && mem_init.contains("config.has_virtual_reserve = false"),
        "Emscripten mem init should null-check config and initialize primitive constants"
    );

    let prim_free = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_free(",
    );
    assert!(
        prim_free.contains("if size == 0")
            && prim_free.contains("unsafe { emmalloc_free(addr) };")
            && prim_free.contains("#[cfg(not(target_os = \"emscripten\"))]"),
        "Emscripten primitive free should preserve size-zero no-op and non-Emscripten fallback"
    );

    let prim_alloc = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_alloc(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/emscripten.rs",
        "pub unsafe extern \"C\" fn _mi_prim_alloc",
        prim_alloc,
    );
    assert!(
        prim_alloc.contains("if let Some(is_large) = unsafe { is_large.as_mut() }")
            && prim_alloc.contains("if let Some(is_zero) = unsafe { is_zero.as_mut() }")
            && prim_alloc.contains("if try_alignment < MIN_EMMALLOC_ALIGN")
            && prim_alloc.contains("unsafe { emmalloc_memalign(try_alignment, size) }")
            && prim_alloc.contains("if let Some(addr) = unsafe { addr.as_mut() }")
            && prim_alloc.contains("*addr = core::ptr::null_mut();")
            && prim_alloc.contains("ENOMEM"),
        "Emscripten primitive allocation should preserve alignment clamp, nullable out-pointers, and ENOMEM fallback"
    );

    for (marker, expected) in [
        (
            "pub unsafe extern \"C\" fn _mi_prim_commit(",
            "if let Some(is_zero) = unsafe { is_zero.as_mut() }",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_decommit(",
            "if let Some(needs_recommit) = unsafe { needs_recommit.as_mut() }",
        ),
    ] {
        let item = source_item("src/platform/emscripten.rs", &emscripten_rs, marker);
        assert_no_rust_heap_constructs("src/platform/emscripten.rs", marker, item);
        assert!(
            item.contains(expected) && item.contains("\n    0\n}"),
            "{marker} should stay a no-allocation compatibility shim with nullable out-pointer writes"
        );
    }

    let huge_pages = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages(",
    );
    assert!(
        huge_pages.contains("if let Some(is_zero) = unsafe { is_zero.as_mut() }")
            && huge_pages.contains("*is_zero = true;")
            && huge_pages.contains("if let Some(addr) = unsafe { addr.as_mut() }")
            && huge_pages.contains("*addr = core::ptr::null_mut();")
            && huge_pages.contains("ENOSYS"),
        "Emscripten huge-page primitive should publish unsupported results through nullable out-pointers"
    );

    let clock = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_clock_now(",
    );
    assert!(
        clock.contains("unsafe { emscripten_date_now() as mi_msecs_t }")
            && clock.contains("SystemTime::now()")
            && clock.contains("duration_since(UNIX_EPOCH)")
            && clock.contains(".unwrap_or(0)"),
        "Emscripten clock primitive should keep platform import and host fallback"
    );

    let stderr = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_out_stderr(",
    );
    assert!(
        stderr.contains("if msg.is_null()")
            && stderr.contains("unsafe { emscripten_console_error(msg) };")
            && stderr.contains("CStr::from_ptr(msg)")
            && stderr.contains("stderr.write_all(bytes)")
            && stderr.contains("stderr.flush()"),
        "Emscripten stderr primitive should null-check and preserve console/host fallback output"
    );

    let getenv = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_getenv(",
    );
    assert!(
        getenv.contains("let _ = name;")
            && getenv.contains("let _ = result;")
            && getenv.contains("let _ = result_size;")
            && getenv.contains("false")
            && !getenv.contains("*name")
            && !getenv.contains("*result"),
        "Emscripten getenv primitive should stay an unsupported non-dereferencing shim"
    );

    let pthread_done = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn mi_pthread_done(",
    );
    assert!(
        pthread_done.contains("if value.is_null()")
            && pthread_done.contains(
                "unsafe { crate::internal::_mi_thread_done(value.cast::<mi_theap_t>()) };"
            ),
        "Emscripten pthread destructor should null-check and call thread teardown through the local boundary"
    );

    let thread_init = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
    );
    assert!(
        thread_init.contains(
            "let key_is_invalid = unsafe { _mi_heap_default_key == PTHREAD_KEY_INVALID };"
        ) && thread_init.contains("pthread_key_create(&mut key, Some(mi_pthread_done))")
            && thread_init.contains("unsafe { _mi_heap_default_key = key };")
            && thread_init.contains("unsafe { _mi_heap_default_key = 0 };"),
        "Emscripten thread init should preserve key creation, destructor registration, and fallback key sentinel"
    );

    let thread_done = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
    );
    assert!(
        thread_done.contains("let key = unsafe { _mi_heap_default_key };")
            && thread_done.contains("unsafe { pthread_key_delete(key) }")
            && thread_done
                .contains("MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(ptr::null_mut()))")
            && thread_done.contains("unsafe { _mi_heap_default_key = PTHREAD_KEY_INVALID };"),
        "Emscripten thread teardown should snapshot/delete the key and clear fallback TLS"
    );

    let thread_assoc = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
    );
    assert!(
        thread_assoc.contains("let key = unsafe { _mi_heap_default_key };")
            && thread_assoc
                .contains("unsafe { pthread_setspecific(key, theap.cast_const().cast()) }")
            && thread_assoc
                .contains("MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(theap.cast()))"),
        "Emscripten thread association should snapshot the key and preserve pthread/fallback storage"
    );

    let thread_yield = source_item(
        "src/platform/emscripten.rs",
        &emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_yield(",
    );
    assert!(
        thread_yield.contains("let _ = unsafe { sched_yield() };")
            && thread_yield.contains("std::thread::yield_now();"),
        "Emscripten thread yield should preserve scheduler import and host fallback"
    );
}
