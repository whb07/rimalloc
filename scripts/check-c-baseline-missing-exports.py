#!/usr/bin/env python3
"""Verify public-header symbols missing from the vendored C baselines."""

from __future__ import annotations

import pathlib
import re
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/c-baseline-missing-exports"
STATIC_OBJ = BUILD_DIR / "static.o"
CMAKE_DIR = BUILD_DIR / "cmake"
CMAKE_ARCHIVE = CMAKE_DIR / "libmimalloc.a"

MISSING_EXPORTS = {
    "mi_collect_reduce",
    "mi_stats_merge",
    "mi_theap_set_default",
    "mi_uzalloc_small",
}

PRESENT_CONTROLS = {
    "mi_theap_malloc_aligned",
    "mi_theap_malloc_small",
    "mi_umalloc_aligned",
    "mi_uzalloc_aligned",
}


def run(cmd: list[str], *, cwd: pathlib.Path = ROOT) -> None:
    subprocess.run(cmd, cwd=cwd, check=True)


def defined_symbols(path: pathlib.Path) -> set[str]:
    completed = subprocess.run(
        ["nm", "-g", "--defined-only", str(path)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    symbols: set[str] = set()
    for line in completed.stdout.splitlines():
        match = re.search(r"\b([A-Za-z_][A-Za-z0-9_]*)$", line)
        if match is not None:
            symbols.add(match.group(1))
    return symbols


def compile_static_translation_unit() -> pathlib.Path:
    run(
        [
            "cc",
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            "-c",
            str(ROOT / "mimalloc-3.3.2/src/static.c"),
            "-o",
            str(STATIC_OBJ),
        ]
    )
    return STATIC_OBJ


def build_cmake_static_archive() -> pathlib.Path:
    run(
        [
            "cmake",
            "-S",
            str(ROOT / "mimalloc-3.3.2"),
            "-B",
            str(CMAKE_DIR),
            "-DMI_BUILD_SHARED=OFF",
            "-DMI_BUILD_TESTS=OFF",
            "-DMI_BUILD_OBJECT=OFF",
            "-DMI_BUILD_STATIC=ON",
            "-DMI_OVERRIDE=OFF",
        ]
    )
    run(["cmake", "--build", str(CMAKE_DIR), "--target", "mimalloc-static", "-j2"])
    if not CMAKE_ARCHIVE.exists():
        raise AssertionError(f"expected CMake static archive at {CMAKE_ARCHIVE}")
    return CMAKE_ARCHIVE


def verify(label: str, path: pathlib.Path) -> None:
    symbols = defined_symbols(path)
    unexpected_present = sorted(MISSING_EXPORTS & symbols)
    missing_controls = sorted(PRESENT_CONTROLS - symbols)
    if unexpected_present:
        raise AssertionError(f"{label} unexpectedly defines: {', '.join(unexpected_present)}")
    if missing_controls:
        raise AssertionError(f"{label} is missing control symbols: {', '.join(missing_controls)}")


def main() -> int:
    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    verify("static.c object", compile_static_translation_unit())
    verify("CMake static archive", build_cmake_static_archive())

    print("ok: vendored C baselines lack documented unresolved public exports")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(f"command failed with exit status {err.returncode}: {err.cmd}\n")
        raise
