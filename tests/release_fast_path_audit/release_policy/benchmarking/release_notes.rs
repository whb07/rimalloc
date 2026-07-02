use super::*;

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
