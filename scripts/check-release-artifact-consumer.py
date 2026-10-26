#!/usr/bin/env python3
"""Build C consumers using only staged release artifacts."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
RELEASE_DIR = ROOT / "target/release-artifacts"


def manifest() -> dict[str, object]:
    path = RELEASE_DIR / "release-artifacts.json"
    if not (os.environ.get("RIMALLOC_REUSE_RELEASE_ARTIFACTS") and path.exists()):
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


def verify_headers(data: dict[str, object], staged: pathlib.Path) -> None:
    expected = {
        "mimalloc.h",
        "mimalloc-stats.h",
        "mimalloc-override.h",
        "mimalloc-new-delete.h",
    }
    actual = {header["name"] for header in data["headers"]}
    missing = expected - actual
    if missing:
        raise SystemExit(f"missing staged release headers: {sorted(missing)}")
    for header in data["headers"]:
        path = staged_manifest_path(staged, str(header["path"]))
        if not path.exists():
            raise SystemExit(f"staged header missing on disk: {path}")


def write_c_abi_consumer(work_dir: pathlib.Path) -> pathlib.Path:
    work_dir.mkdir(parents=True, exist_ok=True)
    source = work_dir / "c_abi_consumer.c"
    source.write_text(
        """\
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <assert.h>
#include <stddef.h>

int main(void) {
  void* p = mi_malloc(64);
  assert(p != NULL);
  assert(mi_usable_size(p) >= 64);
  mi_stats_t stats;
  mi_stats_init(&stats);
  mi_free(p);
  return 0;
}
"""
    )
    return source


def write_interpose_consumer(work_dir: pathlib.Path) -> pathlib.Path:
    work_dir.mkdir(parents=True, exist_ok=True)
    source = work_dir / "interpose_consumer.c"
    source.write_text(
        """\
#include <mimalloc.h>

#include <assert.h>
#include <stddef.h>
#include <stdlib.h>

int main(void) {
  void* p = malloc(64);
  assert(p != NULL);
  assert(mi_check_owned(p));
  p = realloc(p, 128);
  assert(p != NULL);
  assert(mi_check_owned(p));
  free(p);

  void* q = calloc(4, 32);
  assert(q != NULL);
  assert(mi_check_owned(q));
  free(q);
  return 0;
}
"""
    )
    return source


def compile_and_run(
    cc: str,
    *,
    include_dir: pathlib.Path,
    library: pathlib.Path,
    source: pathlib.Path,
    binary: pathlib.Path,
    dynamic: bool,
    cxx_runtime: bool,
) -> None:
    command = [
        cc,
        "-std=c11",
        "-I",
        str(include_dir),
        str(source),
        str(library),
    ]
    if dynamic:
        command.append("-Wl,-rpath," + str(library.parent))
    if cxx_runtime:
        command.append("-lstdc++")
    command.extend(["-pthread", "-ldl", "-lm", "-o", str(binary)])
    subprocess.run(command, cwd=source.parent, check=True)
    env = os.environ.copy()
    if dynamic:
        existing = env.get("LD_LIBRARY_PATH")
        library_path = str(library.parent)
        env["LD_LIBRARY_PATH"] = (
            library_path if existing is None else library_path + os.pathsep + existing
        )
    subprocess.run([str(binary)], cwd=source.parent, env=env, check=True)


def run_consumers(cc: str, data: dict[str, object], staged: pathlib.Path) -> None:
    include_dir = staged / "include"
    cases = [
        (
            "c-abi-cdylib",
            write_c_abi_consumer,
            "c-abi-dynamic",
            True,
            False,
        ),
        (
            "c-abi-static-preview",
            write_c_abi_consumer,
            "c-abi-static",
            False,
            False,
        ),
        (
            "interpose-cdylib",
            write_interpose_consumer,
            "interpose-dynamic",
            True,
            True,
        ),
        (
            "interpose-static-preview",
            write_interpose_consumer,
            "interpose-static",
            False,
            True,
        ),
    ]
    for artifact_name, write_source, case_name, dynamic, cxx_runtime in cases:
        work_dir = staged.parent / case_name
        source = write_source(work_dir)
        library = artifact_path(data, staged, artifact_name)
        compile_and_run(
            cc,
            include_dir=include_dir,
            library=library,
            source=source,
            binary=work_dir / case_name,
            dynamic=dynamic,
            cxx_runtime=cxx_runtime,
        )


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    data = manifest()
    with tempfile.TemporaryDirectory(prefix="rimalloc-c-consumers-") as tmp:
        staged = pathlib.Path(tmp) / RELEASE_DIR.name
        shutil.copytree(RELEASE_DIR, staged, ignore=shutil.ignore_patterns("build"))
        verify_headers(data, staged)
        run_consumers(cc, data, staged)
    print("ok: C consumers built and ran for staged dynamic and static artifacts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
