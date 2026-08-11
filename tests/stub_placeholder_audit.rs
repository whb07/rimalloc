use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PLACEHOLDER_MARKERS: [&str; 5] = [
    "todo!",
    "unimplemented!",
    "panic!(\"todo",
    "panic!(\"unimplemented",
    "stubbed for translation",
];

const ALLOWED_EMPTY_LINES: [&str; 10] = [
    "_ => {}",
    "pub unsafe fn mi_track_init() {}",
    "pub unsafe fn mi_track_done() {}",
    "pub const fn MI_UNUSED<T>(_x: &T) {}",
    "pub const fn MI_UNUSED_RELEASE<T>(_x: &T) {}",
    "pub(crate) unsafe extern \"C\" fn _mi_allocator_done() {}",
    "unsafe fn mi_win_tls_slot_set(_slot: usize, _extended_slot: usize, _value: *mut c_void) {}",
    "fn mi_set_errno(_err: c_int) {}",
    "unsafe fn unix_mprotect_hint(_err: c_int) {}",
    "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done() {}",
];

const ALLOWED_NOTHING_FUNCTIONS: [&str; 11] = [
    "mi_malloc_destroy_zone",
    "mi_malloc_set_zone_name",
    "mi__malloc_fork_prepare",
    "mi__malloc_fork_parent",
    "mi__malloc_fork_child",
    "mi_malloc_printf",
    "zone_log",
    "zone_print",
    "zone_print_ptr_info",
    "zone_register",
    "zone_unregister",
];

fn rust_sources_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("read source entry");
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .display()
        .to_string()
}

fn previous_signature_names(lines: &[&str], line_index: usize) -> String {
    let start = line_index.saturating_sub(5);
    lines[start..line_index].join("\n")
}

fn looks_like_empty_function(trimmed: &str) -> bool {
    trimmed.contains(" fn ") || trimmed.starts_with("fn ")
}

#[test]
fn production_sources_have_no_unresolved_placeholder_markers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources_under(&root, &mut sources);

    let mut hits = Vec::new();
    for path in sources {
        let contents = fs::read_to_string(&path).expect("read source file");
        for (line_index, line) in contents.lines().enumerate() {
            for marker in PLACEHOLDER_MARKERS {
                if line.contains(marker) {
                    hits.push(format!(
                        "{}:{} contains `{}`",
                        relative(&path),
                        line_index + 1,
                        marker
                    ));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "production placeholder markers found:\n{}",
        hits.join("\n")
    );
}

#[test]
fn production_empty_functions_are_known_compatibility_shims() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed_empty: BTreeSet<&str> = ALLOWED_EMPTY_LINES.into_iter().collect();
    let mut sources = Vec::new();
    rust_sources_under(&root, &mut sources);

    let mut hits = Vec::new();
    for path in sources {
        let contents = fs::read_to_string(&path).expect("read source file");
        for (line_index, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if looks_like_empty_function(trimmed)
                && trimmed.ends_with("{}")
                && !allowed_empty.contains(trimmed)
            {
                hits.push(format!(
                    "{}:{} has unallowlisted empty body `{}`",
                    relative(&path),
                    line_index + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "unallowlisted production empty bodies found:\n{}",
        hits.join("\n")
    );
}

#[test]
fn production_nothing_comments_are_known_platform_hooks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources_under(&root, &mut sources);

    let mut hits = Vec::new();
    for path in sources {
        let contents = fs::read_to_string(&path).expect("read source file");
        let lines: Vec<&str> = contents.lines().collect();
        for (line_index, line) in lines.iter().enumerate() {
            if !line.contains("// nothing") {
                continue;
            }
            let prior = previous_signature_names(&lines, line_index);
            let allowed = if relative(&path) == "src/platform/wasi.rs" {
                prior.contains("_mi_prim_thread_init_auto_done")
            } else if relative(&path) == "src/platform/macos.rs" {
                ALLOWED_NOTHING_FUNCTIONS
                    .iter()
                    .any(|name| prior.contains(name))
            } else {
                false
            };
            if !allowed {
                hits.push(format!(
                    "{}:{} has unallowlisted `// nothing` hook",
                    relative(&path),
                    line_index + 1
                ));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "unallowlisted production no-op comments found:\n{}",
        hits.join("\n")
    );
}
