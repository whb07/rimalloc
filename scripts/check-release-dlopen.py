#!/usr/bin/env python3
"""Validate focused dlopen/dlsym behavior for the release C ABI cdylib."""

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
    source = work_dir / "dlopen_probe.c"
    source.write_text(
        """\
#include <mimalloc.h>

#include <assert.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

typedef void* (*mi_malloc_fn)(size_t);
typedef void (*mi_free_fn)(void*);
typedef size_t (*mi_good_size_fn)(size_t);
typedef void (*mi_stats_reset_fn)(void);
typedef long (*mi_option_get_fn)(mi_option_t);
typedef bool (*mi_check_owned_fn)(const void*);

static void* required_symbol(void* handle, const char* name) {
  dlerror();
  void* symbol = dlsym(handle, name);
  const char* error = dlerror();
  if (error != NULL || symbol == NULL) {
    fprintf(stderr, "missing required symbol %s: %s\\n", name, error);
    exit(10);
  }
  return symbol;
}

static void assert_missing_symbol(void* handle, const char* name) {
  dlerror();
  void* symbol = dlsym(handle, name);
  const char* error = dlerror();
  if (error == NULL || symbol != NULL) {
    fprintf(stderr, "unexpected clean C ABI symbol: %s\\n", name);
    exit(11);
  }
}

int main(int argc, char** argv) {
  assert(argc == 2);
  void* handle = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
  if (handle == NULL) {
    fprintf(stderr, "dlopen failed: %s\\n", dlerror());
    return 12;
  }

  mi_malloc_fn resolved_malloc = (mi_malloc_fn)required_symbol(handle, "mi_malloc");
  mi_free_fn resolved_free = (mi_free_fn)required_symbol(handle, "mi_free");
  mi_good_size_fn resolved_good_size =
      (mi_good_size_fn)required_symbol(handle, "mi_good_size");
  mi_stats_reset_fn resolved_stats_reset =
      (mi_stats_reset_fn)required_symbol(handle, "mi_stats_reset");
  mi_option_get_fn resolved_option_get =
      (mi_option_get_fn)required_symbol(handle, "mi_option_get");
  mi_check_owned_fn resolved_check_owned =
      (mi_check_owned_fn)required_symbol(handle, "mi_check_owned");

  assert(resolved_good_size(33) >= 33);
  resolved_stats_reset();
  (void)resolved_option_get(mi_option_show_errors);

  void* p = resolved_malloc(96);
  assert(p != NULL);
  assert(resolved_check_owned(p));
  resolved_free(p);

  void* libc_p = malloc(96);
  assert(libc_p != NULL);
  assert(!resolved_check_owned(libc_p));
  free(libc_p);

  assert_missing_symbol(handle, "mi_heap_check_owned");
  dlclose(handle);
  return 0;
}
"""
    )
    return source


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    data = build_release_artifacts()
    with tempfile.TemporaryDirectory(prefix="mechanical-mimalloc-dlopen-") as tmp:
        tmp_dir = pathlib.Path(tmp)
        staged = tmp_dir / RELEASE_DIR.name
        shutil.copytree(RELEASE_DIR, staged, ignore=shutil.ignore_patterns("build"))
        library = artifact_path(data, staged, "c-abi-cdylib")
        source = write_probe(tmp_dir / "probe")
        binary = source.parent / "dlopen_probe"
        subprocess.run(
            [
                cc,
                "-std=c11",
                "-I",
                str(staged / "include"),
                str(source),
                "-ldl",
                "-o",
                str(binary),
            ],
            cwd=source.parent,
            check=True,
        )
        subprocess.run([str(binary), str(library)], cwd=source.parent, check=True)
    print("ok: focused dlopen/dlsym C ABI checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
