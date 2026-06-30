#!/usr/bin/env python3
"""Build release library artifacts and write a machine-readable manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_OUT_DIR = ROOT / "target/release-artifacts"
CRATE_NAME = "mechanical_mimalloc"
HEADER_DIR = ROOT / "mimalloc-3.3.2/include"


HEADER_SPECS = [
    ("mimalloc.h", "c-api"),
    ("mimalloc-stats.h", "stats"),
    ("mimalloc-override.h", "interpose"),
    ("mimalloc-new-delete.h", "cxx-overrides"),
]

LINK_MODE_NOTES = (
    "c-abi cdylib is the production-supported C ABI artifact; "
    "interpose cdylib opts into malloc-family interposition; "
    "static archives are preview-only in this release policy"
)


def run(cmd: list[str], *, env: dict[str, str] | None = None) -> None:
    print("+ " + " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=ROOT, env=env, check=True)


def command_output(cmd: list[str]) -> str:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()


def rustc_info() -> tuple[str, str]:
    text = command_output(["rustc", "-vV"])
    host = ""
    for line in text.splitlines():
        if line.startswith("host: "):
            host = line.split(": ", 1)[1]
            break
    if not host:
        raise SystemExit("could not determine rustc host triple")
    return text, os.environ.get("CARGO_BUILD_TARGET", host)


def package_version() -> str:
    metadata = json.loads(
        command_output(["cargo", "metadata", "--no-deps", "--format-version", "1"])
    )
    return metadata["packages"][0]["version"]


def release_dirs(target_dir: pathlib.Path, target_triple: str) -> list[pathlib.Path]:
    if os.environ.get("CARGO_BUILD_TARGET"):
        release = target_dir / target_triple / "release"
    else:
        release = target_dir / "release"
    return [release, release / "deps"]


def newest(paths: list[pathlib.Path], description: str) -> pathlib.Path:
    existing = [path for path in paths if path.exists()]
    if not existing:
        raise SystemExit(f"no artifact found for {description}")
    return max(existing, key=lambda path: path.stat().st_mtime)


def find_artifact(target_dir: pathlib.Path, target_triple: str, crate_type: str) -> pathlib.Path:
    dirs = release_dirs(target_dir, target_triple)
    candidates: list[pathlib.Path] = []
    if crate_type == "rlib":
        for directory in dirs:
            candidates.extend(directory.glob(f"lib{CRATE_NAME}*.rlib"))
    elif crate_type == "staticlib":
        for directory in dirs:
            candidates.extend(directory.glob(f"lib{CRATE_NAME}*.a"))
    elif crate_type == "cdylib":
        patterns = [
            f"lib{CRATE_NAME}.so",
            f"lib{CRATE_NAME}*.so",
            f"lib{CRATE_NAME}.dylib",
            f"lib{CRATE_NAME}*.dylib",
            f"{CRATE_NAME}.dll",
            f"{CRATE_NAME}*.dll",
        ]
        for directory in dirs:
            for pattern in patterns:
                candidates.extend(directory.glob(pattern))
    else:
        raise SystemExit(f"unsupported crate type: {crate_type}")
    return newest(candidates, crate_type)


def sha256(path: pathlib.Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def copy_artifact(
    source: pathlib.Path, artifacts_dir: pathlib.Path, output_name: str
) -> pathlib.Path:
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    destination = artifacts_dir / f"{output_name}{source.suffix}"
    shutil.copy2(source, destination)
    return destination


def copy_headers(include_dir: pathlib.Path) -> list[dict[str, object]]:
    include_dir.mkdir(parents=True, exist_ok=True)
    headers: list[dict[str, object]] = []
    for filename, role in HEADER_SPECS:
        source = HEADER_DIR / filename
        if not source.exists():
            raise SystemExit(f"missing release header: {source}")
        destination = include_dir / filename
        shutil.copy2(source, destination)
        headers.append(
            {
                "name": filename,
                "role": role,
                "path": display_path(destination),
                "source_path": display_path(source),
                "bytes": destination.stat().st_size,
                "sha256": sha256(destination),
            }
        )
    return headers


def artifact_filename(artifacts: list[dict[str, object]], name: str) -> str:
    for artifact in artifacts:
        if artifact["name"] == name:
            return pathlib.Path(str(artifact["path"])).name
    raise SystemExit(f"missing artifact for link metadata: {name}")


def metadata_entry(name: str, role: str, path: pathlib.Path) -> dict[str, object]:
    return {
        "name": name,
        "role": role,
        "path": display_path(path),
        "bytes": path.stat().st_size,
        "sha256": sha256(path),
    }


def write_pkg_config(
    out_dir: pathlib.Path, version: str, artifacts: list[dict[str, object]]
) -> pathlib.Path:
    path = out_dir / "lib/pkgconfig/mechanical_mimalloc.pc"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "prefix=${pcfiledir}/../..",
                "includedir=${prefix}/include",
                "artifactdir=${prefix}/artifacts",
                "",
                f"c_abi_cdylib=${{artifactdir}}/{artifact_filename(artifacts, 'c-abi-cdylib')}",
                f"interpose_cdylib=${{artifactdir}}/{artifact_filename(artifacts, 'interpose-cdylib')}",
                f"c_abi_static_preview=${{artifactdir}}/{artifact_filename(artifacts, 'c-abi-static-preview')}",
                f"interpose_static_preview=${{artifactdir}}/{artifact_filename(artifacts, 'interpose-static-preview')}",
                f"artifact_mode_notes={LINK_MODE_NOTES}",
                "",
                "Name: mechanical_mimalloc",
                "Description: mechanical mimalloc C ABI release artifacts",
                f"Version: {version}",
                "Cflags: -I${includedir}",
                "Libs: ${c_abi_cdylib} -Wl,-rpath,${artifactdir} -pthread -ldl -lm",
                "Libs.private: -pthread -ldl -lm",
                "",
            ]
        )
    )
    return path


def write_cmake_config(out_dir: pathlib.Path, artifacts: list[dict[str, object]]) -> pathlib.Path:
    path = out_dir / "lib/cmake/mechanical_mimalloc/mechanical_mimalloc-config.cmake"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "# Generated by scripts/build-release-artifacts.py.",
                "include(CMakeFindDependencyMacro)",
                "find_dependency(Threads)",
                "",
                'get_filename_component(_MECHANICAL_MIMALLOC_PREFIX "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)',
                'set(mechanical_mimalloc_INCLUDE_DIR "${_MECHANICAL_MIMALLOC_PREFIX}/include")',
                'set(mechanical_mimalloc_ARTIFACT_DIR "${_MECHANICAL_MIMALLOC_PREFIX}/artifacts")',
                f'set(mechanical_mimalloc_ARTIFACT_MODE_NOTES "{LINK_MODE_NOTES}")',
                "",
                "function(_mechanical_mimalloc_add_imported target kind filename)",
                "  if(NOT TARGET ${target})",
                "    add_library(${target} ${kind} IMPORTED)",
                "    set_target_properties(${target} PROPERTIES",
                '      IMPORTED_LOCATION "${mechanical_mimalloc_ARTIFACT_DIR}/${filename}"',
                '      INTERFACE_INCLUDE_DIRECTORIES "${mechanical_mimalloc_INCLUDE_DIR}"',
                '      INTERFACE_LINK_LIBRARIES "Threads::Threads;${CMAKE_DL_LIBS};m"',
                "    )",
                "  endif()",
                "endfunction()",
                "",
                "_mechanical_mimalloc_add_imported(",
                "  mechanical_mimalloc::c_abi",
                "  SHARED",
                f"  {artifact_filename(artifacts, 'c-abi-cdylib')}",
                ")",
                "_mechanical_mimalloc_add_imported(",
                "  mechanical_mimalloc::interpose",
                "  SHARED",
                f"  {artifact_filename(artifacts, 'interpose-cdylib')}",
                ")",
                "_mechanical_mimalloc_add_imported(",
                "  mechanical_mimalloc::c_abi_static_preview",
                "  STATIC",
                f"  {artifact_filename(artifacts, 'c-abi-static-preview')}",
                ")",
                "_mechanical_mimalloc_add_imported(",
                "  mechanical_mimalloc::interpose_static_preview",
                "  STATIC",
                f"  {artifact_filename(artifacts, 'interpose-static-preview')}",
                ")",
                "",
                "set(mechanical_mimalloc_LIBRARIES mechanical_mimalloc::c_abi)",
                "set(mechanical_mimalloc_FOUND TRUE)",
                "",
            ]
        )
    )
    return path


def write_link_metadata(
    out_dir: pathlib.Path, version: str, artifacts: list[dict[str, object]]
) -> list[dict[str, object]]:
    pkg_config = write_pkg_config(out_dir, version, artifacts)
    cmake_config = write_cmake_config(out_dir, artifacts)
    return [
        metadata_entry("mechanical_mimalloc.pc", "pkg-config", pkg_config),
        metadata_entry(
            "mechanical_mimalloc-config.cmake", "cmake-package-config", cmake_config
        ),
    ]


def display_path(path: pathlib.Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def artifact_specs() -> list[dict[str, object]]:
    return [
        {
            "name": "rust-rlib",
            "features": [],
            "crate_type": None,
            "command": ["cargo", "build", "--release", "--lib"],
            "output_name": "mechanical_mimalloc-rust-rlib",
            "manifest_kind": "rlib",
        },
        {
            "name": "c-abi-cdylib",
            "features": ["c-abi"],
            "crate_type": "cdylib",
            "command": [
                "cargo",
                "rustc",
                "--release",
                "--features",
                "c-abi",
                "--lib",
                "--",
                "--crate-type",
                "cdylib",
            ],
            "output_name": "mechanical_mimalloc-c-abi-cdylib",
            "manifest_kind": "cdylib",
        },
        {
            "name": "c-abi-static-preview",
            "features": ["c-abi"],
            "crate_type": "staticlib",
            "command": [
                "cargo",
                "rustc",
                "--release",
                "--features",
                "c-abi",
                "--lib",
                "--",
                "--crate-type",
                "staticlib",
            ],
            "output_name": "mechanical_mimalloc-c-abi-static-preview",
            "manifest_kind": "staticlib",
        },
        {
            "name": "interpose-cdylib",
            "features": ["interpose"],
            "crate_type": "cdylib",
            "command": [
                "cargo",
                "rustc",
                "--release",
                "--features",
                "interpose",
                "--lib",
                "--",
                "--crate-type",
                "cdylib",
            ],
            "output_name": "mechanical_mimalloc-interpose-cdylib",
            "manifest_kind": "cdylib",
        },
        {
            "name": "interpose-static-preview",
            "features": ["interpose"],
            "crate_type": "staticlib",
            "command": [
                "cargo",
                "rustc",
                "--release",
                "--features",
                "interpose",
                "--lib",
                "--",
                "--crate-type",
                "staticlib",
            ],
            "output_name": "mechanical_mimalloc-interpose-static-preview",
            "manifest_kind": "staticlib",
        },
    ]


def build_artifacts(out_dir: pathlib.Path) -> dict[str, object]:
    rustc_version, target_triple = rustc_info()
    version = package_version()
    artifacts_dir = out_dir / "artifacts"
    build_root = out_dir / "build"
    include_dir = out_dir / "include"
    artifacts: list[dict[str, object]] = []
    headers: list[dict[str, object]] = []
    link_metadata: list[dict[str, object]] = []
    manifest: dict[str, object] = {
        "schema_version": 1,
        "package": CRATE_NAME,
        "version": version,
        "profile": "release",
        "target_triple": target_triple,
        "rustc": rustc_version,
        "artifacts": artifacts,
        "headers": headers,
        "link_metadata": link_metadata,
    }

    if out_dir.exists():
        shutil.rmtree(out_dir)
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    build_root.mkdir(parents=True, exist_ok=True)
    headers.extend(copy_headers(include_dir))

    for spec in artifact_specs():
        name = str(spec["name"])
        target_dir = build_root / name
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(target_dir)
        command = list(spec["command"])
        run(command, env=env)

        crate_type = str(spec["crate_type"] or "rlib")
        source = find_artifact(target_dir, target_triple, crate_type)
        copied = copy_artifact(source, artifacts_dir, str(spec["output_name"]))
        artifact = {
            "name": name,
            "kind": spec["manifest_kind"],
            "features": spec["features"],
            "crate_type": crate_type,
            "build_command": command,
            "cargo_target_dir": display_path(target_dir),
            "path": display_path(copied),
            "source_path": display_path(source),
            "sha256": sha256(copied),
        }
        artifacts.append(artifact)

    link_metadata.extend(write_link_metadata(out_dir, version, artifacts))

    manifest_path = out_dir / "release-artifacts.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"wrote {display_path(manifest_path)}")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--out-dir",
        type=pathlib.Path,
        default=DEFAULT_OUT_DIR,
        help="directory for release artifact outputs",
    )
    args = parser.parse_args()
    build_artifacts(args.out_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
