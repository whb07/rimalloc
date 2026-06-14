#!/usr/bin/env python3
"""Verify the current custom-subprocess heap-main baseline behavior."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/subproc-heap-main-baseline"

PROBE = r"""
#include <mimalloc.h>

#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

static int custom_subproc_heap_main_status(void) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }

    mi_subproc_id_t subproc = mi_subproc_new();
    if (subproc._mi_subproc_id == NULL) _exit(10);

    mi_subproc_add_current_thread(subproc);
    if (mi_subproc_current()._mi_subproc_id != subproc._mi_subproc_id) _exit(11);

    mi_heap_t* heap = mi_heap_main();
    _exit(heap == NULL ? 16 : 0);
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

int main(void) {
  int status = custom_subproc_heap_main_status();
  printf("subproc_heap_main_custom_status=%d\n", status);
  return 0;
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


def compile_probe(cc: str, output: pathlib.Path, objects: list[pathlib.Path]) -> None:
    src = BUILD_DIR / "subproc-heap-main.c"
    src.write_text(PROBE)
    run(
        [
            cc,
            "-std=c11",
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


def capture_status(binary: pathlib.Path) -> int:
    completed = run([str(binary)])
    prefix = "subproc_heap_main_custom_status="
    for line in completed.stdout.splitlines():
        if line.startswith(prefix):
            return int(line.removeprefix(prefix))
    raise AssertionError(f"missing status output from {binary}: {completed.stdout!r}")


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    rust_lib = rust_staticlib()
    c_bin = BUILD_DIR / "subproc-heap-main-c"
    rust_bin = BUILD_DIR / "subproc-heap-main-rust"
    compile_probe(cc, c_bin, [ROOT / "mimalloc-3.3.2/src/static.c"])
    compile_probe(cc, rust_bin, [rust_lib])

    c_status = capture_status(c_bin)
    rust_status = capture_status(rust_bin)
    expected = 134
    if c_status != expected or rust_status != expected:
        raise AssertionError(
            "unexpected custom subprocess heap-main status: "
            f"C={c_status} Rust={rust_status} expected={expected}"
        )

    print("ok: C and Rust currently abort on custom subprocess mi_heap_main")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
