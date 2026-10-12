#!/usr/bin/env python3
"""Summarize Criterion JSON output for benchmark reports.

Criterion writes structured JSON under target/criterion/<bench>/<param>/new.
This script reads those files directly instead of scraping terminal output.
Times are reported as per-iteration nanoseconds, matching Criterion's raw
metric.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from statistics import mean, median


def percentile(values: list[float], pct: float) -> float:
    if not values:
        raise ValueError("empty percentile input")
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * pct
    low = math.floor(rank)
    high = math.ceil(rank)
    if low == high:
        return ordered[low]
    frac = rank - low
    return ordered[low] * (1.0 - frac) + ordered[high] * frac


def format_time(ns: float) -> str:
    if ns >= 1_000_000_000:
        return f"{ns / 1_000_000_000:.4g} s"
    if ns >= 1_000_000:
        return f"{ns / 1_000_000:.4g} ms"
    if ns >= 1_000:
        return f"{ns / 1_000:.4g} us"
    return f"{ns:.4g} ns"


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def collect_results(
    root: Path, prefixes: list[str], exact_ids: set[str], exact_order: list[str]
) -> list[dict[str, object]]:
    results_by_id: dict[str, dict[str, object]] = {}
    for sample_path in root.glob("**/new/sample.json"):
        new_dir = sample_path.parent
        bench_path = new_dir / "benchmark.json"
        estimates_path = new_dir / "estimates.json"
        if not bench_path.exists() or not estimates_path.exists():
            continue

        benchmark = load_json(bench_path)
        sample = load_json(sample_path)
        estimates = load_json(estimates_path)

        full_id = str(benchmark.get("full_id", ""))
        if exact_ids and full_id not in exact_ids:
            continue
        if prefixes and not any(full_id.startswith(prefix) for prefix in prefixes):
            continue

        iters = [float(value) for value in sample["iters"]]
        times = [float(value) for value in sample["times"]]
        per_iter = [time / iteration for time, iteration in zip(times, iters) if iteration > 0]
        if not per_iter:
            continue

        mean_estimate = float(estimates["mean"]["point_estimate"])
        median_estimate = float(estimates["median"]["point_estimate"])
        ci = estimates["mean"]["confidence_interval"]
        ci_low = float(ci["lower_bound"])
        ci_high = float(ci["upper_bound"])
        ci_width_pct = ((ci_high - ci_low) / mean_estimate * 100.0) if mean_estimate else 0.0
        p95 = percentile(per_iter, 0.95)
        min_value = min(per_iter)
        max_value = max(per_iter)
        spread_pct = ((p95 - min_value) / median_estimate * 100.0) if median_estimate else 0.0
        noisy = ci_width_pct > 5.0 or spread_pct > 10.0

        result = {
                "full_id": full_id,
                "group_id": str(benchmark.get("group_id", "")),
                "value": str(benchmark.get("value_str", "")),
                "sample_path": str(sample_path),
                "sample_mtime": sample_path.stat().st_mtime,
                "samples": len(per_iter),
                "mean": mean_estimate,
                "median": median_estimate,
                "sample_mean": mean(per_iter),
                "sample_median": median(per_iter),
                "min": min_value,
                "max": max_value,
                "p95": p95,
                "ci_width_pct": ci_width_pct,
                "spread_pct": spread_pct,
                "noisy": noisy,
            }
        old = results_by_id.get(full_id)
        if old is None or float(result["sample_mtime"]) > float(old["sample_mtime"]):
            results_by_id[full_id] = result

    if exact_ids:
        missing = sorted(exact_ids.difference(results_by_id))
        if missing:
            raise SystemExit(
                "missing Criterion results:\n" + "\n".join(f"- {item}" for item in missing)
            )

    order = {full_id: index for index, full_id in enumerate(exact_order)}
    return sorted(
        results_by_id.values(),
        key=lambda item: order.get(str(item["full_id"]), len(order)),
    )


def native_pairs(results: list[dict[str, object]]) -> list[dict[str, object]]:
    by_group: dict[str, dict[str, dict[str, object]]] = {}
    for result in results:
        full_id = str(result["full_id"])
        prefix = "mimalloc-bench-native/"
        if not full_id.startswith(prefix):
            continue
        rest = full_id.removeprefix(prefix)
        try:
            group, value = rest.rsplit("/", 1)
        except ValueError:
            continue
        by_group.setdefault(group, {})[value] = result

    pairs: list[dict[str, object]] = []
    for group, values in sorted(by_group.items()):
        rust = values.get("mechanical_mimalloc_raw") or values.get("rust")
        c = values.get("c-mimalloc-3.3.2")
        if not rust or not c:
            continue
        rust_median = float(rust["median"])
        c_median = float(c["median"])
        ratio = rust_median / c_median if c_median else math.nan
        pairs.append(
            {
                "group": group.removeprefix("mimalloc-bench-native/"),
                "rust": rust,
                "c": c,
                "ratio": ratio,
                "diff_pct": (ratio - 1.0) * 100.0,
            }
        )
    return pairs


def geomean_ratio(pairs: list[dict[str, object]]) -> float | None:
    ratios = [float(pair["ratio"]) for pair in pairs if float(pair["ratio"]) > 0.0]
    if not ratios:
        return None
    return math.exp(sum(math.log(ratio) for ratio in ratios) / len(ratios))


def print_markdown(results: list[dict[str, object]]) -> None:
    print("### Criterion Summary")
    print()
    print(
        "| Benchmark | Samples | Mean | Median | Min | P95 | Max | CI width | Noise |"
    )
    print("|---|---:|---:|---:|---:|---:|---:|---:|---|")
    for result in results:
        print(
            "| {full_id} | {samples} | {mean} | {median} | {min} | {p95} | {max} | {ci:.2f}% | {noise} |".format(
                full_id=result["full_id"],
                samples=result["samples"],
                mean=format_time(float(result["mean"])),
                median=format_time(float(result["median"])),
                min=format_time(float(result["min"])),
                p95=format_time(float(result["p95"])),
                max=format_time(float(result["max"])),
                ci=float(result["ci_width_pct"]),
                noise="noisy" if result["noisy"] else "stable",
            )
        )

    pairs = native_pairs(results)
    if not pairs:
        return

    print()
    print("### Rust/C Native Scenario Ratios")
    print()
    print("| Scenario | Rust median | C median | Rust vs C | Noise |")
    print("|---|---:|---:|---:|---|")
    for pair in pairs:
        rust = pair["rust"]
        c = pair["c"]
        noisy = bool(rust["noisy"]) or bool(c["noisy"])
        print(
            "| {scenario} | {rust} | {c} | {diff:+.2f}% | {noise} |".format(
                scenario=pair["group"],
                rust=format_time(float(rust["median"])),
                c=format_time(float(c["median"])),
                diff=float(pair["diff_pct"]),
                noise="noisy" if noisy else "stable",
            )
        )

    ratio = geomean_ratio(pairs)
    if ratio is not None:
        print()
        print(f"Native scenario geomean Rust/C median ratio: `{ratio:.4f}` ({(ratio - 1.0) * 100.0:+.2f}%).")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--criterion-dir",
        default="target/criterion",
        help="Criterion output directory",
    )
    parser.add_argument(
        "--prefix",
        action="append",
        default=[],
        help="Only include full_id values with this prefix; may be repeated",
    )
    parser.add_argument(
        "--id",
        action="append",
        default=[],
        help="Include this exact Criterion full_id; may be repeated",
    )
    parser.add_argument(
        "--id-file",
        help="Read exact Criterion full_id values, one per line",
    )
    args = parser.parse_args()

    exact_order = list(args.id)
    if args.id_file:
        id_file = Path(args.id_file)
        exact_order.extend(
            [
                line.strip()
                for line in id_file.read_text(encoding="utf-8").splitlines()
                if line.strip() and not line.lstrip().startswith("#")
            ]
        )
    exact_ids = set(exact_order)

    results = collect_results(Path(args.criterion_dir), args.prefix, exact_ids, exact_order)
    if not results:
        raise SystemExit("no Criterion results matched")
    print_markdown(results)


if __name__ == "__main__":
    main()
