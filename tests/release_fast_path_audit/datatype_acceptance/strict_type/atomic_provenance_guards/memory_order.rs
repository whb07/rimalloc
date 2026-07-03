use super::DatatypeAcceptanceSources;

pub(super) fn assert_memory_order_boundaries(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let atomic_rs = audit_sources.source("src/types/atomic.rs");
    let atomic_fields_rs = audit_sources.source("src/types/atomic/fields.rs");
    let internal_atomic_rs = audit_sources.source("src/internal/atomic.rs");
    let ptr_core_rs = audit_sources.source("src/types/ptr.rs");

    assert!(
        internal_atomic_rs.contains("pub enum mi_memory_order_e {")
            && internal_atomic_rs.contains("pub type mi_memory_order = mi_memory_order_e;")
            && internal_atomic_rs.contains("fn load_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn store_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn rmw_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs
                .contains("fn cas_failure_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e)")
            && atomic_rs.contains("pub(crate) type MemoryOrder = Ordering;")
            && atomic_fields_rs
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
}
