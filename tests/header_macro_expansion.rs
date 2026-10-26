use std::fs;
use std::process::Command;

#[test]
fn representative_header_macros_expand_from_downstream_crate() {
    let root = env!("CARGO_MANIFEST_DIR");
    let temp = std::env::temp_dir().join(format!(
        "rimalloc_header_macro_smoke_{}",
        std::process::id()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp crate");
    }
    fs::create_dir_all(temp.join("src")).expect("create temp crate");
    fs::write(
        temp.join("Cargo.toml"),
        format!(
            "[package]\nname = \"header_macro_smoke\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nrimalloc = {{ path = {root:?}, features = [\"std\", \"c-header-macros\"] }}\n"
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
        let p = rimalloc::malloc!(8);
        let p = rimalloc::realloc!(p, 16);
        let _ = rimalloc::malloc_usable_size!(p);
        rimalloc::cfree!(p);

        let text = CString::new("header macro smoke").unwrap();
        let s = rimalloc::strdup!(text.as_ptr());
        rimalloc::raw::mi_free(s.cast());

        let mut aligned = ptr::null_mut();
        if rimalloc::posix_memalign!(&mut aligned, 16, 32) == 0 {
            rimalloc::raw::mi_free(aligned);
        }

        let aligned = rimalloc::_aligned_malloc!(32, 16);
        rimalloc::_aligned_free!(aligned);

        let object = rimalloc::operatornew!(16);
        rimalloc::operatordelete!(object);

        let object = rimalloc::operatornew_nothrow!(16, ());
        rimalloc::operatordelete_nothrow!(object, ());
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

#[test]
fn header_macros_are_not_available_without_feature() {
    let root = env!("CARGO_MANIFEST_DIR");
    let temp = std::env::temp_dir().join(format!(
        "rimalloc_header_macro_default_gate_{}",
        std::process::id()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp crate");
    }
    fs::create_dir_all(temp.join("src")).expect("create temp crate");
    fs::write(
        temp.join("Cargo.toml"),
        format!(
            "[package]\nname = \"header_macro_default_gate\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nrimalloc = {{ path = {root:?}, features = [\"std\"] }}\n"
        ),
    )
    .expect("write temp Cargo.toml");
    fs::write(
        temp.join("src/main.rs"),
        r#"
fn main() {
    unsafe {
        let p = rimalloc::malloc!(8);
        rimalloc::raw::mi_free(p);
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() && stderr.contains("could not find `malloc`"),
        "header macros should be gated by c-header-macros\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    fs::remove_dir_all(&temp).expect("remove temp crate");
}

#[test]
fn c_header_macros_still_require_unsafe_boundary() {
    let root = env!("CARGO_MANIFEST_DIR");
    let temp = std::env::temp_dir().join(format!(
        "rimalloc_header_macro_unsafe_gate_{}",
        std::process::id()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp crate");
    }
    fs::create_dir_all(temp.join("src")).expect("create temp crate");
    fs::write(
        temp.join("Cargo.toml"),
        format!(
            "[package]\nname = \"header_macro_unsafe_gate\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nrimalloc = {{ path = {root:?}, features = [\"std\", \"c-header-macros\"] }}\n"
        ),
    )
    .expect("write temp Cargo.toml");
    fs::write(
        temp.join("src/main.rs"),
        r#"
fn main() {
    let p = rimalloc::malloc!(8);
    let q = rimalloc::mi_malloc_tp!(u64);
    let _ = (p, q);
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success()
            && stderr.contains("call to unsafe function")
            && stderr.contains("requires unsafe"),
        "c-header macros should preserve caller-side unsafe boundary\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    fs::remove_dir_all(&temp).expect("remove temp crate");
}
