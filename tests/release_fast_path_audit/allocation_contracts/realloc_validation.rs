use std::fs;
use std::path::Path;

#[test]
fn realloc_validation_uses_pointer_lookup_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let aligned_rs =
        fs::read_to_string(root.join("src/allocator/aligned.rs")).expect("read alloc_aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    assert!(
        alloc_rs.contains(
            "unsafe fn mi_validate_ptr_page(\n    p: *const c_void,\n    msg: *const c_char,\n) -> crate::core_types::PointerLookup"
        ) && alloc_rs.contains("crate::free::mi_validate_pointer(p.cast_mut(), msg)")
            && alloc_rs.contains("unsafe fn mi_realloc_old_allocation(")
            && alloc_rs.contains("crate::core_types::PointerLookup::Valid(valid)")
            && alloc_rs.contains("let (user, page, location) = valid.into_parts();")
            && alloc_rs.contains("let block = location.block();")
            && alloc_rs.contains("Some(crate::core_types::OldAllocation::from_validated_parts(")
            && alloc_rs.contains("unsafe fn mi_realloc_request_from_ptr(")
            && alloc_rs
                .contains("let Some(old) = mi_realloc_old_allocation(p, c\"mi_realloc\".as_ptr())")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_validated_core(")
            && alloc_rs.contains("zero_new_tail: crate::core_types::Zeroed,")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_null(")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_existing(")
            && alloc_rs.contains("req.old(),")
            && alloc_rs.contains("let page = old.page().as_ptr();")
            && alloc_rs.contains("let size = old.usable();")
            && alloc_rs
                .contains("crate::core_types::PreserveBytes::new(old.usable(), new_requested)")
            && alloc_rs.contains("let preserved = preserve.map_or(0, |preserve| preserve.get());")
            && alloc_rs.contains("let Some(preserve) = preserve else")
            && alloc_rs.contains("let copysize = preserve.get();")
            && !alloc_rs.contains("let copysize = if newsize > size { size } else { newsize };")
            && aligned_rs.contains("let preserve = crate::core_types::PreserveBytes::new(")
            && aligned_rs.contains("crate::core_types::UsableSize::new(size)")
            && aligned_rs.contains("_mi_memcpy_aligned(newp, p, preserve.get())")
            && !aligned_rs.contains("usize::min(newsize, size)")
            && alloc_rs.contains("crate::core_types::PointerLookup::Invalid { .. }")
            && layout_rs.contains("pub(crate) struct OldAllocation")
            && layout_rs.contains("pub(crate) struct PreserveBytes(usize);")
            && layout_rs.contains("Self(usize::min(old.get(), new.get()))")
            && layout_rs.contains("unsafe fn new(")
            && !layout_rs.contains("pub(crate) unsafe fn new(")
            && layout_rs.contains("pub(crate) unsafe fn from_validated_parts(")
            && layout_rs.contains("unsafe { Self::new(ptr, usable, page, block) }")
            && layout_rs.contains("pub(crate) fn usable(self) -> UsableSize")
            && layout_rs.contains("pub(crate) fn page(self) -> PagePtr")
            && layout_rs.contains("enum ReallocSource")
            && layout_rs.contains("ReallocSource::Null")
            && layout_rs.contains("ReallocSource::Existing(old)")
            && layout_rs.contains("pub(crate) fn from_null(")
            && layout_rs.contains("pub(crate) fn from_existing(")
            && layout_rs.contains("pub(crate) fn old(&self) -> Option<OldAllocation>")
            && !layout_rs.contains("old: Option<OldAllocation>")
            && !layout_rs.contains("with_old")
            && !alloc_rs.contains("unsafe fn mi_validate_ptr_page(p: *const c_void, msg: *const c_char) -> *mut mi_page_t")
            && !alloc_rs.contains("_mi_safe_ptr_page"),
        "realloc must use PointerLookup and construct OldAllocation only after validation"
    );
}
