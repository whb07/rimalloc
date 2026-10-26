#!/usr/bin/env python3
"""Validate focused LD_PRELOAD behavior for the release interpose cdylib."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
RELEASE_DIR = ROOT / "target/release-artifacts"


def build_release_artifacts() -> dict[str, object]:
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


def write_probe(work_dir: pathlib.Path) -> pathlib.Path:
    work_dir.mkdir(parents=True, exist_ok=True)
    source = work_dir / "ld_preload_probe.c"
    source.write_text(
        """\
#define _GNU_SOURCE

#include <assert.h>
#include <dlfcn.h>
#include <malloc.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef bool (*mi_check_owned_fn)(const void*);

static mi_check_owned_fn resolve_check_owned(void) {
  dlerror();
  mi_check_owned_fn check_owned =
      (mi_check_owned_fn)dlsym(RTLD_DEFAULT, "mi_check_owned");
  assert(dlerror() == NULL);
  assert(check_owned != NULL);
  return check_owned;
}

static void assert_owned(mi_check_owned_fn check_owned, void* p, size_t size) {
  assert(p != NULL);
  assert(check_owned(p));
  assert(malloc_usable_size(p) >= size);
}

int main(void) {
  mi_check_owned_fn check_owned = resolve_check_owned();

  void* p = malloc(64);
  assert_owned(check_owned, p, 64);

  p = realloc(p, 128);
  assert_owned(check_owned, p, 128);
  free(p);

  void* c = calloc(4, 32);
  assert_owned(check_owned, c, 128);
  free(c);

  void* a = aligned_alloc(64, 128);
  assert_owned(check_owned, a, 128);
  assert(((uintptr_t)a % 64) == 0);
  free(a);

  void* pm = NULL;
  assert(posix_memalign(&pm, 64, 96) == 0);
  assert_owned(check_owned, pm, 96);
  assert(((uintptr_t)pm % 64) == 0);
  free(pm);

  void* m = memalign(64, 80);
  assert_owned(check_owned, m, 80);
  assert(((uintptr_t)m % 64) == 0);
  free(m);

  void* v = valloc(4096);
  assert_owned(check_owned, v, 4096);
  free(v);

  return 0;
}
"""
    )
    return source


def compile_probe(cc: str, source: pathlib.Path) -> pathlib.Path:
    binary = source.parent / "ld_preload_probe"
    subprocess.run(
        [
            cc,
            "-std=c11",
            str(source),
            "-Wl,--no-as-needed",
            "-lstdc++",
            "-Wl,--as-needed",
            "-ldl",
            "-o",
            str(binary),
        ],
        cwd=source.parent,
        check=True,
    )
    return binary


def preload_env(library: pathlib.Path, *, stats: bool) -> dict[str, str]:
    env = os.environ.copy()
    env["LD_PRELOAD"] = str(library)
    if stats:
        env["mimalloc_show_stats"] = "1"
    return env


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    data = build_release_artifacts()
    with tempfile.TemporaryDirectory(prefix="rimalloc-ld-preload-") as tmp:
        tmp_dir = pathlib.Path(tmp)
        staged = tmp_dir / RELEASE_DIR.name
        shutil.copytree(RELEASE_DIR, staged, ignore=shutil.ignore_patterns("build"))
        library = artifact_path(data, staged, "interpose-cdylib")
        binary = compile_probe(cc, write_probe(tmp_dir / "probe"))

        subprocess.run([str(binary)], cwd=binary.parent, env=preload_env(library, stats=False), check=True)
        stats_run = subprocess.run(
            [str(binary)],
            cwd=binary.parent,
            env=preload_env(library, stats=True),
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        output = (stats_run.stdout + "\n" + stats_run.stderr).lower()
        expected_stats_sections = ("subproc", "pages", "arenas", "process")
        if not all(section in output for section in expected_stats_sections):
            raise SystemExit("mimalloc_show_stats=1 did not emit shutdown stats")
    print("ok: focused LD_PRELOAD interposition checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
