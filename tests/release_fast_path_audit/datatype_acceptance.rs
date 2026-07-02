pub(super) struct DatatypeAcceptanceSources<'a> {
    pub(super) rust_sources: &'a [(String, String)],
    pub(super) types_spec: &'a str,
    pub(super) unsafe_migration_allowlist: &'a str,
    pub(super) strict_type_allowlist: &'a str,
    pub(super) strict_type_checker: &'a str,
    pub(super) todo: &'a str,
}

impl<'a> DatatypeAcceptanceSources<'a> {
    fn source(&self, rel: &str) -> &'a str {
        self.rust_sources
            .iter()
            .find_map(|(path, source)| (path == rel).then_some(source.as_str()))
            .unwrap_or_else(|| panic!("collected Rust sources should include {rel}"))
    }
}

#[path = "datatype_acceptance/callbacks.rs"]
mod callbacks;
#[path = "datatype_acceptance/domain_wrappers.rs"]
mod domain_wrappers;
#[path = "datatype_acceptance/fixture_audit.rs"]
mod fixture_audit;
#[path = "datatype_acceptance/heap_lifecycle.rs"]
mod heap_lifecycle;
#[path = "datatype_acceptance/padding_and_page_state.rs"]
mod padding_and_page_state;
#[path = "datatype_acceptance/page_queues.rs"]
mod page_queues;
#[path = "datatype_acceptance/spec_reconciliation.rs"]
mod spec_reconciliation;
#[path = "datatype_acceptance/strict_type.rs"]
mod strict_type;
#[path = "datatype_acceptance/strict_type_allowlist_inventory.rs"]
mod strict_type_allowlist_inventory;

pub(super) fn assert_datatype_acceptance_criteria_have_cross_cutting_guards(
    audit_sources: DatatypeAcceptanceSources<'_>,
) {
    domain_wrappers::assert_domain_wrapper_acceptance(&audit_sources);
    heap_lifecycle::assert_heap_lifecycle_acceptance(&audit_sources);
    page_queues::assert_page_queue_and_memid_acceptance(&audit_sources);
    fixture_audit::assert_typed_fixture_audit_splits(&audit_sources);
    strict_type::assert_strict_type_and_provenance_acceptance(&audit_sources);
    strict_type_allowlist_inventory::assert_strict_type_allowlist_inventory(&audit_sources);
    callbacks::assert_callback_acceptance(&audit_sources);
    padding_and_page_state::assert_padding_and_page_state_acceptance(&audit_sources);
    spec_reconciliation::assert_type_spec_reconciliation(&audit_sources);
}
