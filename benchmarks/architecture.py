"""Run and report the C++ architectural probes through bench.py."""
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import shlex
import statistics

SCENARIOS = ("segment-occupancy", "cross-thread", "decommit-oscillation", "thread-churn")


def expected_phases(scenario, cycles):
    if scenario == "segment-occupancy":
        return [(phase, 0) for phase in (
            "baseline", "filled", "half-per-page", "half-per-page-collect",
            "tenth-per-page", "tenth-per-page-collect", "one-per-page",
            "one-per-page-collect", "empty", "empty-collect", "packed-same-live")]
    phases = {"cross-thread": ("fan-in-allocate", "fan-in-free", "fan-out-allocate", "fan-out-free"),
              "decommit-oscillation": ("refill-touch", "free", "forced-purge"),
              "thread-churn": ("create-allocate-exit", "free-after-owner-exit", "collect-abandoned")}[scenario]
    result = [("baseline", 0)] + [(phase, cycle) for cycle in range(cycles) for phase in phases]
    if scenario == "cross-thread":
        result.append(("final-collect", cycles))
    return result


def validate(rows, scenario, cycles):
    if [(r["phase"], r["cycle"]) for r in rows] != expected_phases(scenario, cycles):
        raise ValueError("missing, duplicate, or out-of-order phases")
    for row in rows:
        for key in ("cycle", "operations", "live_bytes", "rss_bytes", "minor_faults", "major_faults"):
            if type(row[key]) is not int or row[key] < 0:
                raise ValueError(f"invalid {key}")
        if type(row["occupied_pages"]) is not int or row["occupied_pages"] < -1:
            raise ValueError("invalid occupied_pages")
        if type(row["rss_delta_bytes"]) is not int:
            raise ValueError("invalid RSS delta")
        ns = row["elapsed_ns"]
        if type(ns) not in (float, int) or not math.isfinite(ns) or ns < 0:
            raise ValueError("invalid elapsed time")
        if row["phase"] != "baseline" and ns <= 0:
            raise ValueError("invalid phase duration")
    return rows


