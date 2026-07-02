use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct AllowEntry {
    kind: String,
    path: String,
    symbol: String,
    domain: String,
    disposition: String,
}

fn read_allowlist(root: &Path) -> Vec<AllowEntry> {
    fs::read_to_string(root.join("strict-type-allowlist.txt"))
        .expect("read strict-type allowlist")
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .map(|line| {
            let parts = line.split('|').collect::<Vec<_>>();
            assert_eq!(
                parts.len(),
                6,
                "strict-type allowlist row should have 6 fields"
            );
            AllowEntry {
                kind: parts[0].trim().to_owned(),
                path: parts[1].trim().to_owned(),
                symbol: parts[2].trim().to_owned(),
                domain: parts[3].trim().to_owned(),
                disposition: parts[5].trim().to_owned(),
            }
        })
        .collect()
}

fn increment<K: Ord>(counts: &mut BTreeMap<K, usize>, key: K) {
    *counts.entry(key).or_default() += 1;
}

fn assert_doc_snippets(source_name: &str, source: &str, snippets: &[&str]) {
    for snippet in snippets {
        assert!(
            source.contains(snippet),
            "{source_name} should document strict-type allowlist governance snippet `{snippet}`"
        );
    }
}

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
        634,
        "strict-type allowlist active entry count drifted"
    );
    assert_eq!(kind_counts.get("raw-pointer-parameter"), Some(&332));
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
            "has 634 active entries: 332\n  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40\n  metadata-reference entries",
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
            "634 active entries",
            "332 raw-pointer-parameter",
            "zero wildcard entries",
            "zero removable entries",
        ],
    );
}

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
