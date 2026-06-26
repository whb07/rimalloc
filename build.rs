use std::env;
use std::path::PathBuf;
use std::process::Command;

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
    if !status.success() {
        panic!(
            "{description} tool `{program}` ({tool_env}) exited with {status}; command `{program} {args}`. \
             Check target toolchain support or set {tool_env} to a compatible executable."
        );
    }
}

fn compile_track_shim(defines: &[&str], flags: &[&str]) {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let obj = out_dir.join("track_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_track_shim.a");

    let mut cc = Command::new(tool_from_env("CC", "cc"));
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
    println!("cargo:rustc-link-lib=static=mechanical_mimalloc_track_shim");
}

fn compile_cxx_new_shim() {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let obj = out_dir.join("cxx_new_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_cxx_new_shim.a");

    let mut cxx = Command::new(tool_from_env("CXX", "c++"));
    cxx.arg("-std=c++17")
        .arg("-c")
        .arg("src/cxx_new_shim.cpp")
        .arg("-o")
        .arg(&obj)
        .arg("-fPIC");
    run_tool("C++ new/delete shim compiler", "CXX", &mut cxx);

    let mut ar = Command::new(tool_from_env("AR", "ar"));
    ar.arg("crs").arg(&lib).arg(&obj);
    run_tool("C++ new/delete shim archiver", "AR", &mut ar);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=mechanical_mimalloc_cxx_new_shim");
}

fn compile_tls_shim() {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let tls_obj = out_dir.join("tls_shim.o");
    let recurse_obj = out_dir.join("tls_recurse_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_tls_shim.a");

    let mut tls_cc = Command::new(tool_from_env("CC", "cc"));
    tls_cc
        .arg("-c")
        .arg("src/tls_shim.c")
        .arg("-o")
        .arg(&tls_obj)
        .arg("-O3")
        .arg("-fPIC");
    run_tool("TLS shim compiler", "CC", &mut tls_cc);

    let mut recurse_cc = Command::new(tool_from_env("CC", "cc"));
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
    println!("cargo:rustc-link-lib=static=mechanical_mimalloc_tls_shim");
}

fn link_linux_cdylib_exports() {
    let out_dir = PathBuf::from(required_env_os("OUT_DIR"));
    let version_script = out_dir.join("mimalloc.exports");
    let exports = r"{
  global:
    _Zda*;
    _Zdl*;
    _Zna*;
    _Znw*;
    __libc_*;
    __posix_memalign*;
    _aligned_malloc*;
    aligned_alloc*;
    calloc*;
    cfree*;
    free*;
    malloc*;
    memalign*;
    posix_memalign*;
    pvalloc*;
    realloc*;
    valloc*;
    mi_*;
  local:
    *;
};
";

    std::fs::write(&version_script, exports).expect("failed to write Linux export script");
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

fn main() {
    let target_os = required_env("CARGO_CFG_TARGET_OS");
    let target_arch = required_env("CARGO_CFG_TARGET_ARCH");
    let profile = required_env("PROFILE");

    println!("cargo:rerun-if-changed=src/track_shim.c");
    println!("cargo:rerun-if-changed=src/cxx_new_shim.cpp");
    println!("cargo:rerun-if-changed=src/tls_shim.c");
    println!("cargo:rerun-if-changed=src/tls_recurse_shim.c");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_VALGRIND");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_ASAN");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SECURE");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GUARDED");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=CXX");
    println!("cargo:rerun-if-env-changed=AR");
    println!("cargo:rustc-check-cfg=cfg(mi_c_malloc_fast)");
    println!("cargo:rustc-check-cfg=cfg(mi_osx_interpose)");
    println!("cargo:rustc-check-cfg=cfg(mi_internal_c_probe)");
    println!("cargo:rustc-check-cfg=cfg(mi_sanitizer_runtime)");

    if target_os != "windows" && target_supports_cxx_shim(&target_os) {
        compile_cxx_new_shim();
    } else if target_os != "windows" {
        println!("cargo:warning=skipping C++ new/delete shim on unsupported target_os={target_os}");
    }
    if target_os == "linux" {
        link_linux_cdylib_exports();
        let default_fast_override = profile == "release"
            && target_arch == "x86_64"
            && env::var_os("CARGO_FEATURE_SECURE").is_none()
            && env::var_os("CARGO_FEATURE_GUARDED").is_none()
            && env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_none()
            && env::var_os("CARGO_FEATURE_TRACK_ASAN").is_none();
        if default_fast_override {
            println!("cargo:rustc-cfg=mi_c_malloc_fast");
        }
        if target_supports_tls_shim(&target_os) {
            compile_tls_shim();
        } else {
            println!("cargo:warning=skipping TLS shim on unsupported target_os={target_os}");
        }
    }
    if target_os == "macos" && profile == "release" {
        println!("cargo:rustc-cfg=mi_osx_interpose");
    }

    let valgrind = env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_some();
    let asan = env::var_os("CARGO_FEATURE_TRACK_ASAN").is_some();

    match (valgrind, asan) {
        (true, _) => compile_track_shim(&["MI_TRACK_VALGRIND=1"], &[]),
        (false, true) => compile_track_shim(&["MI_TRACK_ASAN=1"], &["-fsanitize=address"]),
        (false, false) => {}
    }
}
