#!/usr/bin/env python3
"""Aggregate copy-size tracer TSV files into CSV and Markdown reports."""

from __future__ import annotations

import argparse
import collections
import csv
import json
import pathlib

OperationCounts = dict[str, collections.Counter[tuple[int, int]]]


def load_trace(path: pathlib.Path, sizes: OperationCounts,
               alignments: dict[str, collections.Counter[tuple[int, int]]],
               overlaps: collections.Counter[str]) -> None:
    with path.open(encoding="utf-8") as source:
        for line in source:
            fields = line.rstrip("\n").split("\t")
            if not fields:
                continue
            if fields[0] == "size":
                _, operation, _kind, lower, upper, calls = fields
                sizes[operation][(int(lower), int(upper))] += int(calls)
            elif fields[0] == "alignment":
                _, operation, dst, src, calls = fields
                alignments[operation][(int(dst), int(src))] += int(calls)
            elif fields[0] == "overlap":
                _, _operation, kind, calls = fields
                overlaps[kind] += int(calls)


def percentage(part: int, total: int) -> str:
    return "0.00%" if total == 0 else f"{100.0 * part / total:.2f}%"


def format_size(lower: int, upper: int) -> str:
    if lower == upper:
        return f"{lower:,} B"
    return f"{lower:,}–{upper:,} B"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=pathlib.Path,
                        default=pathlib.Path("target/size-analysis"))
    parser.add_argument("--markdown", type=pathlib.Path)
    parser.add_argument("--csv", type=pathlib.Path)
    parser.add_argument("--top", type=int, default=20)
    args = parser.parse_args()

    root = args.input.resolve()
    statuses: list[dict[str, object]] = []
    status_path = root / "status.jsonl"
    if status_path.exists():
        statuses = [json.loads(line) for line in status_path.read_text(encoding="utf-8").splitlines() if line]

    workload_runs: dict[tuple[str, int], OperationCounts] = {}
    alignment_runs: dict[tuple[str, int], dict[str, collections.Counter[tuple[int, int]]]] = {}
    overlap_runs: dict[tuple[str, int], collections.Counter[str]] = {}
    for workload_dir in sorted(path for path in root.iterdir() if path.is_dir()):
        for run_dir in sorted(workload_dir.glob("run-*")):
            try:
                repeat = int(run_dir.name.removeprefix("run-"))
            except ValueError:
                continue
            key = (workload_dir.name, repeat)
            sizes: OperationCounts = {
                "memcpy": collections.Counter(), "memmove": collections.Counter()
            }
            alignments = {
                "memcpy": collections.Counter(), "memmove": collections.Counter()
            }
            overlaps: collections.Counter[str] = collections.Counter()
            for trace in run_dir.glob("copytrace-*.tsv"):
                load_trace(trace, sizes, alignments, overlaps)
            workload_runs[key] = sizes
            alignment_runs[key] = alignments
            overlap_runs[key] = overlaps

    rows: list[dict[str, object]] = []
    for (workload, repeat), operations in sorted(workload_runs.items()):
        for operation, counts in operations.items():
            total = sum(counts.values())
            for (lower, upper), calls in sorted(counts.items()):
                rows.append({
                    "workload": workload, "repeat": repeat,
                    "operation": operation, "lower_bytes": lower,
                    "upper_bytes": upper, "calls": calls,
                    "call_share": 0.0 if total == 0 else calls / total,
                })

    csv_path = args.csv or root / "copy-size-counts.csv"
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    with csv_path.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=(
            "workload", "repeat", "operation", "lower_bytes", "upper_bytes",
            "calls", "call_share",
        ))
        writer.writeheader()
        writer.writerows(rows)

    lines = [
        "# Complete workload copy-size results", "",
        "Counts below are exported `memcpy`/`memmove` calls. Exact lengths are",
        "retained through 64 KiB; larger lengths use power-of-two ranges.", "",
        "## Run status", "",
        "| workload | run | status | elapsed | trace files |", "|---|---:|---|---:|---:|",
    ]
    for status in statuses:
        lines.append(
            f"| {status['workload']} | {status.get('repeat', '—')} | {status['status']} | "
            f"{status.get('elapsed_seconds', '—')} | {status.get('trace_files', '—')} |"
        )

    lines.extend(["", "## Per-workload totals", "",
                  "| workload | run | operation | calls | distinct sizes/ranges |",
                  "|---|---:|---|---:|---:|"])
    for (workload, repeat), operations in sorted(workload_runs.items()):
        for operation, counts in operations.items():
            lines.append(
                f"| {workload} | {repeat} | `{operation}` | {sum(counts.values()):,} | {len(counts):,} |"
            )

    lines.extend(["", "## Address alignment", "",
                  "Percentages require both source and destination to satisfy the alignment.", "",
                  "| workload | run | operation | 8 B | 16 B | 32 B | 64 B | same offset mod 64 |",
                  "|---|---:|---|---:|---:|---:|---:|---:|"])
    for (workload, repeat), operations in sorted(alignment_runs.items()):
        for operation, counts in operations.items():
            total = sum(counts.values())
            if total == 0:
                continue
            shares = []
            for alignment in (8, 16, 32, 64):
                aligned = sum(
                    calls for (dst, src), calls in counts.items()
                    if dst % alignment == 0 and src % alignment == 0
                )
                shares.append(percentage(aligned, total))
            same_offset = sum(
                calls for (dst, src), calls in counts.items() if dst == src
            )
            lines.append(
                f"| {workload} | {repeat} | `{operation}` | "
                + " | ".join(shares)
                + f" | {percentage(same_offset, total)} |"
            )

    lines.extend(["", f"## Top {args.top} requested lengths per workload", ""])
    for (workload, repeat), operations in sorted(workload_runs.items()):
        for operation, counts in operations.items():
            total = sum(counts.values())
            if total == 0:
                continue
            lines.extend([
                f"### {workload}, run {repeat}: `{operation}`", "",
                "| requested length | calls | share | cumulative share |",
                "|---|---:|---:|---:|",
            ])
            cumulative = 0
            for (lower, upper), calls in counts.most_common(args.top):
                cumulative += calls
                lines.append(
                    f"| {format_size(lower, upper)} | {calls:,} | "
                    f"{percentage(calls, total)} | {percentage(cumulative, total)} |"
                )
            lines.append("")

    lines.extend(["## `memmove` overlap classification", "",
                  "| workload | run | class | calls | share |",
                  "|---|---:|---|---:|---:|"])
    for (workload, repeat), overlaps in sorted(overlap_runs.items()):
        total = sum(overlaps.values())
        for kind, calls in overlaps.most_common():
            lines.append(
                f"| {workload} | {repeat} | {kind} | {calls:,} | {percentage(calls, total)} |"
            )

    markdown_path = args.markdown or root / "copy-size-results.md"
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {csv_path}")
    print(f"wrote {markdown_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
