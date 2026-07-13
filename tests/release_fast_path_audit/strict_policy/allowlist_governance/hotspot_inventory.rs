use super::*;

#[test]
fn strict_type_allowlist_hotspots_and_domains_stay_governed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = read_allowlist(root);
    let strict_docs =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read strict type docs");

    let mut file_counts = BTreeMap::new();
    let mut kind_file_counts = BTreeMap::new();
    let mut raw_pointer_domains = BTreeMap::new();
    for entry in &entries {
        increment(&mut file_counts, entry.path.as_str());
        increment(
            &mut kind_file_counts,
            (entry.kind.as_str(), entry.path.as_str()),
        );
        if entry.kind == "raw-pointer-parameter" {
            increment(&mut raw_pointer_domains, entry.domain.as_str());
        }
    }

    for (path, expected) in [
        ("src/allocator/aligned.rs", 36),
        ("src/internal/bitmap.rs", 35),
        ("src/allocator/alloc.rs", 30),
        ("src/allocator/page.rs", 26),
        ("src/allocator/thread_heap.rs", 25),
        ("src/allocator/free.rs", 24),
    ] {
        assert_eq!(
            file_counts.get(path),
            Some(&expected),
            "strict-type allowlist file hotspot count drifted for {path}"
        );
    }
    assert!(
        file_counts.values().all(|count| *count <= 40),
        "strict-type allowlist has a new file hotspot above the documented cap"
    );

    for ((kind, path), expected) in [
        (("bare-domain-usize", "src/types/scalar.rs"), 24),
        (("raw-pointer-parameter", "src/allocator/alloc.rs"), 24),
        (
            ("raw-pointer-parameter", "src/allocator/thread_heap.rs"),
            24,
        ),
        (("bare-domain-usize", "src/internal/bitmap.rs"), 23),
        (("bare-domain-usize", "src/raw/macros.rs"), 23),
        (("raw-pointer-parameter", "src/allocator/aligned.rs"), 23),
    ] {
        assert_eq!(
            kind_file_counts.get(&(kind, path)),
            Some(&expected),
            "strict-type kind/file hotspot count drifted for {kind} in {path}"
        );
    }
    assert!(
        kind_file_counts.values().all(|count| *count <= 25),
        "strict-type allowlist has a new kind/file hotspot above the documented cap"
    );

    for (domain, expected) in [("mi_page_t", 110), ("mi_theap_t", 109), ("mi_heap_t", 67)] {
        assert_eq!(
            raw_pointer_domains.get(domain),
            Some(&expected),
            "strict-type raw-pointer domain hotspot count drifted for {domain}"
        );
    }
    assert!(
        raw_pointer_domains.values().all(|count| *count <= 110),
        "strict-type allowlist has a new raw-pointer domain above the documented cap"
    );

    assert_doc_snippets(
        "docs/strict-types.md",
        &strict_docs,
        &[
            "Permanent hotspot families",
            "`mi_theap_t`",
            "`mi_page_t`",
            "`mi_heap_t`",
            "Future rewrite candidates",
            "`remove:`",
        ],
    );
}
