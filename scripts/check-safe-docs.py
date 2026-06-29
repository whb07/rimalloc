#!/usr/bin/env python3
"""Check that Tier S safe facade public items have rustdoc."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_FILES = [
    ROOT / "src/safe.rs",
    *sorted((ROOT / "src/safe").glob("*.rs")),
]

PUBLIC_ITEM = re.compile(
    r"^\s*pub\s+"
    r"(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(?:struct|enum|trait|fn|const)\s+[A-Za-z_][A-Za-z0-9_]*\b"
)


def has_doc(lines: list[str], index: int) -> bool:
    cursor = index - 1
    while cursor >= 0:
        stripped = lines[cursor].strip()
        if not stripped:
            cursor -= 1
            continue
        if stripped.startswith("#["):
            cursor -= 1
            continue
        return stripped.startswith("///")
    return False


def main() -> int:
    missing: list[str] = []
    for path in SAFE_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            if PUBLIC_ITEM.match(line) and not has_doc(lines, index):
                missing.append(f"{rel}:{index + 1}: {line.strip()}")

    if missing:
        print("Tier S public items missing rustdoc:")
        print("\n".join(missing))
        return 1

    print(f"ok: Tier S rustdoc covers {len(SAFE_FILES)} safe facade files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
