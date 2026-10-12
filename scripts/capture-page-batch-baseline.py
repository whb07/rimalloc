#!/usr/bin/env python3
"""Propose or promote reviewed plain/secure page-batch thresholds."""

from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path


MODES = {
    "page-batch-plain": ["bench", "bench-diagnostics"],
    "page-batch-secure": ["bench", "bench-diagnostics", "secure"],
}


def load(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def criterion_values(root: Path) -> dict[str, tuple[float, float, int]]:
    values = {}
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
        if benchmark_id.startswith("page_batch/") and per_iteration:
            values[benchmark_id] = (
                statistics.median(per_iteration),
                percentile(per_iteration, 0.95),
                len(per_iteration),
            )
    return values


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", action="append", required=True)
    parser.add_argument("--thresholds", default="benchmarks/thresholds.json")
    parser.add_argument("--output", default="target/page-batch-baseline-proposal.json")
    parser.add_argument("--host-profile", required=True)
    parser.add_argument("--median-limit", type=float, default=1.15)
    parser.add_argument("--p95-limit", type=float, default=1.25)
    parser.add_argument("--write-reviewed-baseline", action="store_true")
    args = parser.parse_args()
    if len(args.artifact_dir) != len(MODES):
        raise SystemExit("provide exactly one clean artifact directory for each page-batch mode")
    if min(args.median_limit, args.p95_limit) < 1.0:
        raise SystemExit("hard limits must be ratios of at least 1.0")

    manifests = {}
    measurements = {}
    for value in args.artifact_dir:
        artifact = Path(value)
        for required in ("environment.txt", "manifest.json", "commands.txt", "criterion"):
            if not (artifact / required).exists():
                raise SystemExit(f"incomplete page-batch artifact {artifact}: missing {required}")
        manifest = load(artifact / "manifest.json")
        mode = str(manifest.get("artifact_mode", ""))
        if mode not in MODES or mode in manifests:
            raise SystemExit(f"duplicate or unexpected page-batch mode: {mode or '<missing>'}")
        if manifest.get("dirty"):
            raise SystemExit(f"page-batch artifact was captured from a dirty tree: {artifact}")
        if manifest.get("features") != MODES[mode]:
            raise SystemExit(f"page-batch feature mismatch for {mode}")
        values = criterion_values(artifact / "criterion")
        if not values:
            raise SystemExit(f"no page-batch Criterion samples found in {artifact}")
        manifests[mode] = manifest
        measurements[mode] = values

    if set(manifests) != set(MODES):
        raise SystemExit("both plain and secure page-batch modes are required")
    commits = {str(manifest["commit"]) for manifest in manifests.values()}
    if len(commits) != 1:
        raise SystemExit("page-batch artifacts must come from the same commit")
    controls = {
        json.dumps(manifest.get("controls"), sort_keys=True)
        for manifest in manifests.values()
    }
    if len(controls) != 1:
        raise SystemExit("page-batch artifacts must use identical benchmark controls")

    current = load(Path(args.thresholds))
    retained = [
        entry
        for entry in current.get("thresholds", [])
        if entry.get("artifact_mode") not in MODES
    ]
    proposed = []
    commit = next(iter(commits))
    for mode in sorted(MODES):
        for benchmark_id, (median, p95, sample_size) in sorted(measurements[mode].items()):
            proposed.append({
                "id": benchmark_id,
                "artifact_mode": mode,
                "feature_set": MODES[mode],
                "category": "page-batch slow path",
                "baseline": {"median_ns": median, "p95_ns": p95},
                "limits": {
                    "median_ratio": args.median_limit,
                    "p95_ratio": args.p95_limit,
                },
                "host_profile": args.host_profile,
                "baseline_commit": commit,
                "review": {"criterion_sample_size": sample_size},
            })
    document = {**current, "thresholds": retained + proposed}
    destination = Path(args.thresholds) if args.write_reviewed_baseline else Path(args.output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    action = "updated reviewed baseline" if args.write_reviewed_baseline else "wrote baseline proposal"
    print(f"{action}: {destination} ({len(proposed)} page-batch thresholds)")


if __name__ == "__main__":
    main()
