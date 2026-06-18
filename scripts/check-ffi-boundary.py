#!/usr/bin/env python3
"""Verify exported ABI attributes stay in approved boundary files."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]

APPROVED_EXPORT_FILES = {
    "src/alloc.rs",
    "src/alloc_aligned.rs",
    "src/alloc_override.rs",
    "src/alloc_posix.rs",
    "src/arena.rs",
    "src/free.rs",
    "src/heap.rs",
    "src/init.rs",
    "src/lib.rs",
    "src/options.rs",
    "src/os.rs",
    "src/page.rs",
    "src/page_map.rs",
    "src/prim.rs",
    "src/prim/osx.rs",
    "src/prim/unix.rs",
    "src/stats.rs",
    "src/theap.rs",
}

EXPORT_PATTERNS = [
    re.compile(r"unsafe\s*\(\s*no_mangle\s*\)"),
    re.compile(r"unsafe\s*\(\s*export_name\s*="),
    re.compile(r"unsafe\s*\(\s*naked\s*\)"),
    re.compile(r"\bglobal_asm!\s*\("),
    re.compile(r"\bnaked_asm!\s*\("),
]


def main() -> int:
    offenders: list[str] = []
    for path in sorted((ROOT / "src").rglob("*.rs")):
        rel = path.relative_to(ROOT).as_posix()
        text = path.read_text()
        if not any(pattern.search(text) for pattern in EXPORT_PATTERNS):
            continue
        if rel not in APPROVED_EXPORT_FILES:
            offenders.append(rel)

    if offenders:
        raise AssertionError(
            "export ABI attributes outside approved boundary files: "
            + ", ".join(offenders)
        )

    print(f"ok: FFI/export boundary attributes limited to {len(APPROVED_EXPORT_FILES)} approved files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
