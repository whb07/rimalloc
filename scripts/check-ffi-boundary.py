#!/usr/bin/env python3
"""Verify exported ABI attributes stay in classified boundary files."""

from __future__ import annotations

import argparse
import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]

EXPORT_FILE_CLASSIFICATION = {
    "src/allocator/alloc.rs": "c-abi",
    "src/allocator/alloc/cxx_new.rs": "c-abi",
    "src/allocator/alloc/public_malloc.rs": "c-abi",
    "src/allocator/alloc/string_path.rs": "c-abi",
    "src/allocator/alloc/zeroed.rs": "c-abi",
    "src/allocator/aligned.rs": "c-abi",
    "src/allocator/aligned/thread_heap.rs": "c-abi",
    "src/raw/macros.rs": "malloc-cxx-override",
    "src/raw/posix.rs": "c-abi",
    "src/allocator/arena.rs": "c-abi-internal-classified",
    "src/allocator/arena/debug.rs": "c-abi",
    "src/allocator/arena/geometry.rs": "c-abi-internal-classified",
    "src/allocator/arena/heap_pages.rs": "c-abi-internal-classified",
    "src/allocator/arena/ids.rs": "c-abi-internal-classified",
    "src/allocator/arena/os_memory.rs": "c-abi-internal-classified",
    "src/allocator/bench_diagnostics.rs": "benchmark-diagnostics",
    "src/allocator/free.rs": "c-abi",
    "src/allocator/heap.rs": "c-abi",
    "src/allocator/init.rs": "runtime-boundary",
    "src/lib.rs": "panic-runtime-boundary",
    "src/allocator/options.rs": "c-abi",
    "src/platform/os.rs": "c-abi-internal-classified",
    "src/allocator/page/bins.rs": "c-abi",
    "src/allocator/page/deferred.rs": "c-abi",
    "src/allocator/page.rs": "c-abi-internal-classified",
    "src/allocator/page_map.rs": "c-abi-internal-classified",
    "src/allocator/page_map/lookup.rs": "c-abi-internal-classified",
    "src/platform/mod.rs": "runtime-boundary",
    "src/platform/macos.rs": "malloc-cxx-override",
    "src/platform/unix.rs": "runtime-boundary",
    "src/allocator/stats.rs": "c-abi",
    "src/allocator/thread_heap.rs": "c-abi-internal-classified",
}

EXPORT_PATTERNS = [
    re.compile(r"unsafe\s*\(\s*no_mangle\s*\)"),
    re.compile(r"unsafe\s*\(\s*export_name\s*="),
    re.compile(r"unsafe\s*\(\s*naked\s*\)"),
    re.compile(r"\bglobal_asm!\s*\("),
    re.compile(r"\bnaked_asm!\s*\("),
]

VERSION_SCRIPT_PATTERNS = [
    "_Zda*",
    "_Zdl*",
    "_Zna*",
    "_Znw*",
    "__libc_*",
    "__posix_memalign*",
    "_aligned_malloc*",
    "aligned_alloc*",
    "calloc*",
    "cfree*",
    "free*",
    "malloc*",
    "memalign*",
    "posix_memalign*",
    "pvalloc*",
    "realloc*",
    "valloc*",
    "mi_*",
]

REENTRY_PATTERNS = [
    re.compile(r"\b(?:crate|\$crate)::raw::mi_[A-Za-z0-9_]+\s*\("),
    re.compile(r"\b(?:crate|\$crate)::abi::mi_[A-Za-z0-9_]+\s*\("),
]

REENTRY_ALLOWLIST = {
    "src/ffi/abi.rs": "SAFETY",
    "src/ffi/headers.rs": "SAFETY",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def source_lines(text: str) -> list[tuple[int, str]]:
    return [
        (line_number, line)
        for line_number, line in enumerate(text.splitlines(), start=1)
        if not line.strip().startswith("//")
    ]


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
    reentry_offenders: list[str] = []
    reentry_allowlisted: list[tuple[str, int, str]] = []
    for path in sorted((ROOT / "src").rglob("*.rs")):
        rel = path.relative_to(ROOT).as_posix()
        text = path.read_text()
        if not any(pattern.search(text) for pattern in EXPORT_PATTERNS):
            classification = None
        else:
            classification = EXPORT_FILE_CLASSIFICATION.get(rel)
            if classification is None:
                offenders.append(rel)
            else:
                classified[classification] = classified.get(classification, 0) + 1
                for line_number, line in source_lines(text):
                    if any(pattern.search(line) for pattern in EXPORT_PATTERNS):
                        occurrences.append((rel, line_number, classification, line.strip()))

        reentry_marker = REENTRY_ALLOWLIST.get(rel)
        for line_number, line in source_lines(text):
            if not any(pattern.search(line) for pattern in REENTRY_PATTERNS):
                continue
            if reentry_marker is not None and reentry_marker in text:
                reentry_allowlisted.append((rel, line_number, line.strip()))
            else:
                reentry_offenders.append(f"{rel}:{line_number}: {line.strip()}")

    require(
        not offenders,
        "export ABI attributes outside classified boundary files: " + ", ".join(offenders),
    )
    require(
        not reentry_offenders,
        "internal source re-enters public raw/ABI mi_* functions outside allowlist:\n"
        + "\n".join(reentry_offenders),
    )

    build_rs = (ROOT / "build.rs").read_text()
    require(
        "c_abi" in build_rs
        and "version_script" in build_rs
        and "rustc-link-arg" in build_rs
        and "--version-script" in build_rs,
        "build.rs must gate the Linux cdylib linker version script on the C ABI build mode",
    )
    for pattern in VERSION_SCRIPT_PATTERNS:
        require(
            pattern in build_rs,
            f"Linux linker version script missing public export pattern {pattern}",
        )

    if args.list:
        for rel, line_number, classification, line in occurrences:
            print(f"{rel}:{line_number}: {classification}: {line}")
        for rel, line_number, line in reentry_allowlisted:
            print(f"{rel}:{line_number}: c-header-macro-reentry: {line}")

    summary = ", ".join(f"{name}={count}" for name, count in sorted(classified.items()))
    print(
        "ok: FFI/export boundary attributes limited to "
        f"{len(EXPORT_FILE_CLASSIFICATION)} classified files ({summary}); "
        f"{len(occurrences)} export sites checked; "
        f"{len(reentry_allowlisted)} public raw/ABI re-entry sites allowlisted; "
        f"version script covers {len(VERSION_SCRIPT_PATTERNS)} public patterns"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
