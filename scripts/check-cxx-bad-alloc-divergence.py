#!/usr/bin/env python3
"""Verify the documented C++ bad_alloc divergence for Rust mimalloc."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/cxx-bad-alloc-divergence"

PROBE = r"""
#include <mimalloc.h>

#include <new>
#include <stdint.h>
#include <stdio.h>

int main(void) {
  int caught = 0;
  try {
    void* p = mi_new_n((size_t)-1, 2);
    (void)p;
  }
  catch (const std::bad_alloc&) {
    caught = 1;
  }
  catch (...) {
    caught = 2;
  }
  printf("caught=%d\n", caught);
  return (caught == 1 ? 0 : 1);
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
        (target_dir / "debug/deps").glob("librimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def compile_probe(cxx: str, output: pathlib.Path, objects: list[pathlib.Path]) -> None:
    src = BUILD_DIR / "bad-alloc.cpp"
    src.write_text(PROBE)
    run(
        [
            cxx,
            "-std=c++17",
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
        ]
    )


def main() -> int:
    cxx = os.environ.get("CXX") or shutil.which("c++") or shutil.which("g++") or shutil.which("clang++")
    if cxx is None:
        raise SystemExit("no C++ compiler found; set CXX")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    rust_lib = rust_staticlib()
    c_bin = BUILD_DIR / "bad-alloc-c"
    rust_bin = BUILD_DIR / "bad-alloc-rust"
    compile_probe(cxx, c_bin, [ROOT / "mimalloc-3.3.2/src/static.c"])
    compile_probe(cxx, rust_bin, [rust_lib])

    c_result = run([str(c_bin)], check=False)
    if c_result.returncode != 0 or "caught=1" not in c_result.stdout:
        raise AssertionError(
            "C++-compiled C mimalloc did not throw/catch std::bad_alloc as expected\n"
            f"status={c_result.returncode}\nstdout:\n{c_result.stdout}\nstderr:\n{c_result.stderr}"
        )

    rust_result = run([str(rust_bin)], check=False)
    if rust_result.returncode not in (-6, 134):
        raise AssertionError(
            "Rust mimalloc did not abort on the documented no-handler bad_alloc path\n"
            f"status={rust_result.returncode}\nstdout:\n{rust_result.stdout}\nstderr:\n{rust_result.stderr}"
        )

    print("ok: C++ C mimalloc throws std::bad_alloc")
    print("ok: Rust mimalloc aborts on the documented C++ bad_alloc boundary")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
