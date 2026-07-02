use super::*;

#[test]
fn strict_type_allowlist_inventory_stays_measured_by_concern() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = read_allowlist(root);
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
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
        632,
        "strict-type allowlist active entry count drifted"
    );
    assert_eq!(kind_counts.get("raw-pointer-parameter"), Some(&330));
    assert_eq!(kind_counts.get("bare-domain-usize"), Some(&220));
    assert_eq!(kind_counts.get("metadata-reference"), Some(&40));
    assert_eq!(kind_counts.get("pointer-usize-roundtrip"), Some(&23));
    assert_eq!(kind_counts.get("atomic-cast"), Some(&14));
    assert_eq!(kind_counts.get("heap-allocating-type"), Some(&5));
    assert_eq!(
        removable_entries, 0,
        "removable strict-type entries should be explicitly introduced and documented"
    );
    assert_eq!(
        wildcard_entries, 0,
        "strict-type wildcard entries should not silently return"
    );

    assert_doc_snippets(
        "TODO.md",
        &todo,
        &[
            "has 632 active entries: 330\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
            "23 pointer-usize-roundtrip entries, 14 atomic-cast\n  entries, and 5 heap-allocating-type entries",
            "It has no retained wildcard\n  entries",
            "Strict-type wildcard guard: total=0; none.",
        ],
    );
    assert_doc_snippets(
        "docs/strict-types.md",
        &strict_docs,
        &[
            "## Strict-Type Allowlist Governance",
            "632 active entries",
            "330 raw-pointer-parameter",
            "zero wildcard entries",
            "zero removable entries",
        ],
    );
}
