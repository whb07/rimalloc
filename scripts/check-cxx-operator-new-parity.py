#!/usr/bin/env python3
"""Compare C++ global new/delete override behavior against C mimalloc."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/cxx-operator-new-parity"
STANDARDS = ["c++11", "c++14", "c++17", "c++20"]

PROBE = r"""
#include <mimalloc.h>

#include <new>
#include <stdint.h>
#include <stdio.h>

static int handler_calls = 0;

static void one_shot_new_handler(void) {
  handler_calls++;
  std::set_new_handler(NULL);
}

static int pointer_aligned(void* p, size_t alignment) {
  return p != NULL && (((uintptr_t)p % alignment) == 0);
}

int main(void) {
  int scalar_caught = 0;
  try {
    void* p = ::operator new((size_t)-1);
    (void)p;
  }
  catch (const std::bad_alloc&) {
    scalar_caught = 1;
  }
  catch (...) {
    scalar_caught = 2;
  }

  std::set_new_handler(one_shot_new_handler);
  int handler_caught = 0;
  try {
    void* p = ::operator new((size_t)-1);
    (void)p;
  }
  catch (const std::bad_alloc&) {
    handler_caught = 1;
  }
  catch (...) {
    handler_caught = 2;
  }
  std::set_new_handler(NULL);

  int* scalar = new int(42);
  int scalar_owned = (scalar != NULL && *scalar == 42 && mi_check_owned(scalar));
  delete scalar;

  int* array = new int[8];
  int array_owned = (array != NULL && mi_check_owned(array));
  delete[] array;

  char* nothrow_array = new (std::nothrow) char[33];
  int nothrow_owned = (nothrow_array != NULL && mi_check_owned(nothrow_array));
  delete[] nothrow_array;

  void* sized = ::operator new(64);
  int sized_owned = (sized != NULL && mi_check_owned(sized));
#if __cplusplus >= 201402L
  ::operator delete(sized, (size_t)64);
#else
  ::operator delete(sized);
#endif

  int aligned_owned = -1;
  int aligned_nothrow_owned = -1;
#if __cplusplus >= 201703L
  void* aligned = ::operator new(96, std::align_val_t(64));
  aligned_owned = (pointer_aligned(aligned, 64) && mi_check_owned(aligned));
  ::operator delete(aligned, std::align_val_t(64));

  void* aligned_nothrow = ::operator new[](128, std::align_val_t(64), std::nothrow);
  aligned_nothrow_owned = (pointer_aligned(aligned_nothrow, 64) && mi_check_owned(aligned_nothrow));
  ::operator delete[](aligned_nothrow, std::align_val_t(64), std::nothrow);
#endif

  printf("@@CXX_OPERATOR scalar_throw=%d handler_calls=%d handler_throw=%d scalar=%d array=%d nothrow=%d sized=%d aligned=%d aligned_nothrow=%d\n",
         scalar_caught,
         handler_calls,
         handler_caught,
         scalar_owned,
         array_owned,
         nothrow_owned,
         sized_owned,
         aligned_owned,
         aligned_nothrow_owned);
  return (scalar_caught == 1 &&
          handler_calls == 1 &&
          handler_caught == 1 &&
          scalar_owned == 1 &&
          array_owned == 1 &&
          nothrow_owned == 1 &&
          sized_owned == 1 &&
          aligned_owned != 0 &&
          aligned_nothrow_owned != 0 ? 0 : 1);
}
"""


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
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(["cargo", "rustc", "--lib", "--", "--crate-type", "staticlib"], env=env)
    libs = sorted(
        (target_dir / "debug/deps").glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def c_static_object(cxx: str) -> pathlib.Path:
    obj = BUILD_DIR / "mimalloc-static-cxx17.o"
    run(
        [
            cxx,
            "-std=c++17",
            "-DMI_MALLOC_OVERRIDE=0",
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


def compile_probe(
    cxx: str,
    standard: str,
    output: pathlib.Path,
    objects: list[pathlib.Path],
) -> bool:
    src = BUILD_DIR / f"operator-new-{standard}.cpp"
    src.write_text(PROBE)
    completed = run(
        [
            cxx,
            f"-std={standard}",
            "-DMI_MALLOC_OVERRIDE=0",
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
        ],
        check=False,
    )
    if completed.returncode == 0:
        return True
    sys.stderr.write(completed.stdout)
    sys.stderr.write(completed.stderr)
    raise AssertionError(f"{standard} probe failed to compile for {output}")


def parse_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 10 or fields[0] != "@@CXX_OPERATOR":
        raise AssertionError(f"unexpected C++ operator capture: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    return values


def validate_capture(standard: str, values: dict[str, int]) -> None:
    expected = {
        "scalar_throw": 1,
        "handler_calls": 1,
        "handler_throw": 1,
        "scalar": 1,
        "array": 1,
        "nothrow": 1,
        "sized": 1,
        "aligned": 1 if standard in {"c++17", "c++20"} else -1,
        "aligned_nothrow": 1 if standard in {"c++17", "c++20"} else -1,
    }
    if values != expected:
        raise AssertionError(f"{standard} unexpected C++ operator values: {values}")


def main() -> int:
    cxx = os.environ.get("CXX") or shutil.which("c++") or shutil.which("g++") or shutil.which("clang++")
    if cxx is None:
        raise SystemExit("no C++ compiler found; set CXX")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    c_obj = c_static_object(cxx)
    rust_lib = rust_staticlib()
    checked: list[str] = []
    for standard in STANDARDS:
        c_bin = BUILD_DIR / f"operator-new-c-{standard}"
        rust_bin = BUILD_DIR / f"operator-new-rust-{standard}"
        c_supported = compile_probe(cxx, standard, c_bin, [c_obj])
        rust_supported = compile_probe(cxx, standard, rust_bin, [rust_lib])
        if c_supported != rust_supported:
            raise AssertionError(f"{standard} support differs: C={c_supported} Rust={rust_supported}")
        if not c_supported:
            continue

        c_values = parse_capture(run([str(c_bin)]).stdout)
        rust_values = parse_capture(run([str(rust_bin)]).stdout)
        validate_capture(standard, c_values)
        validate_capture(standard, rust_values)
        if c_values != rust_values:
            raise AssertionError(f"{standard} mismatch: C={c_values} Rust={rust_values}")
        checked.append(standard)

    if not checked:
        raise AssertionError("no C++ standards were checked")
    print(f"ok: C++ global new/delete parity for {', '.join(checked)}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
