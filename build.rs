use std::env;
use std::path::PathBuf;
use std::process::Command;

const PUBLIC_HEADERS: [&str; 2] = [
    "mimalloc-3.3.2/include/mimalloc.h",
    "mimalloc-3.3.2/include/mimalloc-stats.h",
];
const CXX_EXPORT_PATTERNS: [&str; 4] = ["_Zda*", "_Zdl*", "_Zna*", "_Znw*"];
const MALLOC_EXPORT_PATTERNS: [&str; 13] = [
    "__libc_*",
    "__posix_memalign*",
    "_aligned_malloc*",
    "aligned_alloc*",
    "calloc*",
    "cfree*",
    "free*",
    "malloc*",
    "memalign*",
    "posix_memalign*",
    "pvalloc*",
    "realloc*",
    "valloc*",
];

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|err| {
        panic!("Cargo did not provide required build environment variable {name}: {err}")
    })
}

fn required_env_os(name: &str) -> std::ffi::OsString {
    env::var_os(name).unwrap_or_else(|| {
        panic!("Cargo did not provide required build environment variable {name}")
    })
}

fn tool_from_env(name: &str, default: &str) -> std::ffi::OsString {
    env::var_os(name).unwrap_or_else(|| default.into())
}

fn run_tool(description: &str, tool_env: &str, command: &mut Command) {
    let program = command.get_program().to_string_lossy().into_owned();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    let status = command.status().unwrap_or_else(|err| {
        panic!(
            "failed to run {description} tool `{program}` ({tool_env}); command `{program} {args}`: {err}. \
             Install the tool or set {tool_env} to a working executable."
        )
    });
    assert!(
        status.success(),
        "{description} tool `{program}` ({tool_env}) exited with {status}; command `{program} {args}`. \
         Check target toolchain support or set {tool_env} to a compatible executable."
    );
}

fn compile_track_shim(defines: &[&str], flags: &[&str]) {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let obj = out_dir.join("track_shim.o");
    let lib = out_dir.join("librimalloc_track_shim.a");

    let mut cc = target_compiler("CC", "cc");
    cc.arg("-c")
        .arg("src/track_shim.c")
        .arg("-o")
        .arg(&obj)
        .arg("-fPIC");
    for flag in flags {
        cc.arg(flag);
    }
    for define in defines {
        cc.arg(format!("-D{define}"));
    }
    run_tool("C track shim compiler", "CC", &mut cc);

    let mut ar = Command::new(tool_from_env("AR", "ar"));
    ar.arg("crs").arg(&lib).arg(&obj);
    run_tool("track shim archiver", "AR", &mut ar);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=rimalloc_track_shim");
}

fn compile_cxx_new_shim(fast_new: bool) {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let obj = out_dir.join("cxx_new_shim.o");
    let lib = out_dir.join("librimalloc_cxx_new_shim.a");

    let mut cxx = target_compiler("CXX", "c++");
    cxx.arg("-std=c++17");
    if fast_new {
        cxx.arg("-DMI_CXX_USE_FAST_NEW=1");
    }
    cxx.arg("-c")
        .arg("src/cxx_new_shim.cpp")
        .arg("-o")
        .arg(&obj)
        .arg("-O3")
        .arg("-fPIC");
    run_tool("C++ new/delete shim compiler", "CXX", &mut cxx);

    let mut ar = Command::new(tool_from_env("AR", "ar"));
    ar.arg("crs").arg(&lib).arg(&obj);
    run_tool("C++ new/delete shim archiver", "AR", &mut ar);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive=rimalloc_cxx_new_shim");
}

