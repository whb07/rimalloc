use std::env;
use std::path::PathBuf;
use std::process::Command;

fn compile_track_shim(defines: &[&str], flags: &[&str]) {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let obj = out_dir.join("track_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_track_shim.a");

    let mut cc = Command::new("cc");
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
    let status = cc.status().expect("failed to run cc for track shim");
    assert!(status.success(), "cc failed to compile track shim");

    let status = Command::new("ar")
        .arg("crus")
        .arg(&lib)
        .arg(&obj)
        .status()
        .expect("failed to run ar for track shim");
    assert!(status.success(), "ar failed to archive track shim");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=mechanical_mimalloc_track_shim");
}

fn compile_cxx_new_shim() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let obj = out_dir.join("cxx_new_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_cxx_new_shim.a");

    let cxx = env::var_os("CXX").unwrap_or_else(|| "c++".into());
    let status = Command::new(cxx)
        .arg("-std=c++17")
        .arg("-c")
        .arg("src/cxx_new_shim.cpp")
        .arg("-o")
        .arg(&obj)
        .arg("-fPIC")
        .status()
        .expect("failed to run C++ compiler for new/delete shim");
    assert!(
        status.success(),
        "C++ compiler failed to compile new/delete shim"
    );

    let status = Command::new("ar")
        .arg("crus")
        .arg(&lib)
        .arg(&obj)
        .status()
        .expect("failed to run ar for C++ new/delete shim");
    assert!(status.success(), "ar failed to archive C++ new/delete shim");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=mechanical_mimalloc_cxx_new_shim");
}

fn compile_tls_shim() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let obj = out_dir.join("tls_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_tls_shim.a");

    let status = Command::new("cc")
        .arg("-c")
        .arg("src/tls_shim.c")
        .arg("-o")
        .arg(&obj)
        .arg("-O3")
        .arg("-fPIC")
        .status()
        .expect("failed to run cc for TLS shim");
    assert!(status.success(), "cc failed to compile TLS shim");

    let status = Command::new("ar")
        .arg("crus")
        .arg(&lib)
        .arg(&obj)
        .status()
        .expect("failed to run ar for TLS shim");
    assert!(status.success(), "ar failed to archive TLS shim");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=mechanical_mimalloc_tls_shim");
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("target OS is set by cargo");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target arch is set by cargo");
    let profile = env::var("PROFILE").expect("profile is set by cargo");

    println!("cargo:rerun-if-changed=src/track_shim.c");
    println!("cargo:rerun-if-changed=src/cxx_new_shim.cpp");
    println!("cargo:rerun-if-changed=src/tls_shim.c");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_VALGRIND");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_ASAN");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SECURE");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GUARDED");
    println!("cargo:rerun-if-env-changed=CXX");
    println!("cargo:rustc-check-cfg=cfg(mi_c_malloc_fast)");

    if target_os != "windows" {
        compile_cxx_new_shim();
    }
    if target_os == "linux" {
        let default_fast_override = profile == "release"
            && target_arch == "x86_64"
            && env::var_os("CARGO_FEATURE_SECURE").is_none()
            && env::var_os("CARGO_FEATURE_GUARDED").is_none()
            && env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_none()
            && env::var_os("CARGO_FEATURE_TRACK_ASAN").is_none();
        if default_fast_override {
            println!("cargo:rustc-cfg=mi_c_malloc_fast");
        }
        compile_tls_shim();
    }

    let valgrind = env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_some();
    let asan = env::var_os("CARGO_FEATURE_TRACK_ASAN").is_some();

    match (valgrind, asan) {
        (true, false) => compile_track_shim(&["MI_TRACK_VALGRIND=1"], &[]),
        (false, true) => compile_track_shim(&["MI_TRACK_ASAN=1"], &["-fsanitize=address"]),
        _ => {}
    }
}
