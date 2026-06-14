#!/usr/bin/env python3
"""Compare secure-mode free-list behavior for C and Rust mimalloc."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/secure-mode-parity"

PROBE_C = r"""
#include <mimalloc.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int encoded_next_is_not_raw_null(void) {
  void* p = mi_malloc(64);
  if (p == NULL) return 10;
  mi_free(p);
  uintptr_t encoded_next = *((uintptr_t*)p);
  printf("encoded_next_nonzero=%d\n", encoded_next != 0);
  return (encoded_next != 0 ? 0 : 11);
}

static int allocation_order_is_randomized(void) {
  enum { COUNT = 64 };
  void* blocks[COUNT];
  for (int i = 0; i < COUNT; i++) {
    blocks[i] = mi_malloc(64);
    if (blocks[i] == NULL) return 20;
  }

  size_t usable = mi_usable_size(blocks[0]);
  int all_forward_adjacent = 1;
  for (int i = 1; i < COUNT; i++) {
    uintptr_t prev = (uintptr_t)blocks[i - 1];
    uintptr_t current = (uintptr_t)blocks[i];
    if (current != prev + usable) {
      all_forward_adjacent = 0;
      break;
    }
  }

  for (int i = 0; i < COUNT; i++) {
    mi_free(blocks[i]);
  }

  printf("allocation_order_randomized=%d\n", !all_forward_adjacent);
  return (!all_forward_adjacent ? 0 : 21);
}

static int double_free_reports_error(void) {
  void* p = mi_malloc(64);
  if (p == NULL) return 30;
  mi_free(p);
  mi_free(p);
  return 0;
}

static int corrupt_free_list_aborts(void) {
  void* p = mi_malloc(64);
  void* q = mi_malloc(64);
  if (p == NULL || q == NULL) return 40;
  mi_free(p);
  mi_collect(true);
  *((uintptr_t*)p) = 0x1234;
  void* r = mi_malloc(64);
  printf("corrupt_alloc=%p\n", r);
  mi_free(q);
  mi_free(r);
  return 0;
}

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  if (strcmp(argv[1], "encoded-next") == 0) return encoded_next_is_not_raw_null();
  if (strcmp(argv[1], "randomized-order") == 0) return allocation_order_is_randomized();
  if (strcmp(argv[1], "double-free") == 0) return double_free_reports_error();
  if (strcmp(argv[1], "corrupt-free-list") == 0) return corrupt_free_list_aborts();
  fprintf(stderr, "unknown case: %s\n", argv[1]);
  return 3;
}
"""

CASES = {
    "encoded-next": {
        "status": 0,
        "stdout": "encoded_next_nonzero=1",
        "stderr": None,
    },
    "randomized-order": {
        "status": 0,
        "stdout": "allocation_order_randomized=1",
        "stderr": None,
    },
    "double-free": {
        "status": 0,
        "stdout": None,
        "stderr": "double free detected",
    },
    "corrupt-free-list": {
        "status": -6,
        "stdout": None,
        "stderr": "corrupted free list entry",
    },
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


def rust_staticlib_secure() -> pathlib.Path:
    target_dir = BUILD_DIR / "rust-target"
    deps_dir = target_dir / "debug/deps"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(
        [
            "cargo",
            "rustc",
            "--features",
            "secure",
            "--lib",
            "--",
            "--crate-type",
            "staticlib",
        ],
        env=env,
    )
    libs = sorted(
        deps_dir.glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust secure staticlib artifact found")
    return libs[0]


def c_mimalloc_object(cc: str) -> pathlib.Path:
    obj = BUILD_DIR / "mimalloc-static-secure.o"
    run(
        [
            cc,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-DMI_SECURE=4",
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
    src = BUILD_DIR / "secure-mode-probe.c"
    src.write_text(PROBE_C)
    run(
        [
            cc,
            "-std=c11",
            "-DMI_SECURE=4",
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


def run_case(binary: pathlib.Path, case: str) -> subprocess.CompletedProcess[str]:
    return run([str(binary), case], check=False)


def verify_case(label: str, case: str, completed: subprocess.CompletedProcess[str]) -> None:
    expected = CASES[case]
    if completed.returncode != expected["status"]:
        raise SystemExit(
            f"{label} {case} exited {completed.returncode}, expected {expected['status']}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    stdout = expected["stdout"]
    if stdout is not None and stdout not in completed.stdout:
        raise SystemExit(f"{label} {case} stdout missing {stdout!r}: {completed.stdout!r}")
    stderr = expected["stderr"]
    if stderr is not None and stderr not in completed.stderr:
        raise SystemExit(f"{label} {case} stderr missing {stderr!r}: {completed.stderr!r}")


def main() -> int:
    cc = shutil.which("cc")
    if cc is None:
        raise SystemExit("cc is required for this check")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    c_bin = BUILD_DIR / "probe-c"
    rust_bin = BUILD_DIR / "probe-rust"
    compile_probe(cc, c_bin, [c_mimalloc_object(cc)])
    compile_probe(cc, rust_bin, [rust_staticlib_secure()])

    for case in CASES:
        c_result = run_case(c_bin, case)
        rust_result = run_case(rust_bin, case)
        verify_case("C", case, c_result)
        verify_case("Rust", case, rust_result)
        print(f"ok: {case}")

    print("ok: C and Rust secure-mode free-list behavior matches")
    return 0


if __name__ == "__main__":
    sys.exit(main())
