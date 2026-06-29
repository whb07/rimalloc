use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

const HEADER_MACROS: &[&str] = &[
    "_aligned_free",
    "_aligned_malloc",
    "_aligned_offset_realloc",
    "_aligned_recalloc",
    "_dupenv_s",
    "_expand",
    "_recalloc",
    "_strdup",
    "_wcsdup",
    "cfree",
    "malloc",
    "malloc_usable_size",
    "memalign",
    "operatordelete",
    "operatordelete_aligned",
    "operatordelete_aligned_nothrow",
    "operatordelete_array",
    "operatordelete_array_aligned",
    "operatordelete_array_aligned_nothrow",
    "operatordelete_array_nothrow",
    "operatordelete_array_sized",
    "operatordelete_array_sized_aligned",
    "operatordelete_nothrow",
    "operatordelete_sized",
    "operatordelete_sized_aligned",
    "operatornew",
    "operatornew_aligned",
    "operatornew_aligned_nothrow",
    "operatornew_array",
    "operatornew_array_aligned",
    "operatornew_array_aligned_nothrow",
    "operatornew_array_nothrow",
    "operatornew_nothrow",
    "posix_memalign",
    "realpath",
    "realloc",
    "reallocarray",
    "reallocf",
    "strdup",
    "valloc",
];

#[test]
fn header_macro_inventory_is_public_compatibility_surface() {
    let root = env!("CARGO_MANIFEST_DIR");
    let source = fs::read_to_string(format!("{root}/src/ffi/headers.rs")).expect("read headers.rs");

    let actual = source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("macro_rules! ")
                .and_then(|rest| rest.split_whitespace().next())
        })
        .collect::<BTreeSet<_>>();
    let expected = HEADER_MACROS.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert!(
        source.contains("$crate::raw::"),
        "header macros should route through public raw paths"
    );
    for private_path in [
        "$crate::alloc::",
        "$crate::alloc_aligned::",
        "$crate::alloc_posix::",
        "$crate::free::",
        "$crate::mimalloc::",
        "$crate::prim::",
    ] {
        assert!(
            !source.contains(private_path),
            "header macro expansion still references private path {private_path}"
        );
    }
}

#[test]
fn representative_header_macros_expand_from_downstream_crate() {
    let root = env!("CARGO_MANIFEST_DIR");
    let temp = std::env::temp_dir().join(format!(
        "mechanical_mimalloc_header_macro_smoke_{}",
        std::process::id()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp crate");
    }
    fs::create_dir_all(temp.join("src")).expect("create temp crate");
    fs::write(
        temp.join("Cargo.toml"),
        format!(
            "[package]\nname = \"header_macro_smoke\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nmechanical_mimalloc = {{ path = {root:?}, features = [\"std\"] }}\n"
        ),
    )
    .expect("write temp Cargo.toml");
    fs::write(
        temp.join("src/main.rs"),
        r#"
use std::ffi::CString;
use std::ptr;

fn main() {
    unsafe {
        let p = mechanical_mimalloc::malloc!(8);
        let p = mechanical_mimalloc::realloc!(p, 16);
        let _ = mechanical_mimalloc::malloc_usable_size!(p);
        mechanical_mimalloc::cfree!(p);

        let text = CString::new("header macro smoke").unwrap();
        let s = mechanical_mimalloc::strdup!(text.as_ptr());
        mechanical_mimalloc::raw::mi_free(s.cast());

        let mut aligned = ptr::null_mut();
        if mechanical_mimalloc::posix_memalign!(&mut aligned, 16, 32) == 0 {
            mechanical_mimalloc::raw::mi_free(aligned);
        }

        let aligned = mechanical_mimalloc::_aligned_malloc!(32, 16);
        mechanical_mimalloc::_aligned_free!(aligned);

        let object = mechanical_mimalloc::operatornew!(16);
        mechanical_mimalloc::operatordelete!(object);

        let object = mechanical_mimalloc::operatornew_nothrow!(16, ());
        mechanical_mimalloc::operatordelete_nothrow!(object, ());
    }
}
"#,
    )
    .expect("write temp main.rs");

    let output = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(temp.join("Cargo.toml"))
        .output()
        .expect("run downstream cargo check");

    assert!(
        output.status.success(),
        "downstream header macro smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&temp).expect("remove temp crate");
}
