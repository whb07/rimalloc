#![cfg(feature = "unstable-raw-internals")]

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

fn production_lines(contents: &str) -> Vec<(usize, &str)> {
    let mut lines = Vec::new();
    let mut skip_test_module = false;
    let mut pending_test_cfg = false;
    let mut brace_depth = 0usize;

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//!") || trimmed.starts_with("///") {
            continue;
        }

        if skip_test_module {
            brace_depth = brace_depth
                .saturating_add(line.matches('{').count())
                .saturating_sub(line.matches('}').count());
            if brace_depth == 0 {
                skip_test_module = false;
            }
            continue;
        }

        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test,") {
            pending_test_cfg = true;
            continue;
        }

        if pending_test_cfg
            && (trimmed.starts_with("mod ")
                || trimmed.starts_with("pub mod ")
                || trimmed.starts_with("pub(crate) mod "))
        {
            skip_test_module = true;
            brace_depth = line
                .matches('{')
                .count()
                .saturating_sub(line.matches('}').count());
            if brace_depth == 0 {
                skip_test_module = false;
            }
            pending_test_cfg = false;
            continue;
        }

        pending_test_cfg = false;
        lines.push((line_index, line));
    }

    lines
}

#[test]
fn production_sources_do_not_use_panic_prone_shortcuts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources_under(&root, &mut sources);

    let mut hits = Vec::new();
    for path in sources {
        let contents = fs::read_to_string(&path).expect("read source file");
        for (line_index, line) in production_lines(&contents) {
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
fn callback_unwind_policy_is_documented_and_safe_guards_catch_panics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw_rs = fs::read_to_string(root.join("src/raw.rs")).expect("read src/raw.rs");
    let types_rs = fs::read_to_string(root.join("src/types.rs")).expect("read src/types.rs");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let safe_rs = fs::read_to_string(root.join("src/safe.rs")).expect("read src/safe.rs");
    let safe_callbacks_rs =
        fs::read_to_string(root.join("src/safe/callbacks.rs")).expect("read safe callbacks");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");

    assert!(
        raw_rs.contains("Raw callbacks registered through this facade must not panic or unwind")
            && abi_doc.contains("Callbacks must not unwind across the C ABI boundary.")
            && abi_doc.contains(
                "catch panic or unwind before returning across the\ncallback C ABI boundary"
            ),
        "raw facade and ABI docs should state that callbacks must not unwind"
    );
    assert!(
        types_rs.matches("Unwind:").count() >= 6
            && types_rs.matches("panic").count() >= 6
            && types_rs.matches("unwind").count() >= 6,
        "public callback type docs should include panic/unwind behavior"
    );

    assert!(
        lib_rs.contains("ErrorCallbackGuard")
            && lib_rs.contains("DeferredFreeCallbackGuard")
            && safe_rs.contains(
                "pub use callbacks::{CallbackError, DeferredFreeCallbackGuard, ErrorCallbackGuard};"
            )
            && safe_callbacks_rs.contains("pub struct ErrorCallbackGuard")
            && safe_callbacks_rs.contains("pub struct DeferredFreeCallbackGuard"),
        "safe callback guards should be explicit crate-root APIs"
    );
    assert!(
        safe_callbacks_rs.contains("std::panic::catch_unwind")
            && safe_callbacks_rs.contains("error_callback_trampoline")
            && safe_callbacks_rs.contains("deferred_free_callback_trampoline")
            && safe_callbacks_rs.contains("mi_register_error")
            && safe_callbacks_rs.contains("mi_register_deferred_free"),
        "safe callback guards should catch Rust panics before callback ABI return"
    );
    assert!(
        !safe_callbacks_rs.contains("mi_register_output"),
        "safe API should not expose an output callback guard until output recursion semantics are designed"
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
