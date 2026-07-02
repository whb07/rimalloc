use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn assert_source_marker_cap(root: &Path, cap: usize) {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut rust_files = Vec::new();
    collect_rust_files(&root.join("src"), &mut rust_files);

    for path in rust_files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        for audit_id in source_audit_ids(&source) {
            *counts.entry(audit_id.to_owned()).or_default() += 1;
        }
    }

    let mut offenders = counts
        .into_iter()
        .filter(|(_, count)| *count > cap)
        .collect::<Vec<_>>();
    offenders.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    assert!(
        offenders.is_empty(),
        "source audit IDs should have at most {cap} source markers; offenders: {}",
        offenders
            .iter()
            .take(12)
            .map(|(audit_id, count)| format!("{audit_id}={count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

fn collect_rust_files(dir: &Path, rust_files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display())) {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, rust_files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            rust_files.push(path);
        }
    }
}

fn source_audit_ids(source: &str) -> impl Iterator<Item = &str> {
    source
        .match_indices("SAFETY[AUDIT:")
        .filter_map(|(start, _)| {
            let id_start = start + "SAFETY[AUDIT:".len();
            let rest = source.get(id_start..)?;
            let id_len = rest.find(']')?;
            Some(&rest[..id_len])
        })
}