/// A C or C++ compiler command for Cargo's target. On macOS `cc` otherwise
/// builds for the host architecture and running OS version, so pass the target
/// architecture and rustc's deployment target to match the Rust objects.
fn target_compiler(tool_env: &str, default: &str) -> Command {
    let mut compiler = Command::new(tool_from_env(tool_env, default));
    if required_env("CARGO_CFG_TARGET_OS") != "macos" {
        return compiler;
    }
    let target = required_env("TARGET");
    let mut rustc = Command::new(tool_from_env("RUSTC", "rustc"));
    rustc.args(["--print", "deployment-target", "--target", &target]);
    let output = rustc
        .output()
        .unwrap_or_else(|err| panic!("failed to query the {target} deployment target: {err}"));
    assert!(
        output.status.success(),
        "rustc could not report the {target} deployment target"
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    let version = printed
        .trim()
        .strip_prefix("MACOSX_DEPLOYMENT_TARGET=")
        .unwrap_or_else(|| panic!("unexpected rustc deployment target output: {printed}"));
    let arch = match required_env("CARGO_CFG_TARGET_ARCH").as_str() {
        "aarch64" => "arm64".to_owned(),
        arch => arch.to_owned(),
    };
    compiler
        .args(["-arch", &arch])
        .arg(format!("-mmacosx-version-min={version}"));
    compiler
}

fn compile_macos_interpose() {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let obj = out_dir.join("macos_interpose.o");
    let lib = out_dir.join("librimalloc_macos_interpose.a");
    let mut cc = target_compiler("CC", "cc");
    // Warnings are not fatal here: a newer SDK must not break downstream builds.
    // scripts/check-macos-interpose.py compiles this file with -Werror.
    cc.args(["-std=c11", "-O3", "-fPIC", "-Wall", "-Wextra"])
        .args([
            "-I",
            "mimalloc-3.3.2/include",
            "-c",
            "src/macos_interpose.c",
            "-o",
        ])
        .arg(&obj);
    run_tool("macOS interposition compiler", "CC", &mut cc);
    let mut ar = Command::new(tool_from_env("AR", "ar"));
    ar.arg("crs").arg(&lib).arg(&obj);
    run_tool("macOS interposition archiver", "AR", &mut ar);
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive=rimalloc_macos_interpose");
}

fn compile_tls_shim() {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let tls_obj = out_dir.join("tls_shim.o");
    let recurse_obj = out_dir.join("tls_recurse_shim.o");
    let lib = out_dir.join("librimalloc_tls_shim.a");

    let mut tls_cc = target_compiler("CC", "cc");
    tls_cc
        .arg("-c")
        .arg("src/tls_shim.c")
        .arg("-o")
        .arg(&tls_obj)
        .arg("-O3")
        .arg("-fPIC");
    run_tool("TLS shim compiler", "CC", &mut tls_cc);

    let mut recurse_cc = target_compiler("CC", "cc");
    recurse_cc
        .arg("-c")
        .arg("src/tls_recurse_shim.c")
        .arg("-o")
        .arg(&recurse_obj)
        .arg("-O3")
        .arg("-fPIC");
    run_tool("TLS recurse shim compiler", "CC", &mut recurse_cc);

    let mut ar = Command::new(tool_from_env("AR", "ar"));
    ar.arg("crs").arg(&lib).arg(&tls_obj).arg(&recurse_obj);
    run_tool("TLS shim archiver", "AR", &mut ar);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=rimalloc_tls_shim");
}

fn without_c_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_block = false;
    let mut in_line = false;

    while let Some(ch) = chars.next() {
        if in_block {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if in_line {
            if ch == '\n' {
                in_line = false;
                output.push('\n');
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block = true;
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line = true;
            continue;
        }
        output.push(ch);
    }

    output
}

fn public_mi_symbols_from_headers() -> Vec<String> {
    let mut symbols = Vec::new();
    for header in PUBLIC_HEADERS {
        let text = std::fs::read_to_string(header)
            .unwrap_or_else(|err| panic!("failed to read public header {header}: {err}"));
        for line in without_c_comments(&text).lines() {
            if !line.contains("mi_decl") {
                continue;
            }
            for token in line.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
                if token.starts_with("mi_")
                    && !token.starts_with("mi_decl")
                    && !token.starts_with("mi_attr")
                    && line.contains(&format!("{token}("))
                    && !symbols.iter().any(|existing| existing == token)
                {
                    symbols.push(token.to_owned());
                }
            }
        }
    }
    assert!(
        !symbols.is_empty(),
        "public header symbol allowlist generation produced no mi_* exports"
    );
    symbols
}

fn append_export_patterns(exports: &mut String, patterns: &[&str]) {
    for pattern in patterns {
        exports.push_str("    ");
        exports.push_str(pattern);
        exports.push_str(";\n");
    }
}

fn link_linux_cdylib_exports(interpose: bool, cxx_shim_linked: bool) {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let version_script = out_dir.join("mimalloc.exports");
    let mut exports = String::from(
        r"{
  global:
",
    );
    if cxx_shim_linked {
        append_export_patterns(&mut exports, &CXX_EXPORT_PATTERNS);
    }
    if interpose {
        append_export_patterns(&mut exports, &MALLOC_EXPORT_PATTERNS);
    }
    for symbol in public_mi_symbols_from_headers() {
        exports.push_str("    ");
        exports.push_str(&symbol);
        exports.push_str(";\n");
    }
    exports.push_str(
        r"  local:
    *;
};
",
    );

    std::fs::write(&version_script, exports).expect("failed to write Linux export script");
    println!("cargo:rustc-link-arg=-Wl,--undefined-version");
    println!(
        "cargo:rustc-link-arg=-Wl,--version-script={}",
        version_script.display()
    );
}

fn target_supports_cxx_shim(target_os: &str) -> bool {
    !matches!(target_os, "emscripten" | "wasi")
}

fn target_supports_tls_shim(target_os: &str) -> bool {
    target_os == "linux"
}

