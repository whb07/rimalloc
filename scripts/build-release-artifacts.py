#!/usr/bin/env python3
"""Build release library artifacts and write a machine-readable manifest."""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_OUT_DIR = ROOT / "target/release-artifacts"
CRATE_NAME = "rimalloc"
HEADER_DIR = ROOT / "mimalloc-3.3.2/include"
SIGNING_KEY_ENV = "RIMALLOC_RELEASE_SIGNING_KEY"
SIGNING_IDENTITY_ENV = "RIMALLOC_RELEASE_SIGNING_IDENTITY"
DEFAULT_SIGNING_IDENTITY = "rimalloc_release"
SIGNING_NAMESPACE = "file"


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


def command_output_with_stderr(cmd: list[str]) -> str:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    ).stdout


def command_output_optional(cmd: list[str]) -> str:
    completed = subprocess.run(
        cmd,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    return completed.stdout.strip()


def command_output_bytes(cmd: list[str]) -> bytes:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout


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


def cargo_metadata_all_features() -> dict[str, object]:
    return json.loads(
        command_output(
            ["cargo", "metadata", "--locked", "--format-version", "1", "--all-features"]
        )
    )


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


def release_path(path: str | pathlib.Path) -> pathlib.Path:
    candidate = pathlib.Path(path)
    if candidate.is_absolute():
        return candidate
    return ROOT / candidate


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


def signing_identity() -> str:
    return os.environ.get(SIGNING_IDENTITY_ENV, DEFAULT_SIGNING_IDENTITY)


def signing_key() -> tuple[pathlib.Path, str]:
    configured = os.environ.get(SIGNING_KEY_ENV)
    if configured:
        key = release_path(configured)
        if not key.exists():
            raise SystemExit(f"{SIGNING_KEY_ENV} points to missing key: {key}")
        return key, "configured-release-key"

    key_dir = pathlib.Path(
        tempfile.mkdtemp(prefix="rimalloc-release-signing-")
    )
    key = key_dir / "release-signing-dry-run-key"
    key.parent.mkdir(parents=True, exist_ok=True)
    key.parent.chmod(0o700)
    run(
        [
            "ssh-keygen",
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            "rimalloc release dry-run key",
            "-f",
            str(key),
        ]
    )
    key.chmod(0o600)
    return key, "ephemeral-dry-run-key"


def write_signing_public_key(
    out_dir: pathlib.Path, key: pathlib.Path, identity: str
) -> tuple[pathlib.Path, pathlib.Path, str]:
    signing_dir = out_dir / "integrity/signing"
    signing_dir.mkdir(parents=True, exist_ok=True)
    public_key = signing_dir / "release-signing.pub"
    allowed_signers = signing_dir / "release-allowed-signers"

    public_key.write_bytes(command_output_bytes(["ssh-keygen", "-y", "-f", str(key)]))
    allowed_signers.write_text(f"{identity} {public_key.read_text().strip()}\n")
    fingerprint = command_output(["ssh-keygen", "-l", "-E", "sha256", "-f", str(public_key)])
    return public_key, allowed_signers, fingerprint


def sign_checksum_file(out_dir: pathlib.Path, key: pathlib.Path) -> pathlib.Path:
    checksum_path = out_dir / "SHA256SUMS"
    signature_path = out_dir / "SHA256SUMS.sig"
    if signature_path.exists():
        signature_path.unlink()
    run(
        [
            "ssh-keygen",
            "-Y",
            "sign",
            "-f",
            str(key),
            "-n",
            SIGNING_NAMESPACE,
            str(checksum_path),
        ]
    )
    if not signature_path.exists():
        raise SystemExit(f"missing checksum signature: {signature_path}")
    return signature_path


def verify_checksum_signature(
    checksum_path: pathlib.Path,
    signature_path: pathlib.Path,
    allowed_signers: pathlib.Path,
    identity: str,
) -> None:
    with checksum_path.open("rb") as checksum:
        subprocess.run(
            [
                "ssh-keygen",
                "-Y",
                "verify",
                "-f",
                str(allowed_signers),
                "-I",
                identity,
                "-n",
                SIGNING_NAMESPACE,
                "-s",
                str(signature_path),
            ],
            cwd=ROOT,
            stdin=checksum,
            check=True,
        )


def parse_symbol_version(name: str) -> tuple[str, str]:
    symbol, separator, version = name.partition("@")
    if not separator:
        return name, "-"
    return symbol, version.lstrip("@") or "-"


def dynamic_symbol_entries(path: pathlib.Path) -> list[tuple[str, str, str, str, str]]:
    readelf = command_output(["readelf", "-W", "--dyn-syms", str(path)])
    entries: dict[str, tuple[str, str, str, str, str]] = {}
    for line in readelf.splitlines():
        parts = line.split()
        if len(parts) < 8:
            continue
        index, _value, _size, kind, bind, visibility, section, name = parts[:8]
        if not index.endswith(":"):
            continue
        if section == "UND" or bind not in {"GLOBAL", "WEAK"}:
            continue
        if visibility != "DEFAULT" or kind not in {"FUNC", "OBJECT", "NOTYPE"}:
            continue
        symbol, version = parse_symbol_version(name)
        entries[symbol] = (symbol, bind, visibility, version, kind)
    return [entries[symbol] for symbol in sorted(entries)]


def static_symbol_entries(path: pathlib.Path) -> list[tuple[str, str]]:
    nm = subprocess.run(
        ["nm", "-g", "--defined-only", str(path)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout
    entries: dict[str, tuple[str, str]] = {}
    for line in nm.splitlines():
        parts = line.split()
        if len(parts) < 3 or parts[0].endswith(":"):
            continue
        kind = parts[-2]
        symbol = parts[-1]
        if len(kind) == 1:
            entries[symbol] = (symbol, kind)
    return [entries[symbol] for symbol in sorted(entries)]


def render_symbol_manifest(artifact: dict[str, object]) -> str:
    artifact_path = release_path(str(artifact["path"]))
    lines = [
        "# rimalloc release ABI symbol manifest",
        f"# artifact: {artifact['name']}",
        f"# kind: {artifact['kind']}",
        f"# path: {artifact['path']}",
        f"# sha256: {artifact['sha256']}",
        "# build-command: " + " ".join(str(part) for part in artifact["build_command"]),
    ]
    if artifact["kind"] == "cdylib":
        lines.append("# columns: symbol\tbinding\tvisibility\tversion\ttype")
        lines.append("symbol\tbinding\tvisibility\tversion\ttype")
        lines.extend("\t".join(entry) for entry in dynamic_symbol_entries(artifact_path))
    elif artifact["kind"] == "staticlib":
        lines.append("# columns: symbol\ttype")
        lines.append("symbol\ttype")
        lines.extend("\t".join(entry) for entry in static_symbol_entries(artifact_path))
    else:
        raise SystemExit(f"unsupported symbol manifest kind: {artifact['kind']}")
    return "\n".join(lines) + "\n"


def write_symbol_manifests(
    out_dir: pathlib.Path, artifacts: list[dict[str, object]]
) -> list[dict[str, object]]:
    symbol_dir = out_dir / "integrity/abi-symbols"
    symbol_dir.mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, object]] = []
    for artifact in artifacts:
        if artifact["kind"] not in {"cdylib", "staticlib"}:
            continue
        path = symbol_dir / f"{artifact['name']}.tsv"
        path.write_text(render_symbol_manifest(artifact))
        entry = metadata_entry(path.name, "abi-symbol-manifest", path)
        entry["artifact"] = artifact["name"]
        entries.append(entry)
    return entries


def write_environment_note(out_dir: pathlib.Path) -> dict[str, object]:
    path = out_dir / "integrity/environment.txt"
    path.parent.mkdir(parents=True, exist_ok=True)
    output = command_output_with_stderr(["scripts/env-info.sh"])
    path.write_text(
        "# rimalloc release environment\n\n"
        "Captured with `scripts/env-info.sh` during release artifact staging.\n\n"
        "```text\n"
        f"{output.rstrip()}\n"
        "```\n"
    )
    return metadata_entry(path.name, "environment-reproducibility-note", path)


def write_benchmark_note(out_dir: pathlib.Path) -> dict[str, object]:
    path = out_dir / "integrity/benchmark-evidence.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    commit = command_output(["git", "rev-parse", "HEAD"])
    path.write_text(
        "\n".join(
            [
                "# Release Benchmark Evidence",
                "",
                f"- Commit: `{commit}`",
                "- Policy: `docs/performance.md`",
                "- Project benchmark report: `BENCHMARK_RESULTS.md`",
                "- Full release command: `scripts/check-bench-full.sh`",
                "- Smoke iteration command: `scripts/check-bench-smoke.sh`",
                "- Environment capture command: `scripts/env-info.sh`",
                "",
                "This release artifact dry run does not attach fresh benchmark",
                "measurements. `docs/performance.md` permits skipping benchmark",
                "reruns for documentation, packaging, lint-only, and export-only",
                "changes when allocator logic and hot-path control flow do not",
                "move. A release candidate that changes allocator behavior must",
                "attach the full benchmark summary required by that policy.",
                "",
            ]
        )
    )
    return metadata_entry(path.name, "benchmark-evidence-reference", path)


def package_label(package: dict[str, object]) -> str:
    source = package.get("source")
    source_text = str(source) if source else "path"
    return f"{package['name']} {package['version']} ({source_text})"


def root_dependency_names(metadata: dict[str, object], kind: str | None) -> list[str]:
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict):
        return []
    root = resolve.get("root")
    nodes = {
        node["id"]: node
        for node in resolve.get("nodes", [])
        if isinstance(node, dict) and "id" in node
    }
    root_node = nodes.get(root)
    if not isinstance(root_node, dict):
        return []
    names: list[str] = []
    for dep in root_node.get("deps", []):
        if not isinstance(dep, dict):
            continue
        if any(dep_kind.get("kind") == kind for dep_kind in dep.get("dep_kinds", [])):
            names.append(str(dep["name"]))
    return sorted(names)


