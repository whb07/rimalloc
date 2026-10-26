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

RUST = {"rust", "rimalloc_raw"}
# Native result columns are `<allocator>_median_ns`; Nginx rows name the allocator.
C_MIMALLOC = {"c", "mimalloc", "c-mimalloc-3.3.2"}
TOLERANCE = {"c-mimalloc": 1.03, "other": 1.10}
# An Nginx repeat is one load run per case, so fewer than this cannot settle a gap of about 1%.
# Native runs take several Criterion samples per scenario within each repeat.
MIN_REPEATS = 3


def rival_name(allocator):
    return "c-mimalloc" if allocator in C_MIMALLOC else allocator


def native_values(directory):
    """{(suite, scenario): {allocator: median time}} from a native run directory."""
    out = {}
    with (directory / "results.csv").open() as handle:
        for row in csv.DictReader(handle):
            suite = "applications" if row["scenario"] in applications.SCENARIOS else "native"
            for column, value in row.items():
                if column.endswith("_median_ns") and value:
                    name = column.removesuffix("_median_ns")
                    allocator = "rust" if name == "rust" else rival_name(name)
                    out.setdefault((suite, row["scenario"]), {})[allocator] = float(value)
    return out


def nginx_values(directory):
    """{("nginx", profile/case): {allocator: requests per second}} from an Nginx run directory."""
    out = {}
    with (directory / "results.csv").open() as handle:
        for row in csv.DictReader(handle):
            allocator = "rust" if row["allocator"] in RUST else rival_name(row["allocator"])
            out.setdefault(("nginx", f"{row['profile']}/{row['case']}"), {})[allocator] = float(row["requests_per_second"])
    return out


def ratios(values):
    """{suite: {rival: {scenario: ratio}}}; below 1.0 favours Rust. Nginx compares throughput,
    so its ratio is rival/Rust; the other suites compare time, Rust/rival."""
    out = {}
    for (suite, scenario), by_allocator in values.items():
        rust = by_allocator.get("rust")
        for allocator, value in by_allocator.items():
            if allocator != "rust" and rust:
                ratio = value / rust if suite == "nginx" else rust / value
                out.setdefault(suite, {}).setdefault(allocator, {})[scenario] = ratio
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


def caveats(suites, sources, repeats):
    """{(suite, rival): note} counting scenarios whose Rust and rival values come from different
    runs (host drift between sessions enters the ratio) or, for Nginx, from runs with too few
    repeats."""
    notes = {}
    for suite, rivals in suites.items():
        for rival, by_scenario in rivals.items():
            origin = [sources[(suite, scenario)] for scenario in by_scenario]
            crossed = sum(o["rust"] != o[rival] for o in origin)
            thin = sum(min(repeats[o["rust"]], repeats[o[rival]]) < MIN_REPEATS for o in origin) if suite == "nginx" else 0
            parts = [f"{crossed} cross-run"] * bool(crossed) + [f"{thin} under {MIN_REPEATS} repeats"] * bool(thin)
            notes[(suite, rival)] = ", ".join(parts)
    return notes


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directories", nargs="+",
                        help="saved native (bench.py run) or nginx result directories; later ones override earlier")
    args = parser.parse_args(argv)
    # Later directories override earlier ones per scenario and allocator, so a fresh run of
    # changed allocators combines with older results for unchanged ones before ratios form.
    values, sources, repeats = {}, {}, []
    for index, directory in enumerate(map(Path, args.directories)):
        manifest = json.loads((directory / "manifest.json").read_text())
        repeats.append(manifest.get("controls", {}).get("repeats", 1))
        for key, by_allocator in (nginx_values(directory) if manifest.get("suite") == "nginx" else native_values(directory)).items():
            values.setdefault(key, {}).update(by_allocator)
            sources.setdefault(key, {}).update(dict.fromkeys(by_allocator, index))
    suites = ratios(values)
    notes = caveats(suites, sources, repeats)
    rows = score(suites)
    print(f"{'suite':13} {'rival':12} {'Rust/rival':>10}  {'worst scenario':34} {'ratio':>6}  {'result':10} caveats")
    for suite, rival, geomean, worst, ratio, breaches, won in rows:
        result = "WIN" if won else "breach: " + ", ".join(breaches) if breaches else "LOSS"
        print(f"{suite:13} {rival:12} {geomean:10.4f}  {worst:34} {ratio:6.3f}  {result:10} {notes[(suite, rival)]}")
    print("Ratios below 1.0 favour Rust (time for native/applications, inverse throughput for Nginx).")
    print("Caveats count scenarios whose Rust and rival values come from different runs, or Nginx cases "
          f"from runs with fewer than {MIN_REPEATS} repeats; treat a result near 1.0 with such caveats as unsettled.")
    return 0 if all(row[-1] for row in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
