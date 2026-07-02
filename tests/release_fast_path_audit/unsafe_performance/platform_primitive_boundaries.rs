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
        wasi_rs
            .matches("SAFETY[AUDIT:wasi-primitive-boundary]")
            .count()
            >= 40,
        "WASI primitive backend should keep local operation-level audit markers"
    );
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
fn macos_malloc_zone_override_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let macos_rs =
        fs::read_to_string(root.join("src/platform/macos.rs")).expect("read src/platform/macos.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/macos.rs\t")),
        "macOS malloc-zone override should stay off the unsafe migration allowlist"
    );
    assert!(
        macos_rs
            .matches("SAFETY[AUDIT:macos-zone-override]")
            .count()
            >= 55,
        "macOS malloc-zone override should keep local operation-level audit markers"
    );
    assert!(
        macos_rs.contains("pub static mut mi_malloc_zone: malloc_zone_t = malloc_zone_t {")
            && macos_rs.contains("zone_name: MI_MALLOC_ZONE_NAME.as_ptr().cast(),")
            && macos_rs.contains("pub static _mi_zone_interposes: [mi_interpose_s; 27]")
            && macos_rs.contains("#[unsafe(link_section = \"__DATA,__interpose\")]")
            && macos_rs.contains("#[unsafe(link_section = \"__DATA,__mod_init_func\")]"),
        "macOS malloc-zone source should preserve zone metadata and section-based interposition shape"
    );
    for import in [
        "fn malloc_default_zone() -> *mut malloc_zone_t;",
        "fn malloc_default_purgeable_zone() -> *mut malloc_zone_t;",
        "fn malloc_get_all_zones(",
        "fn malloc_zone_register(zone: *mut malloc_zone_t);",
        "fn malloc_zone_unregister(zone: *mut malloc_zone_t);",
        "fn malloc_zone_batch_free(zone: *mut malloc_zone_t, ps: *mut *mut c_void, count: u32);",
        "fn malloc_zone_batch_malloc(",
    ] {
        assert!(
            macos_rs.contains(import),
            "macOS malloc-zone import missing: {import}"
        );
    }

    let zone_struct = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub struct malloc_zone_t",
    );
    assert!(
        zone_struct.contains("pub size: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub malloc: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub calloc: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub valloc: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub free: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub realloc:")
            && zone_struct.contains("pub destroy: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub zone_name: *const c_char")
            && zone_struct.contains("pub batch_malloc:")
            && zone_struct.contains("pub batch_free: Option<unsafe extern \"C\" fn"),
        "malloc_zone_t should keep the Darwin callback layout and raw C ABI slots"
    );

    for (marker, expected) in [
        (
            "pub unsafe extern \"C\" fn mi_malloc_create_zone(",
            "unsafe { mi_get_default_zone() }",
        ),
        (
            "pub unsafe extern \"C\" fn mi_malloc_default_zone()",
            "unsafe { mi_get_default_zone() }",
        ),
        (
            "pub unsafe extern \"C\" fn mi_malloc_default_purgeable_zone()",
            "unsafe { mi_get_default_zone() }",
        ),
    ] {
        let item = source_item("src/platform/macos.rs", &macos_rs, marker);
        assert_no_rust_heap_constructs("src/platform/macos.rs", marker, item);
        assert!(
            item.contains(expected),
            "{marker} should continue returning the process-global mimalloc zone"
        );
    }

    let all_zones = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn mi_malloc_get_all_zones(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/macos.rs",
        "pub unsafe extern \"C\" fn mi_malloc_get_all_zones",
        all_zones,
    );
    assert!(
        all_zones.contains("if !addresses.is_null()")
            && all_zones.contains("*addresses = null_mut();")
            && all_zones.contains("if !count.is_null()")
            && all_zones.contains("*count = 0;")
            && all_zones.contains("KERN_SUCCESS"),
        "macOS all-zones shim should preserve nullable out-pointer writes and success result"
    );

    let zone_size = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_size(",
    );
    assert!(
        zone_size.contains("mi_any_heap_contains(p)")
            && zone_size.contains("crate::mimalloc::mi_usable_size(p)")
            && zone_size.contains("!unsafe { is_mimalloc_zone(zone) }")
            && zone_size.contains("match unsafe { (*zone).size }")
            && zone_size.contains("Some(size_fn) => unsafe { size_fn(zone, p) }"),
        "zone_size should preserve mimalloc ownership detection and foreign-zone forwarding"
    );

    let zone_free = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_free(",
    );
    assert!(
        zone_free.contains("mi_any_heap_contains(p.cast_const())")
            && zone_free.contains("crate::mimalloc::mi_free(p)")
            && zone_free.contains("!unsafe { is_mimalloc_zone(zone) }")
            && zone_free.contains("let Some(free_fn) = unsafe { (*zone).free }")
            && zone_free.contains("unsafe { free_fn(zone, p) };"),
        "zone_free should keep mimalloc local free and non-mimalloc zone forwarding"
    );

    let zone_realloc = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_realloc(",
    );
    assert!(
        zone_realloc.contains("if p.is_null() || unsafe { mi_any_heap_contains(p.cast_const()) }")
            && zone_realloc.contains("crate::mimalloc::mi_realloc(p, newsize)")
            && zone_realloc.contains("match unsafe { (*zone).realloc }")
            && zone_realloc.contains("Some(realloc_fn) => unsafe { realloc_fn(zone, p, newsize) }")
            && zone_realloc.contains("None => null_mut()"),
        "zone_realloc should preserve null/mimalloc routing and foreign-zone fallback"
    );

    let batch_malloc = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_batch_malloc(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/macos.rs",
        "pub unsafe extern \"C\" fn zone_batch_malloc",
        batch_malloc,
    );
    assert!(
        batch_malloc.contains("while i < count")
            && batch_malloc.contains("let p = unsafe { zone_malloc(zone, size) };")
            && batch_malloc.contains("unsafe { *ps.add(i as usize) = p };")
            && batch_malloc.contains("if p.is_null()")
            && batch_malloc.contains("i += 1;"),
        "zone_batch_malloc should fill caller pointer slots and stop at first failure"
    );

    let batch_free = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn zone_batch_free(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/macos.rs",
        "pub unsafe extern \"C\" fn zone_batch_free",
        batch_free,
    );
    assert!(
        batch_free.contains("let p = unsafe { *ps.add(i as usize) };")
            && batch_free.contains("unsafe { zone_free(zone, p) };")
            && batch_free.contains("unsafe { *ps.add(i as usize) = null_mut() };")
            && batch_free.contains("i += 1;"),
        "zone_batch_free should free each caller slot and clear it afterward"
    );

    for symbol in [
        "pub unsafe extern \"C\" fn _malloc_fork_prepare()",
        "pub unsafe extern \"C\" fn _malloc_fork_parent()",
        "pub unsafe extern \"C\" fn _malloc_fork_child()",
    ] {
        let item = source_item("src/platform/macos.rs", &macos_rs, symbol);
        assert!(
            item.contains("#[unsafe(no_mangle)]")
                || macos_rs[..macos_rs.find(symbol).expect("fork hook symbol present")]
                    .rsplit("\n\n")
                    .next()
                    .is_some_and(|prefix| prefix.contains("#[unsafe(no_mangle)]")),
            "{symbol} should keep the exported Darwin fork-hook symbol"
        );
    }

    let override_malloc = source_item(
        "src/platform/macos.rs",
        &macos_rs,
        "pub unsafe extern \"C\" fn _mi_macos_override_malloc(",
    );
    assert_no_rust_heap_constructs(
        "src/platform/macos.rs",
        "pub unsafe extern \"C\" fn _mi_macos_override_malloc",
        override_malloc,
    );
    assert!(
        override_malloc.contains("malloc_zone_register(addr_of_mut!(mi_malloc_zone))")
            && override_malloc
                .contains("malloc_get_all_zones(0, null_mut(), &mut zones, &mut count)")
            && override_malloc.contains("ret == KERN_SUCCESS && count > 0 && !zones.is_null()")
            && override_malloc.contains("unsafe { *zones }")
            && override_malloc.contains("unsafe { malloc_default_zone() }")
            && override_malloc.contains("malloc_zone_unregister(default_zone)")
            && override_malloc.contains("malloc_zone_register(default_zone)")
            && override_malloc.contains("if !purgeable_zone.is_null()")
            && override_malloc.contains("malloc_zone_unregister(purgeable_zone)")
            && override_malloc.contains("malloc_zone_register(purgeable_zone)"),
        "macOS override setup should preserve default and purgeable zone ordering"
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
        emscripten_rs
            .matches("SAFETY[AUDIT:emscripten-primitive-boundary]")
            .count()
            >= 45,
        "Emscripten primitive backend should keep local operation-level audit markers"
    );
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

