use super::*;

#[test]
fn strict_type_allowlist_inventory_stays_measured_by_concern() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = read_allowlist(root);
    let strict_docs =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read strict type docs");

    let mut kind_counts = BTreeMap::new();
    let mut removable_entries = 0usize;
    let mut wildcard_entries = 0usize;
    for entry in &entries {
        increment(&mut kind_counts, entry.kind.as_str());
        removable_entries += usize::from(entry.disposition.starts_with("remove:"));
        wildcard_entries += usize::from(entry.symbol == "*");
        assert!(
            entry.disposition.starts_with("permanent:"),
            "current strict-type allowlist entries should be permanent compatibility edges until a concrete remove task is added: {entry:?}"
        );
    }

    assert_eq!(
        entries.len(),
        633,
        "strict-type allowlist active entry count drifted"
    );
    assert_eq!(kind_counts.get("raw-pointer-parameter"), Some(&330));
    assert_eq!(kind_counts.get("bare-domain-usize"), Some(&219));
    assert_eq!(kind_counts.get("metadata-reference"), Some(&40));
    assert_eq!(kind_counts.get("pointer-usize-roundtrip"), Some(&23));
    assert_eq!(kind_counts.get("atomic-cast"), Some(&14));
    assert_eq!(kind_counts.get("heap-allocating-type"), Some(&7));
    assert_eq!(
        removable_entries, 0,
        "removable strict-type entries should be explicitly introduced and documented"
    );
    assert_eq!(
        wildcard_entries, 0,
        "strict-type wildcard entries should not silently return"
    );

    assert_doc_snippets(
        "docs/strict-types.md",
        &strict_docs,
        &[
            "## Strict-Type Allowlist Governance",
            "633 active entries",
            "330 raw-pointer-parameter",
            "zero wildcard entries",
            "zero removable entries",
        ],
    );
}
