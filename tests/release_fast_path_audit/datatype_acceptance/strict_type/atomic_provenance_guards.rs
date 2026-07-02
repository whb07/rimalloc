use super::super::DatatypeAcceptanceSources;

#[path = "atomic_provenance_guards/atomic_wrappers.rs"]
mod atomic_wrappers;
#[path = "atomic_provenance_guards/memory_order.rs"]
mod memory_order;
#[path = "atomic_provenance_guards/metadata_references.rs"]
mod metadata_references;
#[path = "atomic_provenance_guards/provenance.rs"]
mod provenance;

pub(super) fn assert_atomic_provenance_and_metadata_guards(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    atomic_wrappers::assert_atomic_wrapper_guards(audit_sources);
    memory_order::assert_memory_order_boundaries(audit_sources);
    provenance::assert_pointer_provenance_boundaries(audit_sources);
    metadata_references::assert_metadata_reference_boundaries(audit_sources);
}
