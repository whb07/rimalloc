use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: [&str; 5] = ["panic!", "todo!", "unimplemented!", ".unwrap(", ".expect("];

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

#[test]
fn production_sources_do_not_use_panic_prone_shortcuts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources_under(&root, &mut sources);

    let mut hits = Vec::new();
    for path in sources {
        let contents = fs::read_to_string(&path).expect("read source file");
        for (line_index, line) in contents.lines().enumerate() {
            for marker in FORBIDDEN {
                if line.contains(marker) {
                    hits.push(format!(
                        "{}:{} contains `{}`",
                        path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                            .unwrap_or(&path)
                            .display(),
                        line_index + 1,
                        marker
                    ));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "panic-prone production source markers found:\n{}",
        hits.join("\n")
    );
}
