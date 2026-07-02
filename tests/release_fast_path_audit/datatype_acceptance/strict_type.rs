use super::DatatypeAcceptanceSources;

#[path = "strict_type/atomic_provenance_guards.rs"]
mod atomic_provenance_guards;
#[path = "strict_type/checker_allowlist_symbols.rs"]
mod checker_allowlist_symbols;
#[path = "strict_type/page_thread_bits.rs"]
mod page_thread_bits;
#[path = "strict_type/pointer_freelist_wrappers.rs"]
mod pointer_freelist_wrappers;

pub(super) fn assert_strict_type_and_provenance_acceptance(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    page_thread_bits::assert_page_thread_and_free_bit_domains(audit_sources);
    pointer_freelist_wrappers::assert_pointer_and_freelist_wrappers(audit_sources);
    atomic_provenance_guards::assert_atomic_provenance_and_metadata_guards(audit_sources);
    checker_allowlist_symbols::assert_strict_checker_and_allowlist_symbols(audit_sources);
}
