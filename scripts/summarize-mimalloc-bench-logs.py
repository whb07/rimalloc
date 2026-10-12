#!/usr/bin/env python3
"""Summarize balanced mimalloc-bench transcript pairs."""

from __future__ import annotations

import argparse
import statistics
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Result:
    seconds: float
    rss_kib: int
    page_faults: int
    page_reclaims: int


def parse_seconds(value: str) -> float:
    fields = value.split(":")
    if len(fields) == 1:
        return float(fields[0])
    if len(fields) == 2:
        return float(fields[0]) * 60.0 + float(fields[1])
    raise ValueError(f"unsupported elapsed-time value: {value}")


def parse_summary(path: Path, allocator_labels: set[str]) -> dict[str, dict[str, list[Result]]]:
    lines = path.read_text(encoding="utf-8", errors="replace").replace("\r", "").splitlines()
    markers = [index for index, line in enumerate(lines) if line.startswith("# test    alloc")]
    if not markers:
        raise ValueError(f"{path}: final result table was not found")

    parsed: dict[str, dict[str, list[Result]]] = {}
    for line in lines[markers[-1] + 1 :]:
        fields = line.split()
        if len(fields) != 8 or fields[1] not in allocator_labels:
            continue
        test, allocator = fields[0], fields[1]
        result = Result(
            seconds=parse_seconds(fields[2]),
            rss_kib=int(fields[3]),
            page_faults=int(fields[6]),
            page_reclaims=int(fields[7]),
        )
        parsed.setdefault(test, {}).setdefault(allocator, []).append(result)
    if not parsed:
        raise ValueError(f"{path}: final result table contained no requested allocators")
    return parsed


def values(results: list[Result], field: str) -> list[float]:
    return [float(getattr(result, field)) for result in results]


def mean(results: list[Result], field: str = "seconds") -> float:
    return statistics.fmean(values(results, field))


def median(results: list[Result], field: str = "seconds") -> float:
    return statistics.median(values(results, field))


def stdev(results: list[Result]) -> float:
    return statistics.stdev(values(results, "seconds")) if len(results) > 1 else 0.0


def validate(
    name: str,
    parsed: dict[str, dict[str, list[Result]]],
    allocators: tuple[str, str],
    expected_samples: int,
) -> None:
    for test, by_allocator in parsed.items():
        for allocator in allocators:
            count = len(by_allocator.get(allocator, []))
            if count != expected_samples:
                raise ValueError(
                    f"{name}: {test}/{allocator} has {count} samples; "
                    f"expected {expected_samples}"
                )


def format_table(
    c_first: dict[str, dict[str, list[Result]]],
    rust_first: dict[str, dict[str, list[Result]]],
    c_label: str,
    rust_label: str,
) -> str:
    if set(c_first) != set(rust_first):
        missing_left = sorted(set(rust_first) - set(c_first))
        missing_right = sorted(set(c_first) - set(rust_first))
        raise ValueError(
            f"workload sets differ; missing from C-first={missing_left}, "
            f"missing from Rust-first={missing_right}"
        )

    header = (
        "| Workload | N each | C median s | Rust median s | Rust/C | "
        "C mean ± SD (CV) | Rust mean ± SD (CV) | C-first Rust/C | "
        "Rust-first Rust/C | C/Rust median RSS KiB | C/Rust mean faults | "
        "C/Rust mean reclaims |"
    )
    divider = "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    rows = [header, divider]
    for test in c_first:
        c_left = c_first[test][c_label]
        r_left = c_first[test][rust_label]
        c_right = rust_first[test][c_label]
        r_right = rust_first[test][rust_label]
        c_all = c_left + c_right
        r_all = r_left + r_right
        c_mean, r_mean = mean(c_all), mean(r_all)
        c_sd, r_sd = stdev(c_all), stdev(r_all)
        rows.append(
            "| {test} | {n} | {cm:.4f} | {rm:.4f} | {ratio:.4f} | "
            "{ca:.4f} ± {cs:.4f} ({ccv:.2f}%) | "
            "{ra:.4f} ± {rs:.4f} ({rcv:.2f}%) | {lr:.4f} | {rr:.4f} | "
            "{crss:.0f}/{rrss:.0f} | {cf:.2f}/{rf:.2f} | {crec:.2f}/{rrec:.2f} |".format(
                test=test,
                n=len(c_all),
                cm=median(c_all),
                rm=median(r_all),
                ratio=r_mean / c_mean,
                ca=c_mean,
                cs=c_sd,
                ccv=100.0 * c_sd / c_mean,
                ra=r_mean,
                rs=r_sd,
                rcv=100.0 * r_sd / r_mean,
                lr=mean(r_left) / mean(c_left),
                rr=mean(r_right) / mean(c_right),
                crss=median(c_all, "rss_kib"),
                rrss=median(r_all, "rss_kib"),
                cf=mean(c_all, "page_faults"),
                rf=mean(r_all, "page_faults"),
                crec=mean(c_all, "page_reclaims"),
                rrec=mean(r_all, "page_reclaims"),
            )
        )
    return "\n".join(rows) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--c-first", type=Path, required=True)
    parser.add_argument("--rust-first", type=Path, required=True)
    parser.add_argument("--c-label", default="c")
    parser.add_argument("--rust-label", default="rust")
    parser.add_argument("--samples-per-order", type=int, default=5)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.samples_per_order < 1 or args.c_label == args.rust_label:
        raise SystemExit("invalid sample count or allocator labels")

    labels = {args.c_label, args.rust_label}
    try:
        c_first = parse_summary(args.c_first, labels)
        rust_first = parse_summary(args.rust_first, labels)
        allocators = (args.c_label, args.rust_label)
        validate("C-first", c_first, allocators, args.samples_per_order)
        validate("Rust-first", rust_first, allocators, args.samples_per_order)
        table = format_table(c_first, rust_first, args.c_label, args.rust_label)
    except (OSError, ValueError) as error:
        raise SystemExit(f"cannot summarize benchmark logs: {error}") from error

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(table, encoding="utf-8")
        print(f"wrote {args.output}")
    else:
        print(table, end="")


if __name__ == "__main__":
    main()
