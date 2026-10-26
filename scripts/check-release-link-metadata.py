#!/usr/bin/env python3
"""Validate pkg-config and CMake metadata from staged release artifacts."""

from __future__ import annotations

import json
import os
import pathlib
import shlex
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
RELEASE_DIR = ROOT / "target/release-artifacts"


def run(cmd: list[str], *, cwd: pathlib.Path, env: dict[str, str] | None = None) -> None:
    print("+ " + " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def command_output(
    cmd: list[str], *, cwd: pathlib.Path, env: dict[str, str] | None = None
) -> str:
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()


def ensure_release_artifacts() -> dict[str, object]:
    manifest_path = RELEASE_DIR / "release-artifacts.json"
    if not manifest_path.exists():
        run(["scripts/build-release-artifacts.py"], cwd=ROOT)
    data = json.loads(manifest_path.read_text())
    if "link_metadata" not in data:
        run(["scripts/build-release-artifacts.py"], cwd=ROOT)
        data = json.loads(manifest_path.read_text())
    return data


def require_tool(name: str) -> str:
    tool = shutil.which(name)
    if tool is None:
        raise SystemExit(f"required tool not found: {name}")
    return tool


def require_cc() -> str:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")
    return cc


def verify_metadata_manifest(data: dict[str, object], staged: pathlib.Path) -> None:
    expected = {
        "rimalloc.pc",
        "rimalloc-config.cmake",
    }
    metadata = data.get("link_metadata")
    if not isinstance(metadata, list):
        raise SystemExit("release manifest is missing link_metadata")
    actual = {entry["name"] for entry in metadata}
    missing = expected - actual
    if missing:
        raise SystemExit(f"missing link metadata entries: {sorted(missing)}")
    release_relative = RELEASE_DIR.relative_to(ROOT)
    for entry in metadata:
        path = staged / pathlib.Path(str(entry["path"])).relative_to(release_relative)
        if not path.exists():
            raise SystemExit(f"staged link metadata missing on disk: {path}")


def write_consumer(path: pathlib.Path) -> pathlib.Path:
    path.mkdir(parents=True, exist_ok=True)
    source = path / "consumer.c"
    source.write_text(
        """\
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <assert.h>
#include <stddef.h>

int main(void) {
  void* p = mi_malloc(128);
  assert(p != NULL);
  assert(mi_usable_size(p) >= 128);
  mi_stats_t stats;
  mi_stats_init(&stats);
  mi_free(p);
  return 0;
}
"""
    )
    return source


def with_library_path(env: dict[str, str], staged: pathlib.Path) -> dict[str, str]:
    library_path = str(staged / "artifacts")
    key = "LD_LIBRARY_PATH"
    existing = env.get(key)
    env[key] = library_path if not existing else library_path + os.pathsep + existing
    return env


def run_pkg_config_consumer(staged: pathlib.Path, work_dir: pathlib.Path) -> None:
    pkg_config = shutil.which("pkg-config") or shutil.which("pkgconf")
    if pkg_config is None:
        raise SystemExit("required tool not found: pkg-config or pkgconf")
    cc = require_cc()
    env = os.environ.copy()
    env["PKG_CONFIG_PATH"] = str(staged / "lib/pkgconfig")
    env = with_library_path(env, staged)
    source = write_consumer(work_dir)
    binary = work_dir / "pkg-config-consumer"
    cflags = shlex.split(
        command_output([pkg_config, "--cflags", "rimalloc"], cwd=work_dir, env=env)
    )
    libs = shlex.split(
        command_output([pkg_config, "--libs", "rimalloc"], cwd=work_dir, env=env)
    )
    run([cc, "-std=c11", *cflags, str(source), *libs, "-o", str(binary)], cwd=work_dir, env=env)
    run([str(binary)], cwd=work_dir, env=env)


def run_cmake_consumer(staged: pathlib.Path, work_dir: pathlib.Path) -> None:
    cmake = require_tool("cmake")
    source = write_consumer(work_dir)
    cmake_lists = work_dir / "CMakeLists.txt"
    cmake_lists.write_text(
        f"""\
cmake_minimum_required(VERSION 3.16)
project(rimalloc_release_consumer C)

find_package(rimalloc REQUIRED CONFIG)

add_executable(cmake_consumer {source.name})
target_link_libraries(cmake_consumer PRIVATE rimalloc::c_abi)
"""
    )
    build_dir = work_dir / "build"
    env = with_library_path(os.environ.copy(), staged)
    run(
        [
            cmake,
            "-S",
            str(work_dir),
            "-B",
            str(build_dir),
            "-DCMAKE_PREFIX_PATH=" + str(staged),
        ],
        cwd=work_dir,
        env=env,
    )
    run([cmake, "--build", str(build_dir), "--target", "cmake_consumer", "-j2"], cwd=work_dir, env=env)
    run([str(build_dir / "cmake_consumer")], cwd=work_dir, env=env)


def main() -> int:
    data = ensure_release_artifacts()
    with tempfile.TemporaryDirectory(prefix="rimalloc-link-metadata-") as tmp:
        tmp_dir = pathlib.Path(tmp)
        staged = tmp_dir / RELEASE_DIR.name
        shutil.copytree(RELEASE_DIR, staged, ignore=shutil.ignore_patterns("build"))
        verify_metadata_manifest(data, staged)
        run_pkg_config_consumer(staged, tmp_dir / "pkg-config-consumer")
        run_cmake_consumer(staged, tmp_dir / "cmake-consumer")
    print("ok: pkg-config and CMake consumers built outside the repository")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
