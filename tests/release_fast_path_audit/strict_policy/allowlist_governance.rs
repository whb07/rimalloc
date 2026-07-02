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

impl AllowEntry {
    fn permanent_reason(&self) -> &str {
        self.disposition
            .strip_prefix("permanent: ")
            .expect("strict-type allowlist entry should be permanent")
            .trim_end_matches('.')
    }
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

#[path = "allowlist_governance/hotspot_inventory.rs"]
mod hotspot_inventory;
#[path = "allowlist_governance/inventory.rs"]
mod inventory;
#[path = "allowlist_governance/permanent_groups.rs"]
mod permanent_groups;
