#!/usr/bin/env python3
"""Compare Valgrind tracking reports for C and Rust mimalloc."""

from __future__ import annotations

import pathlib
import re
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/valgrind-tracking-parity"

PROBE_C = r"""
#include <mimalloc.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

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

  if (strcmp(argv[1], "leak") == 0) {
    unsigned char* p = (unsigned char*)mi_malloc(123);
    if (p == NULL) _exit(11);
    p[0] = 0xA5;
    _exit(0);
  }

  if (strcmp(argv[1], "uninit") == 0) {
    unsigned char* p = (unsigned char*)mi_malloc(32);
    if (p == NULL) return 13;
    volatile unsigned char value = p[0];
    mi_free(p);
    return value == 255 ? 14 : 0;
  }

  if (strcmp(argv[1], "calloc-defined") == 0) {
    unsigned char* p = (unsigned char*)mi_calloc(4, 8);
    if (p == NULL) return 15;
    volatile unsigned char value = p[0];
    mi_free(p);
    return value == 0 ? 0 : 16;
  }

  if (strcmp(argv[1], "free-noaccess") == 0) {
    unsigned char* p = (unsigned char*)mi_malloc(32);
    if (p == NULL) return 17;
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
    "leak": "reachable",
    "uninit": "uninit",
    "calloc-defined": "clean",
    "free-noaccess": "invalid_write",
}


def run(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def rust_staticlib_valgrind() -> pathlib.Path:
    run(["cargo", "rustc", "--features", "track-valgrind", "--lib", "--", "--crate-type", "staticlib"])
    libs = sorted(
        (ROOT / "target/debug/deps").glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust Valgrind staticlib artifact found")
    return libs[0]


def c_mimalloc_object(cc: str) -> pathlib.Path:
    obj = BUILD_DIR / "mimalloc-static-valgrind.o"
    run(
        [
            cc,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-DMI_TRACK_VALGRIND=1",
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
    src = BUILD_DIR / "valgrind-tracking-probe.c"
    src.write_text(PROBE_C)
    run(
        [
            cc,
            "-std=c11",
            "-DMI_TRACK_VALGRIND=1",
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


DEFINITE_LEAK_RE = re.compile(r"definitely lost:\s+([0-9,]+) bytes in ([0-9,]+) blocks")
REACHABLE_RE = re.compile(r"still reachable:\s+([0-9,]+) bytes in ([0-9,]+) blocks")


def relevant_error_contexts(stderr: str) -> list[str]:
    contexts: list[str] = []
    lines = stderr.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        if "Invalid write" not in line and "Use of uninitialised value" not in line and "Conditional jump or move depends on uninitialised value" not in line:
            i += 1
            continue
        start = i
        i += 1
        while i < len(lines) and lines[i].strip() != "":
            i += 1
        context = "\n".join(lines[start:i])
        if re.search(r": main \(", context):
            contexts.append(context)
    return contexts


def normalize_valgrind(stderr: str) -> tuple[str, tuple[int, int] | None]:
    contexts = relevant_error_contexts(stderr)
    if any("Invalid write" in context for context in contexts):
        return ("invalid_write", None)
    if any(
        "Use of uninitialised value" in context
        or "Conditional jump or move depends on uninitialised value" in context
        for context in contexts
    ):
        return ("uninit", None)
    match = DEFINITE_LEAK_RE.search(stderr)
    if match is not None:
        bytes_lost = int(match.group(1).replace(",", ""))
        blocks_lost = int(match.group(2).replace(",", ""))
        if bytes_lost > 0 and blocks_lost > 0:
            return ("leak", (bytes_lost, blocks_lost))
    match = REACHABLE_RE.search(stderr)
    if match is not None:
        bytes_reachable = int(match.group(1).replace(",", ""))
        blocks_reachable = int(match.group(2).replace(",", ""))
        if bytes_reachable > 0 and blocks_reachable > 0:
            return ("reachable", (bytes_reachable, blocks_reachable))
    if not contexts and ("All heap blocks were freed -- no leaks are possible" in stderr or "definitely lost: 0 bytes" in stderr):
        return ("clean", None)
    return ("other", None)


def run_valgrind(binary: pathlib.Path, case: str) -> tuple[int, str, tuple[int, int] | None]:
    completed = run(
        [
            "valgrind",
            "--tool=memcheck",
            "--leak-check=full",
            "--show-leak-kinds=definite",
            "--errors-for-leak-kinds=definite",
            "--error-exitcode=91",
            str(binary),
            case,
        ],
        check=False,
    )
    report, leak = normalize_valgrind(completed.stderr)
    return completed.returncode, report, leak


def main() -> int:
    if shutil.which("valgrind") is None:
        raise SystemExit("valgrind is required for this check")

    cc = shutil.which("cc")
    if cc is None:
        raise SystemExit("cc is required for this check")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    c_bin = BUILD_DIR / "probe-c"
    rust_bin = BUILD_DIR / "probe-rust"
    compile_probe(cc, c_bin, [c_mimalloc_object(cc)])
    compile_probe(cc, rust_bin, [rust_staticlib_valgrind()])

    for case, expected in CASES.items():
        c_status, c_report, c_leak = run_valgrind(c_bin, case)
        rust_status, rust_report, rust_leak = run_valgrind(rust_bin, case)

        if c_report != expected:
            raise SystemExit(f"C Valgrind report for {case} was {c_report}, expected {expected}")
        if rust_report != c_report:
            raise SystemExit(f"Rust Valgrind report for {case} was {rust_report}, expected {c_report}")
        if expected in {"invalid_write", "uninit", "leak"} and (c_status == 0 or rust_status == 0):
            raise SystemExit(f"Valgrind did not signal expected issue for {case}: C={c_status} Rust={rust_status}")
        if c_leak != rust_leak:
            raise SystemExit(f"Valgrind leak summary mismatch for {case}: C={c_leak} Rust={rust_leak}")

        print(f"ok: {case} -> {rust_report}")

    print("ok: C and Rust Valgrind tracking reports match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
