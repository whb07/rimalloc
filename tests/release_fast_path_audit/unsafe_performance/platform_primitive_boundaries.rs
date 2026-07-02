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
