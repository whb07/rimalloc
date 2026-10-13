#!/usr/bin/env python3
"""Per-suite Rust-versus-rival geomeans with per-scenario tolerances.

Reads saved `bench.py run` and `bench.py nginx` result directories. A suite is
won against a rival when the geomean of Rust/rival time (rival/Rust for Nginx
throughput) is below 1.0 and no scenario exceeds its tolerance.
"""
import argparse
import csv
import json
import math
from pathlib import Path
import sys

import applications

RUST = {"rust", "mechanical_mimalloc_raw"}
# Native result columns are `<allocator>_median_ns`; Nginx rows name the allocator.
C_MIMALLOC = {"c", "mimalloc", "c-mimalloc-3.3.2"}
TOLERANCE = {"c-mimalloc": 1.03, "other": 1.10}


def rival_name(allocator):
    return "c-mimalloc" if allocator in C_MIMALLOC else allocator


def native_ratios(directory):
    """{suite: {rival: {scenario: rust_time / rival_time}}} from a native run."""
    out = {}
    with (directory / "results.csv").open() as handle:
        for row in csv.DictReader(handle):
            suite = "applications" if row["scenario"] in applications.SCENARIOS else "native"
            rust = float(row["rust_median_ns"])
            for column, value in row.items():
                if column.endswith("_median_ns") and column != "rust_median_ns" and value:
                    rival = rival_name(column.removesuffix("_median_ns"))
                    out.setdefault(suite, {}).setdefault(rival, {})[row["scenario"]] = rust / float(value)
    return out


def nginx_ratios(directory):
    """{suite: {rival: {profile/case: rival_rps / rust_rps}}}: below 1.0 means Rust is faster."""
    rows = {}
    with (directory / "results.csv").open() as handle:
        for row in csv.DictReader(handle):
            rows.setdefault((row["profile"], row["case"]), {})[row["allocator"]] = float(row["requests_per_second"])
    out = {}
    for (profile, case), values in rows.items():
        rust = next((v for k, v in values.items() if k in RUST), None)
        for allocator, rps in values.items():
            if allocator not in RUST and rust:
                out.setdefault("nginx", {}).setdefault(rival_name(allocator), {})[f"{profile}/{case}"] = rps / rust
    return out


def score(suites):
    """Rows of (suite, rival, geomean, worst scenario, worst ratio, breaches, won)."""
    rows = []
    for suite, rivals in suites.items():
        for rival, ratios in sorted(rivals.items()):
            geomean = math.exp(sum(map(math.log, ratios.values())) / len(ratios))
            limit = TOLERANCE["c-mimalloc" if rival == "c-mimalloc" else "other"]
            worst = max(ratios, key=ratios.get)
            breaches = sorted(name for name, ratio in ratios.items() if ratio > limit)
            rows.append((suite, rival, geomean, worst, ratios[worst], breaches, geomean < 1.0 and not breaches))
    return rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directories", nargs="+", help="saved native (bench.py run) or nginx result directories")
    args = parser.parse_args(argv)
    suites = {}
    for directory in map(Path, args.directories):
        suite = json.loads((directory / "manifest.json").read_text()).get("suite")
        ratios = nginx_ratios(directory) if suite == "nginx" else native_ratios(directory)
        for suite, rivals in ratios.items():
            for rival, values in rivals.items():
                suites.setdefault(suite, {}).setdefault(rival, {}).update(values)
    rows = score(suites)
    print(f"{'suite':13} {'rival':12} {'Rust/rival':>10}  {'worst scenario':34} {'ratio':>6}  result")
    for suite, rival, geomean, worst, ratio, breaches, won in rows:
        result = "WIN" if won else "breach: " + ", ".join(breaches) if breaches else "LOSS"
        print(f"{suite:13} {rival:12} {geomean:10.4f}  {worst:34} {ratio:6.3f}  {result}")
    print("Ratios below 1.0 favour Rust (time for native/applications, inverse throughput for Nginx).")
    return 0 if all(row[-1] for row in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
