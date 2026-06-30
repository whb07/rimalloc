#!/usr/bin/env python3
"""Check that release artifact inventory and ABI manifests reproduce."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT_DIR = pathlib.Path("target/release-artifacts")
SYMBOL_DIR = OUT_DIR / "integrity/abi-symbols"


def run(cmd: list[str], cwd: pathlib.Path) -> str:
    completed = subprocess.run(
        cmd,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if completed.returncode != 0:
        print(completed.stdout)
        raise subprocess.CalledProcessError(completed.returncode, cmd)
    return completed.stdout


def tracked_files() -> list[pathlib.Path]:
    output = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    return [
        ROOT / pathlib.Path(name.decode())
        for name in output.split(b"\0")
        if name
    ]


def materialize_checkout(destination: pathlib.Path) -> None:
    for source in tracked_files():
        relative = source.relative_to(ROOT)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            os.symlink(os.readlink(source), target)
        else:
            shutil.copy2(source, target)
    initialize_git_checkout(destination)


def initialize_git_checkout(destination: pathlib.Path) -> None:
    # These are tracked-file checkouts with enough Git metadata for env capture.
    env = {
        **os.environ,
        "GIT_AUTHOR_NAME": "mechanical_mimalloc release check",
        "GIT_AUTHOR_EMAIL": "release-check@example.invalid",
        "GIT_AUTHOR_DATE": "2000-01-01T00:00:00Z",
        "GIT_COMMITTER_NAME": "mechanical_mimalloc release check",
        "GIT_COMMITTER_EMAIL": "release-check@example.invalid",
        "GIT_COMMITTER_DATE": "2000-01-01T00:00:00Z",
    }
    subprocess.run(["git", "init", "-q"], cwd=destination, check=True, env=env)
    subprocess.run(["git", "add", "-A"], cwd=destination, check=True, env=env)
    subprocess.run(
        ["git", "commit", "-q", "-m", "materialize release checkout"],
        cwd=destination,
        check=True,
        env=env,
    )


def manifest_path(checkout: pathlib.Path) -> pathlib.Path:
    return checkout / OUT_DIR / "release-artifacts.json"


def release_path(checkout: pathlib.Path, value: str) -> pathlib.Path:
    return checkout / value


def file_names(entries: list[dict[str, object]]) -> list[str]:
    return sorted(str(entry["path"]) for entry in entries)


def checksum_name(path: str) -> str:
    return pathlib.Path(path).relative_to(OUT_DIR).as_posix()


def artifact_inventory(manifest: dict[str, object]) -> list[dict[str, object]]:
    artifacts = []
    for artifact in manifest["artifacts"]:
        artifacts.append(
            {
                "name": artifact["name"],
                "kind": artifact["kind"],
                "crate_type": artifact["crate_type"],
                "features": artifact["features"],
                "path": artifact["path"],
                "rustflags": artifact["rustflags"],
                "target_triple": manifest["target_triple"],
                "profile": manifest["profile"],
                "rustc": manifest["rustc"],
            }
        )
    return sorted(artifacts, key=lambda artifact: str(artifact["name"]))


def normalized_symbol_manifest(path: pathlib.Path) -> list[str]:
    lines = path.read_text().splitlines()
    return [line for line in lines if not line.startswith("# sha256:")]


def symbol_manifests(checkout: pathlib.Path) -> dict[str, list[str]]:
    manifests: dict[str, list[str]] = {}
    for path in sorted((checkout / SYMBOL_DIR).glob("*.tsv")):
        manifests[path.name] = normalized_symbol_manifest(path)
    return manifests


def validate_manifest(checkout: pathlib.Path, manifest: dict[str, object]) -> None:
    checksum_file = release_path(checkout, str(manifest["checksum_file"]))
    checksum_text = checksum_file.read_text()

    for section in ["artifacts", "headers", "link_metadata", "integrity_files"]:
        entries = manifest[section]
        if not isinstance(entries, list):
            raise SystemExit(f"{section} must be a list in release manifest")
        for path in file_names(entries):
            if not release_path(checkout, path).exists():
                raise SystemExit(f"manifest path missing from checkout: {path}")
            if checksum_name(path) not in checksum_text:
                raise SystemExit(f"SHA256SUMS does not include manifest path: {path}")

    binary_names = {
        str(artifact["name"])
        for artifact in manifest["artifacts"]
        if artifact["kind"] in {"cdylib", "staticlib"}
    }
    manifest_names = {path.stem for path in (checkout / SYMBOL_DIR).glob("*.tsv")}
    if binary_names != manifest_names:
        raise SystemExit(
            "ABI symbol manifest names do not match binary artifact names: "
            f"{sorted(binary_names)} != {sorted(manifest_names)}"
        )


def build_release(checkout: pathlib.Path) -> dict[str, object]:
    run(["scripts/build-release-artifacts.py"], cwd=checkout)
    manifest = json.loads(manifest_path(checkout).read_text())
    validate_manifest(checkout, manifest)
    return {
        "artifacts": artifact_inventory(manifest),
        "symbols": symbol_manifests(checkout),
    }


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mechanical-mimalloc-repro-") as tmp:
        temp_root = pathlib.Path(tmp)
        checkouts = [temp_root / "checkout-a", temp_root / "checkout-b"]
        for checkout in checkouts:
            materialize_checkout(checkout)

        first = build_release(checkouts[0])
        second = build_release(checkouts[1])
        if first != second:
            raise SystemExit(
                "release artifacts are not reproducible across fresh checkouts"
            )

    print("ok: release artifact names and ABI symbol manifests reproduce")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
