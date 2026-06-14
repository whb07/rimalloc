#!/usr/bin/env python3
"""Compare secure/debug free-list hardening behavior for C and Rust mimalloc."""

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

static int invalid_free_reports_error(void) {
  uintptr_t local = 0;
  mi_free(&local);
  return 0;
}

static int padding_overrun_reports_error(void) {
  unsigned char* p = (unsigned char*)mi_malloc(16);
  if (p == NULL) return 35;
  p[16] = 0xA5;
  mi_free(p);
  return 0;
}

static int wrong_heap_check_rejects_pointer(void) {
  mi_heap_t* heap = mi_heap_new();
  mi_heap_t* other = mi_heap_new();
  if (heap == NULL || other == NULL) return 36;
  void* p = mi_heap_malloc(heap, 64);
  if (p == NULL) return 37;
  int rejected = !mi_heap_contains(other, p) && mi_heap_contains(heap, p) && mi_check_owned(p);
  printf("wrong_heap_rejected=%d\n", rejected);
  mi_free(p);
  mi_heap_delete(heap);
  mi_heap_delete(other);
  return (rejected ? 0 : 38);
}

static int wrong_size_hint_aborts(void) {
  void* p = mi_malloc(32);
  if (p == NULL) return 39;
  mi_free_size(p, 1024 * 1024);
  return 0;
}

static int wrong_size_alignment_hint_aborts(void) {
  void* p = mi_malloc_aligned_at(32, 64, 32);
  if (p == NULL) return 41;
  mi_free_size_aligned(p, 32, 64);
  return 0;
}

static int wrong_alignment_hint_aborts(void) {
  void* p = mi_malloc_aligned_at(32, 64, 32);
  if (p == NULL) return 42;
  mi_free_aligned(p, 64);
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
  mi_process_init();
  if (strcmp(argv[1], "encoded-next") == 0) return encoded_next_is_not_raw_null();
  if (strcmp(argv[1], "randomized-order") == 0) return allocation_order_is_randomized();
  if (strcmp(argv[1], "double-free") == 0) return double_free_reports_error();
  if (strcmp(argv[1], "invalid-free") == 0) return invalid_free_reports_error();
  if (strcmp(argv[1], "padding-overrun") == 0) return padding_overrun_reports_error();
  if (strcmp(argv[1], "wrong-heap") == 0) return wrong_heap_check_rejects_pointer();
  if (strcmp(argv[1], "wrong-size-hint") == 0) return wrong_size_hint_aborts();
  if (strcmp(argv[1], "wrong-size-alignment-hint") == 0) return wrong_size_alignment_hint_aborts();
  if (strcmp(argv[1], "wrong-alignment-hint") == 0) return wrong_alignment_hint_aborts();
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
    "invalid-free": {
        "status": 0,
        "stdout": None,
        "stderr": "invalid pointer",
    },
    "padding-overrun": {
        "status": -6,
        "stdout": None,
        "stderr": "buffer overflow",
    },
    "wrong-heap": {
        "status": 0,
        "stdout": "wrong_heap_rejected=1",
        "stderr": None,
    },
    "wrong-size-hint": {
        "status": -6,
        "stdout": None,
        "stderr": "assertion failed",
    },
    "wrong-size-alignment-hint": {
        "status": -6,
        "stdout": None,
        "stderr": "assertion failed",
    },
    "wrong-alignment-hint": {
        "status": -6,
        "stdout": None,
        "stderr": "assertion failed",
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


def rust_staticlib(features: list[str], name: str) -> pathlib.Path:
    target_dir = BUILD_DIR / f"rust-target-{name}"
    deps_dir = target_dir / "debug/deps"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    cmd = ["cargo", "rustc"]
    if features:
        cmd.extend(["--features", ",".join(features)])
    cmd.extend(["--lib", "--", "--crate-type", "staticlib"])
    run(cmd, env=env)
    libs = sorted(
        deps_dir.glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit(f"no Rust {name} staticlib artifact found")
    return libs[0]


def c_mimalloc_object(cc: str, secure: bool) -> pathlib.Path:
    mode = "secure" if secure else "debug"
    obj = BUILD_DIR / f"mimalloc-static-{mode}.o"
    cmd = [
        cc,
        "-std=c11",
        "-DMI_MALLOC_OVERRIDE=0",
    ]
    if secure:
        cmd.append("-DMI_SECURE=4")
    cmd.extend(
        [
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
    run(cmd)
    return obj


def compile_probe(cc: str, output: pathlib.Path, objects: list[pathlib.Path], secure: bool) -> None:
    src = BUILD_DIR / "secure-mode-probe.c"
    src.write_text(PROBE_C)
    cmd = [
        cc,
        "-std=c11",
    ]
    if secure:
        cmd.append("-DMI_SECURE=4")
    cmd.extend(
        [
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
    run(cmd)


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

    c_secure_bin = BUILD_DIR / "probe-c-secure"
    rust_secure_bin = BUILD_DIR / "probe-rust-secure"
    compile_probe(cc, c_secure_bin, [c_mimalloc_object(cc, secure=True)], secure=True)
    compile_probe(cc, rust_secure_bin, [rust_staticlib(["secure"], "secure")], secure=True)

    for case in CASES:
        c_result = run_case(c_secure_bin, case)
        rust_result = run_case(rust_secure_bin, case)
        verify_case("C secure", case, c_result)
        verify_case("Rust secure", case, rust_result)
        print(f"ok: secure {case}")

    c_debug_bin = BUILD_DIR / "probe-c-debug"
    rust_debug_bin = BUILD_DIR / "probe-rust-debug"
    compile_probe(cc, c_debug_bin, [c_mimalloc_object(cc, secure=False)], secure=False)
    compile_probe(cc, rust_debug_bin, [rust_staticlib([], "debug")], secure=False)

    for case in [
        "encoded-next",
        "double-free",
        "invalid-free",
        "padding-overrun",
        "wrong-heap",
        "wrong-size-hint",
        "wrong-size-alignment-hint",
        "wrong-alignment-hint",
        "corrupt-free-list",
    ]:
        c_result = run_case(c_debug_bin, case)
        rust_result = run_case(rust_debug_bin, case)
        verify_case("C debug", case, c_result)
        verify_case("Rust debug", case, rust_result)
        print(f"ok: debug {case}")

    print("ok: C and Rust secure/debug free-list hardening behavior matches")
    return 0


if __name__ == "__main__":
    sys.exit(main())
