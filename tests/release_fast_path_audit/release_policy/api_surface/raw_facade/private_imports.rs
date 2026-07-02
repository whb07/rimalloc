use std::fs;
use std::path::Path;

#[test]
fn repository_tests_do_not_import_private_raw_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = root.join("tests");
    let internal_modules = [
        "alloc",
        "alloc_aligned",
        "alloc_override",
        "alloc_posix",
        "arena",
        "arena_meta",
        "atomic",
        "bitmap",
        "bits",
        "free",
        "heap",
        "init",
        "internal",
        "libc",
        "memory",
        "options",
        "os",
        "page",
        "page_map",
        "prim",
        "random",
        "static_c",
        "stats",
        "theap",
        "threadlocal",
        "track",
        "types",
    ];

    for entry in fs::read_dir(tests_dir).expect("read tests directory") {
        let path = entry.expect("read tests entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read test source");
        let uses_raw_internal = internal_modules.iter().any(|module| {
            let direct = format!("use mechanical_mimalloc::raw::{module}");
            let group_inline = format!("use mechanical_mimalloc::raw::{{{module}");
            let group_multiline = format!("use mechanical_mimalloc::raw::{{\n    {module}");
            source.contains(&direct)
                || source.contains(&group_inline)
                || source.contains(&group_multiline)
        });

        assert!(
            !uses_raw_internal,
            "{} imports removed private raw implementation modules",
            path.display()
        );
    }
}
