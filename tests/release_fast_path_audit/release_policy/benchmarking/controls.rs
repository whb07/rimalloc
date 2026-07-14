use super::*;

#[test]
fn full_gate_and_baseline_controls_stay_methodology_equivalent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let full = fs::read_to_string(root.join("scripts/check-bench-full.sh"))
        .expect("read full benchmark gate");
    let baseline = fs::read_to_string(root.join("scripts/run-benchmark-baseline.sh"))
        .expect("read benchmark baseline runner");

    for control in [
        "${MIMALLOC_BENCH_SAMPLE_SIZE:=20}",
        "${MIMALLOC_BENCH_MEASUREMENT_SECS:=5}",
        "${MIMALLOC_BENCH_WARMUP_SECS:=2}",
        "${MIMALLOC_HOTPATH_SAMPLE_SIZE:=100}",
        "${MIMALLOC_HOTPATH_MEASUREMENT_SECS:=10}",
        "${MIMALLOC_HOTPATH_WARMUP_SECS:=1}",
    ] {
        assert!(
            full.contains(control),
            "missing full-gate control {control}"
        );
    }
    for control in [
        "${MIMALLOC_BASELINE_NATIVE_SAMPLE_SIZE:-20}",
        "${MIMALLOC_BASELINE_NATIVE_MEASUREMENT_SECS:-5}",
        "${MIMALLOC_BASELINE_NATIVE_WARMUP_SECS:-2}",
        "${MIMALLOC_BASELINE_HOTPATH_SAMPLE_SIZE:-100}",
        "${MIMALLOC_BASELINE_HOTPATH_MEASUREMENT_SECS:-10}",
        "${MIMALLOC_BASELINE_HOTPATH_WARMUP_SECS:-1}",
    ] {
        assert!(
            baseline.contains(control),
            "missing baseline control {control}"
        );
    }
}
