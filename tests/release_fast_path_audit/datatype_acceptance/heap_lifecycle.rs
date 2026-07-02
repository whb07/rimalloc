use super::DatatypeAcceptanceSources;

pub(super) fn assert_heap_lifecycle_acceptance(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let heap_delete_rs = audit_sources.source("src/allocator/heap.rs");
    let heap_delete_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_delete(heap: *mut mi_heap_t)")
        .nth(1)
        .expect("find mi_heap_delete body");
    let owned_reject = heap_delete_body
        .find("let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else")
        .expect("mi_heap_delete should reject the main heap through OwnedHeap");
    let delete_typed = heap_delete_body
        .find("unsafe { mi_heap_delete_typed(heap) };")
        .expect("mi_heap_delete should enter the typed delete path after validation");
    assert!(
        owned_reject < delete_typed,
        "mi_heap_delete must reject MainHeap before entering the OwnedHeap delete path"
    );
    let heap_delete_typed_body = heap_delete_rs
        .split("unsafe fn mi_heap_delete_typed(heap: OwnedHeap)")
        .nth(1)
        .expect("find mi_heap_delete_typed body");
    assert!(
        heap_delete_typed_body.contains("mi_heap_free_theaps(heap);")
            && heap_delete_typed_body.contains("_mi_heap_move_pages(heap_raw, mi_heap_main());")
            && heap_delete_typed_body.contains("mi_heap_free(heap);"),
        "mi_heap_delete_typed should free thread heaps through OwnedHeap, move pages, and free OwnedHeap"
    );
    assert!(
        heap_delete_rs.contains("unsafe fn mi_heap_delete_typed(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_free_theaps(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && !heap_delete_rs.contains("unsafe fn mi_heap_delete_typed(heap: MainHeap)")
            && !heap_delete_rs.contains("unsafe fn mi_heap_free(heap: MainHeap)"),
        "main heaps should have no typed entry point into owned deletion helpers"
    );
    let heap_destroy_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_destroy(heap: *mut mi_heap_t)")
        .nth(1)
        .expect("find mi_heap_destroy body");
    let destroy_main_reject = heap_destroy_body
        .find("let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else")
        .expect("mi_heap_destroy should reject the main heap through OwnedHeap");
    let force_destroy = heap_destroy_body
        .find("mi_heap_force_destroy_owned(heap)")
        .expect("mi_heap_destroy should force-destroy an OwnedHeap after validation");
    assert!(
        destroy_main_reject < force_destroy,
        "mi_heap_destroy must reject MainHeap before the owned destroy path"
    );
    assert!(
        heap_delete_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_force_destroy_main(heap: MainHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_force_destroy_owned(heap: OwnedHeap)")
            && heap_delete_rs.contains("if let Some(heap) = MainHeap::new(heap)")
            && heap_delete_rs.contains("} else if let Some(heap) = OwnedHeap::new(heap)"),
        "heap deletion helpers should use OwnedHeap/MainHeap typed teardown witnesses"
    );
    let heap_check_owned_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_check_owned(heap: *mut mi_heap_t, p: *const c_void) -> bool")
        .nth(1)
        .expect("find mi_heap_check_owned body");
    assert!(
        heap_check_owned_body.contains("BorrowedHeap::from_lookup_raw(heap)")
            && heap_check_owned_body.contains("unsafe { mi_heap_check_owned_typed(heap, p) }")
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_typed(heap: BorrowedHeap, p: *const c_void) -> bool"
            )
            && heap_delete_rs.contains("OwnedHeap::new(heap.ptr())")
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_owned(heap: OwnedHeap, p: *const c_void) -> bool"
            )
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_borrowed(heap: BorrowedHeap, p: *const c_void) -> bool"
            ),
        "mi_heap_check_owned should parse heap arguments through BorrowedHeap and route non-main checks through OwnedHeap"
    );
}