def report(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    measurements, errors = [], []
    for repeat in range(1, manifest["controls"]["repeats"] + 1):
        for scenario in manifest["scenarios"]:
            for allocator in manifest["allocators"]:
                relative = f"round-{repeat:03}/{scenario}/{allocator}"
                try:
                    worker = next((w for w in manifest["workers"] if w["directory"] == relative), None)
                    if worker is None or worker.get("exit_status") != 0:
                        raise ValueError("worker did not finish successfully")
                    rows = validate(json.loads((directory / relative / "measurements.json").read_text()),
                                    scenario, manifest["controls"]["cycles"])
                    measurements.extend({"scenario": scenario, "allocator": allocator, "repeat": repeat, **r}
                                        for r in rows)
                except (OSError, ValueError, KeyError, TypeError) as error:
                    errors.append(f"{relative}: {error}")
    # Keep every cycle and phase: first refill and steady-state behavior differ.
    metrics = ("elapsed_ns", "rss_bytes", "live_bytes", "occupied_pages", "minor_faults", "major_faults")
    summaries = []
    for scenario in manifest["scenarios"]:
        for phase, cycle in expected_phases(scenario, manifest["controls"]["cycles"]):
            row = {"scenario": scenario, "phase": phase, "cycle": cycle}
            complete = True
            for allocator, label in zip(manifest["allocators"], ("rust", "c")):
                values = [m for m in measurements if (m["scenario"], m["phase"], m["cycle"], m["allocator"])
                          == (scenario, phase, cycle, allocator)]
                if len(values) != manifest["controls"]["repeats"]:
                    complete = False
                    break
                for metric in metrics:
                    row[f"{label}_{metric}"] = statistics.median(m[metric] for m in values)
            if complete:
                summaries.append(row)
    complete = manifest["status"] == "complete" and not errors
    (directory / "results.json").write_text(json.dumps(
        {"complete": complete, "summary": summaries, "measurements": measurements, "errors": errors}, indent=2) + "\n")
    fields = ["scenario", "phase", "cycle"] + [f"{a}_{m}" for a in ("rust", "c") for m in metrics]
    with (directory / "results.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(summaries)
    lines = [f"# Architectural comparison ({'complete' if complete else 'incomplete'})", "",
             "Values are medians across independent process repetitions, separately for each cycle. "
             "Times include payload touching for allocation phases, synchronization for concurrent phases, "
             "and OS thread creation/exit for churn. RSS covers the whole process, including harness storage. "
             "Fault deltas cover each phase through its checkpoint. No combined score or pass/fail threshold.", "",
             "| Scenario / phase | Cycle | Rust ms | C ms | Rust RSS MiB | C RSS MiB | Rust/C live MiB | Rust/C occupied pages | Rust/C minor faults |",
             "|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for r in summaries:
        lines.append(f"| {r['scenario']} / {r['phase']} | {r['cycle']} | "
                     f"{r['rust_elapsed_ns'] / 1e6:.3f} | {r['c_elapsed_ns'] / 1e6:.3f} | "
                     f"{r['rust_rss_bytes'] / 2**20:.2f} | {r['c_rss_bytes'] / 2**20:.2f} | "
                     f"{r['rust_live_bytes'] / 2**20:.2f}/{r['c_live_bytes'] / 2**20:.2f} | "
                     f"{r['rust_occupied_pages']:g}/{r['c_occupied_pages']:g} | "
                     f"{r['rust_minor_faults']:g}/{r['c_minor_faults']:g} |")
    if manifest.get("error"):
        lines += ["", manifest["error"]]
    lines += ["", *errors] if errors else []
    (directory / "summary.md").write_text("\n".join(lines) + "\n")
    print(f"Report: {directory / 'summary.md'}", flush=True)
    return complete


def run(args, driver):
    controls = {k: getattr(args, k) for k in ("bytes", "block_size", "threads", "cycles", "repeats")}
    if args.bytes // args.block_size < args.threads:
        raise driver.BenchError("provide at least one block per thread")
    directory = driver.run_directory(args.output, "architecture-")
    manifest = driver.run_manifest(suite="architecture", scenarios=list(dict.fromkeys(args.scenario or SCENARIOS)),
                                   controls=controls, allocators=list(driver.ALLOCATORS), workers=[],
                                   process_layout="isolated-scenario-allocator")
    path = directory / "manifest.json"
    driver.write_json(path, manifest)
    print(f"Results: {directory}", flush=True)
    with driver.run_lifecycle(path, manifest, manifest["workers"]):
        artifacts = driver.build(args.jobs)
        executable = Path(artifacts["cache"]) / "architectural-bench"
        command = shlex.split(os.environ.get("CXX", "c++")) + ["-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror"]
        command += shlex.split(os.environ.get("CXXFLAGS", ""))
        command += ["-pthread", "-I", str(driver.ROOT / "mimalloc-3.3.2/include"),
                    str(driver.ROOT / "benches/architectural_bench.cpp"), "-ldl"]
        command += shlex.split(os.environ.get("LDFLAGS", "")) + ["-o", str(executable)]
        driver.execute(command, directory / "cpp-build.log")
        artifacts["architecture_executable"] = str(executable)
        artifacts["sha256"]["architecture_executable"] = hashlib.sha256(executable.read_bytes()).hexdigest()
        manifest.update(artifacts=artifacts, cpp_build_command=command, status="running")
        for name in ("harness.log", "rust.log", "c-configure.log", "c-build.log"):
            (directory / name).write_bytes((Path(artifacts["cache"]) / name).read_bytes())
        for repeat in range(1, args.repeats + 1):
            order = driver.ALLOCATORS if repeat % 2 else driver.ALLOCATORS[::-1]
            for scenario in manifest["scenarios"]:
                for allocator in order:
                    relative = f"round-{repeat:03}/{scenario}/{allocator}"
                    library = artifacts["rust_library" if allocator == driver.ALLOCATORS[0] else "c_library"]
                    command = [str(executable), "--library", library, "--scenario", scenario]
                    for flag, name in (("bytes", "bytes"), ("block-size", "block_size"), ("threads", "threads"), ("cycles", "cycles")):
                        command += [f"--{flag}", str(getattr(args, name))]
                    worker = {"directory": relative, "command": command, "started_utc": driver.utc()}
                    manifest["workers"].append(worker)
                    driver.write_json(path, manifest)
                    print(f"Round {repeat}/{args.repeats}: {scenario} / {allocator}", flush=True)
                    output = driver.execute(command, directory / relative / "worker.log")
                    rows = validate([json.loads(line) for line in output.splitlines() if line.startswith('{"phase":')],
                                    scenario, args.cycles)
                    driver.write_json(directory / relative / "measurements.json", rows)
                    worker.update(exit_status=0, finished_utc=driver.utc())
                    driver.write_json(path, manifest)
    complete = report(directory)
    if manifest.get("error"):
        print(manifest["error"], file=driver.sys.stderr)
    return 0 if complete else manifest.get("exit_status", 1)
