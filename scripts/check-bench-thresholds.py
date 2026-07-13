#!/usr/bin/env python3
"""Validate artifact-specific Criterion results against benchmark thresholds."""

from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import tempfile
from pathlib import Path


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def collect_criterion(criterion_dir: Path) -> dict[str, dict[str, float]]:
    results: dict[str, dict[str, float]] = {}
    for benchmark_path in criterion_dir.glob("**/new/benchmark.json"):
        sample_path = benchmark_path.parent / "sample.json"
        if not sample_path.exists():
            continue
        benchmark = load_json(benchmark_path)
        sample = load_json(sample_path)
        full_id = str(benchmark.get("full_id", ""))
        iterations = sample.get("iters", [])
        times = sample.get("times", [])
        if not full_id or not iterations or len(iterations) != len(times):
            continue
        per_iteration = [
            float(time) / float(count)
            for count, time in zip(iterations, times, strict=True)
            if float(count) > 0.0
        ]
        if not per_iteration:
            continue
        results[full_id] = {
            "median_ns": statistics.median(per_iteration),
            "mean_ns": statistics.fmean(per_iteration),
            "min_ns": min(per_iteration),
            "p95_ns": percentile(per_iteration, 0.95),
            "max_ns": max(per_iteration),
        }
    return results


def normalize_entry(entry: dict[str, object], schema: int) -> dict[str, object]:
    if schema == 1:
        return {
            "id": str(entry["id"]),
            "category": str(entry.get("category", "")),
            "artifact_mode": None,
            "feature_set": None,
            "baseline_median_ns": float(entry["baseline_median_ns"]),
            "baseline_p95_ns": None,
            "median_ratio_limit": float(entry["max_ratio"]),
            "p95_ratio_limit": None,
        }
    if schema != 2:
        raise ValueError(f"unsupported threshold schema: {schema}")
    baseline = entry.get("baseline")
    limits = entry.get("limits")
    if not isinstance(baseline, dict) or not isinstance(limits, dict):
        raise ValueError(f"{entry.get('id', '<unknown>')}: malformed baseline or limits")
    median = float(baseline["median_ns"])
    p95 = float(baseline["p95_ns"])
    median_limit = float(limits["median_ratio"])
    p95_limit = float(limits["p95_ratio"])
    if min(median, p95, median_limit, p95_limit) <= 0.0:
        raise ValueError(f"{entry.get('id', '<unknown>')}: baseline values and limits must be positive")
    feature_set = entry.get("feature_set")
    if not isinstance(feature_set, list) or not feature_set:
        raise ValueError(f"{entry.get('id', '<unknown>')}: feature_set must be non-empty")
    return {
        "id": str(entry["id"]),
        "category": str(entry.get("category", "")),
        "artifact_mode": str(entry["artifact_mode"]),
        "feature_set": sorted(str(value) for value in feature_set),
        "baseline_median_ns": median,
        "baseline_p95_ns": p95,
        "median_ratio_limit": median_limit,
        "p95_ratio_limit": p95_limit,
    }


CSV_FIELDS = [
    "id", "category", "artifact_mode", "feature_set", "status",
    "baseline_median_ns", "current_median_ns", "median_ratio", "median_ratio_limit",
    "investigation_ratio_limit", "review_required",
    "baseline_p95_ns", "current_p95_ns", "p95_ratio", "p95_ratio_limit",
    "current_mean_ns", "current_min_ns", "current_max_ns", "passed",
]


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_FIELDS)
        writer.writeheader()
        writer.writerows(rows)


