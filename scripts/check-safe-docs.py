#!/usr/bin/env python3
"""Check that Tier S safe facade public items have rustdoc."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_FILES = [
    ROOT / "src/safe/mod.rs",
    *sorted((ROOT / "src/safe").glob("*.rs")),
]

PUBLIC_ITEM = re.compile(
    r"^\s*pub\s+"
    r"(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(?:struct|enum|trait|fn|const)\s+[A-Za-z_][A-Za-z0-9_]*\b"
)
PUBLIC_ENUM = re.compile(r"^\s*pub\s+enum\s+[A-Za-z_][A-Za-z0-9_]*\b")
ENUM_VARIANT = re.compile(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:[,({=]|$)")


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


def missing_public_enum_variant_docs(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    in_public_enum = False
    brace_depth = 0

    for index, line in enumerate(lines):
        if not in_public_enum:
            if PUBLIC_ENUM.match(line):
                in_public_enum = True
                brace_depth = line.count("{") - line.count("}")
            continue

        brace_depth += line.count("{") - line.count("}")
        stripped = line.strip()
        if not stripped or stripped.startswith("#[") or stripped.startswith("//"):
            if brace_depth <= 0:
                in_public_enum = False
            continue

        match = ENUM_VARIANT.match(line)
        if match and not has_doc(lines, index):
            missing.append(f"{rel}:{index + 1}: enum variant {match.group(1)}")

        if brace_depth <= 0:
            in_public_enum = False

    return missing


def main() -> int:
    missing: list[str] = []
    synthetic = ["pub enum Synthetic {", "    Undocumented,", "}"]
    if not missing_public_enum_variant_docs("synthetic.rs", synthetic):
        print("safe-docs regression failed to flag an undocumented enum variant")
        return 1

    for path in SAFE_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            if PUBLIC_ITEM.match(line) and not has_doc(lines, index):
                missing.append(f"{rel}:{index + 1}: {line.strip()}")
        missing.extend(missing_public_enum_variant_docs(rel, lines))

    if missing:
        print("Tier S public items missing rustdoc:")
        print("\n".join(missing))
        return 1

    print(
        f"ok: Tier S rustdoc covers public items and enum variants in {len(SAFE_FILES)} safe facade files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
