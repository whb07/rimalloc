#!/usr/bin/env python3
"""Validate no-unwind behavior at the release c-abi cdylib boundary."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/no-unwind-artifact"

PROBE = r"""
#include <mimalloc.h>

#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int error_calls = 0;
static int error_alloc_ok = 1;
static int error_last = 0;
static int deferred_calls = 0;
static int deferred_alloc_ok = 1;

static int callback_alloc_roundtrip(size_t size) {
  void* p = mi_malloc(size);
  if (p == NULL) return 0;
  mi_free(p);
  return 1;
}

static void error_callback(int err, void* arg) {
  if (arg != (void*)0xE11A) error_alloc_ok = 0;
  error_calls++;
  error_last = err;
  if (!callback_alloc_roundtrip(37)) error_alloc_ok = 0;
}

static void deferred_callback(bool force, unsigned long long heartbeat, void* arg) {
  (void)force;
  if (arg != (void*)0xDEF || heartbeat == 0) deferred_alloc_ok = 0;
  deferred_calls++;
  if (!callback_alloc_roundtrip(43)) deferred_alloc_ok = 0;
}

static int run_release_paths(void) {
  mi_option_set(mi_option_show_errors, 0);

  void* p = mi_malloc(64);
  if (p == NULL) return 10;
  mi_free(p);

  mi_register_error(error_callback, (void*)0xE11A);
  errno = 0;
  void* overflow = mi_calloc(SIZE_MAX, 2);
  if (overflow != NULL) return 11;
  if (error_calls == 0 || error_last == 0 || !error_alloc_ok) return 12;
  mi_register_error(NULL, NULL);

  void* oom = mi_malloc(SIZE_MAX);
  if (oom != NULL) {
    mi_free(oom);
    return 13;
  }

  mi_register_deferred_free(deferred_callback, (void*)0xDEF);
  mi_collect(false);
  mi_collect(true);
  if (deferred_calls == 0 || !deferred_alloc_ok) return 14;
  mi_register_deferred_free(NULL, NULL);

  return 0;
}

int main(void) {
  int release_paths = run_release_paths();
  if (release_paths != 0) {
    fprintf(stderr, "release path check failed: %d\n", release_paths);
    return release_paths;
  }

  return 0;
}
"""


def run(
    cmd: list[str],
    *,
    cwd: pathlib.Path = ROOT,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def cc() -> str:
    compiler = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if compiler is None:
        raise SystemExit("no C compiler found; set CC")
    return compiler


def build_c_abi_cdylib() -> pathlib.Path:
    target_dir = BUILD_DIR / "rust-target"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(
        [
            "cargo",
            "rustc",
            "--features",
            "c-abi",
            "--lib",
            "--release",
            "--",
            "--crate-type",
            "cdylib",
        ],
        env=env,
    )

    candidates = sorted(
        list((target_dir / "release").glob("librimalloc*.so"))
        + list((target_dir / "release/deps").glob("librimalloc*.so")),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not candidates:
        raise SystemExit("no release c-abi cdylib artifact found")
    return candidates[0]


def assert_release_profile_aborts() -> None:
    cargo_toml = (ROOT / "Cargo.toml").read_text()
    if "[profile.release]\npanic = \"abort\"" not in cargo_toml:
        raise AssertionError("release profile must use panic = \"abort\"")


def assert_no_unwind_exports(cdylib: pathlib.Path) -> None:
    completed = run(["readelf", "-W", "--dyn-syms", str(cdylib)])
    if "rust_eh_personality" not in completed.stdout:
        raise AssertionError("release c-abi cdylib must export the aborting Rust personality shim")
    forbidden = [
        "_Unwind_",
        "panic_unwind",
        "__gcc_personality_v0",
        "__gxx_personality_v0",
    ]
    found = [symbol for symbol in forbidden if symbol in completed.stdout]
    if found:
        raise AssertionError(
            "release c-abi cdylib exports unwind/personality symbols: " + ", ".join(found)
        )


def compile_probe(compiler: str, cdylib: pathlib.Path) -> pathlib.Path:
    probe_dir = BUILD_DIR / "probe"
    probe_dir.mkdir(parents=True, exist_ok=True)
    source = probe_dir / "no_unwind_probe.c"
    binary = probe_dir / "no_unwind_probe"
    source.write_text(PROBE)
    run(
        [
            compiler,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            str(source),
            str(cdylib),
            "-Wl,-rpath," + str(cdylib.parent),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ]
    )
    return binary


def run_probe(binary: pathlib.Path, cdylib: pathlib.Path) -> None:
    env = dict(os.environ)
    ld_library_path = str(cdylib.parent)
    if env.get("LD_LIBRARY_PATH"):
        ld_library_path = ld_library_path + os.pathsep + env["LD_LIBRARY_PATH"]
    env["LD_LIBRARY_PATH"] = ld_library_path
    run([str(binary)], env=env)


def run_callback_panic_containment_tests() -> None:
    run(
        [
            "cargo",
            "test",
            "--quiet",
            "--features",
            "std",
            "callback",
            "--lib",
            "--",
            "--ignored",
            "--test-threads=1",
        ]
    )


def main() -> int:
    if sys.platform != "linux":
        raise SystemExit("no-unwind artifact check currently requires Linux")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    assert_release_profile_aborts()
    cdylib = build_c_abi_cdylib()
    assert_no_unwind_exports(cdylib)
    binary = compile_probe(cc(), cdylib)
    run_probe(binary, cdylib)
    run_callback_panic_containment_tests()

    print("ok: release c-abi cdylib no-unwind artifact checks passed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
