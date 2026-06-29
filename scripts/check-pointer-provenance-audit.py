#!/usr/bin/env python3
"""Check that pointer provenance and arithmetic audit coverage is current."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "POINTER_PROVENANCE_AUDIT.md"

PATTERN = re.compile(
    r"\.add\(|\.sub\(|\.offset\(|offset_from|wrapping_add|wrapping_sub|"
    r"checked_add|checked_sub|checked_mul|_mi_align_(?:up|down)|"
    r"\bas usize\b|\bas \*mut\b|\bas \*const\b"
)

REQUIRED_AUDIT_SNIPPETS = [
    "Miri was checked for this toolchain but is not available",
    "Pointer-to-integer casts are used for C-equivalent alignment checks",
    "`.add`, `.sub`, and `offset_from` are used only within allocator-owned arrays",
    "Count/size multiplication that accepts public input goes through checked",
    "Wrapping arithmetic is limited to bit operations",
    "`scripts/check-behavioral-parity.py`",
    "`scripts/check-unix-syscall-parity.py`",
]


def files_with_pointer_arithmetic() -> set[str]:
    files: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        if PATTERN.search(path.read_text()):
            files.add(path.relative_to(ROOT).as_posix())
    return files


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = files_with_pointer_arithmetic()
    missing = sorted(path for path in found if path not in audit_text)
    require(
        not missing,
        f"{AUDIT.name} is missing pointer-provenance source entries: {', '.join(missing)}",
    )

    for path in sorted(found):
        require(path in audit_text, f"{path} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing pointer audit invariant: {snippet}")

    mentions = sum(len(PATTERN.findall((ROOT / path).read_text())) for path in found)
    print(f"ok: pointer provenance audit covers {len(found)} source files and {mentions} pointer/arithmetic markers")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
