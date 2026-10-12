#!/usr/bin/env python3
"""Compare equally repeated baseline/candidate `perf stat -x` files."""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path


def read(paths: list[Path]) -> dict[str, list[float]]:
    events: dict[str, list[float]] = {}
    for path in paths:
        with path.open(encoding="utf-8") as source:
            for row in csv.reader(source):
                if len(row) < 3 or not row[0] or row[0].startswith("#"):
                    continue
                try:
                    value = float(row[0])
                except ValueError:
                    continue
                events.setdefault(row[2], []).append(value)
    return events


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, nargs="+", required=True)
    parser.add_argument("--candidate", type=Path, nargs="+", required=True)
    args = parser.parse_args()
    if len(args.baseline) != len(args.candidate):
        parser.error("baseline and candidate must have the same number of files")

    baseline = read(args.baseline)
    candidate = read(args.candidate)
    if baseline.keys() != candidate.keys():
        raise SystemExit("baseline and candidate event sets differ")

    print("| Event | Baseline mean | Candidate mean | Candidate delta |")
    print("|---|---:|---:|---:|")
    for event in baseline:
        before = statistics.fmean(baseline[event])
        after = statistics.fmean(candidate[event])
        print(f"| {event} | {before:.0f} | {after:.0f} | {(after / before - 1) * 100:+.2f}% |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
