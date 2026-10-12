#!/usr/bin/env python3
"""Run a release artifact dry run and validate generated integrity files."""

from __future__ import annotations

import importlib.util
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILDER_PATH = ROOT / "scripts/build-release-artifacts.py"
OUT_DIR = ROOT / "target/release-artifacts"


def load_builder():
    spec = importlib.util.spec_from_file_location("build_release_artifacts", BUILDER_PATH)
    if spec is None or spec.loader is None:
        raise SystemExit(f"could not load {BUILDER_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    builder = load_builder()
    manifest = builder.build_artifacts(OUT_DIR)
    builder.validate_integrity(OUT_DIR, manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
