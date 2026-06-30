#!/usr/bin/env python3
"""Check that public safe facade docs include required API sections."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_FILES = sorted(
    {
        ROOT / "src/safe/mod.rs",
        *((ROOT / "src/safe").glob("*.rs")),
    }
)
UNSAFE_SECTION_FILES = [
    *SAFE_FILES,
    ROOT / "src/ffi/callbacks.rs",
    *sorted((ROOT / "src/raw").glob("*.rs")),
]

PUBLIC_ITEM = re.compile(
    r"^\s*pub\s+"
    r"(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(?:struct|enum|trait|fn|const)\s+[A-Za-z_][A-Za-z0-9_]*\b"
)
PUBLIC_ENUM = re.compile(r"^\s*pub\s+enum\s+[A-Za-z_][A-Za-z0-9_]*\b")
ENUM_VARIANT = re.compile(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:[,({=]|$)")
PUBLIC_FN = re.compile(
    r"^\s*pub\s+"
    r"(?P<qualifiers>(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?)"
    r"fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
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


def doc_block(lines: list[str], index: int) -> list[str]:
    cursor = index - 1
    while cursor >= 0 and lines[cursor].strip().startswith("#["):
        cursor -= 1

    docs: list[str] = []
    while cursor >= 0:
        stripped = lines[cursor].strip()
        if not stripped.startswith("///"):
            break
        docs.append(stripped.removeprefix("///").strip())
        cursor -= 1

    docs.reverse()
    return docs


def has_section(docs: list[str], section: str) -> bool:
    return any(line == section for line in docs)


def function_signature(lines: list[str], index: int) -> str:
    signature = lines[index].strip()
    cursor = index
    while not re.search(r"[{;]\s*(?://.*)?$", signature) and cursor + 1 < len(lines):
        cursor += 1
        signature = f"{signature} {lines[cursor].strip()}"
    return signature


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


def missing_safe_result_sections(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    for index, line in enumerate(lines):
        match = PUBLIC_FN.match(line)
        if not match or "unsafe" in match.group("qualifiers").split():
            continue

        signature = function_signature(lines, index)
        if "-> Result<" not in signature:
            continue

        docs = doc_block(lines, index)
        if not has_section(docs, "# Errors"):
            missing.append(f"{rel}:{index + 1}: missing # Errors for {match.group('name')}")
        if not has_section(docs, "# Panics"):
            missing.append(f"{rel}:{index + 1}: missing # Panics for {match.group('name')}")
    return missing


def missing_unsafe_sections(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    for index, line in enumerate(lines):
        match = PUBLIC_FN.match(line)
        if not match or "unsafe" not in match.group("qualifiers").split():
            continue

        if not has_section(doc_block(lines, index), "# Safety"):
            missing.append(f"{rel}:{index + 1}: missing # Safety for {match.group('name')}")
    return missing


def main() -> int:
    missing: list[str] = []
    synthetic = ["pub enum Synthetic {", "    Undocumented,", "}"]
    if not missing_public_enum_variant_docs("synthetic.rs", synthetic):
        print("safe-docs regression failed to flag an undocumented enum variant")
        return 1
    synthetic_result = ["/// Docs.", "pub fn fallible() -> Result<(), ()> { Ok(()) }"]
    if len(missing_safe_result_sections("synthetic.rs", synthetic_result)) != 2:
        print("safe-docs regression failed to flag missing fallible method sections")
        return 1
    synthetic_unsafe = ["/// Docs.", "pub unsafe fn raw() {}"]
    if not missing_unsafe_sections("synthetic.rs", synthetic_unsafe):
        print("safe-docs regression failed to flag a missing unsafe method section")
        return 1

    for path in SAFE_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            if PUBLIC_ITEM.match(line) and not has_doc(lines, index):
                missing.append(f"{rel}:{index + 1}: {line.strip()}")
        missing.extend(missing_public_enum_variant_docs(rel, lines))
        missing.extend(missing_safe_result_sections(rel, lines))

    for path in UNSAFE_SECTION_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        missing.extend(missing_unsafe_sections(rel, lines))

    if missing:
        print("Public API docs missing required rustdoc coverage:")
        print("\n".join(missing))
        return 1

    print(
        "ok: public docs cover Tier S items, fallible safe method sections, "
        f"and public unsafe safety sections across {len(UNSAFE_SECTION_FILES)} files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
