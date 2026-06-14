#!/usr/bin/env python3
"""Compare guarded allocation behavior for C and Rust mimalloc."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/guarded-mode-parity"

PROBE_C = r"""
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static int bytes_are_zero(const unsigned char* p, size_t size) {
  for (size_t i = 0; i < size; i++) {
    if (p[i] != 0) return 0;
  }
  return 1;
}

static int guarded_allocation(void) {
  mi_theap_t* theap = mi_theap_get_default();
  if (theap == NULL) return 10;
  mi_theap_guarded_set_size_bound(theap, 16, 4096);
  mi_theap_guarded_set_sample_rate(theap, 1, 1);

  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  if (!mi_stats_get(&before)) return 11;

  unsigned char* p = (unsigned char*)mi_zalloc(64);
  if (p == NULL) return 12;
  size_t usable = mi_usable_size(p);
  int zeroed = usable >= 64 && bytes_are_zero(p, 64);
  long page_size = sysconf(_SC_PAGESIZE);
  int aligned = (page_size > 0 && (((uintptr_t)p + 64) % (size_t)page_size) == 0);
  if (!mi_stats_get(&after)) return 13;
  long guarded_delta = (long)(after.malloc_guarded_count.total - before.malloc_guarded_count.total);

  printf("guarded_delta=%ld usable=%zu zeroed=%d end_aligned=%d\n",
         guarded_delta, usable, zeroed, aligned);
  mi_free(p);
  return (guarded_delta == 1 && usable >= 64 && zeroed && aligned ? 0 : 14);
}

static int guarded_sampling(void) {
  mi_theap_t* theap = mi_theap_get_default();
  if (theap == NULL) return 20;
  mi_theap_guarded_set_size_bound(theap, 16, 4096);
  mi_theap_guarded_set_sample_rate(theap, 3, 1);

  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  if (!mi_stats_get(&before)) return 21;

  void* a = mi_malloc(64);
  void* b = mi_malloc(64);
  void* c = mi_malloc(64);
  if (a == NULL || b == NULL || c == NULL) return 22;
  if (!mi_stats_get(&after)) return 23;
  long guarded_delta = (long)(after.malloc_guarded_count.total - before.malloc_guarded_count.total);
  long page_size = sysconf(_SC_PAGESIZE);
  int a_guarded = (page_size > 0 && (((uintptr_t)a + 64) % (size_t)page_size) == 0);
  int b_guarded = (page_size > 0 && (((uintptr_t)b + 64) % (size_t)page_size) == 0);
  int c_guarded = (page_size > 0 && (((uintptr_t)c + 64) % (size_t)page_size) == 0);
  printf("sample_guarded_delta=%ld sample_slots=%d%d%d\n",
         guarded_delta, a_guarded, b_guarded, c_guarded);
  mi_free(a);
  mi_free(b);
  mi_free(c);
  return (guarded_delta == 1 && !a_guarded && b_guarded && !c_guarded ? 0 : 24);
}

static int guarded_size_bound(void) {
  mi_theap_t* theap = mi_theap_get_default();
  if (theap == NULL) return 30;
  mi_theap_guarded_set_size_bound(theap, 128, 4096);
  mi_theap_guarded_set_sample_rate(theap, 1, 1);

  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  if (!mi_stats_get(&before)) return 31;

  void* small = mi_malloc(64);
  void* guarded = mi_malloc(256);
  if (small == NULL || guarded == NULL) return 32;
  if (!mi_stats_get(&after)) return 33;
  long guarded_delta = (long)(after.malloc_guarded_count.total - before.malloc_guarded_count.total);
  printf("bound_guarded_delta=%ld\n", guarded_delta);
  mi_free(small);
  mi_free(guarded);
  return (guarded_delta == 1 ? 0 : 34);
}

static int guarded_generic_allocation(void) {
  mi_theap_t* theap = mi_theap_get_default();
  if (theap == NULL) return 50;
  mi_theap_guarded_set_size_bound(theap, 16, 4096);
  mi_theap_guarded_set_sample_rate(theap, 1, 1);

  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  if (!mi_stats_get(&before)) return 51;

  unsigned char* p = (unsigned char*)mi_zalloc(2048);
  if (p == NULL) return 52;
  size_t usable = mi_usable_size(p);
  int zeroed = usable >= 2048 && bytes_are_zero(p, 2048);
  long page_size = sysconf(_SC_PAGESIZE);
  int aligned = (page_size > 0 && (((uintptr_t)p + 2048) % (size_t)page_size) == 0);
  if (!mi_stats_get(&after)) return 53;
  long guarded_delta = (long)(after.malloc_guarded_count.total - before.malloc_guarded_count.total);

  printf("generic_guarded_delta=%ld usable=%zu zeroed=%d end_aligned=%d\n",
         guarded_delta, usable, zeroed, aligned);
  mi_free(p);
  return (guarded_delta == 1 && usable >= 2048 && zeroed && aligned ? 0 : 54);
}

