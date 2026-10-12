#!/usr/bin/env python3
"""Validate C++ operator new/delete overrides from staged release artifacts."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
RELEASE_DIR = ROOT / "target/release-artifacts"


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

struct alignas(64) AlignedBlock {
  unsigned char bytes[64];
};

int main(void) {
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
  ::operator delete(sized, (size_t)64);

  AlignedBlock* aligned_object = new AlignedBlock();
  int aligned_object_owned = (aligned_object != NULL &&
                              pointer_aligned(aligned_object, alignof(AlignedBlock)) &&
                              mi_check_owned(aligned_object));
  delete aligned_object;

  void* aligned = ::operator new(96, std::align_val_t(64));
  int aligned_owned = (pointer_aligned(aligned, 64) && mi_check_owned(aligned));
  ::operator delete(aligned, std::align_val_t(64));

  void* aligned_nothrow = ::operator new[](128, std::align_val_t(64), std::nothrow);
  int aligned_nothrow_owned = (pointer_aligned(aligned_nothrow, 64) &&
                               mi_check_owned(aligned_nothrow));
  ::operator delete[](aligned_nothrow, std::align_val_t(64), std::nothrow);

  printf("@@CXX_RELEASE handler_calls=%d handler_throw=%d scalar=%d array=%d nothrow=%d sized=%d aligned_object=%d aligned=%d aligned_nothrow=%d\n",
         handler_calls,
         handler_caught,
         scalar_owned,
         array_owned,
         nothrow_owned,
         sized_owned,
         aligned_object_owned,
         aligned_owned,
         aligned_nothrow_owned);

  return (handler_calls == 1 &&
          handler_caught == 1 &&
          scalar_owned == 1 &&
          array_owned == 1 &&
          nothrow_owned == 1 &&
          sized_owned == 1 &&
          aligned_object_owned == 1 &&
          aligned_owned == 1 &&
          aligned_nothrow_owned == 1 ? 0 : 1);
}
"""


def build_release_artifacts() -> dict[str, object]:
    path = RELEASE_DIR / "release-artifacts.json"
    if not (os.environ.get("MECHANICAL_MIMALLOC_REUSE_RELEASE_ARTIFACTS") and path.exists()):
        subprocess.run(["scripts/build-release-artifacts.py"], cwd=ROOT, check=True)
    return json.loads(path.read_text())


def staged_manifest_path(staged: pathlib.Path, path: str) -> pathlib.Path:
    release_relative = RELEASE_DIR.relative_to(ROOT)
    relative = pathlib.Path(path).relative_to(release_relative)
    return staged / relative


def artifact_path(data: dict[str, object], staged: pathlib.Path, name: str) -> pathlib.Path:
    for artifact in data["artifacts"]:
        if artifact["name"] == name:
            return staged_manifest_path(staged, str(artifact["path"]))
    raise SystemExit(f"missing release artifact in manifest: {name}")


def write_probe(work_dir: pathlib.Path) -> pathlib.Path:
    work_dir.mkdir(parents=True, exist_ok=True)
    source = work_dir / "cxx_release_probe.cpp"
    source.write_text(PROBE)
    return source


def compile_probe(
    cxx: str,
    *,
    include_dir: pathlib.Path,
    library: pathlib.Path,
    source: pathlib.Path,
    binary: pathlib.Path,
    dynamic: bool,
) -> None:
    command = [
        cxx,
        "-std=c++20",
        "-I",
        str(include_dir),
        str(source),
        str(library),
    ]
    if dynamic:
        command.append("-Wl,-rpath," + str(library.parent))
    command.extend(["-pthread", "-ldl", "-lm", "-o", str(binary)])
    subprocess.run(command, cwd=source.parent, check=True)


def run_probe(binary: pathlib.Path, library: pathlib.Path, *, dynamic: bool) -> None:
    env = os.environ.copy()
    if dynamic:
        existing = env.get("LD_LIBRARY_PATH")
        library_path = str(library.parent)
        env["LD_LIBRARY_PATH"] = (
            library_path if existing is None else library_path + os.pathsep + existing
        )
    completed = subprocess.run(
        [str(binary)],
        cwd=binary.parent,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    fields = completed.stdout.strip().split()
    if len(fields) != 10 or fields[0] != "@@CXX_RELEASE":
        raise AssertionError(f"unexpected C++ release probe output: {completed.stdout!r}")
    values = dict(field.split("=", 1) for field in fields[1:])
    if any(value != "1" for value in values.values()):
        raise AssertionError(f"C++ release probe failed for {binary.name}: {values}")


def run_case(
    cxx: str,
    *,
    include_dir: pathlib.Path,
    library: pathlib.Path,
    work_dir: pathlib.Path,
    case_name: str,
    dynamic: bool,
) -> None:
    source = write_probe(work_dir)
    binary = work_dir / case_name
    compile_probe(
        cxx,
        include_dir=include_dir,
        library=library,
        source=source,
        binary=binary,
        dynamic=dynamic,
    )
    run_probe(binary, library, dynamic=dynamic)


def main() -> int:
    cxx = os.environ.get("CXX") or shutil.which("c++") or shutil.which("g++") or shutil.which("clang++")
    if cxx is None:
        raise SystemExit("no C++ compiler found; set CXX")

    data = build_release_artifacts()
    with tempfile.TemporaryDirectory(prefix="mechanical-mimalloc-cxx-artifacts-") as tmp:
        tmp_dir = pathlib.Path(tmp)
        staged = tmp_dir / RELEASE_DIR.name
        shutil.copytree(RELEASE_DIR, staged, ignore=shutil.ignore_patterns("build"))
        include_dir = staged / "include"
        cases = [
            ("interpose-cdylib", "interpose-cxx-dynamic", True),
            ("interpose-static-preview", "interpose-cxx-static", False),
        ]
        for artifact_name, case_name, dynamic in cases:
            run_case(
                cxx,
                include_dir=include_dir,
                library=artifact_path(data, staged, artifact_name),
                work_dir=tmp_dir / case_name,
                case_name=case_name,
                dynamic=dynamic,
            )
    print("ok: C++ release artifact operator new/delete checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
