use super::*;

#[path = "benchmarking/release_notes.rs"]
mod release_notes;

#[test]
fn release_candidates_require_full_benchmark_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path| fs::read_to_string(root.join(path)).expect("read release evidence input");
    let release_md = read("docs/release.md");
    let performance_md = read("docs/performance.md");
    let benchmark_results_md = read("BENCHMARK_RESULTS.md");
    let check_bench_full = read("scripts/check-bench-full.sh");
    let manifest_writer = read("scripts/write-benchmark-manifest.py");
    let resource_thresholds = read("benchmarks/resource-thresholds.json");
    let threshold_checker = read("scripts/check-bench-thresholds.py");

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n   `scripts/env-info.sh`",
        "resource-results.jsonl",
        "evidence-report.json",
        "threshold-results.csv",
        "benchmark-review.json",
        "stages-0-3-evidence-<commit>.tar.zst",
        "artifact modes, Rust/C ratios, and noisy-benchmark notes",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require full benchmark evidence: {required}"
        );
    }
    for control in [
        "hotpath_sample_size",
        "native_sample_size",
        "native_scenarios",
        "resource_operations",
        "resource_repeats",
    ] {
        assert!(
            manifest_writer.contains(control),
            "benchmark manifest must preserve resolved control {control}"
        );
    }
    for metric in [
        "peak_commit_per_logical_peak_byte",
        "max_commit_end_minus_start_bytes",
        "peak_rss_per_logical_peak_byte",
        "page_faults_per_million_operations",
    ] {
        assert!(
            resource_thresholds.contains(metric),
            "resource thresholds must enforce {metric}"
        );
    }
    for budget in [
        "hot_path_median_ratio",
        "workload_median_ratio",
        "overall_geomean_ratio",
    ] {
        assert!(
            threshold_checker.contains(budget),
            "threshold checker must enforce investigation budget {budget}"
        );
    }
    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "manifest.json",
        "resource-results.jsonl",
        "evidence-report.json",
        "threshold-results.csv",
        "benchmark-review.json",
        "evidence-index.json",
        "Rust-vs-C ratios come from the native scenario benchmarks in the default Rust\nlibrary mode",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should define release candidate benchmark evidence: {required}"
        );
    }
    for required in [
        "CARGO_TARGET_DIR=\"$default_target_dir\"",
        "CARGO_TARGET_DIR=\"$interpose_target_dir\"",
        "--artifact-mode ld-preload-interpose",
        "--ld-preload",
        "--required-allocator preloaded-libc",
        "--csv-out \"$default_target_dir/threshold-results.csv\"",
        "--review-out \"$default_target_dir/benchmark-review.json\"",
        "scripts/check-evidence-report.py --report \"$default_target_dir/evidence-report.json\"",
        "--csv-out \"$interpose_target_dir/threshold-results.csv\"",
        "scripts/archive-benchmark-evidence.py",
        "cargo bench --features \"bench interpose\" --bench hotpath",
        "--allow-no-applicable",
    ] {
        assert!(
            check_bench_full.contains(required),
            "full benchmark script should keep release artifact modes separate: {required}"
        );
    }
    for required in [
        "## Release Candidate Artifact-Mode Evidence",
        "default Rust library",
        "interpose feature-linked hot path",
        "LD_PRELOAD interpose",
        "Do not reuse Criterion baselines across artifact modes.",
        "Rust-vs-C ratios emitted by `scripts/summarize-criterion.py`",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report template should identify artifact mode evidence: {required}"
        );
    }
}

#[test]
fn strict_wrapper_hot_path_changes_require_assembly_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let strict_types_md =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read docs/strict-types.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let capture_release_assembly =
        fs::read_to_string(root.join("scripts/capture-release-assembly.sh"))
            .expect("read capture-release-assembly.sh");

    for required in [
        "New strict internal wrappers on allocator hot paths require release assembly",
        "Run `scripts/capture-release-assembly.sh`",
        "free-list pop, local free, remote free, small\n  page lookup, and any changed hot path",
        "no added hot-path branch, call, load, store,\n  atomic memory operation, lock prefix, stack spill, formatting, allocation,\n  panic path, bounds-check slow path, or wrapper-constructor traffic",
        "rejects\n  dynamic dispatch in allocator, internal, platform, and strict-type modules",
    ] {
        assert!(
            strict_types_md.contains(required),
            "strict-type policy should gate new hot-path wrappers with assembly evidence: {required}"
        );
    }
    for required in [
        "A new strict internal wrapper changes release assembly for free-list pop",
        "local free, remote free, small page lookup",
        "adding a branch, call, memory operation, lock prefix, stack spill, formatting",
        "panic path, bounds-check slow path, or wrapper-constructor traffic",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should treat unproven strict-wrapper assembly changes as regressions: {required}"
        );
    }
    for required in [
        "For new strict internal wrappers on allocator hot paths",
        "`scripts/capture-release-assembly.sh` output covering free-list pop, local\n   free, remote free, small page lookup, and any changed hot path",
        "fail signoff\n   unless the evidence shows no added branch, call, memory traffic, stack spill",
        "panic path, bounds-check slow path, or wrapper\n   constructor traffic without measured justification",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should fail unproven strict-wrapper hot-path changes: {required}"
        );
    }

    for required in [
        "## Strict-Type Assembly Comparison Checklist",
        "Use `scripts/capture-release-assembly.sh` before and after each strict-type\nhot-path migration.",
        "`mi_malloc`",
        "`mi_malloc_small`",
        "`mi_free`",
        "`_mi_theap_malloc_zero_ex`",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report should preserve strict-wrapper assembly evidence guidance: {required}"
        );
    }

    for symbol in [
        "capture_symbol mi_malloc",
        "capture_symbol mi_malloc_small",
        "capture_symbol mi_free",
        "capture_symbol _mi_theap_malloc_zero_ex",
    ] {
        assert!(
            capture_release_assembly.contains(symbol),
            "assembly capture script should include strict-wrapper hot-path symbol: {symbol}"
        );
    }
}
