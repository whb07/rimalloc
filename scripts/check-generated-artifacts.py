#!/usr/bin/env python3
"""Check that generated artifacts are ignored or documented as evidence."""

from __future__ import annotations

import fnmatch
import pathlib
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]

REQUIRED_IGNORED = (
    "target/.artifact-hygiene",
    "fuzz/target/.artifact-hygiene",
    "mimalloc-bench/.artifact-hygiene",
    "RUST_DEPENDENCY_EDGES.csv",
    "parse_bench.py",
    "parse_bench_10x.py",
    "scratch.profraw",
    "scratch.profdata",
    "scratch.gcda",
    "scratch.gcno",
)

GENERATED_UNTRACKED_PATTERNS = (
    "RUST_DEPENDENCY_EDGES.csv",
    "parse_bench*.py",
    "*.profraw",
    "*.profdata",
    "*.gcda",
    "*.gcno",
    "benchmark-thresholds*.csv",
    "release-artifacts.json",
    "SHA256SUMS",
    "SHA256SUMS.sig",
)

DOC_MARKERS = (
    "scripts/check-generated-artifacts.py",
    "scripts/check-public-symbols.py --write-dynamic-manifests",
    "scripts/check-bench-full.sh",
    "scripts/build-release-artifacts.py",
    "target/release-artifacts/",
    "target/criterion",
    "RUST_DEPENDENCY_EDGES.csv",
    "parse_bench*.py",
)


def git(*args: str) -> str:
    return subprocess.check_output(("git", *args), cwd=ROOT, text=True)


def check_required_ignores() -> list[str]:
    missing: list[str] = []
    for path in REQUIRED_IGNORED:
        result = subprocess.run(
            ("git", "check-ignore", "-q", path),
            cwd=ROOT,
            check=False,
        )
        if result.returncode != 0:
            missing.append(path)
    return missing


def untracked_generated_artifacts() -> list[str]:
    output = git("ls-files", "--others", "--exclude-standard")
    paths = [line.strip() for line in output.splitlines() if line.strip()]
    return [
        path
        for path in paths
        if any(
            fnmatch.fnmatch(pathlib.PurePosixPath(path).as_posix(), pattern)
            for pattern in GENERATED_UNTRACKED_PATTERNS
        )
    ]


def missing_doc_markers() -> list[str]:
    doc = (ROOT / "docs/generated-artifacts.md").read_text()
    return [marker for marker in DOC_MARKERS if marker not in doc]


def main() -> None:
    failures: list[str] = []

    ignored = check_required_ignores()
    if ignored:
        failures.append(
            "generated/local scratch patterns are not ignored:\n"
            + "\n".join(f"  - {path}" for path in ignored)
        )

    untracked = untracked_generated_artifacts()
    if untracked:
        failures.append(
            "generated-looking artifacts are untracked and not ignored; "
            "either promote them under a documented evidence path or add a "
            "local-only ignore rule:\n"
            + "\n".join(f"  - {path}" for path in untracked)
        )

    markers = missing_doc_markers()
    if markers:
        failures.append(
            "docs/generated-artifacts.md is missing ownership markers:\n"
            + "\n".join(f"  - {marker}" for marker in markers)
        )

    if failures:
        raise SystemExit("\n\n".join(failures))

    print("ok: generated artifact hygiene is documented and local scratch outputs are ignored")


if __name__ == "__main__":
    main()
