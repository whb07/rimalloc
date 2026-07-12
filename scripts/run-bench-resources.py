#!/usr/bin/env python3
"""Run fresh-process resource probes and derive normalized metrics."""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
from pathlib import Path


def ratio(numerator: int | None, denominator: int) -> float | None:
    if numerator is None or denominator == 0:
        return None
    return numerator / denominator


def difference(end: int | None, start: int | None) -> int | None:
    if end is None or start is None:
        return None
    return end - start


def enrich(record: dict[str, object]) -> None:
    live_peak = int(record["logical_live_bytes_peak"])
    operations = int(record["operations"])
    record["peak_rss_per_logical_peak_byte"] = ratio(record["rss_peak_end"], live_peak)
    record["peak_commit_per_logical_peak_byte"] = ratio(record["commit_peak_end"], live_peak)
    record["allocator_peak_committed_per_logical_peak_byte"] = ratio(
        record["allocator_committed_peak_end"], live_peak
    )
    record["rss_end_minus_start"] = difference(record["rss_current_end"], record["rss_current_start"])
    record["commit_end_minus_start"] = difference(
        record["commit_current_end"], record["commit_current_start"]
    )
    record["page_faults_per_million_operations"] = (
        int(record["page_faults_delta"]) * 1_000_000.0 / operations if operations else None
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    parser.add_argument("--allocator", action="append", dest="allocators")
    parser.add_argument("--scenario", action="append", dest="scenarios")
    parser.add_argument("--size", type=int, default=64)
    parser.add_argument("--alignment", type=int, default=8)
    parser.add_argument("--operations", type=int, default=100000)
    parser.add_argument("--live-set", type=int, default=4096)
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()

    artifact_dir = Path(args.artifact_dir).resolve()
    target_dir = artifact_dir
    feature_csv = ",".join(sorted(args.features))
    subprocess.run(
        ["cargo", "build", "--release", "--features", feature_csv, "--example", "benchmark_probe"],
        check=True,
        env={**os.environ, "CARGO_TARGET_DIR": str(target_dir)},
    )
    executable = target_dir / "release" / "examples" / "benchmark_probe"
    allocators = args.allocators or ["mechanical_mimalloc_public", "mechanical_mimalloc_raw", "system"]
    scenarios = args.scenarios or ["alloc_free", "remote_free_1x1"]
    records: list[dict[str, object]] = []
    for scenario in scenarios:
        for allocator in allocators:
            for repeat in range(args.repeats):
                threads = 2 if scenario == "remote_free_1x1" else 1
                environment = {
                    **os.environ,
                    "MIMALLOC_PROBE_SCENARIO": scenario,
                    "MIMALLOC_PROBE_ALLOCATOR": allocator,
                    "MIMALLOC_PROBE_ARTIFACT_MODE": args.artifact_mode,
                    "MIMALLOC_PROBE_FEATURES": feature_csv,
                    "MIMALLOC_PROBE_SIZE": str(args.size),
                    "MIMALLOC_PROBE_ALIGNMENT": str(args.alignment),
                    "MIMALLOC_PROBE_OPERATIONS": str(args.operations),
                    "MIMALLOC_PROBE_THREADS": str(threads),
                    "MIMALLOC_PROBE_LIVE_SET": str(args.live_set),
                }
                completed = subprocess.run(
                    [str(executable)], check=False, capture_output=True, text=True, env=environment
                )
                if completed.returncode != 0:
                    raise SystemExit(
                        f"probe failed for {scenario}/{allocator}: {completed.stderr.strip()}"
                    )
                record = json.loads(completed.stdout)
                record["exit_status"] = completed.returncode
                record["repeat"] = repeat
                enrich(record)
                records.append(record)

    groups: dict[tuple[str, str, int, int], list[dict[str, object]]] = {}
    for record in records:
        key = (
            str(record["scenario"]), str(record["allocator"]),
            int(record["size"]), int(record["threads"]),
        )
        groups.setdefault(key, []).append(record)
    for group in groups.values():
        walls = [float(record["wall_ns"]) for record in group]
        cv = statistics.pstdev(walls) / statistics.fmean(walls) if len(walls) > 1 else 0.0
        for record in group:
            record["wall_time_coefficient_of_variation"] = cv

    output = artifact_dir / "resource-results.jsonl"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    print(f"wrote {len(records)} resource records to {output}")


if __name__ == "__main__":
    main()
