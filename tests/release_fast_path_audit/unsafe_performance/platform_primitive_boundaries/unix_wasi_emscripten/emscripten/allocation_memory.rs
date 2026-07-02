use super::*;

pub(super) fn assert_emscripten_allocation_and_memory(emscripten_rs: &str) {
    let mem_init = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
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
        emscripten_rs,
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
        emscripten_rs,
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
        let item = source_item("src/platform/emscripten.rs", emscripten_rs, marker);
        assert_no_rust_heap_constructs("src/platform/emscripten.rs", marker, item);
        assert!(
            item.contains(expected) && item.contains("\n    0\n}"),
            "{marker} should stay a no-allocation compatibility shim with nullable out-pointer writes"
        );
    }

    let huge_pages = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
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
}
