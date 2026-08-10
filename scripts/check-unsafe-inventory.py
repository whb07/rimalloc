#!/usr/bin/env python3
"""Print an advisory lexical summary of unsafe Rust code."""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_COMMANDS = [
    "cargo check",
    "cargo check --features c-abi",
    "cargo check --features interpose",
    "cargo check --features secure",
    "cargo check --features guarded",
    "cargo check --features track-valgrind",
    "cargo check --features track-asan",
]
PATTERNS = {
    "blocks": re.compile(r"\bunsafe\s*\{"),
    "functions": re.compile(r"\bunsafe\s+(?:extern\s+(?:\"[^\"]+\"\s+)?)?fn\b"),
    "extern_blocks": re.compile(r"\bunsafe\s+extern\s+(?:\"[^\"]+\"\s*)?\{"),
    "impls": re.compile(r"\bunsafe\s+impl\b"),
    "attributes": re.compile(r"#\[\s*unsafe\s*\("),
}


def counts(path: pathlib.Path) -> dict[str, int]:
    source = path.read_text()
    result = {"mentions": len(re.findall(r"\bunsafe\b", source))}
    result.update({name: len(pattern.findall(source)) for name, pattern in PATTERNS.items()})
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--verify-builds",
        action="store_true",
        help="run supported advisory cargo check configurations",
    )
    args = parser.parse_args()

    if args.verify_builds:
        for command in BUILD_COMMANDS:
            print(f"+ {command}")
            subprocess.run(command.split(), cwd=ROOT, check=True)

    rows = [
        (path.relative_to(ROOT).as_posix(), counts(path))
        for path in sorted((ROOT / "src").rglob("*.rs"))
    ]
    rows = [(path, row) for path, row in rows if row["mentions"]]
    totals = {name: sum(row[name] for _, row in rows) for name in ("mentions", *PATTERNS)}

    print("advisory lexical unsafe counts (not a release gate)")
    for name, value in totals.items():
        print(f"{name}: {value}")
    print("largest files by unsafe mentions:")
    for path, row in sorted(rows, key=lambda item: item[1]["mentions"], reverse=True)[:10]:
        print(f"  {row['mentions']:4}  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
