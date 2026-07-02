use super::DatatypeAcceptanceSources;

pub(super) fn assert_pointer_provenance_boundaries(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let strict_check_rs = audit_sources.strict_type_checker;
    let allowlist_rs = audit_sources.strict_type_allowlist;
    let ptr_core_rs = audit_sources.source("src/types/ptr.rs");
    let freelist_rs = audit_sources.source("src/types/freelist.rs");
    let freelist_wrappers_rs = audit_sources.source("src/types/freelist_wrappers.rs");
    let internal_rs = audit_sources.source("src/internal/mod.rs");

    assert!(
        ptr_core_rs.contains("pub(crate) fn ptr_exposed_addr<T>(ptr: *const T) -> usize")
            && ptr_core_rs.contains("ptr as usize")
            && ptr_core_rs.contains("ptr.addr()")
            && ptr_core_rs.contains("pub(crate) fn ptr_addr_diff<T, U>(")
            && ptr_core_rs.contains("ptr_exposed_addr(hi).wrapping_sub(ptr_exposed_addr(lo))")
            && ptr_core_rs.contains("pub(crate) fn ptr_align_up<T>(")
            && ptr_core_rs
                .contains(".map_addr(|addr| crate::internal::_mi_align_up(addr, alignment))")
            && ptr_core_rs.contains(
                "crate::internal::_mi_align_up(ptr_exposed_addr(ptr), alignment) as *mut c_void"
            )
            && ptr_core_rs.contains("pub(crate) fn ptr_align_down<T>(")
            && ptr_core_rs
                .contains(".map_addr(|addr| crate::internal::_mi_align_down(addr, alignment))")
            && ptr_core_rs.contains(
                "crate::internal::_mi_align_down(ptr_exposed_addr(ptr), alignment) as *mut c_void"
            )
            && freelist_rs.contains("pub(crate) struct ThreadFreeWord(pub(crate) usize);")
            && freelist_rs.contains("use super::constants::mi_block_t;")
            && freelist_rs.contains("use super::ptr::{BlockPtr, ptr_exposed_addr};")
            && freelist_rs.contains("pub(crate) fn head_raw(self) -> *mut mi_block_t")
            && freelist_rs.contains("crate::provenance::resolve_address(ptr_val)")
            && freelist_rs.contains("ptr_val as *mut mi_block_t")
            && freelist_rs.contains("Some(unsafe { BlockPtr::new_unchecked(ptr) })")
            && freelist_rs.contains("let val = ptr_exposed_addr(ptr.as_ptr());")
            && freelist_rs.contains("crate::provenance::remember_address(val, ptr.as_ptr());")
            && freelist_wrappers_rs.contains("use super::ptr::{BlockPtr, ptr_exposed_addr};")
            && freelist_wrappers_rs.contains("pub(crate) struct EncodedWord(usize);")
            && freelist_wrappers_rs.contains("let word = ptr_exposed_addr(block);")
            && freelist_wrappers_rs.contains("crate::provenance::remember_address(word, block);")
            && freelist_wrappers_rs.contains("word as *mut mi_block_t")
            && internal_rs.contains("pub(crate) fn mi_ptr_decode(")
            && internal_rs.contains(
                "let decoded_addr = mi_rotr(x.get().wrapping_sub(keys[0]), keys[0]) ^ keys[1];"
            )
            && internal_rs.contains("crate::provenance::resolve_encoded(x.get(), null, keys)")
            && internal_rs.contains("let p = decoded_addr as *mut c_void;")
            && internal_rs.contains("pub(crate) fn mi_ptr_encode(")
            && internal_rs.contains("crate::core_types::ptr_exposed_addr(null)")
            && internal_rs.contains("crate::core_types::ptr_exposed_addr(p)")
            && internal_rs
                .contains("crate::provenance::remember_encoded(word.get(), null, keys, p);")
            && strict_check_rs.contains("PTR_USIZE_RE = re.compile")
            && strict_check_rs.contains("ALLOWED_PTR_USIZE = {")
            && strict_check_rs.contains("\"src/types/ptr.rs\"")
            && strict_check_rs.contains("kind=\"pointer-usize-roundtrip\"")
            && strict_check_rs.contains("domain=\"provenance\"")
            && strict_check_rs.contains(
                "message=\"pointer/integer round trip outside typed provenance helpers\""
            )
            && !allowlist_rs.contains(
                "pointer-usize-roundtrip|src/types/freelist_wrappers.rs|*|encoded-pointer|"
            )
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/internal/mod.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/internal/page_location.rs|mi_page_slice_offset_of|packed-pointer|"
            )
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/aligned.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/aligned.rs|mi_theap_malloc_zero_aligned_at_overalloc|alignment/provenance|"
            )
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/alloc.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/alloc.rs|_mi_page_malloc_zero_typed|padding/provenance|"
            )
            && !allowlist_rs
                .contains("pointer-usize-roundtrip|src/types/freelist.rs|*|packed-thread-free|")
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/free.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/free.rs|mi_validate_pointer|validation|"
            )
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/platform/os.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/platform/os.rs|mi_os_page_align_areax|os-memory|"
            )
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/platform/os.rs|_mi_os_free_ex_typed|os-memory|"
            )
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/platform/os.rs|mi_os_alloc_aligned_memid|os-memory|"
            ),
        "pointer-to-integer round trips should stay behind typed packed-pointer/provenance helpers or explicit strict-type allowlist boundaries"
    );
}