#[test]
fn windows_primitive_backend_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let windows_rs = fs::read_to_string(root.join("src/platform/windows.rs"))
        .expect("read src/platform/windows.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/windows.rs\t")),
        "Windows primitive backend should stay off the unsafe migration allowlist"
    );
    assert!(
        !windows_rs.contains("SAFETY[AUDIT:windows-primitive-boundary]"),
        "Windows primitive backend aggregate marker should stay split"
    );
    for (marker, minimum_count) in [
        ("windows-virtual-memory-boundary", 32),
        ("windows-tls-fls-boundary", 27),
        ("windows-large-page-boundary", 22),
        ("windows-process-info-boundary", 11),
        ("windows-numa-boundary", 8),
        ("windows-output-env-boundary", 7),
        ("windows-etw-boundary", 4),
        ("windows-random-boundary", 2),
        ("windows-dynamic-import-boundary", 1),
    ] {
        let needle = format!("SAFETY[AUDIT:{marker}]");
        let count = windows_rs.matches(needle.as_str()).count();
        assert!(
            count >= minimum_count,
            "Windows primitive audit marker `{marker}` should have at least {minimum_count} source markers, found {count}"
        );
    }
    for import in [
        "pub fn VirtualAlloc(",
        "pub fn VirtualFree(lpAddress: PVOID, dwSize: SIZE_T, dwFreeType: DWORD) -> BOOL;",
        "pub fn VirtualProtect(",
        "pub fn VirtualQuery(",
        "pub fn GetProcessTimes(",
        "pub fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;",
        "pub fn WriteConsoleA(",
        "pub fn WriteFile(",
        "pub fn LoadLibraryA(lpLibFileName: LPCSTR) -> HINSTANCE;",
        "pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> FARPROC;",
        "fn atexit(func: Option<unsafe extern \"C\" fn()>) -> c_int;",
        "fn NtCurrentTeb() -> *mut c_void;",
        "pub fn EventWriteTransfer(",
        "pub fn EventRegister(",
        "pub fn EventUnregister(RegHandle: REGHANDLE) -> ULONG;",
    ] {
        assert!(
            windows_rs.contains(import),
            "Windows primitive import missing: {import}"
        );
    }

    let mem_init = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_mem_init(",
    );
    assert!(
        mem_init.contains("if config.is_null()")
            && mem_init.contains("GetSystemInfo(&mut si)")
            && mem_init.contains("config).has_virtual_reserve = true")
            && mem_init.contains("LoadLibraryA(c\"kernelbase.dll\".as_ptr())")
            && mem_init.contains("pVirtualAlloc2 = mi_get_proc")
            && mem_init.contains("pNtAllocateVirtualMemoryEx = mi_get_proc")
            && mem_init.contains("pGetLargePageMinimum = mi_get_proc")
            && mem_init.contains("win_enable_large_os_pages(&mut (*config).large_page_size)"),
        "Windows mem init should preserve system-info setup, dynamic primitive lookup, and large-page probing"
    );

    let alloc = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_alloc(",
    );
    assert!(
        alloc.contains("size > 0 && size.is_multiple_of")
            && alloc.contains("let mut flags = MEM_RESERVE;")
            && alloc.contains("if commit {\n        flags |= MEM_COMMIT;")
            && alloc.contains("win_virtual_alloc(")
            && alloc.contains("if !is_zero.is_null()")
            && alloc.contains("if !addr.is_null()"),
        "Windows primitive allocation should preserve flag construction and nullable out-pointer writes"
    );

    let alloc_once = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "unsafe fn win_virtual_alloc_prim_once(",
    );
    assert!(
        alloc_once.contains("_mi_os_get_aligned_hint(try_alignment, size)")
            && alloc_once.contains("VirtualAlloc(hint, size, flags, PAGE_READWRITE)")
            && alloc_once.contains("try_alignment > unsafe { win_allocation_granularity }")
            && alloc_once.contains("let Some(virtual_alloc2) = unsafe { pVirtualAlloc2 }")
            && alloc_once.contains(
                "MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterAddressRequirements"
            )
            && alloc_once.contains("virtual_alloc2(")
            && alloc_once.contains("VirtualAlloc(addr, size, flags, PAGE_READWRITE)"),
        "Windows virtual allocation helper should keep hinted allocation, VirtualAlloc2 alignment, and VirtualAlloc fallback"
    );

    for (marker, expected) in [
        (
            "pub unsafe extern \"C\" fn _mi_prim_commit(",
            "VirtualAlloc(addr, size, MEM_COMMIT, PAGE_READWRITE)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_decommit(",
            "VirtualFree(addr, size, MEM_DECOMMIT)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_free(",
            "VirtualFree(addr, 0, MEM_RELEASE)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_reset(",
            "VirtualAlloc(addr, size, MEM_RESET, PAGE_READWRITE)",
        ),
        (
            "pub unsafe extern \"C\" fn _mi_prim_protect(",
            "VirtualProtect(",
        ),
    ] {
        let item = source_item("src/platform/windows.rs", &windows_rs, marker);
        assert!(
            item.contains(expected) && item.contains("GetLastError()"),
            "{marker} should preserve the Windows memory primitive and error code path"
        );
    }

    let huge_once = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "unsafe fn win_enable_large_os_pages_once(",
    );
    assert!(
        huge_once.contains("OpenProcessToken(")
            && huge_once.contains("LookupPrivilegeValueA(")
            && huge_once.contains("AdjustTokenPrivileges(")
            && huge_once.contains("pGetLargePageMinimum")
            && huge_once.contains("CloseHandle(token)"),
        "Windows large-page enablement should preserve token privilege and page-size probing"
    );

    let huge_alloc = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "unsafe fn _mi_prim_alloc_huge_os_pagesx(",
    );
    assert!(
        huge_alloc.contains("MEM_LARGE_PAGES | MEM_COMMIT | MEM_RESERVE")
            && huge_alloc.contains("pNtAllocateVirtualMemoryEx")
            && huge_alloc.contains("MI_MEM_EXTENDED_PARAMETER_NONPAGED_HUGE")
            && huge_alloc
                .contains("MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterNumaNode")
            && huge_alloc.contains("pVirtualAlloc2")
            && huge_alloc.contains("VirtualAlloc(hint_addr, size, flags, PAGE_READWRITE)"),
        "Windows huge-page allocation should preserve huge-page, NUMA, VirtualAlloc2, and fallback paths"
    );

    let process_info = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_process_info(",
    );
    assert!(
        process_info.contains("if pinfo.is_null()")
            && process_info.contains(
                "GetProcessTimes(GetCurrentProcess(), &mut ct, &mut et, &mut st, &mut ut)"
            )
            && process_info.contains("pGetProcessMemoryInfo = mi_get_proc")
            && process_info.contains("(*pinfo).current_rss = info.WorkingSetSize")
            && process_info.contains("(*pinfo).page_faults = info.PageFaultCount as usize"),
        "Windows process info should preserve nullable output, process times, and dynamic PSAPI counters"
    );

    let stderr = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_out_stderr(",
    );
    assert!(
        stderr.contains("if msg.is_null() || _mi_preloading()")
            && stderr.contains("GetStdHandle(STD_ERROR_HANDLE)")
            && stderr.contains("GetConsoleScreenBufferInfo(mi_stderr_handle, &mut sbi)")
            && stderr.contains("WriteConsoleA(")
            && stderr.contains("WriteFile(")
            && stderr.contains("std::io::stderr().lock()"),
        "Windows stderr primitive should preserve console, file, and host fallback output"
    );

    let getenv = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_getenv(",
    );
    assert!(
        getenv.contains("if result.is_null() || result_size == 0")
            && getenv.contains("unsafe { *result = 0 };")
            && getenv.contains("if name.is_null()")
            && getenv.contains("CStr::from_ptr(name)")
            && getenv.contains("mi_getenv_value(&key)")
            && getenv.contains(
                "copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), result, bytes.len())"
            )
            && getenv.contains("*result.add(bytes.len()) = 0;"),
        "Windows getenv primitive should preserve null checks, case-insensitive lookup, and bounded copy"
    );

    let fls_done = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"system\" fn mi_fls_done(",
    );
    assert!(
        fls_done.contains("if !theap.is_null()")
            && fls_done.contains("crate::internal::_mi_thread_done(theap)")
            && fls_done.contains("FlsSetValue(mi_fls_key, null_mut())"),
        "Windows FLS destructor should null-check and clear thread-local heap state"
    );

    let thread_init = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
    );
    let thread_done = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
    );
    let thread_assoc = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
    );
    assert!(
        thread_init.contains("FlsAlloc(Some(mi_fls_done))")
            && thread_done.contains("FlsFree(mi_fls_key)")
            && thread_done.contains("mi_fls_key = FLS_OUT_OF_INDEXES")
            && thread_assoc.contains("FlsSetValue(mi_fls_key, theap.cast::<c_void>())"),
        "Windows thread lifecycle should preserve FLS key creation, deletion, and heap association"
    );

    let threadpool = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool(",
    );
    assert!(
        threadpool.contains("NtCurrentTeb()")
            && threadpool.contains("if MI_SIZE_BITS == 32 { 0x0F90 } else { 0x1778 }")
            && threadpool.contains("return !pool_data.is_null();"),
        "Windows thread-pool detection should preserve TEB probing offsets"
    );

    let control = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe extern \"system\" fn McGenControlCallbackV2(",
    );
    let event_write = source_item(
        "src/platform/windows.rs",
        &windows_rs,
        "pub unsafe fn McGenEventWrite(",
    );
    assert!(
        control.contains("EVENT_CONTROL_CODE_ENABLE_PROVIDER")
            && control.contains("EVENT_CONTROL_CODE_DISABLE_PROVIDER")
            && control.contains("write_bytes(ctx_ref.EnableBitMask, 0, word_count)")
            && event_write.contains("EventWriteTransfer(")
            && windows_rs.contains("pub unsafe fn EventWriteETW_MI_ALLOC(")
            && windows_rs.contains("pub unsafe fn EventWriteETW_MI_FREE("),
        "Windows ETW helpers should preserve provider enablement, disable clearing, and event write forwarding"
    );
}
