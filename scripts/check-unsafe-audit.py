#!/usr/bin/env python3
"""Check that the unsafe audit covers every Rust source file using unsafe."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "UNSAFE_AUDIT.md"

REQUIRED_AUDIT_SNIPPETS = [
    "Public `extern \"C\"` APIs accept the same nullable and invalid-pointer inputs",
    "Metadata pointers are obtained from owning allocator structures",
    "Raw pointer arithmetic is limited to C-equivalent block/page/arena layout",
    "Cross-thread metadata mutation goes through the audited atomic helpers",
    "Callback invocation and process/thread lifecycle entry points",
    "`ATOMIC_ORDERING_AUDIT.md`",
    "scripts/check-valgrind-tracking-parity.py",
    "scripts/check-asan-tracking-parity.py",
]


def unsafe_source_files() -> set[str]:
    files: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        text = path.read_text()
        if re.search(r"\bunsafe\b", text):
            files.add(path.relative_to(ROOT).as_posix())
    return files


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = unsafe_source_files()

    missing = sorted(path for path in found if path not in audit_text)
    require(
        not missing,
        f"{AUDIT.name} is missing unsafe source entries: {', '.join(missing)}",
    )

    for path in sorted(found):
        require(path in audit_text, f"{path} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing unsafe audit invariant: {snippet}")

    unsafe_mentions = sum(len(re.findall(r"\bunsafe\b", (ROOT / path).read_text())) for path in found)
    print(
        f"ok: unsafe audit covers {len(found)} source files and "
        f"{unsafe_mentions} unsafe mentions"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
