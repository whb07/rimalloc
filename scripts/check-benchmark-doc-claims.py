#!/usr/bin/env python3
"""Reject stale or artifact-unsafe benchmark documentation claims."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
DOCUMENTS = [
    ROOT / "README.md",
    ROOT / "TYPES_SPEC.md",
    ROOT / "MIGRATION_PHASES.md",
    ROOT / "BENCHMARK_RESULTS.md",
    ROOT / "UPSTREAM_DIFF.md",
    ROOT / "CHANGELOG.md",
    *(ROOT / "docs").glob("*.md"),
]
BENCHMARK_SOURCES = [ROOT / "benches" / "hotpath.rs", ROOT / "benches" / "workloads.rs"]
MARKDOWN_LINK_RE = re.compile(r"\[[^\]]+\]\((?P<target>[^)]+)\)")


def main() -> None:
    failures: list[str] = []
    for path in [*DOCUMENTS, *BENCHMARK_SOURCES]:
        text = path.read_text(encoding="utf-8")
        lowered = text.lower()
        normalized = " ".join(lowered.split())
        relative = path.relative_to(ROOT)
        for match in MARKDOWN_LINK_RE.finditer(text):
            target = match.group("target").strip().strip("<>").split("#", 1)[0]
            if not target or "://" in target or target.startswith(("mailto:", "/")):
                continue
            linked = (path.parent / target).resolve()
            try:
                linked.relative_to(ROOT)
            except ValueError:
                failures.append(f"{relative}: repository link escapes the tree: {target}")
                continue
            if not linked.exists():
                failures.append(f"{relative}: broken repository link: {target}")
        for stale in (
            "does not expose a native rust `globalalloc`",
            "does not expose a native rust globalalloc",
            "all modules are public",
            "benchmark-thresholds-full.csv",
        ):
            if stale in normalized:
                failures.append(f"{relative}: stale claim: {stale!r}")
        if "src/core_types/" in text and not (
            "src/types/" in text and "not a filesystem path" in normalized
        ):
            failures.append(
                f"{relative}: src/core_types/ must be identified as a historical target, "
                "with src/types/ named as the current location"
            )
        for line_number, line in enumerate(text.splitlines(), 1):
            line_lower = line.lower()
            if re.search(r"(?:on-stack.*lifo|lifo.*on-stack)", line_lower):
                failures.append(
                    f"{relative}:{line_number}: LIFO scratch is not an on-stack buffer"
                )
            if "target/bench-full/default-rust" in line and "target/bench-full/interpose" in line:
                failures.append(
                    f"{relative}:{line_number}: benchmark command mixes artifact directories"
                )
    if failures:
        raise SystemExit("benchmark documentation claim check failed:\n" + "\n".join(failures))
    print(f"ok: checked benchmark claims in {len(DOCUMENTS)} documents and 2 benchmark sources")


if __name__ == "__main__":
    main()
