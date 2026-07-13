#!/usr/bin/env python3
"""Archive validated benchmark modes and assembly evidence with a SHA-256 sidecar."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_MODES = {
    "default-rust": {
        "criterion",
        "threshold-results.csv",
        "benchmark-review.json",
        "evidence-report.json",
    },
    "interpose-feature-linked": {
        "criterion",
        "threshold-results.csv",
        "benchmark-review.json",
    },
    "ld-preload-interpose": {"interpose-cdylib.so"},
}
COMMON_FILES = {"manifest.json", "environment.txt", "commands.txt", "resource-results.jsonl"}
ASSEMBLY_SYMBOLS = {
    "mi_malloc.asm",
    "mi_malloc_small.asm",
    "mi_free.asm",
    "mi_route_free.asm",
    "mi_free_block_local.asm",
    "_mi_theap_malloc_zero_ex.asm",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def repository_path(value: str) -> Path:
    path = Path(value).resolve()
    try:
        path.relative_to(ROOT)
    except ValueError as error:
        raise SystemExit(f"evidence path must be inside the repository: {path}") from error
    if not path.exists():
        raise SystemExit(f"evidence path does not exist: {path}")
    return path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", action="append", dest="artifacts", required=True)
    parser.add_argument("--assembly-dir", required=True)
    parser.add_argument("--output-dir", default="target/benchmark-evidence")
    args = parser.parse_args()

    artifact_dirs = [repository_path(value) for value in args.artifacts]
    assembly_dir = repository_path(args.assembly_dir)
    manifests = []
    for artifact_dir in artifact_dirs:
        manifest_path = artifact_dir / "manifest.json"
        if not manifest_path.exists():
            raise SystemExit(f"benchmark artifact has no manifest: {artifact_dir}")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if (
            manifest.get("schema") != 1
            or manifest.get("artifact_mode") != artifact_dir.name
            or manifest.get("dirty") is not False
        ):
            raise SystemExit(f"benchmark artifact manifest is not clean or mode-valid: {artifact_dir}")
        manifests.append((artifact_dir, manifest_path, manifest))
    modes = {str(manifest["artifact_mode"]) for _, _, manifest in manifests}
    if modes != set(REQUIRED_MODES):
        raise SystemExit(f"archive requires exactly these artifact modes: {sorted(REQUIRED_MODES)}")
    commits = {str(manifest["commit"]) for _, _, manifest in manifests}
    if len(commits) != 1:
        raise SystemExit("benchmark artifact manifests do not describe one commit")
    commit = next(iter(commits))
    assembly_manifest = assembly_dir / "manifest.txt"
    if not assembly_manifest.exists() or f"git_head={commit}" not in assembly_manifest.read_text(
        encoding="utf-8"
    ):
        raise SystemExit("assembly evidence is missing or belongs to another commit")

    output_dir = Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    archive = output_dir / f"stages-0-3-evidence-{commit[:12]}.tar.zst"
    relative_inputs: list[str] = []
    archived_files: dict[str, list[str]] = {}
    for artifact_dir, _, manifest in manifests:
        mode = str(manifest["artifact_mode"])
        required = COMMON_FILES | REQUIRED_MODES[mode]
        missing = sorted(name for name in required if not (artifact_dir / name).exists())
        if missing:
            raise SystemExit(f"{mode} archive inputs are incomplete: {', '.join(missing)}")
        children = [artifact_dir / name for name in sorted(required)]
        archived_files[mode] = [str(child.relative_to(artifact_dir)) for child in children]
        relative_inputs.extend(str(child.relative_to(ROOT)) for child in children)
    missing_assembly = sorted(
        name for name in ASSEMBLY_SYMBOLS if not (assembly_dir / name).exists()
    )
    if missing_assembly:
        raise SystemExit(
            "assembly archive inputs are incomplete: " + ", ".join(missing_assembly)
        )
    assembly_children = [
        assembly_manifest,
        *(assembly_dir / name for name in sorted(ASSEMBLY_SYMBOLS)),
    ]
    relative_inputs.extend(str(child.relative_to(ROOT)) for child in assembly_children)
    subprocess.run(
        ["tar", "--zstd", "-cf", str(archive), *relative_inputs],
        check=True,
        cwd=ROOT,
    )
    archive_digest = sha256(archive)
    preload_dir = next(
        path
        for path, _, manifest in manifests
        if manifest["artifact_mode"] == "ld-preload-interpose"
    )
    preload_library = preload_dir / "interpose-cdylib.so"
    index = {
        "schema": 1,
        "commit": commit,
        "archive": str(archive.relative_to(ROOT)),
        "archive_sha256": archive_digest,
        "artifact_modes": [
            {
                "mode": manifest["artifact_mode"],
                "path": str(path.relative_to(ROOT)),
                "manifest_sha256": sha256(manifest_path),
                "archived_files": archived_files[str(manifest["artifact_mode"])],
                "host_profile": {
                    "target": manifest["target"],
                    "cpu_model": manifest["cpu_model"],
                    "kernel": manifest["kernel"],
                },
            }
            for path, manifest_path, manifest in manifests
        ],
        "assembly": {
            "path": str(assembly_dir.relative_to(ROOT)),
            "manifest_sha256": sha256(assembly_manifest),
        },
        "deployed_interpose": {
            "path": str(preload_library.relative_to(ROOT)),
            "sha256": sha256(preload_library),
        },
    }
    index_path = output_dir / "evidence-index.json"
    index_path.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    checksum_path = archive.with_suffix(archive.suffix + ".sha256")
    checksum_path.write_text(f"{archive_digest}  {archive.name}\n", encoding="utf-8")
    print(f"archived benchmark evidence: {archive}")
    print(f"SHA-256: {archive_digest}")


if __name__ == "__main__":
    main()
