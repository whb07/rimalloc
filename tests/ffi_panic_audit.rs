use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

const FORBIDDEN: [&str; 7] = [
    "panic!",
    "todo!",
    "unimplemented!",
    "debug_assert!(",
    "debug_assert_eq!(",
    ".unwrap(",
    ".expect(",
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
                if (marker == "debug_assert!(" || marker == "debug_assert_eq!(")
                    && line.contains("mi_debug_assert")
                {
                    continue;
                }
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

#[test]
fn production_profiles_abort_on_panic() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read Cargo.toml");

    assert!(
        manifest.contains("[profile.dev]\npanic = \"abort\""),
        "dev profile must abort instead of unwinding across C ABI callers"
    );
    assert!(
        manifest.contains("[profile.release]\npanic = \"abort\""),
        "release profile must abort instead of unwinding across C ABI callers"
    );
}

#[test]
fn mimalloc_assert_fail_aborts_instead_of_unwinding() {
    const CHILD_ENV: &str = "MI_FFI_PANIC_AUDIT_ASSERT_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
        unsafe {
            mechanical_mimalloc::raw::options::_mi_assert_fail(
                c"false".as_ptr(),
                c"ffi_panic_audit.rs".as_ptr(),
                line!(),
                c"mimalloc_assert_fail_aborts_instead_of_unwinding".as_ptr(),
            )
        };
    }

    let current_exe = std::env::current_exe().expect("current test executable path");
    let status = Command::new(current_exe)
        .arg("--exact")
        .arg("mimalloc_assert_fail_aborts_instead_of_unwinding")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .status()
        .expect("run assertion-abort child process");

    assert!(
        !status.success(),
        "assert failure child unexpectedly returned"
    );
    #[cfg(unix)]
    assert_eq!(status.signal(), Some(6), "expected SIGABRT from child");
}
