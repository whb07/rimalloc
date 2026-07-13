#!/usr/bin/env python3
"""Validate resource probe records and initial consistency gates."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


MANDATORY = {
    "schema", "scenario", "allocator", "artifact_mode", "features", "size", "alignment",
    "threads", "operations", "seed", "wall_ns", "operations_per_second",
    "logical_live_bytes_start", "logical_live_bytes_peak", "logical_live_bytes_end",
    "rss_current_start", "rss_current_end", "rss_peak_end", "commit_current_start",
    "commit_current_end", "commit_peak_end", "page_faults_delta",
    "allocator_reserved_current_start", "allocator_reserved_current_end",
    "allocator_reserved_peak_end", "allocator_committed_current_start",
    "allocator_committed_current_end", "allocator_committed_peak_end",
    "allocator_pages_current_end", "allocator_pages_abandoned_end", "exit_status",
    "wall_time_coefficient_of_variation",
}


def validate_peak(record: dict[str, object], peak: str, currents: list[str]) -> None:
    peak_value = record[peak]
    if peak_value is None:
        return
    for current in currents:
        value = record[current]
        if value is not None and int(peak_value) < int(value):
            raise ValueError(f"{peak} is less than {current}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", required=True)
    parser.add_argument("--thresholds", default="benchmarks/resource-thresholds.json")
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    parser.add_argument("--required-allocator", action="append", dest="required_allocators")
    args = parser.parse_args()
    thresholds = json.loads(Path(args.thresholds).read_text(encoding="utf-8"))
    max_cv = float(thresholds["initial_gates"]["max_wall_time_coefficient_of_variation"])
    records = [
        json.loads(line)
        for line in Path(args.results).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    if not records:
        raise SystemExit("resource result file is empty")
    expected_features = sorted(args.features)
    seen: set[tuple[str, str]] = set()
    try:
        for index, record in enumerate(records, 1):
            missing = sorted(MANDATORY - record.keys())
            if missing:
                raise ValueError(f"record {index} missing fields: {', '.join(missing)}")
            if record["schema"] != 1 or record["artifact_mode"] != args.artifact_mode:
                raise ValueError(f"record {index} schema or artifact mode mismatch")
            if sorted(record["features"]) != expected_features:
                raise ValueError(f"record {index} feature set mismatch")
            if record["exit_status"] != 0 or record["logical_live_bytes_end"] != 0:
                raise ValueError(f"record {index} failed or leaked logical bytes")
            for field in MANDATORY:
                value = record[field]
                if value is not None and isinstance(value, (int, float)) and value < 0:
                    raise ValueError(f"record {index} has negative {field}")
            validate_peak(record, "rss_peak_end", ["rss_current_start", "rss_current_end"])
            validate_peak(record, "commit_peak_end", ["commit_current_start", "commit_current_end"])
            validate_peak(
                record, "allocator_reserved_peak_end",
                ["allocator_reserved_current_start", "allocator_reserved_current_end"],
            )
            validate_peak(
                record, "allocator_committed_peak_end",
                ["allocator_committed_current_start", "allocator_committed_current_end"],
            )
            if float(record["wall_time_coefficient_of_variation"]) > max_cv:
                raise ValueError(f"record {index} wall-time variation exceeds {max_cv}")
            seen.add((str(record["scenario"]), str(record["allocator"])))
    except ValueError as error:
        raise SystemExit(f"resource validation failed: {error}") from error

    required = {
        (scenario, allocator)
        for scenario in thresholds["required_scenarios"]
        for allocator in (args.required_allocators or thresholds["required_allocators"])
    }
    missing_records = sorted(required - seen)
    if missing_records:
        raise SystemExit(f"missing mandatory resource records: {missing_records}")
    print(f"ok: validated {len(records)} resource records across {len(seen)} tuples")


if __name__ == "__main__":
    main()
