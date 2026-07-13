#!/usr/bin/env python3
"""Validate the rich allocator diagnostics evidence schema."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path


NONNEGATIVE_FIELDS = {
    "peak_rss_kib",
    "baseline_peak_rss_kib",
    "peak_rss_delta_kib",
    "committed_bytes",
    "live_requested_bytes",
    "page_occupancy",
    "page_occupied_bytes",
    "page_capacity_bytes",
    "page_search_length",
    "minor_page_faults",
    "major_page_faults",
    "sampled_alloc_free_p99_ns",
    "sampled_alloc_free_p999_ns",
    "sampled_remote_free_p99_ns",
    "sampled_remote_free_p999_ns",
    "sampled_concurrent_remote_free_p99_ns",
    "sampled_concurrent_remote_free_p999_ns",
}

ALLOCATOR_FIELDS = {
    "page_free_collect_calls",
    "free_list_extend_calls",
    "page_queue_searches",
    "page_queue_candidates_scanned",
    "remote_free_cas_retries",
}


def is_number(value: object) -> bool:
    return not isinstance(value, bool) and isinstance(value, (int, float))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", required=True)
    args = parser.parse_args()

    report = json.loads(Path(args.report).read_text(encoding="utf-8"))
    missing = sorted(NONNEGATIVE_FIELDS - report.keys())
    if missing:
        raise SystemExit("evidence report missing fields: " + ", ".join(missing))
    if report.get("schema") != 1 or report.get("artifact_mode") != "default-rust":
        raise SystemExit("evidence report schema or artifact mode mismatch")
    for field in NONNEGATIVE_FIELDS:
        value = report[field]
        if not is_number(value) or not math.isfinite(value) or value < 0:
            raise SystemExit(f"evidence report field {field} must be finite and nonnegative")
    if report["peak_rss_kib"] < report["baseline_peak_rss_kib"]:
        raise SystemExit("peak RSS cannot be below its pre-workload baseline")
    if report["peak_rss_delta_kib"] != report["peak_rss_kib"] - report["baseline_peak_rss_kib"]:
        raise SystemExit("peak RSS delta is inconsistent")
    if not 0.0 <= report["page_occupancy"] <= 1.0:
        raise SystemExit("page occupancy must be in [0, 1]")
    if report["page_occupied_bytes"] > report["page_capacity_bytes"]:
        raise SystemExit("occupied page bytes exceed page capacity")
    for prefix in ("sampled_alloc_free", "sampled_remote_free", "sampled_concurrent_remote_free"):
        if report[f"{prefix}_p999_ns"] < report[f"{prefix}_p99_ns"]:
            raise SystemExit(f"{prefix} p999 is below p99")
    allocator = report.get("allocator")
    if not isinstance(allocator, dict):
        raise SystemExit("evidence report allocator diagnostics must be an object")
    missing_allocator = sorted(ALLOCATOR_FIELDS - allocator.keys())
    if missing_allocator:
        raise SystemExit("allocator diagnostics missing fields: " + ", ".join(missing_allocator))
    if any(
        isinstance(value, bool) or not isinstance(value, int) or value < 0
        for value in allocator.values()
    ):
        raise SystemExit("allocator diagnostic counters must be nonnegative integers")
    print("ok: rich allocator evidence report is schema-valid")


if __name__ == "__main__":
    main()
