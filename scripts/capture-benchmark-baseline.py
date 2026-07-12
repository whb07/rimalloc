#!/usr/bin/env python3
"""Aggregate five reviewed Criterion runs into a schema-2 baseline proposal."""

from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path


def load(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def criterion_values(root: Path) -> dict[str, tuple[float, float]]:
    values: dict[str, tuple[float, float]] = {}
    for benchmark_path in root.glob("**/new/benchmark.json"):
        sample_path = benchmark_path.parent / "sample.json"
        if not sample_path.exists():
            continue
        benchmark = load(benchmark_path)
        sample = load(sample_path)
        benchmark_id = str(benchmark.get("full_id", ""))
        iterations = sample.get("iters", [])
        times = sample.get("times", [])
        per_iteration = [
            float(time) / float(count)
            for count, time in zip(iterations, times, strict=True)
            if float(count) > 0.0
        ]
        if benchmark_id and per_iteration:
            values[benchmark_id] = (
                statistics.median(per_iteration), percentile(per_iteration, 0.95)
            )
    return values


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-dir", action="append", dest="runs", required=True)
    parser.add_argument("--thresholds", default="benchmarks/thresholds.json")
    parser.add_argument("--output", default="target/benchmark-baseline-proposal.json")
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    parser.add_argument("--host-profile", required=True)
    parser.add_argument("--write-reviewed-baseline", action="store_true")
    args = parser.parse_args()
    if len(args.runs) != 5:
        raise SystemExit("baseline capture requires exactly five complete runs")

    expected_features = sorted(args.features)
    manifests = []
    run_values = []
    for run_value in args.runs:
        run = Path(run_value)
        required = [
            "environment.txt", "manifest.json", "commands.txt", "criterion",
            "resource-results.jsonl", "threshold-results.csv",
        ]
        missing = [name for name in required if not (run / name).exists()]
        if missing:
            raise SystemExit(f"baseline run is incomplete ({run}): {', '.join(missing)}")
        manifest = load(run / "manifest.json")
        if manifest.get("dirty"):
            raise SystemExit(f"baseline run is dirty: {run}")
        if manifest.get("artifact_mode") != args.artifact_mode:
            raise SystemExit(f"baseline artifact mode mismatch: {run}")
        if manifest.get("features") != expected_features:
            raise SystemExit(f"baseline feature set mismatch: {run}")
        manifests.append(manifest)
        run_values.append(criterion_values(run / "criterion"))
    commits = {str(manifest["commit"]) for manifest in manifests}
    if len(commits) != 1:
        raise SystemExit("all baseline runs must use the same commit")

    existing = load(Path(args.thresholds))
    entries = existing.get("thresholds", [])
    proposed = []
    retained = []
    for entry in entries:
        if int(existing.get("schema", 1)) == 2 and (
            entry.get("artifact_mode") != args.artifact_mode
            or sorted(entry.get("feature_set", [])) != expected_features
        ):
            retained.append(entry)
            continue
        benchmark_id = str(entry["id"])
        samples = [values.get(benchmark_id) for values in run_values]
        if any(sample is None for sample in samples):
            raise SystemExit(f"baseline runs are incomplete for {benchmark_id}")
        medians = [sample[0] for sample in samples]
        p95s = [sample[1] for sample in samples]
        if int(existing.get("schema", 1)) == 1:
            median_limit = float(entry["max_ratio"])
            p95_limit = float(entry["max_ratio"])
        else:
            median_limit = float(entry["limits"]["median_ratio"])
            p95_limit = float(entry["limits"]["p95_ratio"])
        proposed.append({
            "id": benchmark_id,
            "artifact_mode": args.artifact_mode,
            "feature_set": expected_features,
            "category": str(entry.get("category", "")),
            "baseline": {
                "median_ns": statistics.median(medians),
                "p95_ns": statistics.median(p95s),
            },
            "limits": {"median_ratio": median_limit, "p95_ratio": p95_limit},
            "host_profile": args.host_profile,
            "baseline_commit": next(iter(commits)),
        })
    document = {
        "schema": 2,
        "notes": [
            "Baseline values are the median of five run medians and five run p95 values.",
            "Updates require manual review and --write-reviewed-baseline.",
        ],
        "thresholds": retained + proposed,
    }
    destination = Path(args.thresholds) if args.write_reviewed_baseline else Path(args.output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    action = "updated reviewed baseline" if args.write_reviewed_baseline else "wrote baseline proposal"
    print(f"{action}: {destination}")


if __name__ == "__main__":
    main()
