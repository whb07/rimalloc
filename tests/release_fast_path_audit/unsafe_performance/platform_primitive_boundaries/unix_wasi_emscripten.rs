use super::*;

#[test]
fn wasi_primitive_backend_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wasi_rs =
        fs::read_to_string(root.join("src/platform/wasi.rs")).expect("read src/platform/wasi.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/wasi.rs\t")),
        "WASI primitive backend should stay off the unsafe migration allowlist"
    );
    assert!(
        !wasi_rs.contains("SAFETY[AUDIT:wasi-primitive-boundary]"),
        "WASI primitive backend markers should stay split by operation"
    );
    for (audit_id, exact_count) in [
        ("wasi-memory-growth-boundary", 13),
        ("wasi-range-primitive-boundary", 12),
        ("wasi-io-query-boundary", 11),
        ("wasi-thread-shim-boundary", 6),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            wasi_rs.matches(&marker).count(),
            exact_count,
            "WASI primitive marker {audit_id} should keep exact operation count"
        );
    }
    assert!(
        wasi_rs.contains("fn getenv(name: *const c_char) -> *const c_char;")
            && wasi_rs.contains("#[cfg(target_os = \"wasi\")]")
            && wasi_rs.contains("fn sbrk(increment: isize) -> *mut c_void;"),
        "WASI primitive backend should keep bounded C import declarations"
    );

    let memory_grow = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe fn mi_memory_grow(",
    );
    assert!(
        memory_grow.contains("isize::try_from(size)")
            && memory_grow.contains("unsafe { sbrk(increment) }")
            && memory_grow.contains("if p as isize == -1 { null_mut() } else { p }")
            && memory_grow.contains("#[cfg(not(target_os = \"wasi\"))]")
            && memory_grow.contains("null_mut()"),
        "WASI memory growth should preserve checked sbrk conversion and non-WASI null fallback"
    );

    let primitive_grow = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "unsafe fn mi_prim_mem_grow(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/wasi.rs",
        "unsafe fn mi_prim_mem_grow",
        primitive_grow,
    );
    assert!(
        primitive_grow.contains("try_alignment <= 1")
            && primitive_grow.contains("_mi_align_up_ptr(current, try_alignment)")
            && primitive_grow.contains("_mi_os_page_size_typed().get()")
            && primitive_grow.contains("saturating_add(size)")
            && primitive_grow.contains("unsafe { mi_memory_grow(alloc_size) }"),
        "WASI primitive growth should keep allocation-free alignment and bounds checks"
    );

    let mem_init = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe extern \"C\" fn _mi_prim_mem_init(",
    );
    assert!(
        mem_init.contains("let Some(config) = (unsafe { config.as_mut() }) else")
            && mem_init.contains("config.page_size = 64 * crate::bits::MI_KiB;")
            && mem_init.contains("config.alloc_granularity = 16")
            && mem_init.contains("config.has_virtual_reserve = false"),
        "WASI mem init should null-check config and initialize primitive constants"
    );

    let prim_alloc = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe extern \"C\" fn _mi_prim_alloc(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/wasi.rs",
        "pub unsafe extern \"C\" fn _mi_prim_alloc",
        prim_alloc,
    );
    assert!(
        prim_alloc.contains("if let Some(is_large) = unsafe { is_large.as_mut() }")
            && prim_alloc.contains("if let Some(is_zero) = unsafe { is_zero.as_mut() }")
            && prim_alloc.contains("let p = unsafe { mi_prim_mem_grow(size, try_alignment) };")
            && prim_alloc.contains("if let Some(addr) = unsafe { addr.as_mut() }")
            && prim_alloc.contains("if p.is_null() { ENOMEM } else { 0 }"),
        "WASI primitive allocation should preserve nullable out-pointer handling and ENOMEM fallback"
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
        let item = source_item("src/platform/wasi.rs", &wasi_rs, marker);
        assert_no_rust_heap_constructs("src/platform/wasi.rs", marker, item);
        assert!(
            item.contains(expected) && item.contains("\n    0\n}"),
            "{marker} should stay a no-allocation compatibility shim with nullable out-pointer writes"
        );
    }

    let huge_pages = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages(",
    );
    assert!(
        huge_pages.contains("if let Some(is_zero) = unsafe { is_zero.as_mut() }")
            && huge_pages.contains("*is_zero = true;")
            && huge_pages.contains("if let Some(addr) = unsafe { addr.as_mut() }")
            && huge_pages.contains("*addr = null_mut();")
            && huge_pages.contains("ENOSYS"),
        "WASI huge-page primitive should publish unsupported results through nullable out-pointers"
    );

    let stderr = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe extern \"C\" fn _mi_prim_out_stderr(",
    );
    assert!(
        stderr.contains("if msg.is_null()")
            && stderr.contains("CStr::from_ptr(msg)")
            && stderr.contains("stderr.write_all(bytes)")
            && stderr.contains("stderr.flush()"),
        "WASI stderr primitive should null-check and write bounded C-string bytes"
    );

    let getenv = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe extern \"C\" fn _mi_prim_getenv(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/wasi.rs",
        "pub unsafe extern \"C\" fn _mi_prim_getenv",
        getenv,
    );
    assert!(
        getenv
            .contains("name.is_null() || result.is_null() || result_size == 0 || _mi_preloading()")
            && getenv.contains("let mut buf = [0 as c_char; 65];")
            && getenv.contains("_mi_strnlen(name, buf.len() - 1)")
            && getenv.contains("let ch = unsafe { *name.add(i) };")
            && getenv.contains("buf[len] = 0;")
            && getenv.contains("s = unsafe { getenv(buf.as_ptr()) };")
            && getenv.contains("unsafe { _mi_strnlen(s, result_size) } >= result_size")
            && getenv.contains("unsafe { _mi_strlcpy(result, s, result_size) };"),
        "WASI getenv primitive should preserve preloading guard, uppercase fallback, and bounded copy"
    );

    let thread_assoc = source_item(
        "src/platform/wasi.rs",
        &wasi_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
    );
    assert!(
        thread_assoc.contains("let _ = theap;")
            && !thread_assoc.contains("*theap")
            && !thread_assoc.contains("as_mut"),
        "WASI thread association shim should not dereference default thread heap pointers"
    );
}

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
        ("emscripten-memory-boundary", 20),
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
