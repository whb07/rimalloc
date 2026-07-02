use super::*;

#[test]
fn strict_type_allowlist_hotspots_and_domains_stay_governed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = read_allowlist(root);
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
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
        ("src/internal/mod.rs", 57),
        ("src/allocator/alloc.rs", 53),
        ("src/allocator/page.rs", 51),
        ("src/allocator/aligned.rs", 50),
        ("src/internal/bitmap.rs", 45),
        ("src/raw/macros.rs", 32),
    ] {
        assert_eq!(
            file_counts.get(path),
            Some(&expected),
            "strict-type allowlist file hotspot count drifted for {path}"
        );
    }
    assert!(
        file_counts.values().all(|count| *count <= 57),
        "strict-type allowlist has a new file hotspot above the documented cap"
    );

    for ((kind, path), expected) in [
        (("raw-pointer-parameter", "src/allocator/alloc.rs"), 47),
        (("raw-pointer-parameter", "src/allocator/page.rs"), 44),
        (("raw-pointer-parameter", "src/internal/mod.rs"), 41),
        (("bare-domain-usize", "src/internal/bitmap.rs"), 33),
        (("bare-domain-usize", "src/raw/macros.rs"), 32),
        (("raw-pointer-parameter", "src/allocator/aligned.rs"), 30),
    ] {
        assert_eq!(
            kind_file_counts.get(&(kind, path)),
            Some(&expected),
            "strict-type kind/file hotspot count drifted for {kind} in {path}"
        );
    }

    for (domain, expected) in [("mi_theap_t", 111), ("mi_page_t", 110), ("mi_heap_t", 67)] {
        assert_eq!(
            raw_pointer_domains.get(domain),
            Some(&expected),
            "strict-type raw-pointer domain hotspot count drifted for {domain}"
        );
    }
    assert!(
        raw_pointer_domains.values().all(|count| *count <= 111),
        "strict-type allowlist has a new raw-pointer domain above the documented cap"
    );

    assert_doc_snippets(
        "TODO.md",
        &todo,
        &[
            "largest strict-type allowlist file hotspots are `src/internal/mod.rs` with\n  57 active entries",
            "`src/allocator/alloc.rs` with 53",
            "`src/allocator/page.rs` with 51",
            "`src/allocator/aligned.rs` with 50",
            "`src/internal/bitmap.rs` with 45",
            "`src/raw/macros.rs` with 32",
            "`raw-pointer-parameter` in\n  `src/allocator/alloc.rs` with 47 active entries",
            "`raw-pointer-parameter` in\n  `src/allocator/page.rs` with 44",
            "`raw-pointer-parameter` in\n  `src/internal/mod.rs` with 41",
            "`bare-domain-usize` in\n  `src/internal/bitmap.rs` with 33",
            "`bare-domain-usize` in `src/raw/macros.rs`\n  with 32",
            "`raw-pointer-parameter` in `src/allocator/aligned.rs` with 30",
            "raw-pointer domains are `mi_theap_t` with 111 entries",
            "`mi_page_t` with 110",
            "`mi_heap_t` with 67",
        ],
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
