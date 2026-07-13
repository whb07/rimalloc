#!/usr/bin/env python3
"""Verify benchmark artifact provenance and required outputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path


CRITERION_REQUIRED = [
    "environment.txt", "manifest.json", "commands.txt", "criterion",
    "resource-results.jsonl", "threshold-results.csv", "benchmark-review.json",
]
PRELOAD_REQUIRED = [
    "environment.txt", "manifest.json", "commands.txt", "resource-results.jsonl",
    "interpose-cdylib.so",
]
CONTROL_FIELDS = {
    "hotpath_sample_size",
    "hotpath_measurement_secs",
    "hotpath_warmup_secs",
    "native_sample_size",
    "native_measurement_secs",
    "native_warmup_secs",
    "native_scenarios",
    "native_allocators",
    "resource_operations",
    "resource_repeats",
}
NUMERIC_CONTROL_FIELDS = CONTROL_FIELDS - {"native_scenarios", "native_allocators"}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    args = parser.parse_args()
    artifact_dir = Path(args.artifact_dir)
    if artifact_dir.name != args.artifact_mode:
        raise SystemExit("artifact directory name and artifact mode disagree")
    required = (
        PRELOAD_REQUIRED
        if args.artifact_mode == "ld-preload-interpose"
        else CRITERION_REQUIRED
    )
    missing = [name for name in required if not (artifact_dir / name).exists()]
    if args.artifact_mode == "default-rust" and not (artifact_dir / "evidence-report.json").exists():
        missing.append("evidence-report.json")
    if missing:
        raise SystemExit("missing benchmark artifacts: " + ", ".join(missing))
    manifest = json.loads((artifact_dir / "manifest.json").read_text(encoding="utf-8"))
    expected_features = sorted(args.features)
    if manifest.get("schema") != 1 or manifest.get("artifact_mode") != args.artifact_mode:
        raise SystemExit("manifest schema or artifact mode mismatch")
    if manifest.get("features") != expected_features:
        raise SystemExit("manifest feature set mismatch")
    controls = manifest.get("controls")
    if not isinstance(controls, dict) or set(controls) != CONTROL_FIELDS:
        raise SystemExit("manifest controls are missing or malformed")
    if any(
        isinstance(controls[field], bool)
        or not isinstance(controls[field], int)
        or controls[field] <= 0
        for field in NUMERIC_CONTROL_FIELDS
    ):
        raise SystemExit("manifest numeric controls must be positive integers")
    for field in ("native_scenarios", "native_allocators"):
        values = controls[field]
        if not isinstance(values, list) or not values or not all(
            isinstance(value, str) and value for value in values
        ):
            raise SystemExit(f"manifest control {field} must be a non-empty string list")
    if manifest.get("dirty") and os.environ.get("MIMALLOC_ALLOW_DIRTY_BENCH") != "1":
        raise SystemExit("dirty benchmark manifest requires MIMALLOC_ALLOW_DIRTY_BENCH=1")
    digest = hashlib.sha256((artifact_dir / "commands.txt").read_bytes()).hexdigest()
    if manifest.get("command_digest") != digest:
        raise SystemExit("manifest command digest mismatch")
    for line_number, line in enumerate(
        (artifact_dir / "resource-results.jsonl").read_text(encoding="utf-8").splitlines(), 1
    ):
        if not line.strip():
            continue
        record = json.loads(line)
        if record.get("artifact_mode") != args.artifact_mode:
            raise SystemExit(f"resource record {line_number} artifact mode mismatch")
        if sorted(record.get("features", [])) != expected_features:
            raise SystemExit(f"resource record {line_number} feature set mismatch")
    if args.artifact_mode != "ld-preload-interpose":
        review = json.loads((artifact_dir / "benchmark-review.json").read_text(encoding="utf-8"))
        if (
            review.get("schema") != 1
            or review.get("artifact_mode") != args.artifact_mode
            or review.get("feature_set") != expected_features
            or review.get("status") not in {"pass", "review-required", "not-applicable"}
        ):
            raise SystemExit("benchmark review status is missing or malformed")
    print(f"ok: verified benchmark manifest and required outputs in {artifact_dir}")


if __name__ == "__main__":
    main()