def validate_thresholds(
    threshold_path: Path,
    criterion_dir: Path,
    csv_out: Path | None,
    allow_missing: bool,
    artifact_mode: str | None,
    feature_set: list[str] | None,
    report_only: bool = False,
    allow_no_applicable: bool = False,
    review_out: Path | None = None,
) -> None:
    threshold_doc = load_json(threshold_path)
    if not isinstance(threshold_doc, dict):
        raise SystemExit("threshold document must be a JSON object")
    schema = int(threshold_doc.get("schema", 1))
    entries = threshold_doc.get("thresholds", [])
    if not isinstance(entries, list) or not entries:
        raise SystemExit("threshold file must contain a non-empty `thresholds` array")
    budgets = threshold_doc.get("investigation_budgets", {})
    try:
        hot_path_budget = float(budgets.get("hot_path_median_ratio", 1.02))
        workload_budget = float(budgets.get("workload_median_ratio", 1.03))
        geomean_budget = float(budgets.get("overall_geomean_ratio", 1.01))
    except (TypeError, ValueError) as error:
        raise SystemExit("investigation budgets must be numeric") from error
    if min(hot_path_budget, workload_budget, geomean_budget) < 1.0:
        raise SystemExit("investigation budgets must be ratios of at least 1.0")
    active_features = sorted(feature_set) if feature_set else None
    results = collect_criterion(criterion_dir)
    rows: list[dict[str, object]] = []
    missing: list[str] = []
    inapplicable: list[str] = []
    failures: list[str] = []

    try:
        normalized = [normalize_entry(entry, schema) for entry in entries]
    except (KeyError, TypeError, ValueError) as error:
        raise SystemExit(f"malformed threshold file: {error}") from error

    for entry in normalized:
        full_id = str(entry["id"])
        if entry["artifact_mode"] is not None and (
            artifact_mode != entry["artifact_mode"] or active_features != entry["feature_set"]
        ):
            inapplicable.append(full_id)
            continue
        result = results.get(full_id)
        if result is None:
            missing.append(full_id)
            continue
        median_ratio = result["median_ns"] / float(entry["baseline_median_ns"])
        p95_ratio = (
            result["p95_ns"] / float(entry["baseline_p95_ns"])
            if entry["baseline_p95_ns"] is not None
            else None
        )
        passed = median_ratio <= float(entry["median_ratio_limit"]) and (
            p95_ratio is None or p95_ratio <= float(entry["p95_ratio_limit"])
        )
        investigation_limit = (
            hot_path_budget
            if full_id.startswith("hotpath/") or "hot-path" in str(entry["category"])
            else workload_budget
        )
        review_required = median_ratio > investigation_limit
        if not passed:
            row_status = "regression"
        elif review_required:
            row_status = "review-required"
        else:
            row_status = "pass"
        row = {
            **entry,
            "feature_set": ",".join(entry["feature_set"] or []),
            "status": row_status,
            "current_median_ns": result["median_ns"],
            "median_ratio": median_ratio,
            "investigation_ratio_limit": investigation_limit,
            "review_required": review_required,
            "current_p95_ns": result["p95_ns"],
            "p95_ratio": p95_ratio,
            "current_mean_ns": result["mean_ns"],
            "current_min_ns": result["min_ns"],
            "current_max_ns": result["max_ns"],
            "passed": passed,
        }
        rows.append(row)
        if not passed:
            failures.append(
                f"{full_id}: median {median_ratio:.3f}x/{entry['median_ratio_limit']:.3f}x, "
                f"p95 {p95_ratio if p95_ratio is not None else 'n/a'}/"
                f"{entry['p95_ratio_limit'] if entry['p95_ratio_limit'] is not None else 'n/a'}"
            )

    if csv_out is not None:
        write_csv(csv_out, rows)
    selected_rows = [
        row for row in rows if str(row["category"]) != "upstream mimalloc comparison"
    ]
    ratios = [
        float(row["median_ratio"])
        for row in selected_rows
        if float(row["median_ratio"]) > 0.0
    ]
    geomean_ratio = (
        math.exp(sum(math.log(value) for value in ratios) / len(ratios))
        if ratios
        else None
    )
    geomean_review = geomean_ratio is not None and geomean_ratio > geomean_budget
    review_items = [str(row["id"]) for row in rows if row["review_required"]]
    if failures:
        review_status = "hard-failure"
    elif not rows:
        review_status = "not-applicable"
    elif review_items or geomean_review:
        review_status = "review-required"
    else:
        review_status = "pass"
    review_document = {
        "schema": 1,
        "artifact_mode": artifact_mode,
        "feature_set": active_features or [],
        "status": review_status,
        "selected_workload_geomean_ratio": geomean_ratio,
        "selected_workloads": [str(row["id"]) for row in selected_rows],
        "overall_geomean_investigation_limit": geomean_budget,
        "geomean_review_required": geomean_review,
        "benchmark_review_required": review_items,
        "hard_ceiling_failures": failures,
    }
    if review_out is not None:
        review_out.parent.mkdir(parents=True, exist_ok=True)
        review_out.write_text(
            json.dumps(review_document, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    if missing and not allow_missing:
        raise SystemExit("missing Criterion results:\n" + "\n".join(f"- {item}" for item in missing))
    if failures and not report_only:
        raise SystemExit("benchmark regressions exceeded thresholds:\n" + "\n".join(failures))
    if not rows:
        detail = "\n".join(f"- {item}" for item in inapplicable)
        message = "no applicable thresholded Criterion results were validated" + (
            f"\n{detail}" if detail else ""
        )
        if not allow_no_applicable:
            raise SystemExit(message)
        print(message)
        print(f"ok: validated 0 thresholds; missing={len(missing)} inapplicable={len(inapplicable)}")
        return
    if missing:
        print("missing threshold results:\n" + "\n".join(f"- {item}" for item in missing))
    if inapplicable:
        print("inapplicable thresholds:\n" + "\n".join(f"- {item}" for item in inapplicable))
    if failures:
        print("reported threshold regressions:\n" + "\n".join(f"- {item}" for item in failures))
    print(f"ok: validated {len(rows)} thresholds; missing={len(missing)} inapplicable={len(inapplicable)}")


def write_synthetic_result(root: Path, benchmark_id: str, samples: list[float]) -> None:
    result_dir = root / "criterion" / "synthetic" / "new"
    result_dir.mkdir(parents=True, exist_ok=True)
    (result_dir / "benchmark.json").write_text(json.dumps({"full_id": benchmark_id}), encoding="utf-8")
    (result_dir / "sample.json").write_text(
        json.dumps({"iters": [1] * len(samples), "times": samples}), encoding="utf-8"
    )


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        thresholds = root / "thresholds.json"
        base_entry = {
            "id": "synthetic/case", "artifact_mode": "default-rust", "feature_set": ["bench"],
            "category": "self-test", "baseline": {"median_ns": 100.0, "p95_ns": 100.0},
            "limits": {"median_ratio": 1.10, "p95_ratio": 1.20},
            "host_profile": "self-test", "baseline_commit": "0" * 40,
        }
        thresholds.write_text(json.dumps({"schema": 2, "thresholds": [base_entry]}), encoding="utf-8")

        cases = [
            ("pass", [100.0] * 20, "default-rust", ["bench"], None),
            ("regression", [130.0] * 20, "default-rust", ["bench"], "regressions"),
            ("missing", [], "default-rust", ["bench"], "missing"),
            ("wrong artifact", [100.0] * 20, "interpose", ["bench"], "applicable"),
            ("wrong feature", [100.0] * 20, "default-rust", ["bench", "secure"], "applicable"),
            ("p95 only", [100.0] * 18 + [130.0] * 2, "default-rust", ["bench"], "regressions"),
        ]
        for name, samples, mode, features, expected in cases:
            criterion = root / name
            if samples:
                write_synthetic_result(criterion, "synthetic/case", samples)
            try:
                validate_thresholds(thresholds, criterion / "criterion", None, False, mode, features)
            except SystemExit as error:
                if expected is None or expected not in str(error):
                    raise SystemExit(f"self-test {name!r} failed unexpectedly: {error}") from error
            else:
                if expected is not None:
                    raise SystemExit(f"self-test {name!r} unexpectedly passed")

        validate_thresholds(
            thresholds,
            root / "wrong artifact" / "criterion",
            None,
            False,
            "interpose",
            ["bench"],
            allow_no_applicable=True,
        )

        review_dir = root / "review"
        write_synthetic_result(review_dir, "synthetic/case", [104.0] * 20)
        review_out = review_dir / "benchmark-review.json"
        validate_thresholds(
            thresholds,
            review_dir / "criterion",
            None,
            False,
            "default-rust",
            ["bench"],
            review_out=review_out,
        )
        review = load_json(review_out)
        if not isinstance(review, dict) or review.get("status") != "review-required":
            raise SystemExit("self-test investigation budget did not require review")

        malformed = dict(base_entry)
        malformed["baseline"] = {"median_ns": 100.0}
        thresholds.write_text(json.dumps({"schema": 2, "thresholds": [malformed]}), encoding="utf-8")
        try:
            validate_thresholds(thresholds, root / "pass" / "criterion", None, False, "default-rust", ["bench"])
        except SystemExit as error:
            if "malformed" not in str(error):
                raise
        else:
            raise SystemExit("self-test malformed baseline unexpectedly passed")

        legacy = {
            "schema": 1,
            "thresholds": [{
                "id": "synthetic/case", "category": "legacy",
                "baseline_median_ns": 100.0, "max_ratio": 1.10,
            }],
        }
        thresholds.write_text(json.dumps(legacy), encoding="utf-8")
        validate_thresholds(
            thresholds, root / "pass" / "criterion", None, False, None, None
        )
    print("ok: threshold checker self-tests passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--thresholds", default="benchmarks/thresholds.json")
    parser.add_argument("--criterion-dir", default="target/criterion")
    parser.add_argument("--csv-out")
    parser.add_argument("--artifact-mode")
    parser.add_argument("--feature", action="append", dest="features")
    parser.add_argument("--allow-missing", action="store_true")
    parser.add_argument("--allow-no-applicable", action="store_true")
    parser.add_argument("--report-only", action="store_true")
    parser.add_argument("--review-out")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    validate_thresholds(
        Path(args.thresholds), Path(args.criterion_dir),
        Path(args.csv_out) if args.csv_out else None,
        args.allow_missing, args.artifact_mode, args.features, args.report_only,
        args.allow_no_applicable,
        Path(args.review_out) if args.review_out else None,
    )


if __name__ == "__main__":
    main()
