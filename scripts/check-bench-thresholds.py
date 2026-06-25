#!/usr/bin/env python3
"""Validate Criterion results against checked benchmark thresholds."""

from __future__ import annotations

import argparse
import csv
import json
import math
import tempfile
from pathlib import Path


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def collect_criterion(criterion_dir: Path) -> dict[str, dict[str, object]]:
    results: dict[str, dict[str, object]] = {}
    for benchmark_path in criterion_dir.glob("**/new/benchmark.json"):
        estimates_path = benchmark_path.parent / "estimates.json"
        sample_path = benchmark_path.parent / "sample.json"
        if not estimates_path.exists() or not sample_path.exists():
            continue
        benchmark = load_json(benchmark_path)
        estimates = load_json(estimates_path)
        full_id = str(benchmark.get("full_id", ""))
        if not full_id:
            continue
        median = estimates.get("median", {})
        mean = estimates.get("mean", {})
        median_ns = float(median["point_estimate"])
        mean_ns = float(mean["point_estimate"])
        results[full_id] = {
            "full_id": full_id,
            "median_ns": median_ns,
            "mean_ns": mean_ns,
            "benchmark_path": str(benchmark_path),
        }
    return results


def check_entry(entry: dict[str, object], result: dict[str, object]) -> dict[str, object]:
    full_id = str(entry["id"])
    baseline_ns = float(entry["baseline_median_ns"])
    max_ratio = float(entry["max_ratio"])
    median_ns = float(result["median_ns"])
    ratio = median_ns / baseline_ns if baseline_ns > 0.0 else math.inf
    passed = ratio <= max_ratio
    return {
        "id": full_id,
        "category": str(entry.get("category", "")),
        "baseline_median_ns": baseline_ns,
        "current_median_ns": median_ns,
        "max_ratio": max_ratio,
        "ratio": ratio,
        "passed": passed,
    }


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[
                "id",
                "category",
                "baseline_median_ns",
                "current_median_ns",
                "max_ratio",
                "ratio",
                "passed",
            ],
        )
        writer.writeheader()
        writer.writerows(rows)


def validate_thresholds(
    threshold_path: Path,
    criterion_dir: Path,
    csv_out: Path | None,
    allow_missing: bool,
) -> None:
    threshold_doc = load_json(threshold_path)
    entries = threshold_doc.get("thresholds", [])
    if not isinstance(entries, list) or not entries:
        raise SystemExit("threshold file must contain a non-empty `thresholds` array")

    results = collect_criterion(criterion_dir)
    rows: list[dict[str, object]] = []
    missing: list[str] = []
    failures: list[str] = []

    for entry in entries:
        full_id = str(entry["id"])
        result = results.get(full_id)
        if result is None:
            missing.append(full_id)
            continue
        row = check_entry(entry, result)
        rows.append(row)
        if not bool(row["passed"]):
            failures.append(
                "{id}: median {current_median_ns:.3f} ns is {ratio:.3f}x baseline "
                "{baseline_median_ns:.3f} ns, max allowed {max_ratio:.3f}x".format(**row)
            )

    if csv_out is not None:
        write_csv(csv_out, rows)

    if missing and not allow_missing:
        raise SystemExit("missing Criterion results:\n" + "\n".join(f"- {item}" for item in missing))
    if failures:
        raise SystemExit("benchmark regressions exceeded thresholds:\n" + "\n".join(failures))
    if not rows:
        raise SystemExit("no thresholded Criterion results were validated")

    categories = sorted({str(row["category"]) for row in rows})
    print(
        f"ok: validated {len(rows)} benchmark thresholds across categories: "
        + ", ".join(categories)
    )


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        result_dir = root / "criterion" / "synthetic" / "new"
        result_dir.mkdir(parents=True)
        (result_dir / "benchmark.json").write_text(
            json.dumps({"full_id": "synthetic/regression"}), encoding="utf-8"
        )
        (result_dir / "sample.json").write_text(
            json.dumps({"iters": [1], "times": [130.0]}), encoding="utf-8"
        )
        (result_dir / "estimates.json").write_text(
            json.dumps(
                {
                    "median": {"point_estimate": 130.0},
                    "mean": {"point_estimate": 130.0},
                }
            ),
            encoding="utf-8",
        )
        thresholds = root / "thresholds.json"
        thresholds.write_text(
            json.dumps(
                {
                    "thresholds": [
                        {
                            "id": "synthetic/regression",
                            "category": "self-test",
                            "baseline_median_ns": 100.0,
                            "max_ratio": 1.10,
                        }
                    ]
                }
            ),
            encoding="utf-8",
        )
        try:
            validate_thresholds(thresholds, root / "criterion", None, allow_missing=False)
        except SystemExit as err:
            if "benchmark regressions exceeded thresholds" in str(err):
                print("ok: synthetic benchmark regression self-test failed as expected")
                return
            raise
        raise SystemExit("synthetic benchmark regression unexpectedly passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--thresholds", default="benchmarks/thresholds.json")
    parser.add_argument("--criterion-dir", default="target/criterion")
    parser.add_argument("--csv-out")
    parser.add_argument("--allow-missing", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    validate_thresholds(
        Path(args.thresholds),
        Path(args.criterion_dir),
        Path(args.csv_out) if args.csv_out else None,
        args.allow_missing,
    )


if __name__ == "__main__":
    main()
