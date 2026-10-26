#!/usr/bin/env python3
"""Run fresh-process resource probes and derive normalized metrics."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


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


def build_ld_preload_probe(
    artifact_dir: Path, existing_library: Path | None = None
) -> tuple[Path, Path]:
    if not sys.platform.startswith("linux"):
        raise SystemExit("ld-preload-interpose resource evidence requires Linux")
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")
    if existing_library is None:
        environment = {**os.environ, "CARGO_TARGET_DIR": str(artifact_dir)}
        subprocess.run(
            [
                "cargo", "rustc", "--release", "--features", "interpose", "--lib", "--",
                "--crate-type", "cdylib",
            ],
            check=True,
            cwd=ROOT,
            env=environment,
        )
        libraries = list((artifact_dir / "release").glob("librimalloc*.so"))
        libraries.extend(
            (artifact_dir / "release" / "deps").glob("librimalloc*.so")
        )
        if not libraries:
            raise SystemExit("release interpose cdylib was not produced")
        library = max(libraries, key=lambda path: path.stat().st_mtime)
        deployed_library = artifact_dir / "interpose-cdylib.so"
        shutil.copy2(library, deployed_library)
    else:
        deployed_library = existing_library.resolve()
        if not deployed_library.is_file():
            raise SystemExit(f"preload library does not exist: {deployed_library}")
    executable = artifact_dir / "release" / "ld_preload_resource_probe"
    executable.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            cc, "-O2", "-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread",
            str(ROOT / "benchmarks" / "ld_preload_resource_probe.c"),
            "-Wl,--no-as-needed", "-lstdc++", "-Wl,--as-needed", "-ldl", "-o",
            str(executable),
        ],
        check=True,
        cwd=ROOT,
    )
    return executable, deployed_library


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    parser.add_argument("--allocator", action="append", dest="allocators")
    parser.add_argument("--scenario", action="append", dest="scenarios")
    parser.add_argument("--size", type=int, default=64)
    parser.add_argument("--alignment", type=int, default=8)
    parser.add_argument("--operations", type=int, default=20_000_000)
    parser.add_argument("--live-set", type=int, default=4096)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--ld-preload", action="store_true")
    parser.add_argument("--preload-library")
    args = parser.parse_args()

    artifact_dir = Path(args.artifact_dir).resolve()
    target_dir = artifact_dir
    feature_csv = ",".join(sorted(args.features))
    preload_library = None
    existing_library = Path(args.preload_library) if args.preload_library else None
    if existing_library is not None and not args.ld_preload:
        raise SystemExit("--preload-library requires --ld-preload")
    if args.ld_preload:
        if existing_library is None and args.artifact_mode != "ld-preload-interpose":
            raise SystemExit("default --ld-preload build requires mode ld-preload-interpose")
        if args.features != ["interpose"]:
            raise SystemExit("--ld-preload requires feature interpose")
        if args.allocators and args.allocators != ["preloaded-libc"]:
            raise SystemExit("--ld-preload supports only the preloaded-libc allocator label")
        executable, preload_library = build_ld_preload_probe(artifact_dir, existing_library)
    else:
        subprocess.run(
            [
                "cargo", "build", "--release", "--features", feature_csv,
                "--example", "benchmark_probe",
            ],
            check=True,
            env={**os.environ, "CARGO_TARGET_DIR": str(target_dir)},
        )
        executable = target_dir / "release" / "examples" / "benchmark_probe"
    allocators = args.allocators or (
        ["preloaded-libc"] if args.ld_preload else
        ["rimalloc_public", "rimalloc_raw", "system"]
    )
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
                if preload_library is not None:
                    environment["LD_PRELOAD"] = str(preload_library)
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
