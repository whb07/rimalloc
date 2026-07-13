use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

fn assert_not_no_mangle_item(source_name: &str, source: &str, internal_name: &str) {
    let Some(start) = source.find(internal_name) else {
        return;
    };
    let item_start = source[..start].rfind("\n\n").map_or(0, |idx| idx + 2);
    assert!(
        !source[item_start..start].contains("#[unsafe(no_mangle)]"),
        "{internal_name} in {source_name} should stay crate-internal instead of a dynamic export"
    );
}

fn source_item<'a>(source_name: &str, source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("{source_name} should contain {marker}"));
    let body_start = source[start..]
        .find('{')
        .map(|idx| start + idx)
        .unwrap_or_else(|| panic!("{marker} should have a body in {source_name}"));
    let mut depth = 0usize;
    for (offset, byte) in source[body_start..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=body_start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("{marker} body should be balanced in {source_name}");
}

fn assert_no_callback_alloc_or_unwind(source_name: &str, marker: &str, item: &str) {
    for forbidden in [
        "Vec<",
        "Vec::",
        "Box<",
        "Box::",
        "String::",
        "format!",
        "to_string(",
        "to_owned(",
        "alloc::",
        "std::",
        "malloc(",
        "mi_malloc",
        "mi_zalloc",
        "mi_calloc",
        "mi_realloc",
        "_mi_meta_alloc",
        "_mi_meta_zalloc",
        "_mi_arenas_alloc",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} should not allocate through {forbidden}"
        );
    }
    for forbidden in [
        "panic!",
        "unreachable!",
        "todo!",
        "unimplemented!",
        "assert!(",
        "debug_assert!(",
        "mi_assert_internal!",
        "mi_debug_assert!",
        "expect(",
        ".unwrap(",
        "catch_unwind",
        "resume_unwind",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} should not panic or unwind through {forbidden}"
        );
    }
}

fn assert_no_rust_heap_constructs(source_name: &str, marker: &str, item: &str) {
    for forbidden in [
        "extern crate std",
        "use std",
        "std::",
        "extern crate alloc",
        "crate::alloc::",
        "::alloc::",
        "use alloc::",
        "Vec<",
        "Vec::",
        "Box<",
        "Box::",
        "String",
        "format!",
        "to_string(",
        "to_owned(",
        "_mi_meta_alloc",
        "_mi_meta_zalloc",
        "_mi_arenas_alloc",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} should not allocate through {forbidden}"
        );
    }
}

fn collect_rs_sources(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("read source entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_sources(base, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path
                .strip_prefix(base)
                .expect("source path under base")
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(&path).expect("read Rust source");
            out.push((rel, source));
        }
    }
}

fn dead_code_allowance_count(source: &str) -> usize {
    source
        .lines()
        .filter(|line| line.contains("dead_code"))
        .count()
}

fn module_dead_code_allowance_count(source: &str) -> usize {
    let mut count = 0;
    let mut in_module_allow = false;
    let mut has_dead_code = false;

    for line in source.lines() {
        if !in_module_allow && line.contains("#![allow") {
            in_module_allow = true;
            has_dead_code = line.contains("dead_code");
        } else if in_module_allow {
            has_dead_code |= line.contains("dead_code");
        }

        if in_module_allow && line.contains(']') {
            count += usize::from(has_dead_code);
            in_module_allow = false;
            has_dead_code = false;
        }
    }

    count
}

fn clippy_allowance_counts(sources: &[(String, String)]) -> BTreeMap<(&str, &str), usize> {
    let mut counts = BTreeMap::new();
    for (rel, source) in sources {
        for line in source.lines() {
            if !line.contains("allow(clippy::") && !line.contains("expect(clippy::") {
                continue;
            }
            let Some(start) = line.find("clippy::") else {
                continue;
            };
            let lint = &line[start + "clippy::".len()..];
            let lint = lint
                .split(|ch: char| ch == ')' || ch == ',' || ch.is_whitespace())
                .next()
                .expect("lint name after clippy::");
            *counts.entry((rel.as_str(), lint)).or_insert(0) += 1;
        }
    }
    counts
}

fn package_clippy_lints(manifest: &str) -> BTreeMap<&str, &str> {
    let section_start = manifest
        .find("[lints.clippy]")
        .expect("manifest has clippy lint section");
    let section = &manifest[section_start..];
    let section_end = section[1..]
        .find("\n[")
        .map_or(section.len(), |idx| idx + 1);
    let mut lints = BTreeMap::new();
    for line in section[..section_end].lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        lints.insert(name.trim(), value.trim());
    }
    lints
}

fn rust_allowance_counts<'a>(
    sources: &'a [(String, String)],
    lints: &'static [&'static str],
) -> BTreeMap<(&'a str, &'static str), usize> {
    let mut counts = BTreeMap::new();
    for (rel, source) in sources {
        for line in source.lines() {
            for &lint in lints {
                if line.contains(lint) {
                    *counts.entry((rel.as_str(), lint)).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

#[path = "release_fast_path_audit/datatype_acceptance.rs"]
mod datatype_acceptance;
#[path = "release_fast_path_audit/release_policy.rs"]
mod release_policy;
#[path = "release_fast_path_audit/runtime_gates.rs"]
mod runtime_gates;
#[path = "release_fast_path_audit/todo_backlog.rs"]
mod todo_backlog;
#[path = "release_fast_path_audit/unsafe_performance.rs"]
mod unsafe_performance;

#[path = "release_fast_path_audit/allocation_contracts.rs"]
mod allocation_contracts;
#[path = "release_fast_path_audit/allowance_inventory.rs"]
mod allowance_inventory;
#[path = "release_fast_path_audit/benchmark_hygiene.rs"]
mod benchmark_hygiene;
#[path = "release_fast_path_audit/capability_linearity.rs"]
mod capability_linearity;
#[path = "release_fast_path_audit/interposition_exports.rs"]
mod interposition_exports;
#[path = "release_fast_path_audit/policy_boundaries.rs"]
mod policy_boundaries;
#[path = "release_fast_path_audit/primitive_options.rs"]
mod primitive_options;
#[path = "release_fast_path_audit/public_facade.rs"]
mod public_facade;
#[path = "release_fast_path_audit/safe_api.rs"]
mod safe_api;
#[path = "release_fast_path_audit/strict_policy.rs"]
mod strict_policy;