static int guarded_fault(void) {
  mi_theap_t* theap = mi_theap_get_default();
  if (theap == NULL) return 40;
  mi_theap_guarded_set_size_bound(theap, 16, 4096);
  mi_theap_guarded_set_sample_rate(theap, 1, 1);
  volatile unsigned char* p = (volatile unsigned char*)mi_malloc(64);
  if (p == NULL) return 41;
  p[64] = 0xA5;
  mi_free((void*)p);
  return 42;
}

static int guarded_generic_fault(void) {
  mi_theap_t* theap = mi_theap_get_default();
  if (theap == NULL) return 60;
  mi_theap_guarded_set_size_bound(theap, 16, 4096);
  mi_theap_guarded_set_sample_rate(theap, 1, 1);
  volatile unsigned char* p = (volatile unsigned char*)mi_malloc(2048);
  if (p == NULL) return 61;
  p[2048] = 0xA5;
  mi_free((void*)p);
  return 62;
}

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  mi_process_init();
  if (strcmp(argv[1], "allocation") == 0) return guarded_allocation();
  if (strcmp(argv[1], "sampling") == 0) return guarded_sampling();
  if (strcmp(argv[1], "size-bound") == 0) return guarded_size_bound();
  if (strcmp(argv[1], "generic-allocation") == 0) return guarded_generic_allocation();
  if (strcmp(argv[1], "fault") == 0) return guarded_fault();
  if (strcmp(argv[1], "generic-fault") == 0) return guarded_generic_fault();
  fprintf(stderr, "unknown case: %s\n", argv[1]);
  return 3;
}
"""

CASES = {
    "allocation": {
        "status": 0,
        "stdout": ["guarded_delta=1", "zeroed=1", "end_aligned=1"],
    },
    "sampling": {
        "status": 0,
        "stdout": ["sample_guarded_delta=1", "sample_slots=010"],
    },
    "size-bound": {
        "status": 0,
        "stdout": ["bound_guarded_delta=1"],
    },
    "generic-allocation": {
        "status": 0,
        "stdout": ["generic_guarded_delta=1", "zeroed=1", "end_aligned=1"],
    },
    "fault": {
        "status": -11,
        "stdout": [],
    },
    "generic-fault": {
        "status": -11,
        "stdout": [],
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
    run(
        [
            "cargo",
            "rustc",
            "--features",
            "guarded",
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
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def c_mimalloc_object(cc: str) -> pathlib.Path:
    obj = BUILD_DIR / "mimalloc-static-guarded.o"
    run(
        [
            cc,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-DMI_GUARDED=1",
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
    src = BUILD_DIR / "guarded-mode-probe.c"
    src.write_text(PROBE_C)
    run(
        [
            cc,
            "-std=c11",
            "-DMI_GUARDED=1",
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


def validate_case(label: str, completed: subprocess.CompletedProcess[str], case: str) -> None:
    expected = CASES[case]
    if completed.returncode != expected["status"]:
        raise AssertionError(
            f"{label} {case}: expected status {expected['status']}, got {completed.returncode}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    for needle in expected["stdout"]:
        if needle not in completed.stdout:
            raise AssertionError(
                f"{label} {case}: expected stdout to contain {needle!r}\n"
                f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
            )


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    rust_lib = rust_staticlib()
    c_obj = c_mimalloc_object(cc)

    c_bin = BUILD_DIR / "guarded-c"
    rust_bin = BUILD_DIR / "guarded-rust"
    compile_probe(cc, c_bin, [c_obj])
    compile_probe(cc, rust_bin, [rust_lib])

    for case in CASES:
        c_result = run_case(c_bin, case)
        rust_result = run_case(rust_bin, case)
        validate_case("C", c_result, case)
        validate_case("Rust", rust_result, case)
        if c_result.returncode != rust_result.returncode:
            raise AssertionError(f"{case}: status mismatch C={c_result.returncode} Rust={rust_result.returncode}")
        print(f"ok: guarded {case}")

    print("ok: C and Rust guarded allocation behavior matches")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
