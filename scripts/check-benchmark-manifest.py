#!/usr/bin/env python3
"""Verify benchmark artifact provenance and required outputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path


REQUIRED = [
    "environment.txt", "manifest.json", "commands.txt", "criterion",
    "resource-results.jsonl", "threshold-results.csv",
]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    args = parser.parse_args()
    artifact_dir = Path(args.artifact_dir)
    if artifact_dir.name != args.artifact_mode:
        raise SystemExit("artifact directory name and artifact mode disagree")
    missing = [name for name in REQUIRED if not (artifact_dir / name).exists()]
    if args.artifact_mode == "default-rust" and not (artifact_dir / "evidence-report.json").exists():
        missing.append("evidence-report.json")
    if missing:
        raise SystemExit("missing benchmark artifacts: " + ", ".join(missing))
    manifest = json.loads((artifact_dir / "manifest.json").read_text(encoding="utf-8"))
    expected_features = sorted(args.features)
    if manifest.get("schema") != 1 or manifest.get("artifact_mode") != args.artifact_mode:
        raise SystemExit("manifest schema or artifact mode mismatch")
    if manifest.get("features") != expected_features:
        raise SystemExit("manifest feature set mismatch")
    if manifest.get("dirty") and os.environ.get("MIMALLOC_ALLOW_DIRTY_BENCH") != "1":
        raise SystemExit("dirty benchmark manifest requires MIMALLOC_ALLOW_DIRTY_BENCH=1")
    digest = hashlib.sha256((artifact_dir / "commands.txt").read_bytes()).hexdigest()
    if manifest.get("command_digest") != digest:
        raise SystemExit("manifest command digest mismatch")
    for line_number, line in enumerate(
        (artifact_dir / "resource-results.jsonl").read_text(encoding="utf-8").splitlines(), 1
    ):
        if not line.strip():
            continue
        record = json.loads(line)
        if record.get("artifact_mode") != args.artifact_mode:
            raise SystemExit(f"resource record {line_number} artifact mode mismatch")
        if sorted(record.get("features", [])) != expected_features:
            raise SystemExit(f"resource record {line_number} feature set mismatch")
    print(f"ok: verified benchmark manifest and required outputs in {artifact_dir}")


if __name__ == "__main__":
    main()
