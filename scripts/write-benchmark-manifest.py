#!/usr/bin/env python3
"""Capture benchmark environment and provenance into an artifact directory."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import platform
import re
import subprocess
from pathlib import Path


def command(*args: str) -> str:
    completed = subprocess.run(args, check=True, capture_output=True, text=True)
    return completed.stdout.strip()


def optional_command(*args: str) -> str:
    try:
        return command(*args)
    except (FileNotFoundError, subprocess.CalledProcessError):
        return "unknown"


def cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unknown"


def physical_cores() -> int:
    output = optional_command("lscpu", "-p=CORE,SOCKET")
    cores = {line for line in output.splitlines() if line and not line.startswith("#")}
    return len(cores) if cores else (os.cpu_count() or 1)


def numa_nodes() -> int:
    nodes = list(Path("/sys/devices/system/node").glob("node[0-9]*"))
    return len(nodes) if nodes else 1


def governor() -> str:
    values = {
        path.read_text(encoding="utf-8").strip()
        for path in Path("/sys/devices/system/cpu").glob("cpu[0-9]*/cpufreq/scaling_governor")
    }
    return ",".join(sorted(values)) if values else "unknown"


def turbo() -> str:
    no_turbo = Path("/sys/devices/system/cpu/intel_pstate/no_turbo")
    try:
        return "disabled" if no_turbo.read_text(encoding="utf-8").strip() == "1" else "enabled"
    except OSError:
        return "unknown"


def env_int(name: str, default: int) -> int:
    try:
        value = int(os.environ.get(name, str(default)))
    except ValueError as error:
        raise SystemExit(f"{name} must be an integer") from error
    if value <= 0:
        raise SystemExit(f"{name} must be positive")
    return value


def env_list(name: str, default: str) -> list[str]:
    values = [value.strip() for value in os.environ.get(name, default).split(",")]
    return [value for value in values if value]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--artifact-mode", required=True)
    parser.add_argument("--feature", action="append", dest="features", required=True)
    parser.add_argument("--commands", required=True)
    args = parser.parse_args()

    root = Path(command("git", "rev-parse", "--show-toplevel")).resolve()
    artifact_dir = Path(args.artifact_dir).resolve()
    try:
        artifact_dir.relative_to(root)
    except ValueError as error:
        raise SystemExit("artifact directory must be inside the repository") from error
    dirty = bool(command("git", "status", "--porcelain", "--untracked-files=normal"))
    if dirty and os.environ.get("MIMALLOC_ALLOW_DIRTY_BENCH") != "1":
        raise SystemExit("refusing benchmark evidence from a dirty tree; set MIMALLOC_ALLOW_DIRTY_BENCH=1 to override")
    commit = command("git", "rev-parse", "HEAD")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise SystemExit("could not resolve a full commit hash")
    commands_path = Path(args.commands).resolve()
    commands_bytes = commands_path.read_bytes()
    rustc = command("rustc", "--version", "--verbose")
    llvm = next((line.split(":", 1)[1].strip() for line in rustc.splitlines() if line.startswith("LLVM version:")), "unknown")
    target = next((line.split(":", 1)[1].strip() for line in rustc.splitlines() if line.startswith("host:")), "unknown")
    c_compiler = optional_command("cc", "--version").splitlines()[0]
    environment = "\n\n".join([
        f"git_root\n{root}", f"git_commit\n{commit}", f"rustc\n{rustc}",
        f"cargo\n{command('cargo', '--version', '--verbose')}",
        f"kernel\n{platform.platform()}", f"cpu\n{optional_command('lscpu')}",
        f"c_compiler\n{optional_command('cc', '--version')}",
    ]) + "\n"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    (artifact_dir / "environment.txt").write_text(environment, encoding="utf-8")
    manifest = {
        "schema": 1,
        "repository": "whb07/mechanical_mimalloc",
        "commit": commit,
        "dirty": dirty,
        "artifact_mode": args.artifact_mode,
        "features": sorted(args.features),
        "target": target,
        "rustc": rustc.splitlines()[0],
        "llvm": llvm,
        "c_compiler": c_compiler,
        "kernel": platform.release(),
        "cpu_model": cpu_model(),
        "logical_cpus": os.cpu_count() or 1,
        "physical_cores": physical_cores(),
        "numa_nodes": numa_nodes(),
        "governor": governor(),
        "turbo": turbo(),
        "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "command_digest": hashlib.sha256(commands_bytes).hexdigest(),
        "controls": {
            "hotpath_sample_size": env_int("MIMALLOC_HOTPATH_SAMPLE_SIZE", 100),
            "hotpath_measurement_secs": env_int("MIMALLOC_HOTPATH_MEASUREMENT_SECS", 10),
            "hotpath_warmup_secs": env_int("MIMALLOC_HOTPATH_WARMUP_SECS", 1),
            "native_sample_size": env_int("MIMALLOC_BENCH_SAMPLE_SIZE", 100),
            "native_measurement_secs": env_int("MIMALLOC_BENCH_MEASUREMENT_SECS", 10),
            "native_warmup_secs": env_int("MIMALLOC_BENCH_WARMUP_SECS", 1),
            "native_scenarios": env_list(
                "MIMALLOC_BENCH_SCENARIOS",
                "glibc-thread,malloc-large,cache-scratch1,xmalloc-testN",
            ),
            "native_allocators": env_list(
                "MIMALLOC_BENCH_ALLOCATORS",
                "mechanical_mimalloc_raw,c-mimalloc-3.3.2",
            ),
            "resource_operations": env_int("MIMALLOC_BENCH_RESOURCE_OPERATIONS", 100000),
            "resource_repeats": env_int("MIMALLOC_BENCH_RESOURCE_REPEATS", 3),
        },
    }
    (artifact_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
