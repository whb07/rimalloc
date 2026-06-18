#!/usr/bin/env python3
"""Verify exported ABI attributes stay in classified boundary files."""

from __future__ import annotations

import argparse
import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]

EXPORT_FILE_CLASSIFICATION = {
    "src/alloc.rs": "c-abi",
    "src/alloc_aligned.rs": "c-abi",
    "src/alloc_override.rs": "malloc-cxx-override",
    "src/alloc_posix.rs": "c-abi",
    "src/arena.rs": "c-abi-internal-classified",
    "src/free.rs": "c-abi",
    "src/heap.rs": "c-abi",
    "src/init.rs": "runtime-boundary",
    "src/lib.rs": "panic-runtime-boundary",
    "src/options.rs": "c-abi",
    "src/os.rs": "c-abi-internal-classified",
    "src/page.rs": "c-abi-internal-classified",
    "src/page_map.rs": "c-abi-internal-classified",
    "src/prim.rs": "runtime-boundary",
    "src/prim/osx.rs": "malloc-cxx-override",
    "src/prim/unix.rs": "runtime-boundary",
    "src/stats.rs": "c-abi",
    "src/theap.rs": "c-abi-internal-classified",
}

EXPORT_PATTERNS = [
    re.compile(r"unsafe\s*\(\s*no_mangle\s*\)"),
    re.compile(r"unsafe\s*\(\s*export_name\s*="),
    re.compile(r"unsafe\s*\(\s*naked\s*\)"),
    re.compile(r"\bglobal_asm!\s*\("),
    re.compile(r"\bnaked_asm!\s*\("),
]

VERSION_SCRIPT_PATTERNS = [
    "_Zda*;",
    "_Zdl*;",
    "_Zna*;",
    "_Znw*;",
    "__libc_*;",
    "__posix_memalign*;",
    "_aligned_malloc*;",
    "aligned_alloc*;",
    "calloc*;",
    "cfree*;",
    "free*;",
    "malloc*;",
    "memalign*;",
    "posix_memalign*;",
    "pvalloc*;",
    "realloc*;",
    "valloc*;",
    "mi_*;",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify and optionally list classified exported ABI sites."
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="print every classified exported ABI attribute or assembly alias site",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    offenders: list[str] = []
    classified: dict[str, int] = {}
    occurrences: list[tuple[str, int, str, str]] = []
    for path in sorted((ROOT / "src").rglob("*.rs")):
        rel = path.relative_to(ROOT).as_posix()
        text = path.read_text()
        if not any(pattern.search(text) for pattern in EXPORT_PATTERNS):
            continue
        classification = EXPORT_FILE_CLASSIFICATION.get(rel)
        if classification is None:
            offenders.append(rel)
            continue
        classified[classification] = classified.get(classification, 0) + 1
        for line_number, line in enumerate(text.splitlines(), start=1):
            if any(pattern.search(line) for pattern in EXPORT_PATTERNS):
                occurrences.append((rel, line_number, classification, line.strip()))

    require(
        not offenders,
        "export ABI attributes outside classified boundary files: " + ", ".join(offenders),
    )

    build_rs = (ROOT / "build.rs").read_text()
    require(
        "version_script" in build_rs and "--version-script" in build_rs,
        "build.rs must emit the Linux cdylib linker version script",
    )
    for pattern in VERSION_SCRIPT_PATTERNS:
        require(
            pattern in build_rs,
            f"Linux linker version script missing public export pattern {pattern}",
        )

    if args.list:
        for rel, line_number, classification, line in occurrences:
            print(f"{rel}:{line_number}: {classification}: {line}")

    summary = ", ".join(f"{name}={count}" for name, count in sorted(classified.items()))
    print(
        "ok: FFI/export boundary attributes limited to "
        f"{len(EXPORT_FILE_CLASSIFICATION)} classified files ({summary}); "
        f"{len(occurrences)} export sites checked; "
        f"version script covers {len(VERSION_SCRIPT_PATTERNS)} public patterns"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
