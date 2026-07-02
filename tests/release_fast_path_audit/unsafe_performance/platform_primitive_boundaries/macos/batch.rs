use super::*;

pub(super) fn assert_macos_batch_boundaries(macos_rs: &str) {
    let batch_malloc = source_item(
        "src/platform/macos.rs",
        macos_rs,
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
        macos_rs,
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
}
