use super::super::DatatypeAcceptanceSources;

pub(super) fn assert_atomic_provenance_and_metadata_guards(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let atomic_rs = audit_sources.source("src/types/atomic.rs");
    let internal_atomic_rs = audit_sources.source("src/internal/atomic.rs");
    let strict_check_rs = audit_sources.strict_type_checker;
    let allowlist_rs = audit_sources.strict_type_allowlist;
    let ptr_core_rs = audit_sources.source("src/types/ptr.rs");
    let freelist_rs = audit_sources.source("src/types/freelist.rs");
    let freelist_wrappers_rs = audit_sources.source("src/types/freelist_wrappers.rs");
    let internal_rs = audit_sources.source("src/internal/mod.rs");
    let atomic_marker_count = |marker| atomic_rs.matches(marker).count();
    assert!(
        atomic_rs.contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && atomic_rs.contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && atomic_rs.contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && atomic_rs.contains("let atomic = unsafe { AtomicUsize::from_ptr(self.0) };")
            && atomic_rs.contains("let atomic = unsafe { AtomicI64::from_ptr(self.0) };")
            && atomic_rs.contains("let atomic = unsafe { AtomicPtr::from_ptr(self.0) };")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder)")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder)")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && !atomic_rs.contains("SAFETY[AUDIT:typed-atomic-domain]")
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-usize-field]") >= 8
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-i64-field]") >= 3
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-pointer-field]") >= 5
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-page-thread-id]") >= 8
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-thread-free]") >= 13
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-abandoned-count]") >= 3
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-thread-heap-refcount]") >= 3
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-arena-page]") >= 2
            && atomic_marker_count("SAFETY[AUDIT:typed-atomic-guard-once-lock]") >= 6
            && strict_check_rs.contains("ALLOWED_ATOMIC_CAST = {")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"atomic-cast\"")
            && strict_check_rs
                .contains("message=\"pointer-to-atomic cast outside atomic field wrappers\"")
            && strict_check_rs.contains("ATOMIC_CAST_RE = re.compile")
            && !strict_check_rs.contains("\"src/internal/atomic.rs\",\n}")
            && strict_check_rs.contains(
                "\"src/internal/bitmap/bfield.rs\": {\"src/bitmap.rs\", \"src/internal/bitmap.rs\"}"
            )
            && !strict_check_rs
                .contains("\"src/allocator/page/bins.rs\": {\"src/allocator/page.rs\"}")
            && !strict_check_rs
                .contains("\"src/allocator/page/counts.rs\": {\"src/allocator/page.rs\"}")
            && !strict_check_rs
                .contains("\"src/allocator/page/flags.rs\": {\"src/allocator/page.rs\"}")
            && !strict_check_rs
                .contains("\"src/allocator/page/validation.rs\": {\"src/allocator/page.rs\"}")
            && strict_check_rs.contains("ALLOWLIST = ROOT / \"strict-type-allowlist.txt\""),
        "pointer-to-atomic casts for allocator metadata should be centralized in typed atomic field wrappers and guarded by the strict checker"
    );
    assert!(
        strict_check_rs.contains("REMOVE_ENTRY_CAP = 0")
            && !allowlist_rs.contains("bare-domain-usize|src/allocator/aligned.rs|*|usize|")
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|aligned_layout_or_report|usize|"
            )
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|mi_theap_malloc_zero_aligned_at|usize|"
            )
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|mi_theap_recalloc_aligned|usize|"
            ),
        "aligned strict-type allowlist coverage should stay symbol-level with no removable broad entries"
    );
    assert!(
        internal_atomic_rs.contains("pub enum mi_memory_order_e {")
            && internal_atomic_rs.contains("pub type mi_memory_order = mi_memory_order_e;")
            && internal_atomic_rs.contains("fn load_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn store_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn rmw_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn cas_failure_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e)")
            && atomic_rs.contains("pub(crate) type MemoryOrder = Ordering;")
            && atomic_rs
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount")
            && ptr_core_rs.contains(
                "pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder)"
            )
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount"),
        "C-compatible mi_memory_order_e should stay in the atomic facade while internal wrappers use MemoryOrder/Ordering"
    );
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
                "pointer-usize-roundtrip|src/internal/mod.rs|mi_page_slice_offset_of|packed-pointer|"
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
    assert!(
        strict_check_rs.contains("METADATA_REF_RE = re.compile")
            && strict_check_rs.contains("kind=\"metadata-reference\"")
            && strict_check_rs.contains("domain=\"metadata\"")
            && strict_check_rs
                .contains("message=\"broad raw metadata reference outside typed field helpers\"")
            && !allowlist_rs.contains("metadata-reference|src/bitmap.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/internal/bitmap.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/free.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/options.rs|*|")
            && allowlist_rs
                .contains("metadata-reference|src/internal/bitmap.rs|mi_bitmap_setN|bitmap|")
            && allowlist_rs.contains(
                "metadata-reference|src/allocator/free.rs|mi_page_decode_padding|mi_page_t|"
            )
            && allowlist_rs.contains(
                "metadata-reference|src/allocator/options.rs|with_options_table|options|"
            )
            && !allowlist_rs.contains("metadata-reference|src/allocator/stats.rs|*|")
            && allowlist_rs
                .contains("metadata-reference|src/allocator/stats.rs|mi_heap_stats_get|mi_heap_t|")
            && !allowlist_rs.contains("metadata-reference|src/types/atomic.rs|*|atomic-field|")
            && allowlist_rs.contains(
                "metadata-reference|src/types/atomic.rs|AtomicArenaPage::load_acquire|atomic-field|"
            )
            && allowlist_rs.contains(
                "metadata-reference|src/types/atomic.rs|AtomicArenaPage::store_release|atomic-field|"
            )
            && ptr_core_rs.contains("let raw = self.as_ptr();")
            && ptr_core_rs.contains("ptr::read(ptr::addr_of!((*raw).block_size))")
            && ptr_core_rs
                .contains("ptr::write(ptr::addr_of_mut!((*raw).block_size), block_size.get())")
            && ptr_core_rs
                .contains("AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).free), head)")
            && ptr_core_rs
                .contains("LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).local_free), head)")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_id_cell(self) -> AtomicPageThreadId")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_free_cell(self) -> AtomicThreadFree")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_cell(self) -> AtomicPtrField<mi_heap_t>")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count_cell(self) -> AtomicUsizeField")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count_cell(self) -> AtomicUsizeField"),
        "aliasable allocator metadata should be accessed through pointer wrapper field helpers and guarded against broad Rust references"
    );
}
