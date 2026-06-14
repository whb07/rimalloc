use std::env;
use std::path::PathBuf;
use std::process::Command;

fn compile_track_shim(defines: &[&str]) {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let obj = out_dir.join("track_shim.o");
    let lib = out_dir.join("libmechanical_mimalloc_track_shim.a");

    let mut cc = Command::new("cc");
    cc.arg("-c")
        .arg("src/track_shim.c")
        .arg("-o")
        .arg(&obj)
        .arg("-fPIC");
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

fn main() {
    println!("cargo:rerun-if-changed=src/track_shim.c");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_VALGRIND");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TRACK_ASAN");

    let valgrind = env::var_os("CARGO_FEATURE_TRACK_VALGRIND").is_some();
    let asan = env::var_os("CARGO_FEATURE_TRACK_ASAN").is_some();

    match (valgrind, asan) {
        (true, false) => compile_track_shim(&["MI_TRACK_VALGRIND=1"]),
        (false, true) => compile_track_shim(&["MI_TRACK_ASAN=1"]),
        _ => {}
    }
}
