#!/usr/bin/env python3
"""Validate a C consumer against copied release FFI assets."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
RELEASE_DIR = ROOT / "target/release-artifacts"


def run(command: list[str], *, cwd: pathlib.Path, env: dict[str, str] | None = None) -> None:
    print("+ " + " ".join(command))
    subprocess.run(command, cwd=cwd, env=env, check=True)


def build_release_assets() -> dict[str, object]:
    manifest = RELEASE_DIR / "release-artifacts.json"
    if os.environ.get("MECHANICAL_MIMALLOC_REUSE_RELEASE_ARTIFACTS") != "1" or not manifest.exists():
        run(["scripts/build-release-artifacts.py"], cwd=ROOT)
    with manifest.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def staged_path(staged: pathlib.Path, manifest_path: str) -> pathlib.Path:
    parts = pathlib.PurePosixPath(manifest_path).parts
    if parts[:2] != ("target", "release-artifacts"):
        raise SystemExit(f"unexpected release manifest path: {manifest_path}")
    return staged.joinpath(*parts[2:])


def artifact_path(data: dict[str, object], staged: pathlib.Path, name: str) -> pathlib.Path:
    for artifact in data["artifacts"]:
        if artifact["name"] == name:
            return staged_path(staged, str(artifact["path"]))
    raise SystemExit(f"missing release artifact in manifest: {name}")


def write_consumer(work_dir: pathlib.Path) -> pathlib.Path:
    source = work_dir / "consumer.c"
    source.write_text(
        r"""
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <assert.h>
#include <stddef.h>
#include <string.h>

int main(void) {
  void* p = mi_malloc(64);
  assert(p != NULL);
  memset(p, 0xA5, 64);
  assert(mi_usable_size(p) >= 64);
  mi_free(p);

  mi_stats_reset();
  mi_stats_print_out(NULL, NULL);
  return 0;
}
""".lstrip(),
        encoding="utf-8",
    )
    return source


def run_consumer(data: dict[str, object], staged: pathlib.Path, work_dir: pathlib.Path) -> None:
    cc = os.environ.get("CC", "cc")
    include_dir = staged / "include"
    library = artifact_path(data, staged, "c-abi-cdylib")
    source = write_consumer(work_dir)
    binary = work_dir / "package-ffi-consumer"
    run(
        [
            cc,
            str(source),
            "-I",
            str(include_dir),
            str(library),
            "-Wl,-rpath," + str(library.parent),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ],
        cwd=work_dir,
    )
    run([str(binary)], cwd=work_dir)


def main() -> None:
    data = build_release_assets()
    with tempfile.TemporaryDirectory(prefix="mechanical-mimalloc-package-ffi-") as tmp:
        work_dir = pathlib.Path(tmp)
        staged = work_dir / "downloaded-release-assets"
        shutil.copytree(RELEASE_DIR, staged)
        if (staged / "scripts").exists():
            raise SystemExit("release assets must not require repository scripts")
        run_consumer(data, staged, work_dir)
    print("ok: package FFI consumer used copied release headers and library assets")


if __name__ == "__main__":
    main()
