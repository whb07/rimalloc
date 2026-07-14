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
        "${MIMALLOC_BENCH_COOLDOWN_SECS:=60}",
        "${MIMALLOC_BENCH_CPU:=0}",
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
        "${MIMALLOC_BASELINE_COOLDOWN_SECS:-60}",
        "${MIMALLOC_BASELINE_CPU:-0}",
    ] {
        assert!(
            baseline.contains(control),
            "missing baseline control {control}"
        );
    }
    assert!(
        full.contains("sleep $MIMALLOC_BENCH_COOLDOWN_SECS"),
        "full gate must cool before the sustained native group"
    );
    assert!(
        baseline.contains("sleep $cooldown_secs"),
        "baseline runner must cool before the sustained native group"
    );
    assert!(
        full.contains("taskset --cpu-list $MIMALLOC_BENCH_CPU"),
        "full gate must pin its selected single-thread threshold probe"
    );
    assert!(
        baseline.contains("taskset --cpu-list $benchmark_cpu"),
        "baseline runner must pin its selected single-thread threshold probe"
    );
}