def resolved_dependency_packages(metadata: dict[str, object]) -> list[dict[str, object]]:
    root = (
        metadata.get("resolve", {}).get("root")
        if isinstance(metadata.get("resolve"), dict)
        else None
    )
    packages = [
        package
        for package in metadata.get("packages", [])
        if isinstance(package, dict) and package.get("id") != root
    ]
    return sorted(
        packages, key=lambda package: (str(package["name"]), str(package["version"]))
    )


def tool_probe(
    name: str, env_name: str | None, default: str, version_args: list[str]
) -> dict[str, str]:
    configured = os.environ.get(env_name) if env_name else None
    executable = configured or shutil.which(default)
    if not executable:
        return {
            "name": name,
            "env": env_name or "-",
            "executable": default,
            "status": "missing",
            "version": "-",
        }
    version = (
        command_output_optional([executable, *version_args]).splitlines()
        if version_args
        else []
    )
    return {
        "name": name,
        "env": env_name or "-",
        "executable": executable,
        "status": "found",
        "version": version[0] if version else "-",
    }


def write_dependency_report(out_dir: pathlib.Path) -> dict[str, object]:
    path = out_dir / "integrity/dependencies.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    metadata = cargo_metadata_all_features()
    root_package = next(
        package
        for package in metadata["packages"]
        if package["id"] == metadata["resolve"]["root"]
    )
    normal_deps = root_dependency_names(metadata, None)
    build_deps = root_dependency_names(metadata, "build")
    dev_deps = root_dependency_names(metadata, "dev")
    resolved = resolved_dependency_packages(metadata)
    tools = [
        tool_probe("Rust compiler", None, "rustc", ["-V"]),
        tool_probe("Cargo", None, "cargo", ["-V"]),
        tool_probe("C compiler", "CC", "cc", ["--version"]),
        tool_probe("C++ compiler", "CXX", "c++", ["--version"]),
        tool_probe("Archiver", "AR", "ar", ["--version"]),
        tool_probe("ELF dynamic symbol reader", None, "readelf", ["--version"]),
        tool_probe("Static symbol reader", None, "nm", ["--version"]),
        tool_probe("SSH signing tool", None, "ssh-keygen", []),
    ]

    lines = [
        "# Release Dependency Report",
        "",
        f"- Package: `{root_package['name']} {root_package['version']}`",
        "- Cargo metadata command: `cargo metadata --locked --format-version 1 --all-features`",
        "- Feature scope: all crate features, all release artifact modes, tests, examples, and benches",
        "- Runtime dependency policy: normal dependencies are release runtime dependencies; dev dependencies are test, benchmark, and tooling inputs only.",
        "",
        "## Direct Runtime Dependencies",
        "",
    ]
    if normal_deps:
        lines.extend(f"- `{name}`" for name in normal_deps)
    else:
        lines.append("- None. `[dependencies]` is intentionally empty.")
    lines.extend(["", "## Direct Build Dependencies", ""])
    if build_deps:
        lines.extend(f"- `{name}`" for name in build_deps)
    else:
        lines.append("- None declared in Cargo metadata.")
    lines.extend(["", "## Direct Development Dependencies", ""])
    if dev_deps:
        lines.extend(f"- `{name}`" for name in dev_deps)
    else:
        lines.append("- None declared in Cargo metadata.")

    lines.extend(
        [
            "",
            "## Build Tool Requirements",
            "",
            "| Tool | Env override | Executable | Status | Version |",
            "|---|---|---|---|---|",
        ]
    )
    for tool in tools:
        lines.append(
            "| {name} | `{env}` | `{executable}` | {status} | {version} |".format(
                **tool
            )
        )

    lines.extend(
        [
            "",
            "## Resolved Third-Party Packages",
            "",
            "| Package | License | Source | Repository |",
            "|---|---|---|---|",
        ]
    )
    for package in resolved:
        license_text = str(package.get("license") or "-")
        source = str(package.get("source") or "path")
        repository = str(package.get("repository") or "-")
        lines.append(
            f"| `{package_label(package)}` | {license_text} | {source} | {repository} |"
        )

    lines.append("")
    path.write_text("\n".join(lines))
    return metadata_entry(path.name, "dependency-report", path)


