use super::*;

#[test]
fn release_candidates_require_full_benchmark_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let check_bench_full = fs::read_to_string(root.join("scripts/check-bench-full.sh"))
        .expect("read full bench script");

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n   `scripts/env-info.sh`",
        "target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv",
        "target/bench-full/interpose/criterion/benchmark-thresholds-full.csv",
        "artifact modes, Rust/C ratios, and noisy-benchmark notes",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require full benchmark evidence: {required}"
        );
    }

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n  `scripts/env-info.sh`",
        "target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv",
        "target/bench-full/interpose/criterion/benchmark-thresholds-full.csv",
        "Rust-vs-C ratios come from the native scenario\nbenchmarks in the default Rust library mode",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should define release candidate benchmark evidence: {required}"
        );
    }

    for required in [
        "CARGO_TARGET_DIR=\"$default_target_dir\"",
        "CARGO_TARGET_DIR=\"$interpose_target_dir\"",
        "--csv-out \"$default_target_dir/criterion/benchmark-thresholds-full.csv\"",
        "--csv-out \"$interpose_target_dir/criterion/benchmark-thresholds-full.csv\"",
        "cargo bench --features \"bench interpose\" --bench hotpath",
        "--allow-missing",
    ] {
        assert!(
            check_bench_full.contains(required),
            "full benchmark script should keep release artifact modes separate: {required}"
        );
    }

    for required in [
        "## Release Candidate Artifact-Mode Evidence",
        "default Rust library",
        "interpose hot path",
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

#[test]
fn release_notes_publish_per_release_performance_summaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let changelog_md = fs::read_to_string(root.join("CHANGELOG.md")).expect("read CHANGELOG.md");

    for required in [
        "## Per-Release Performance Summary Template",
        "native scenario geomean Rust/C median ratio",
        "hot-path deltas against previous release evidence",
        "release-blocking regressions, if any",
        "noisy watch items separated from release-blocking regressions",
        "links to raw Criterion summaries and threshold CSV artifacts",
        "Release notes must link to the matching `BENCHMARK_RESULTS.md` section",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report should define per-release performance summary fields: {required}"
        );
    }

    for required in [
        "Publish a per-release performance summary in `BENCHMARK_RESULTS.md`",
        "Include geomean Rust/C ratios, hot-path deltas against the\n  previous release evidence",
        "release-blocking regressions, noisy watch items",
        "links to raw Criterion summaries or threshold CSV artifacts",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should require per-release summaries: {required}"
        );
    }

    for required in [
        "Publish the per-release performance summary in `BENCHMARK_RESULTS.md`",
        "summarizing geomean Rust/C ratios, hot-path deltas against\n   previous release evidence",
        "noisy watch items, release-blocking regressions",
        "links to raw Criterion summaries or threshold CSV artifacts",
        "link the\n   release notes to `BENCHMARK_RESULTS.md` or the attached performance report",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require linked performance summaries: {required}"
        );
    }

    for required in [
        "Release notes must link to `BENCHMARK_RESULTS.md`",
        "geomean Rust/C ratios, hot-path deltas, noisy watch\n  items",
        "release-blocking regressions, and raw Criterion or threshold CSV\n  artifacts",
    ] {
        assert!(
            changelog_md.contains(required),
            "release notes should carry performance-summary link policy: {required}"
        );
    }
}
