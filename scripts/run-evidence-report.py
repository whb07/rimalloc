#!/usr/bin/env python3
"""Capture the rich allocator diagnostics report in a benchmark artifact."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", required=True)
    args = parser.parse_args()

    artifact_dir = Path(args.artifact_dir).resolve()
    environment = {**os.environ, "CARGO_TARGET_DIR": str(artifact_dir)}
    subprocess.run(
        [
            "cargo", "build", "--release", "--features", "std,bench-diagnostics",
            "--example", "evidence_report",
        ],
        check=True,
        env=environment,
    )
    executable = artifact_dir / "release" / "examples" / "evidence_report"
    completed = subprocess.run(
        [str(executable)], check=True, capture_output=True, text=True, env=environment
    )
    report = json.loads(completed.stdout)
    output = artifact_dir / "evidence-report.json"
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote rich allocator evidence to {output}")


if __name__ == "__main__":
    main()