def write_pkg_config(
    out_dir: pathlib.Path, version: str, artifacts: list[dict[str, object]]
) -> pathlib.Path:
    path = out_dir / "lib/pkgconfig/rimalloc.pc"
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
                "Name: rimalloc",
                "Description: rimalloc C ABI release artifacts",
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
    path = out_dir / "lib/cmake/rimalloc/rimalloc-config.cmake"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "# Generated by scripts/build-release-artifacts.py.",
                "include(CMakeFindDependencyMacro)",
                "find_dependency(Threads)",
                "",
                'get_filename_component(_RIMALLOC_PREFIX "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)',
                'set(rimalloc_INCLUDE_DIR "${_RIMALLOC_PREFIX}/include")',
                'set(rimalloc_ARTIFACT_DIR "${_RIMALLOC_PREFIX}/artifacts")',
                f'set(rimalloc_ARTIFACT_MODE_NOTES "{LINK_MODE_NOTES}")',
                "",
                "function(_rimalloc_add_imported target kind filename)",
                "  if(NOT TARGET ${target})",
                "    add_library(${target} ${kind} IMPORTED)",
                "    set_target_properties(${target} PROPERTIES",
                '      IMPORTED_LOCATION "${rimalloc_ARTIFACT_DIR}/${filename}"',
                '      INTERFACE_INCLUDE_DIRECTORIES "${rimalloc_INCLUDE_DIR}"',
                '      INTERFACE_LINK_LIBRARIES "Threads::Threads;${CMAKE_DL_LIBS};m"',
                "    )",
                "  endif()",
                "endfunction()",
                "",
                "_rimalloc_add_imported(",
                "  rimalloc::c_abi",
                "  SHARED",
                f"  {artifact_filename(artifacts, 'c-abi-cdylib')}",
                ")",
                "_rimalloc_add_imported(",
                "  rimalloc::interpose",
                "  SHARED",
                f"  {artifact_filename(artifacts, 'interpose-cdylib')}",
                ")",
                "_rimalloc_add_imported(",
                "  rimalloc::c_abi_static_preview",
                "  STATIC",
                f"  {artifact_filename(artifacts, 'c-abi-static-preview')}",
                ")",
                "_rimalloc_add_imported(",
                "  rimalloc::interpose_static_preview",
                "  STATIC",
                f"  {artifact_filename(artifacts, 'interpose-static-preview')}",
                ")",
                "",
                "set(rimalloc_LIBRARIES rimalloc::c_abi)",
                "set(rimalloc_FOUND TRUE)",
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
        metadata_entry("rimalloc.pc", "pkg-config", pkg_config),
        metadata_entry(
            "rimalloc-config.cmake", "cmake-package-config", cmake_config
        ),
    ]


