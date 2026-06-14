#!/usr/bin/env python3
"""Compare ASAN tracking reports for C and Rust mimalloc."""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/asan-tracking-parity"

PROBE_C = r"""
#include <mimalloc.h>

#include <stdio.h>
#include <string.h>

int main(int argc, char** argv) {
  if (argc != 2) return 2;

  if (strcmp(argv[1], "clean") == 0) {
    unsigned char* p = (unsigned char*)mi_malloc(64);
    if (p == NULL) return 10;
    p[0] = 0xA5;
    p[63] = 0x5A;
    mi_free(p);
    return 0;
  }

  if (strcmp(argv[1], "calloc-defined") == 0) {
    unsigned char* p = (unsigned char*)mi_calloc(4, 8);
    if (p == NULL) return 11;
    volatile unsigned char value = p[0];
    mi_free(p);
    return value == 0 ? 0 : 12;
  }

  if (strcmp(argv[1], "free-poison") == 0) {
    unsigned char* p = (unsigned char*)mi_malloc(32);
    if (p == NULL) return 13;
    mi_free(p);
    p[0] = 0xA5;
    return 0;
  }

  fprintf(stderr, "unknown case: %s\n", argv[1]);
  return 3;
}
"""


CASES = {
    "clean": "clean",
    "calloc-defined": "clean",
    "free-poison": "use_after_poison",
}


def run(
    cmd: list[str],
    *,
    check: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=check,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def rust_staticlib_asan() -> pathlib.Path:
    target_dir = BUILD_DIR / "rust-target"
    deps_dir = target_dir / "debug/deps"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    cargo_staticlib = [
        "cargo",
        "rustc",
        "--features",
        "track-asan",
        "--lib",
        "--",
        "--crate-type",
        "staticlib",
    ]
    run(cargo_staticlib, env=env)
    libs = sorted(
        deps_dir.glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        run(["cargo", "clean", "-p", "mechanical_mimalloc"], env=env)
        run(cargo_staticlib, env=env)
        libs = sorted(
            deps_dir.glob("libmechanical_mimalloc-*.a"),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )
    if not libs:
        raise SystemExit("no Rust ASAN staticlib artifact found")
    return libs[0]


def c_mimalloc_object(cc: str) -> pathlib.Path:
    obj = BUILD_DIR / "mimalloc-static-asan.o"
    run(
        [
            cc,
            "-std=c11",
            "-fsanitize=address",
            "-fno-omit-frame-pointer",
            "-DMI_TRACK_ASAN=1",
            "-DMI_GUARDED=0",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            "-c",
            str(ROOT / "mimalloc-3.3.2/src/static.c"),
            "-o",
            str(obj),
        ]
    )
    return obj


def compile_probe(cc: str, output: pathlib.Path, objects: list[pathlib.Path]) -> None:
    src = BUILD_DIR / "asan-tracking-probe.c"
    src.write_text(PROBE_C)
    run(
        [
            cc,
            "-std=c11",
            "-fsanitize=address",
            "-fno-omit-frame-pointer",
            "-DMI_TRACK_ASAN=1",
            "-DMI_GUARDED=0",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            str(src),
            *[str(obj) for obj in objects],
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(output),
        ]
    )


def normalize_asan(stderr: str, status: int) -> str:
    if re.search(r"ERROR: AddressSanitizer: use-after-poison", stderr):
        return "use_after_poison"
    if re.search(r"ERROR: AddressSanitizer: use-after-free", stderr):
        return "use_after_free"
    if re.search(r"ERROR: AddressSanitizer:", stderr):
        return "asan_error"
    if status == 0:
        return "clean"
    return "other"


def run_asan(binary: pathlib.Path, case: str) -> tuple[int, str]:
    completed = run(
        [str(binary), case],
        check=False,
    )
    return completed.returncode, normalize_asan(completed.stderr, completed.returncode)


def main() -> int:
    cc = shutil.which("clang") or shutil.which("cc")
    if cc is None:
        raise SystemExit("clang or cc is required for this check")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    c_bin = BUILD_DIR / "probe-c"
    rust_bin = BUILD_DIR / "probe-rust"
    compile_probe(cc, c_bin, [c_mimalloc_object(cc)])
    compile_probe(cc, rust_bin, [rust_staticlib_asan()])

    for case, expected in CASES.items():
        c_status, c_report = run_asan(c_bin, case)
        rust_status, rust_report = run_asan(rust_bin, case)

        if c_report != expected:
            raise SystemExit(f"C ASAN report for {case} was {c_report}, expected {expected}")
        if rust_report != c_report:
            raise SystemExit(f"Rust ASAN report for {case} was {rust_report}, expected {c_report}")
        if (expected == "clean") != (c_status == 0 and rust_status == 0):
            raise SystemExit(
                f"ASAN exit status mismatch for {case}: C={c_status} Rust={rust_status}"
            )

        print(f"ok: {case} -> {rust_report}")

    print("ok: C and Rust ASAN tracking reports match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
