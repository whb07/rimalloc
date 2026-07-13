use std::collections::BTreeMap;

use super::DatatypeAcceptanceSources;

#[derive(Clone, Copy)]
struct AllowEntry<'a> {
    kind: &'a str,
    path: &'a str,
    symbol: &'a str,
    domain: &'a str,
}

fn active_entries(allowlist: &str) -> Vec<AllowEntry<'_>> {
    allowlist
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let fields = line.split('|').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                6,
                "strict-type allowlist entries should keep six pipe-delimited fields"
            );
            assert!(
                !fields[4].trim().is_empty(),
                "strict-type allowlist entries should include a rationale"
            );
            assert!(
                fields[5].trim().starts_with("permanent:"),
                "strict-type allowlist entries should not retain removable work items"
            );
            Some(AllowEntry {
                kind: fields[0].trim(),
                path: fields[1].trim(),
                symbol: fields[2].trim(),
                domain: fields[3].trim(),
            })
        })
        .collect()
}

fn increment<'a>(counts: &mut BTreeMap<&'a str, usize>, key: &'a str) {
    *counts.entry(key).or_default() += 1;
}

fn top_counts<'a>(counts: &BTreeMap<&'a str, usize>, limit: usize) -> Vec<(&'a str, usize)> {
    let mut items = counts
        .iter()
        .map(|(key, count)| (*key, *count))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    items.truncate(limit);
    items
}

fn top_pair_counts<'a>(
    counts: &BTreeMap<(&'a str, &'a str), usize>,
    limit: usize,
) -> Vec<((&'a str, &'a str), usize)> {
    let mut items = counts
        .iter()
        .map(|(key, count)| (*key, *count))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items.truncate(limit);
    items
}

pub(super) fn assert_strict_type_allowlist_inventory(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let entries = active_entries(audit_sources.strict_type_allowlist);
    assert_eq!(
        entries.len(),
        634,
        "strict-type allowlist active entry count should stay measured"
    );

    let mut by_kind = BTreeMap::new();
    let mut by_file = BTreeMap::new();
    let mut by_kind_file = BTreeMap::new();
    let mut raw_pointer_domains = BTreeMap::new();
    let mut wildcard_kinds = BTreeMap::new();

    for entry in &entries {
        increment(&mut by_kind, entry.kind);
        increment(&mut by_file, entry.path);
        *by_kind_file.entry((entry.kind, entry.path)).or_default() += 1;
        if entry.kind == "raw-pointer-parameter" {
            increment(&mut raw_pointer_domains, entry.domain);
        }
        if entry.symbol == "*" {
            increment(&mut wildcard_kinds, entry.kind);
        }
    }

    assert_eq!(
        by_kind,
        BTreeMap::from([
            ("atomic-cast", 14),
            ("bare-domain-usize", 220),
            ("heap-allocating-type", 7),
            ("metadata-reference", 40),
            ("pointer-usize-roundtrip", 23),
            ("raw-pointer-parameter", 330),
        ]),
        "strict-type allowlist count by kind should stay measured"
    );
    assert_eq!(
        top_counts(&by_file, 6),
        [
            ("src/allocator/aligned.rs", 36),
            ("src/internal/bitmap.rs", 35),
            ("src/allocator/alloc.rs", 30),
            ("src/allocator/page.rs", 26),
            ("src/allocator/thread_heap.rs", 25),
            ("src/allocator/free.rs", 24),
        ],
        "strict-type allowlist file hotspots should stay measured"
    );
    assert_eq!(
        top_pair_counts(&by_kind_file, 6),
        [
            (("bare-domain-usize", "src/types/scalar.rs"), 24),
            (("raw-pointer-parameter", "src/allocator/alloc.rs"), 24),
            (
                ("raw-pointer-parameter", "src/allocator/thread_heap.rs"),
                24
            ),
            (("bare-domain-usize", "src/internal/bitmap.rs"), 23),
            (("bare-domain-usize", "src/raw/macros.rs"), 23),
            (("raw-pointer-parameter", "src/allocator/aligned.rs"), 23),
        ],
        "strict-type allowlist kind/file hotspots should stay measured"
    );
    assert_eq!(
        raw_pointer_domains,
        BTreeMap::from([
            ("mi_arena_t", 17),
            ("mi_block_t", 14),
            ("mi_heap_t", 67),
            ("mi_page_t", 110),
            ("mi_subproc_t", 13),
            ("mi_theap_t", 109),
        ]),
        "strict-type raw-pointer domain counts should stay measured"
    );
    assert_eq!(
        wildcard_kinds,
        BTreeMap::new(),
        "strict-type wildcard count should stay at zero"
    );
}