def release_files(out_dir: pathlib.Path) -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for path in out_dir.rglob("*"):
        if not path.is_file():
            continue
        relative = path.relative_to(out_dir)
        if relative.parts[0] == "build" or relative.name in {
            "SHA256SUMS",
            "SHA256SUMS.sig",
        }:
            continue
        files.append(path)
    return sorted(files, key=lambda path: path.relative_to(out_dir).as_posix())


def sha256sums_text(out_dir: pathlib.Path) -> str:
    lines = []
    for path in release_files(out_dir):
        relative = path.relative_to(out_dir).as_posix()
        lines.append(f"{sha256(path)}  {relative}")
    return "\n".join(lines) + "\n"


def write_sha256sums(out_dir: pathlib.Path) -> pathlib.Path:
    path = out_dir / "SHA256SUMS"
    path.write_text(sha256sums_text(out_dir))
    return path


def validate_integrity(out_dir: pathlib.Path, manifest: dict[str, object]) -> None:
    checksum_path = release_path(str(manifest["checksum_file"]))
    if not checksum_path.exists():
        raise SystemExit(f"missing checksum file: {checksum_path}")
    expected_checksums = sha256sums_text(out_dir)
    actual_checksums = checksum_path.read_text()
    if actual_checksums != expected_checksums:
        diff = "\n".join(
            difflib.unified_diff(
                actual_checksums.splitlines(),
                expected_checksums.splitlines(),
                fromfile=str(checksum_path),
                tofile="current",
                lineterm="",
            )
        )
        raise SystemExit(f"SHA256SUMS is stale:\n{diff}")

    integrity_files = manifest.get("integrity_files")
    if not isinstance(integrity_files, list):
        raise SystemExit("release manifest is missing integrity_files")
    for entry in integrity_files:
        path = release_path(str(entry["path"]))
        if not path.exists():
            raise SystemExit(f"missing integrity file: {path}")

    dependency_reports = [
        entry for entry in integrity_files if entry.get("role") == "dependency-report"
    ]
    if len(dependency_reports) != 1:
        raise SystemExit("release manifest must include exactly one dependency report")
    dependency_report = release_path(str(dependency_reports[0]["path"])).read_text()
    for required in [
        "## Direct Runtime Dependencies",
        "## Direct Development Dependencies",
        "## Build Tool Requirements",
        "C compiler",
        "C++ compiler",
        "Archiver",
        "## Resolved Third-Party Packages",
    ]:
        if required not in dependency_report:
            raise SystemExit(f"dependency report is missing {required}")

    signing = manifest.get("signing")
    if not isinstance(signing, dict):
        raise SystemExit("release manifest is missing signing metadata")
    if signing.get("mechanism") != "ssh-ed25519-detached-sha256sums":
        raise SystemExit("release manifest has unexpected signing mechanism")
    signature_path = release_path(str(signing["signature_file"]))
    public_key_path = release_path(str(signing["public_key_file"]))
    allowed_signers_path = release_path(str(signing["allowed_signers_file"]))
    for path in [signature_path, public_key_path, allowed_signers_path]:
        if not path.exists():
            raise SystemExit(f"missing signing file: {path}")
    verify_checksum_signature(
        checksum_path,
        signature_path,
        allowed_signers_path,
        str(signing["identity"]),
    )

    checked = 0
    for artifact in manifest["artifacts"]:
        if artifact["kind"] not in {"cdylib", "staticlib"}:
            continue
        manifest_path = out_dir / "integrity/abi-symbols" / f"{artifact['name']}.tsv"
        actual = manifest_path.read_text()
        expected = render_symbol_manifest(artifact)
        if actual != expected:
            diff = "\n".join(
                difflib.unified_diff(
                    actual.splitlines(),
                    expected.splitlines(),
                    fromfile=str(manifest_path),
                    tofile="current",
                    lineterm="",
                )
            )
            raise SystemExit(f"symbol manifest is stale: {artifact['name']}\n{diff}")
        checked += 1
    print(
        f"ok: release integrity files and checksum signature validated for {checked} binary artifacts"
    )


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
            "output_name": "rimalloc-rust-rlib",
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
            "output_name": "rimalloc-c-abi-cdylib",
            "manifest_kind": "cdylib",
        },
        {
            "name": "c-abi-static-preview",
            "features": ["c-abi"],
            "rustflags": [
                "-C",
                "lto=off",
                "-C",
                "linker-plugin-lto=no",
                "-C",
                "embed-bitcode=no",
            ],
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
            "output_name": "rimalloc-c-abi-static-preview",
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
            "output_name": "rimalloc-interpose-cdylib",
            "manifest_kind": "cdylib",
        },
        {
            "name": "interpose-static-preview",
            "features": ["interpose"],
            "rustflags": [
                "-C",
                "lto=off",
                "-C",
                "linker-plugin-lto=no",
                "-C",
                "embed-bitcode=no",
            ],
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
            "output_name": "rimalloc-interpose-static-preview",
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
    integrity_files: list[dict[str, object]] = []
    signing: dict[str, object] = {}
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
        "integrity_files": integrity_files,
        "checksum_file": display_path(out_dir / "SHA256SUMS"),
        "signing": signing,
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
        rustflags = list(spec.get("rustflags", []))
        if rustflags:
            existing = env.get("RUSTFLAGS")
            env["RUSTFLAGS"] = " ".join([*([] if existing is None else [existing]), *rustflags])
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
            "rustflags": rustflags,
            "source_path": display_path(source),
            "sha256": sha256(copied),
        }
        artifacts.append(artifact)

    link_metadata.extend(write_link_metadata(out_dir, version, artifacts))
    integrity_files.extend(write_symbol_manifests(out_dir, artifacts))
    integrity_files.append(write_environment_note(out_dir))
    integrity_files.append(write_benchmark_note(out_dir))
    integrity_files.append(write_dependency_report(out_dir))
    key, key_source = signing_key()
    identity = signing_identity()
    public_key, allowed_signers, fingerprint = write_signing_public_key(out_dir, key, identity)
    integrity_files.extend(
        [
            metadata_entry("release-signing.pub", "release-signing-public-key", public_key),
            metadata_entry(
                "release-allowed-signers",
                "release-signing-allowed-signers",
                allowed_signers,
            ),
        ]
    )
    signing.update(
        {
            "mechanism": "ssh-ed25519-detached-sha256sums",
            "namespace": SIGNING_NAMESPACE,
            "identity": identity,
            "key_source": key_source,
            "public_key_file": display_path(public_key),
            "allowed_signers_file": display_path(allowed_signers),
            "public_key_fingerprint": fingerprint,
            "signed_file": display_path(out_dir / "SHA256SUMS"),
            "signature_file": display_path(out_dir / "SHA256SUMS.sig"),
            "source_tag_policy": "signed annotated Git tag using the release SSH signing key",
        }
    )

    manifest_path = out_dir / "release-artifacts.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    write_sha256sums(out_dir)
    signature_path = sign_checksum_file(out_dir, key)
    verify_checksum_signature(out_dir / "SHA256SUMS", signature_path, allowed_signers, identity)
    if key_source == "ephemeral-dry-run-key":
        shutil.rmtree(key.parent)
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
