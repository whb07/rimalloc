use super::DatatypeAcceptanceSources;

pub(super) fn assert_atomic_wrapper_guards(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let atomic_rs = audit_sources.source("src/types/atomic.rs");
    let atomic_arena_cells_rs = audit_sources.source("src/types/atomic/arena_cells.rs");
    let atomic_fields_rs = audit_sources.source("src/types/atomic/fields.rs");
    let atomic_guards_rs = audit_sources.source("src/types/atomic/guards.rs");
    let atomic_page_thread_rs = audit_sources.source("src/types/atomic/page_thread.rs");
    let atomic_thread_heap_refcount_rs =
        audit_sources.source("src/types/atomic/thread_heap_refcount.rs");
    let atomic_thread_free_rs = audit_sources.source("src/types/atomic/thread_free.rs");
    let strict_check_rs = audit_sources.strict_type_checker;
    let allowlist_rs = audit_sources.strict_type_allowlist;
    let atomic_marker_count = |marker| {
        atomic_rs.matches(marker).count()
            + atomic_arena_cells_rs.matches(marker).count()
            + atomic_fields_rs.matches(marker).count()
            + atomic_guards_rs.matches(marker).count()
            + atomic_page_thread_rs.matches(marker).count()
            + atomic_thread_heap_refcount_rs.matches(marker).count()
            + atomic_thread_free_rs.matches(marker).count()
    };

    assert!(
        atomic_fields_rs.contains("pub(crate) struct AtomicUsizeField(*mut usize);")
            && atomic_fields_rs.contains("pub(crate) struct AtomicI64Field(*mut i64);")
            && atomic_fields_rs.contains("pub(crate) struct AtomicPtrField<T>(*mut *mut T);")
            && atomic_fields_rs.contains("pub(crate) fn from_raw(field: *mut usize) -> Self")
            && atomic_fields_rs.contains("pub(crate) fn from_raw(field: *mut i64) -> Self")
            && atomic_fields_rs.contains("pub(crate) fn from_raw(field: *mut *mut T) -> Self")
            && atomic_fields_rs.contains("let atomic = unsafe { AtomicUsize::from_ptr(self.0) };")
            && atomic_fields_rs.contains("let atomic = unsafe { AtomicI64::from_ptr(self.0) };")
            && atomic_fields_rs.contains("let atomic = unsafe { AtomicPtr::from_ptr(self.0) };")
            && atomic_fields_rs
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && atomic_fields_rs
                .contains("pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder)")
            && atomic_fields_rs
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64")
            && atomic_fields_rs
                .contains("pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder)")
            && atomic_fields_rs
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_fields_rs
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
            && !allowlist_rs
                .contains("bare-domain-usize|src/allocator/aligned/thread_heap.rs|*|usize|")
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|aligned_layout_or_report|usize|"
            )
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned/thread_heap.rs|mi_theap_malloc_zero_aligned_at|usize|"
            )
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|mi_theap_recalloc_aligned|usize|"
            ),
        "aligned strict-type allowlist coverage should stay symbol-level with no removable broad entries"
    );
}