fn feature_enabled(name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}

fn main() {
    let target_os = required_env("CARGO_CFG_TARGET_OS");
    let target_arch = required_env("CARGO_CFG_TARGET_ARCH");
    let profile = required_env("PROFILE");
    let c_abi = feature_enabled("C_ABI") || feature_enabled("INTERPOSE");
    let cxx_overrides = feature_enabled("CXX_OVERRIDES") || feature_enabled("INTERPOSE");
    let tls_shim = feature_enabled("TLS_SHIM") || feature_enabled("INTERPOSE");
    let interpose = feature_enabled("INTERPOSE");
    let std = feature_enabled("STD");
    let macos_interpose = target_os == "macos" && interpose;
    // Unix C and Itanium C++ allocator names exported from Rust. Mach-O
    // two-level namespaces bind such definitions only to the defining image's
    // own calls, so on Apple targets they would mix two allocators: free() of a
    // libSystem buffer would leak and realloc() would fail. Apple targets
    // override through the `interpose` malloc zone instead.
    let apple = required_env("CARGO_CFG_TARGET_VENDOR") == "apple";
    let export_libc_aliases = !apple && (!c_abi || interpose);
    let export_cxx_aliases = !apple && (!c_abi || cxx_overrides);
    let default_fast_override = target_os == "linux"
        && profile == "release"
        && target_arch == "x86_64"
        && interpose
        && env::var_os("CARGO_FEATURE_SECURE").is_none()
        && env::var_os("CARGO_FEATURE_GUARDED").is_none()
        && env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_none()
        && env::var_os("CARGO_FEATURE_TRACK_ASAN").is_none();

    println!("cargo:rerun-if-changed=src/track_shim.c");
    println!("cargo:rerun-if-changed=src/macos_interpose.c");
    println!("cargo:rerun-if-changed=src/cxx_new_shim.cpp");
    println!("cargo:rerun-if-changed=src/tls_shim.c");
    println!("cargo:rerun-if-changed=src/tls_recurse_shim.c");
    for header in PUBLIC_HEADERS {
        println!("cargo:rerun-if-changed={header}");
    }
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_C_ABI");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_CXX_OVERRIDES");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TLS_SHIM");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_INTERPOSE");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_VALGRIND");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_ASAN");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SECURE");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GUARDED");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=CXX");
    println!("cargo:rerun-if-env-changed=AR");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    println!("cargo:rustc-check-cfg=cfg(mi_c_malloc_fast)");
    println!("cargo:rustc-check-cfg=cfg(mi_cxx_shim_linked)");
    println!("cargo:rustc-check-cfg=cfg(mi_osx_interpose)");
    println!("cargo:rustc-check-cfg=cfg(mi_export_libc_aliases)");
    println!("cargo:rustc-check-cfg=cfg(mi_export_cxx_aliases)");
    println!("cargo:rustc-check-cfg=cfg(mi_internal_c_probe)");
    println!("cargo:rustc-check-cfg=cfg(mi_sanitizer_runtime)");

    let cxx_shim_linked = cxx_overrides
        && !macos_interpose
        && target_os != "windows"
        && target_supports_cxx_shim(&target_os);
    if cxx_shim_linked {
        println!("cargo:rustc-cfg=mi_cxx_shim_linked");
        compile_cxx_new_shim(default_fast_override);
    } else if cxx_overrides && !macos_interpose && target_os != "windows" {
        println!("cargo:warning=skipping C++ new/delete shim on unsupported target_os={target_os}");
    }
    if target_os == "linux" {
        if c_abi {
            link_linux_cdylib_exports(interpose, cxx_shim_linked);
        }
        if default_fast_override {
            println!("cargo:rustc-cfg=mi_c_malloc_fast");
        }
        let needs_c_tls_shim = tls_shim || !std;
        if needs_c_tls_shim && target_supports_tls_shim(&target_os) {
            compile_tls_shim();
        } else if needs_c_tls_shim {
            println!("cargo:warning=skipping TLS shim on unsupported target_os={target_os}");
        }
    }
    if macos_interpose {
        println!("cargo:rustc-cfg=mi_osx_interpose");
        compile_macos_interpose();
    }
    if export_libc_aliases {
        println!("cargo:rustc-cfg=mi_export_libc_aliases");
    }
    if export_cxx_aliases {
        println!("cargo:rustc-cfg=mi_export_cxx_aliases");
    }

    let valgrind = env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_some();
    let asan = env::var_os("CARGO_FEATURE_TRACK_ASAN").is_some();

    match (valgrind, asan) {
        (true, _) => compile_track_shim(&["MI_TRACK_VALGRIND=1"], &[]),
        (false, true) => compile_track_shim(&["MI_TRACK_ASAN=1"], &["-fsanitize=address"]),
        (false, false) => {}
    }
}
