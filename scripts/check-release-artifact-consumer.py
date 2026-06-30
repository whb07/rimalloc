#!/usr/bin/env python3
"""Build a C consumer using only staged release artifacts."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
RELEASE_DIR = ROOT / "target/release-artifacts"
BUILD_DIR = ROOT / "target/release-artifact-consumer"


def manifest() -> dict[str, object]:
    path = RELEASE_DIR / "release-artifacts.json"
    if not path.exists():
        subprocess.run(["scripts/build-release-artifacts.py"], cwd=ROOT, check=True)
    return json.loads(path.read_text())


def artifact_path(data: dict[str, object], name: str) -> pathlib.Path:
    for artifact in data["artifacts"]:
        if artifact["name"] == name:
            return ROOT / str(artifact["path"])
    raise SystemExit(f"missing release artifact in manifest: {name}")


def verify_headers(data: dict[str, object]) -> None:
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
        path = ROOT / str(header["path"])
        if not path.exists():
            raise SystemExit(f"staged header missing on disk: {path}")


def write_consumer_source() -> pathlib.Path:
    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    source = BUILD_DIR / "consumer.c"
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


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    data = manifest()
    verify_headers(data)
    library = artifact_path(data, "c-abi-cdylib")
    source = write_consumer_source()
    binary = BUILD_DIR / "consumer"
    subprocess.run(
        [
            cc,
            "-std=c11",
            "-I",
            str(RELEASE_DIR / "include"),
            str(source),
            str(library),
            "-Wl,-rpath," + str(library.parent),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ],
        cwd=ROOT,
        check=True,
    )
    subprocess.run([str(binary)], cwd=ROOT, check=True)
    print("ok: C consumer built and ran using only staged release headers and library")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
