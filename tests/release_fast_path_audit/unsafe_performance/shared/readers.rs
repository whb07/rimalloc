use std::{fs, path::Path};

pub(super) fn read_source(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

pub(super) fn read_pair(root: &Path, first: &str, second: &str) -> String {
    [read_source(root, first), read_source(root, second)].join("\n")
}

pub(super) fn read_sources(root: &Path, rels: &[&str]) -> String {
    rels.iter()
        .map(|rel| read_source(root, rel))
        .collect::<Vec<_>>()
        .join("\n")
}
