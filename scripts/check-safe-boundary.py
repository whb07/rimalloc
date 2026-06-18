#!/usr/bin/env python3
"""Verify the safe Rust facade keeps unsafe boundaries classified."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_BOUNDARY_FILES = ["src/lib.rs", "src/safe.rs"]
AUDIT_TAG = "SAFETY[AUDIT:"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def public_unsafe_items(text: str) -> list[str]:
    offenders: list[str] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if re.search(r"\bpub\s+unsafe\s+(fn|extern|trait|impl)\b", stripped):
            offenders.append(f"line {lineno}: {stripped}")
    return offenders


def has_recent_audit_tag(lines: list[str], index: int) -> bool:
    start = max(0, index - 4)
    return any(AUDIT_TAG in lines[i] for i in range(start, index))


def untagged_boundary_unsafe(text: str) -> list[str]:
    source = text.split("#[cfg(test)]", maxsplit=1)[0]
    lines = source.splitlines()
    offenders: list[str] = []
    patterns = [
        re.compile(r"^\s*unsafe\s*\{"),
        re.compile(r"^\s*unsafe\s+impl\b"),
        re.compile(r"^\s*unsafe\s+extern\b"),
    ]
    for index, line in enumerate(lines):
        if any(pattern.search(line) for pattern in patterns) and not has_recent_audit_tag(
            lines, index
        ):
            offenders.append(f"line {index + 1}: {line.strip()}")
    return offenders


def main() -> int:
    all_public_unsafe: list[str] = []
    all_untagged: list[str] = []

    for rel in SAFE_BOUNDARY_FILES:
        text = (ROOT / rel).read_text()
        all_public_unsafe.extend(f"{rel}:{item}" for item in public_unsafe_items(text))
        all_untagged.extend(f"{rel}:{item}" for item in untagged_boundary_unsafe(text))

    require(
        not all_public_unsafe,
        "public unsafe Rust items outside raw facade: " + "; ".join(all_public_unsafe),
    )
    require(
        not all_untagged,
        "safe facade unsafe boundary lacks SAFETY[AUDIT:*] tag: "
        + "; ".join(all_untagged),
    )

    print(
        "ok: safe facade unsafe boundary has no public unsafe items "
        "and all unsafe blocks are audit-tagged"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
