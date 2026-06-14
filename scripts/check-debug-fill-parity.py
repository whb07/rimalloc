#!/usr/bin/env python3
"""Compare debug fill and padding behavior for C and Rust mimalloc."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/debug-fill-parity"

PROBE_C = r"""
#include <mimalloc.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define MI_DEBUG_UNINIT 0xD0
#define MI_DEBUG_FREED 0xDF

static int all_bytes_are(const unsigned char* p, size_t size, unsigned char value) {
  for (size_t i = 0; i < size; i++) {
    if (p[i] != value) return 0;
  }
  return 1;
}

static int malloc_is_filled_uninit(void) {
  unsigned char* p = (unsigned char*)mi_malloc(64);
  if (p == NULL) return 10;
  int ok = all_bytes_are(p, 64, MI_DEBUG_UNINIT);
  printf("malloc_uninit=%d\n", ok);
  mi_free(p);
  return (ok ? 0 : 11);
}

static int realloc_growth_is_filled_uninit(void) {
  unsigned char* p = (unsigned char*)mi_mallocn(64, 1);
  if (p == NULL) return 20;
  p = (unsigned char*)mi_reallocn(p, 3, 64);
  if (p == NULL) return 21;
  int ok = all_bytes_are(p, 64 * 3, MI_DEBUG_UNINIT);
  printf("realloc_growth_uninit=%d\n", ok);
  mi_free(p);
  return (ok ? 0 : 22);
}

static int freed_block_is_filled(void) {
  unsigned char* p = (unsigned char*)mi_malloc(64);
  if (p == NULL) return 30;
  mi_free(p);
  int ok = all_bytes_are(p + sizeof(uintptr_t), 64 - sizeof(uintptr_t), MI_DEBUG_FREED);
  printf("freed_fill=%d\n", ok);
  return (ok ? 0 : 31);
}

static int padding_overrun_reports_error(void) {
  unsigned char* p = (unsigned char*)mi_malloc(16);
  if (p == NULL) return 40;
  p[16] = 0xA5;
  mi_free(p);
  return 0;
}

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  mi_process_init();
  if (strcmp(argv[1], "malloc-uninit") == 0) return malloc_is_filled_uninit();
  if (strcmp(argv[1], "realloc-growth-uninit") == 0) return realloc_growth_is_filled_uninit();
  if (strcmp(argv[1], "freed-fill") == 0) return freed_block_is_filled();
  if (strcmp(argv[1], "padding-overrun") == 0) return padding_overrun_reports_error();
  fprintf(stderr, "unknown case: %s\n", argv[1]);
  return 3;
}
"""

CASES = {
    "malloc-uninit": {
        "status": 0,
        "stdout": "malloc_uninit=1",
        "stderr": None,
    },
    "realloc-growth-uninit": {
        "status": 0,
        "stdout": "realloc_growth_uninit=1",
        "stderr": None,
    },
    "freed-fill": {
        "status": 0,
        "stdout": "freed_fill=1",
        "stderr": None,
    },
    "padding-overrun": {
        "status": -6,
        "stdout": None,
        "stderr": "buffer overflow",
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


def rust_staticlib() -> pathlib.Path:
    target_dir = BUILD_DIR / "rust-target"
    deps_dir = target_dir / "debug/deps"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(["cargo", "rustc", "--lib", "--", "--crate-type", "staticlib"], env=env)
    libs = sorted(
        deps_dir.glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def c_mimalloc_object(cc: str) -> pathlib.Path:
    obj = BUILD_DIR / "mimalloc-static-debug.o"
    run(
        [
            cc,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
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
    src = BUILD_DIR / "debug-fill-probe.c"
    src.write_text(PROBE_C)
    run(
        [
            cc,
            "-std=c11",
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
    c_bin = BUILD_DIR / "probe-c-debug"
    rust_bin = BUILD_DIR / "probe-rust-debug"
    compile_probe(cc, c_bin, [c_mimalloc_object(cc)])
    compile_probe(cc, rust_bin, [rust_staticlib()])

    for case in CASES:
        c_result = run([str(c_bin), case], check=False)
        rust_result = run([str(rust_bin), case], check=False)
        verify_case("C debug", case, c_result)
        verify_case("Rust debug", case, rust_result)
        print(f"ok: debug {case}")

    print("ok: C and Rust debug fill and padding behavior matches")
    return 0


if __name__ == "__main__":
    sys.exit(main())
