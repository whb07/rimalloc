#!/usr/bin/env python3
"""Build, compare, and report the existing mimalloc benchmarks."""

from __future__ import annotations

import argparse
import architecture
import applications
import bolt
import nginx
import rivals
import webserver
import contextlib
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import shlex
import shutil
import signal
import statistics
import subprocess
import sys
import time
import uuid

from common import file_sha256, write_json

ROOT = Path(__file__).resolve().parents[1]
ALLOCATORS = ("mechanical_mimalloc_raw", "c-mimalloc-3.3.2")
# `run --allocator` names, as in the nginx and webserver suites; rivals keep their own labels.
ALLOCATOR_NAMES = {"rust": ALLOCATORS[0], "mimalloc": ALLOCATORS[1], **{name: name for name in rivals.NAMES}}
SHORT_NAMES = {"mechanical_mimalloc_raw": "rust", "c-mimalloc-3.3.2": "c"}
QUICK = ("glibc-thread", "malloc-large", "cache-scratch1", "xmalloc-testN")
SCENARIOS = dict.fromkeys((
    "glibc-simple", "malloc-large", "malloc-large-old", "cache-scratch1",
    "cache-thrash1", "cache-scratchN", "cache-thrashN", "mstressN", "sh6benchN",
    "sh8benchN", "alloc-test1", "alloc-testN", "barnes", "cfrac", "mleak10", "mleak100",
), "per-workload")
SCENARIOS.update({
    "glibc-thread": "per-1M-malloc-free-pairs", "xmalloc-testN": "per-1M-frees",
    "larsonN": "per-1M-alloc-free-pairs", "larsonN-sized": "per-1M-alloc-free-pairs",
    "rptestN": "per-1M-rptest-worker-ops",
})
NATIVE_SCENARIOS = dict(SCENARIOS)
SCENARIOS.update(applications.SCENARIOS)
C_OPTIONS = {
    "CMAKE_BUILD_TYPE": "Release", "MI_BUILD_SHARED": "ON", "MI_BUILD_STATIC": "OFF",
    "MI_BUILD_OBJECT": "OFF", "MI_BUILD_TESTS": "OFF", "MI_OVERRIDE": "ON",
    "MI_SECURE": "OFF", "MI_SECURE_FULL": "OFF", "MI_GUARDED": "OFF",
}
# Fast scenarios that together cover the fast paths, cross-thread frees,
# abandon/reclaim, arena, and fixed-duration workloads; used by `screen`.
CANARIES = ("xmalloc-testN", "larsonN", "larsonN-sized", "sh6benchN", "sh8benchN", "glibc-thread",
            "glibc-simple", "malloc-large", "mstressN", "mleak100", "cache-scratchN")
# Workloads that already average over a whole timed run; others take the median of
# several in-process runs in `screen`.
TIMED = ("xmalloc-testN", "larsonN", "larsonN-sized")
GATES = {"smoke": "scripts/check-bench-smoke.sh", "release": "scripts/check-bench-full.sh"}
spec = importlib.util.spec_from_file_location("criterion_summary", ROOT / "scripts/summarize-criterion.py")
SUMMARY = importlib.util.module_from_spec(spec)
spec.loader.exec_module(SUMMARY)


class BenchError(Exception):
    def __init__(self, message: str, code: int = 1):
        super().__init__(message)
        self.code = code


def utc() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def repo_path(value: str) -> Path:
    return (ROOT / value).resolve()


def capture(command: list[str]) -> str:
    return subprocess.check_output(command, cwd=ROOT, text=True, stderr=subprocess.STDOUT).strip()


def run_directory(output: str | None, prefix: str = "") -> Path:
    directory = repo_path(output) if output else ROOT / "target/bench-runs" / (
        prefix + time.strftime("%Y%m%d-%H%M%S", time.gmtime()) + "-" + uuid.uuid4().hex[:8])
    directory.mkdir(parents=True, exist_ok=True)
    if any(directory.iterdir()):
        raise BenchError(f"output directory must be empty: {directory}")
    return directory


def run_manifest(**fields: object) -> dict:
    """Common provenance for every suite's manifest.json."""
    return {"schema": 1, "status": "building", "started_utc": utc(), **fields,
            "commit": capture(["git", "rev-parse", "HEAD"]),
            "dirty": bool(capture(["git", "status", "--porcelain"])),
            "host": {"platform": platform.platform(), "logical_cpus": os.cpu_count(),
                     "affinity": sorted(os.sched_getaffinity(0)), "lscpu": capture(["lscpu"])},
            "allocator_environment": {k: v for k, v in os.environ.items() if k.startswith(("MIMALLOC_", "MALLOC_"))
                                      and not k.startswith("MIMALLOC_BENCH_")
                                      and k not in ("MIMALLOC_RUST_LIB", "MIMALLOC_C_LIB")},
            "caller_ld_preload_removed": os.environ.get("LD_PRELOAD")}


@contextlib.contextmanager
def run_lifecycle(path: Path, manifest: dict, entries: list[dict]):
    """Record the run's final status and close out an interrupted or failed entry."""
    try:
        yield
        manifest["status"] = "complete"
    except KeyboardInterrupt:
        manifest.update(status="interrupted", error="interrupted", exit_status=130)
    except Exception as error:
        expected = isinstance(error, BenchError)
        manifest.update(status="failed", error=str(error) if expected else f"{type(error).__name__}: {error}",
                        exit_status=error.code if expected else 1)
    finally:
        manifest["finished_utc"] = utc()
        if entries and "exit_status" not in entries[-1]:
            entries[-1].update(exit_status=manifest.get("exit_status", 1), finished_utc=manifest["finished_utc"])
        write_json(path, manifest)


def default_jobs() -> int:
    return min(len(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else os.cpu_count() or 1, 16)


def execute(command: list[str], log: Path, environment: dict[str, str] | None = None, cwd: Path | None = None) -> str:
    """Stream output, preserve the full log, and own the complete child process group."""
    log.parent.mkdir(parents=True, exist_ok=True)
    print("+ " + shlex.join(command), flush=True)
    lines = []
    with log.open("w") as handle:
        handle.write("+ " + shlex.join(command) + "\n")
        with subprocess.Popen(command, cwd=cwd or ROOT, env=environment, text=True, errors="replace",
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                              start_new_session=True) as process:
            try:
                for line in process.stdout:
                    handle.write(line)
                    handle.flush()
                    lines.append(line)
                    try:
                        event = json.loads(line) if line.startswith('{"') else None
                    except ValueError:
                        event = None
                    if event is None:
                        print(line, end="", flush=True)
                    elif event.get("reason") == "compiler-message":
                        print(event["message"].get("rendered", ""), end="", flush=True)
                code = process.wait()
            except BaseException:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                raise
    if code:
        raise BenchError(f"command exited {code}; see {log}", code if code > 0 else 1)
    return "".join(lines)


def cargo_artifact(output: str, name: str, shared: bool = False) -> Path:
    candidates = set()
    for line in output.splitlines():
        if not line.startswith('{"'):
            continue
        event = json.loads(line)
        if event.get("reason") != "compiler-artifact" or event["target"]["name"] != name:
            continue
        if shared:
            candidates.update(Path(p) for p in event["filenames"] if p.endswith(".so"))
        elif event.get("executable"):
            candidates.add(Path(event["executable"]))
    if len(candidates) != 1:
        raise BenchError(f"expected one Cargo artifact for {name}, found {sorted(map(str, candidates))}")
    path = candidates.pop().resolve()
    if not path.is_file():
        raise BenchError(f"Cargo artifact is missing: {path}")
    return path


def cargo_config_files() -> dict[str, str]:
    """Cargo config files that can inject rustflags or profiles into the build."""
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    candidates = [d / ".cargo" / n for d in (ROOT, *ROOT.parents) for n in ("config", "config.toml")]
    candidates += [cargo_home / n for n in ("config", "config.toml")]
    return {str(p): p.read_text() for p in dict.fromkeys(candidates) if p.is_file()}


def build(jobs: int, harness: bool = True) -> dict:
    """Build the Rust and C shared libraries, plus the Criterion harness unless harness=False."""
    if sys.platform != "linux":
        raise BenchError("the native Rust/C shared-library comparison requires Linux")
    tools = {"cargo": ["cargo"], "rustc": shlex.split(os.environ.get("RUSTC", "rustc")), "cmake": ["cmake"],
             "cc": shlex.split(os.environ.get("CC", "cc")),
             "cxx": shlex.split(os.environ.get("CXX", "c++"))}
    for command in tools.values():
        if not command or shutil.which(command[0]) is None:
            raise BenchError(f"required build tool is missing: {shlex.join(command)}")
    versions = {name: capture(command + ["--version"]) for name, command in tools.items()}
    rustc = capture(tools["rustc"] + ["--version", "--verbose"])
    host = next((line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: ")), None)
    if host is None:
        raise BenchError(f"{shlex.join(tools['rustc'])} --version --verbose did not report a host triple")
    if os.environ.get("CARGO_BUILD_TARGET", host) != host:
        raise BenchError("comparison executables must target the current host; unset CARGO_BUILD_TARGET")
    flags = {key: os.environ.get(key, "") for key in (
        "CC", "CXX", "CFLAGS", "CXXFLAGS", "LDFLAGS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CMAKE_GENERATOR", "CARGO_INCREMENTAL",
    )}
    flags.update({k: v for k, v in os.environ.items()
                  if k.startswith(("CARGO_PROFILE_", "CARGO_TARGET_", "CARGO_BUILD_")) and k != "CARGO_TARGET_DIR"})
    config = {"tools": versions, "tool_paths": {name: shutil.which(command[0]) for name, command in tools.items()},
              "flags": flags, "cargo_config": cargo_config_files(), "harness_features": ["bench"],
              "rust_library_features": ["interpose"], "c_options": C_OPTIONS}
    use_bolt = os.environ.get("MIMALLOC_BENCH_BOLT") == "1"
    if use_bolt:
        config["bolt"] = {"llvm_bolt": bolt.version(), "profile_sha256": file_sha256(bolt.PROFILE),
                          "options": bolt.OPTIONS}
    key = hashlib.sha256(json.dumps(config, sort_keys=True).encode()).hexdigest()[:16]
    cache = ROOT / "target/bench-build" / key
    cache.mkdir(parents=True, exist_ok=True)
    environment = {**os.environ, "CARGO_TARGET_DIR": str(cache / "harness")}
    print(f"Build cache: {cache}", flush=True)
    artifacts = {"cache": str(cache), "configuration": config, "rustc_verbose": rustc,
                 "logs": ["rust.log", "c-configure.log", "c-build.log"]}
    if harness:
        output = execute(["cargo", "bench", "--locked", "--no-run", "--features", "bench",
                          "--bench", "mimalloc_bench", "--jobs", str(jobs),
                          "--message-format=json-render-diagnostics"], cache / "harness.log", environment)
        artifacts["executable"] = str(cargo_artifact(output, "mimalloc_bench"))
        artifacts["logs"].insert(0, "harness.log")
    environment["CARGO_TARGET_DIR"] = str(cache / "rust-interpose")
    if use_bolt:
        environment["RUSTFLAGS"] = (environment.get("RUSTFLAGS", "") + " " + bolt.LINK_FLAGS).strip()
    output = execute(["cargo", "rustc", "--locked", "--release", "--features", "interpose",
                      "--lib", "--jobs", str(jobs), "--message-format=json-render-diagnostics",
                      "--crate-type", "cdylib"], cache / "rust.log", environment)
    rust_library = cargo_artifact(output, "mechanical_mimalloc", shared=True)
    if use_bolt:
        optimized = cache / "libmechanical_mimalloc.bolt.so"
        artifacts["bolt"] = bolt.optimize(rust_library, optimized, cache / "bolt.log")
        rust_library = optimized
    c_dir = cache / "c-release"
    execute(["cmake", "-S", str(ROOT / "mimalloc-3.3.2"), "-B", str(c_dir)] +
            [f"-D{name}={value}" for name, value in C_OPTIONS.items()], cache / "c-configure.log")
    execute(["cmake", "--build", str(c_dir), "--config", "Release", "--parallel", str(jobs)],
            cache / "c-build.log")
    c_library = (c_dir / "libmimalloc.so").resolve()
    if not c_library.is_file():
        raise BenchError(f"CMake did not produce {c_library}")
    artifacts.update(rust_library=str(rust_library), c_library=str(c_library))
    artifacts["sha256"] = {name: file_sha256(artifacts[name])
                           for name in ("executable", "rust_library", "c_library") if name in artifacts}
    # Library-only builds share this cache; keep their record from replacing the harness build's.
    write_json(cache / ("artifacts.json" if harness else "library-artifacts.json"), artifacts)
    return artifacts


def copy_build_logs(artifacts: dict, directory: Path, prefix: str = "") -> None:
    for name in artifacts["logs"]:
        shutil.copy2(Path(artifacts["cache"]) / name, directory / (prefix + name))
    write_json(directory / (prefix + "artifacts.json"), artifacts)


def without_preload() -> dict[str, str]:
    """Caller environment for workers; a caller LD_PRELOAD would skew allocator measurements."""
    return {k: v for k, v in os.environ.items() if k != "LD_PRELOAD"}


def worker_environment(artifacts: dict, args: argparse.Namespace, criterion: Path,
                       scenario: str, allocator: str) -> dict[str, str]:
    environment = {key: value for key, value in without_preload().items()
                   if not key.startswith("MIMALLOC_BENCH_")}
    environment.update({f"MIMALLOC_BENCH_LIB_{name.upper()}": path
                        for name, path in artifacts.get("rivals", {}).items() if path != "glibc"})
    if set(allocator.split(",")) & set(artifacts.get("rivals", {})):
        # Rivals with initial-exec TLS (jemalloc) need static TLS room to be dlopened.
        environment["GLIBC_TUNABLES"] = rivals.STATIC_TLS_TUNABLE
    environment.update({
        "MIMALLOC_RUST_LIB": artifacts["rust_library"], "MIMALLOC_C_LIB": artifacts["c_library"],
        "MIMALLOC_BENCH_SKIP_BUILD": "1", "MIMALLOC_BENCH_SCENARIOS": scenario,
        "MIMALLOC_BENCH_ALLOCATORS": allocator, "MIMALLOC_BENCH_SAMPLE_SIZE": str(args.samples),
        "MIMALLOC_BENCH_MEASUREMENT_SECS": str(args.measurement_secs),
        "MIMALLOC_BENCH_WARMUP_SECS": str(args.warmup_secs), "CRITERION_HOME": str(criterion),
    })
    return environment


def full_id(scenario: str, allocator: str) -> str:
    return f"mimalloc-bench-native/{scenario}-{SCENARIOS[scenario]}/{allocator}"


def validate_sample(directory: Path, expected: str, samples: int) -> dict:
    benchmarks = list(directory.glob("**/new/benchmark.json"))
    if len(benchmarks) != 1:
        raise ValueError(f"expected one result for {expected}, found {len(benchmarks)}")
    parent = benchmarks[0].parent
    if json.loads(benchmarks[0].read_text())["full_id"] != expected:
        raise ValueError(f"unexpected benchmark ID for {expected}")
    sample = json.loads((parent / "sample.json").read_text())
    if len(sample["iters"]) != samples or len(sample["times"]) != samples:
        raise ValueError(f"wrong sample count for {expected}")
    if not all(type(n) in (int, float) and math.isfinite(n) and n > 0
               for n in sample["iters"] + sample["times"]):
        raise ValueError(f"invalid sample values for {expected}")
    estimates = json.loads((parent / "estimates.json").read_text())
    mean = estimates["mean"]
    numbers = [mean["point_estimate"], estimates["median"]["point_estimate"],
               mean["confidence_interval"]["lower_bound"], mean["confidence_interval"]["upper_bound"]]
    if not all(type(n) in (int, float) and math.isfinite(n) and n > 0 for n in numbers) or numbers[2] > numbers[3]:
        raise ValueError(f"invalid estimates for {expected}")
    result = SUMMARY.collect_results(directory, [], {expected}, [expected])[0]
    if (not all(math.isfinite(result[key]) for key in (
            "mean", "median", "p95", "sample_mean", "sample_median", "min", "max", "ci_width_pct", "spread_pct"))
            or not all(result[key] > 0 for key in ("mean", "median", "p95"))):
        raise ValueError(f"invalid estimates for {expected}")
    return result


def report(directory: Path) -> bool:
    directory = directory.resolve()
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest.get("suite") == "nginx":
        return nginx.report(directory, manifest)
    if manifest.get("suite") == "webserver":
        return webserver.report(directory, manifest)
    if manifest.get("suite") == "architecture":
        return architecture.report(directory, manifest)
    allocators = manifest.get("allocators", list(ALLOCATORS))
    rust_label, c_label = ALLOCATORS
    has_rust, has_c = rust_label in allocators, c_label in allocators
    rival_names = [a for a in allocators if a not in ALLOCATORS]
    measurements = []
    errors = []
    for scenario in manifest["scenarios"]:
        for repeat in range(1, manifest["controls"]["repeats"] + 1):
            for allocator in allocators:
                relative = f"round-{repeat:03}/{scenario}/{allocator}"
                try:
                    worker = next((w for w in manifest["workers"] if w["directory"] == relative), None)
                    if worker is None:
                        raise ValueError("worker was not run")
                    if worker.get("exit_status") != 0:
                        raise ValueError(f"worker did not finish successfully: {relative}")
                    if scenario in applications.SCENARIOS:
                        result = applications.measurement(directory / relative / "application.json", scenario,
                                                          manifest["controls"]["samples"])
                    else:
                        result = validate_sample(directory / relative / "criterion", full_id(scenario, allocator),
                                                 manifest["controls"]["samples"])
                    measurements.append({"scenario": scenario, "repeat": repeat,
                                         "allocator": allocator, **result})
                except (ValueError, KeyError, TypeError, OSError) as error:
                    errors.append(f"{relative}: {error}")
    rows = []
    for scenario in manifest["scenarios"]:
        values = {allocator: [r for r in measurements if r["scenario"] == scenario and r["allocator"] == allocator]
                  for allocator in allocators}
        if any(len(v) != manifest["controls"]["repeats"] for v in values.values()):
            continue
        medians = {a: statistics.median(r["median"] for r in values[a]) for a in allocators}
        row = {"scenario": scenario, "metric": SCENARIOS[scenario]}
        if has_rust:
            row["rust_median_ns"] = medians[rust_label]
        if has_c:
            row["c_median_ns"] = medians[c_label]
        if has_rust and has_c:
            row["rust_c_ratio"] = medians[rust_label] / medians[c_label]
            row["difference_pct"] = (row["rust_c_ratio"] - 1) * 100
        row["noisy"] = any(r["noisy"] for v in values.values() for r in v)
        for name in rival_names:
            row[f"{name}_median_ns"] = medians[name]
            if has_rust:
                row[f"rust_{name}_ratio"] = medians[rust_label] / medians[name]
        rows.append(row)
    complete = not errors and bool(rows) and manifest["status"] == "complete"
    native_rows = [r for r in rows if r["scenario"] in NATIVE_SCENARIOS]
    ratios = complete and native_rows and has_rust
    geomean = (math.exp(statistics.mean(math.log(r["rust_c_ratio"]) for r in native_rows))
               if ratios and has_c else None)
    rival_geomeans = {name: math.exp(statistics.mean(math.log(r[f"rust_{name}_ratio"]) for r in native_rows))
                      for name in rival_names} if ratios else {}
    # Columns in the default layout: Rust, C, Rust vs C, then each rival and Rust vs it.
    columns = [("Rust median", "rust_median_ns", "time")] * has_rust + [("C median", "c_median_ns", "time")] * has_c
    columns += [("Rust vs C", "difference_pct", "pct")] * (has_rust and has_c)
    for name in rival_names:
        columns += [(f"{name} median", f"{name}_median_ns", "time")]
        columns += [(f"Rust vs {name}", f"rust_{name}_ratio", "ratio")] * has_rust
    title = "Rust and C" if allocators[:2] == list(ALLOCATORS) else "allocator"
    lines = [f"# Isolated {title} comparison", "", f"Status: {'complete' if complete else 'INCOMPLETE'}.", "",
             "Each scenario and allocator runs in a fresh process. Negative differences mean less time for Rust.",
             "Medians aggregate repetition medians. Noise describes within-run variation; p95 is a batch statistic,",
             "not individual allocation latency. This exploratory report does not apply release thresholds.", "",
             "| Scenario | Metric | " + "".join(f"{heading} | " for heading, _, _ in columns) + "Noise |",
             "|---|---|" + "---:|" * len(columns) + "---|"]
    for row in rows:
        cells = "".join(f"{SUMMARY.format_time(row[key])} | " if kind == "time"
                        else f"{row[key]:+.2f}% | " if kind == "pct"
                        else f"{(row[key] - 1) * 100:+.2f}% | " for _, key, kind in columns)
        lines.append(f"| {row['scenario']} | {row['metric']} | {cells}{'noisy' if row['noisy'] else 'stable'} |")
    if geomean is not None:
        lines += ["", f"Native scenario geomean Rust/C: {geomean:.4f} ({(geomean - 1) * 100:+.2f}%)."]
        lines += [f"Native scenario geomean Rust/{name}: {g:.4f} ({(g - 1) * 100:+.2f}%)."
                  for name, g in rival_geomeans.items()]
        lines += ["An unweighted scenario geomean does not predict application-wide performance."]
    if manifest.get("unavailable"):
        lines += ["", "Unavailable on this host: " + ", ".join(manifest["unavailable"])]
    if errors:
        lines += ["", "Missing or invalid measurements:"] + [f"- {error}" for error in errors]
    lines += ["", "## Repetitions", "", "| Scenario | Round | Allocator | Median | Batch p95 | Noise |",
              "|---|---:|---|---:|---:|---|"]
    for result in measurements:
        lines.append(f"| {result['scenario']} | {result['repeat']} | {result['allocator']} | "
                     f"{SUMMARY.format_time(result['median'])} | {SUMMARY.format_time(result['p95'])} | "
                     f"{'noisy' if result['noisy'] else 'stable'} |")
    lines += applications.report_lines(measurements)
    (directory / "summary.md").write_text("\n".join(lines) + "\n")
    fields = ["scenario", "metric"] + ["rust_median_ns"] * has_rust + ["c_median_ns"] * has_c
    fields += ["rust_c_ratio", "difference_pct"] * (has_rust and has_c) + ["noisy"]
    for name in rival_names:
        fields += [f"{name}_median_ns"] + [f"rust_{name}_ratio"] * has_rust
    with (directory / "results.csv").open("w") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    write_json(directory / "results.json", {"complete": complete, "geomean": geomean, "rival_geomeans": rival_geomeans,
                                            "summary": rows, "measurements": measurements, "errors": errors})
    print("\n".join(lines), flush=True)
    print(f"\nReport: {directory / 'summary.md'}", flush=True)
    return complete


def compare(args: argparse.Namespace) -> int:
    if sys.platform != "linux":
        raise BenchError("the native Rust/C shared-library comparison requires Linux")
    directory = run_directory(args.output)
    controls = {name: getattr(args, name) for name in ("samples", "measurement_secs", "warmup_secs", "repeats",
                                                       "app_operations", "app_threads", "app_value_size", "app_timeout")}
    selected = list(SCENARIOS) if args.full else args.scenario or list(QUICK)
    if args.allocator:
        allocators = [ALLOCATOR_NAMES[name] for name in dict.fromkeys(args.allocator)]
    else:
        allocators = list(ALLOCATORS) + list(dict.fromkeys(args.rival or []))
    rival_names = [a for a in allocators if a in rivals.NAMES]
    manifest = run_manifest(scenarios=selected, controls=controls, allocators=allocators,
                            process_layout="isolated-scenario-allocator", workers=[])
    path = directory / "manifest.json"
    write_json(path, manifest)
    print(f"Results: {directory}\nScenarios: {', '.join(selected)}\nControls: {controls}", flush=True)
    print("Measurement seconds are a target, not a cap on fixed-duration workloads.", flush=True)
    with run_lifecycle(path, manifest, manifest["workers"]):
        artifacts = build(args.jobs)
        if rival_names:
            manifest["rival_artifacts"] = rivals.build(rival_names, artifacts, args.jobs, sys.modules[__name__])
            artifacts["rivals"] = manifest["rival_artifacts"]["libraries"]
        manifest["artifacts"] = artifacts
        copy_build_logs(artifacts, directory)
        libraries = {ALLOCATORS[0]: artifacts["rust_library"], ALLOCATORS[1]: artifacts["c_library"],
                     **artifacts.get("rivals", {})}
        app_selected = [s for s in selected if s in applications.SCENARIOS]
        if app_selected:
            manifest["application_artifacts"] = applications.build(app_selected, artifacts, args.jobs, sys.modules[__name__])
            write_json(directory / "application-artifacts.json", manifest["application_artifacts"])
            app_cache = Path(artifacts["cache"]) / "applications"
            for log in app_cache.glob("*.log"):
                shutil.copy2(log, directory / ("application-" + log.name))
        listing = execute([artifacts["executable"], "--bench", "--list"], directory / "list.log",
                          worker_environment(artifacts, args, directory / "listing", ",".join(NATIVE_SCENARIOS),
                                             ",".join(allocators)))
        unavailable = [s for s in selected if s in NATIVE_SCENARIOS and not all(full_id(s, a) + ": benchmark" in listing for a in allocators)]
        if unavailable and (not args.full or set(unavailable) - {"cache-scratchN", "cache-thrashN"}):
            raise BenchError("scenarios unavailable on this host: " + ", ".join(unavailable))
        manifest["unavailable"] = unavailable
        manifest["scenarios"] = [s for s in selected if s not in unavailable]
        manifest["status"] = "running"
        write_json(path, manifest)
        for repeat in range(1, args.repeats + 1):
            # Rotate the start allocator each round (alternates Rust/C when there are two).
            shift = (repeat - 1) % len(allocators)
            order = allocators[shift:] + allocators[:shift]
            for scenario in manifest["scenarios"]:
                for allocator in order:
                    relative = f"round-{repeat:03}/{scenario}/{allocator}"
                    worker = {"directory": relative, "started_utc": utc(),
                              "command": [artifacts["executable"], "--bench", "mimalloc-bench-native", "--noplot"]}
                    environment = worker_environment(artifacts, args, directory / relative / "criterion", scenario, allocator)
                    if scenario in applications.SCENARIOS:
                        config = {"scenario": scenario, "samples": args.samples,
                                  "operations": args.app_operations, "threads": args.app_threads,
                                  "value_size": args.app_value_size, "timeout": args.app_timeout,
                                  "library": libraries[allocator],
                                  "apps": manifest["application_artifacts"]}
                        config_path = directory / relative / "config.json"
                        config_path.parent.mkdir(parents=True, exist_ok=True)
                        write_json(config_path, config)
                        worker["command"] = [sys.executable, str(ROOT / "benchmarks/applications.py"),
                                             str(config_path), str(directory / relative)]
                        environment = without_preload()
                        worker["application_config"] = config
                    worker["environment"] = {k: v for k, v in environment.items()
                                             if k.startswith("MIMALLOC_") or k == "CRITERION_HOME"}
                    manifest["workers"].append(worker)
                    write_json(path, manifest)
                    print(f"\nRound {repeat}/{args.repeats}: {scenario} / {allocator}", flush=True)
                    execute(worker["command"], directory / relative / "worker.log", environment)
                    worker.update(exit_status=0, finished_utc=utc())
                    if scenario in applications.SCENARIOS:
                        applications.measurement(directory / relative / "application.json", scenario, args.samples)
                    else:
                        validate_sample(directory / relative / "criterion", full_id(scenario, allocator), args.samples)
                    write_json(path, manifest)
    complete = report(directory)
    if manifest.get("error"):
        print(manifest["error"], file=sys.stderr)
    return 0 if complete else 130 if manifest["status"] == "interrupted" else 1


def baseline_library(revision: str, jobs: int) -> Path:
    """A Rust library built from `revision` with the runner's flags, cached by commit."""
    commit = capture(["git", "rev-parse", "--verify", revision + "^{commit}"])
    cache = ROOT / "target/bench-screen" / commit
    library = cache / "libmechanical_mimalloc.so"
    if library.is_file():
        return library
    cache.mkdir(parents=True, exist_ok=True)
    worktree = cache / "src"
    shutil.rmtree(worktree, ignore_errors=True)
    subprocess.run(["git", "worktree", "add", "--detach", "--force", str(worktree), commit], cwd=ROOT,
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        environment = {**os.environ, "CARGO_TARGET_DIR": str(ROOT / "target/bench-screen/target")}
        output = execute(["cargo", "rustc", "--locked", "--release", "--features", "interpose", "--lib",
                          "--jobs", str(jobs), "--message-format=json-render-diagnostics", "--crate-type", "cdylib"],
                         cache / "build.log", environment, worktree)
        shutil.copy2(cargo_artifact(output, "mechanical_mimalloc", shared=True), library)
    finally:
        subprocess.run(["git", "worktree", "remove", "--force", str(worktree)], cwd=ROOT,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return library


def entry_offsets(library: Path) -> dict[str, int]:
    """Cache-line offsets of the hand-written fast paths; timing is sensitive to them."""
    symbols = dict(line.split()[::-2] for line in capture(["nm", "-D", "--defined-only", str(library)]).splitlines()
                   if len(line.split()) == 3)
    return {name: int(symbols[name], 16) % 64 for name in ("malloc", "free") if name in symbols}


def screen_verdict(ratios: list[float], weak: float = 0.005, strong: float = 0.02) -> str:
    """Direction needs all but one paired round to agree. Calibration against held-out runs
    found single scenarios move up to ~1.5% from noise, so moves under `strong` are only "likely"."""
    median = statistics.median(ratios)
    for word, moved, agree in (("faster", 1 - median, sum(r < 1 for r in ratios)),
                               ("SLOWER", median - 1, sum(r > 1 for r in ratios))):
        if moved > weak and agree >= len(ratios) - 1:
            return word if moved >= strong else "likely " + word.lower()
    return "~"


def screen(args: argparse.Namespace) -> int:
    artifacts = build(args.jobs)
    candidate = Path(args.candidate).resolve() if args.candidate else Path(artifacts["rust_library"])
    baseline = Path(args.baseline).resolve() if Path(args.baseline).is_file() else baseline_library(args.baseline, args.jobs)
    libraries = {"baseline": baseline, "candidate": candidate}
    scenarios = args.scenario or list(CANARIES)
    offsets = {label: entry_offsets(path) for label, path in libraries.items()}
    print(f"baseline {baseline}\ncandidate {candidate}\nfast-path cache-line offsets: {offsets}", flush=True)
    if offsets["baseline"] != offsets["candidate"]:
        print("warning: fast-path layout differs; fast-path-bound scenarios may move from placement alone", flush=True)
    times: dict[str, dict[str, list[float]]] = {s: {label: [] for label in libraries} for s in scenarios}
    for round_index in range(args.rounds):
        for scenario in scenarios:
            order = list(libraries) if round_index % 2 == 0 else list(libraries)[::-1]
            for label in order:
                environment = {k: v for k, v in without_preload().items() if not k.startswith("MIMALLOC_BENCH_")}
                environment.update(MIMALLOC_RUST_LIB=str(libraries[label]), MIMALLOC_C_LIB=artifacts["c_library"],
                                   MIMALLOC_BENCH_SKIP_BUILD="1", MIMALLOC_BENCH_SCENARIOS=scenario,
                                   MIMALLOC_BENCH_ALLOCATORS=ALLOCATORS[0],
                                   MIMALLOC_BENCH_QUICK_RUNS="1" if scenario in TIMED else str(args.runs + 1),
                                   MIMALLOC_BENCH_TIMED_SECS=str(args.timed_secs))
                output = subprocess.run([artifacts["executable"], "--bench", "mimalloc-bench-native"], env=environment,
                                        cwd=ROOT, check=True, capture_output=True, text=True, timeout=300).stdout
                lines = [json.loads(line[6:]) for line in output.splitlines() if line.startswith("QUICK {")]
                if len(lines) != (1 if scenario in TIMED else args.runs + 1):
                    raise BenchError(f"{scenario}: unexpected QUICK result count {len(lines)}")
                # The first short run is a cold-start warmup (arena setup, first-touch faults).
                measured = lines if scenario in TIMED else lines[1:]
                times[scenario][label].append(statistics.median(line["ns"] for line in measured))
        print(f"round {round_index + 1}/{args.rounds} done", flush=True)
    rows, logs = [], []
    for scenario, values in times.items():
        ratios = [c / b for b, c in zip(values["baseline"], values["candidate"])]
        rows.append({"scenario": scenario, "median_ratio": statistics.median(ratios),
                     "verdict": screen_verdict(ratios), "ratios": ratios})
        logs.append(math.log(statistics.median(ratios)))
    print(f"\n{'scenario':16} {'candidate vs baseline':>22}  {'verdict':14} per-round")
    for row in rows:
        per_round = " ".join(f"{(r - 1) * 100:+.1f}" for r in row["ratios"])
        print(f"{row['scenario']:16} {(row['median_ratio'] - 1) * 100:+21.2f}%  {row['verdict']:14} {per_round}")
    geomean = math.exp(statistics.mean(logs))
    print(f"\ncanary geomean {(geomean - 1) * 100:+.2f}%  (screening only; confirm wins with a held-out run)")
    (ROOT / "target/bench-screen").mkdir(parents=True, exist_ok=True)
    write_json(ROOT / "target/bench-screen/last.json", {"baseline": str(baseline), "candidate": str(candidate),
                                                          "offsets": offsets, "rows": rows, "geomean": geomean})
    return 1 if any(row["verdict"] == "SLOWER" for row in rows) else 0


def minimum(value: str, lower: int = 1) -> int:
    number = int(value)
    if number < lower:
        raise argparse.ArgumentTypeError(f"must be at least {lower}")
    return number


def parse_args(argv: list[str]) -> argparse.Namespace:
    if not argv or (argv[0].startswith("-") and argv[0] not in ("-h", "--help")):
        argv = ["run", *argv]
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="build, compare Rust/C, and report (default)")
    selection = run.add_mutually_exclusive_group()
    selection.add_argument("--full", action="store_true", help="all native scenarios plus Redis and RocksDB (downloads and builds them)")
    selection.add_argument("--scenario", action="append", choices=sorted(SCENARIOS), help="select a case; repeat for more")
    allocator_selection = run.add_mutually_exclusive_group()
    allocator_selection.add_argument("--rival", action="append", choices=rivals.NAMES,
                                     help="also measure this allocator (builds pinned jemalloc/tcmalloc); repeat for more")
    allocator_selection.add_argument("--allocator", action="append", choices=list(ALLOCATOR_NAMES),
                                     help="measure only these allocators (default: rust and mimalloc); repeat for more")
    run.add_argument("--samples", type=lambda v: minimum(v, 10), default=10)
    run.add_argument("--measurement-secs", type=minimum, default=3)
    run.add_argument("--warmup-secs", type=minimum, default=1)
    run.add_argument("--repeats", type=minimum, default=1)
    run.add_argument("--app-operations", type=minimum, default=1000000, help="Redis requests per command / RocksDB writes per thread")
    run.add_argument("--app-threads", type=minimum, default=1, help="Redis client / RocksDB worker threads")
    run.add_argument("--app-value-size", type=minimum, default=256, help="application value bytes")
    run.add_argument("--app-timeout", type=minimum, default=120, help="timeout in seconds per application command")
    run.add_argument("--output", help="new or empty results directory; relative paths start at the repository root")
    nginx_parser = sub.add_parser("nginx", help="compare Nginx production/debug-palloc HTTP workloads")
    nginx.add_arguments(nginx_parser, minimum, default_jobs())
    webserver_parser = sub.add_parser(
        "webserver", help="TechEmpower axum json/plaintext/fortunes per allocator (not in the default suites)")
    webserver.add_arguments(webserver_parser, minimum, default_jobs())
    architectural = sub.add_parser("architecture", help="compare C++ architectural probes against Rust/C")
    architectural.add_argument("--scenario", action="append", choices=architecture.SCENARIOS)
    architectural.add_argument("--bytes", type=minimum, default=64 * 1024 * 1024, help="requested live payload bytes")
    architectural.add_argument("--block-size", type=minimum, default=4096)
    architectural.add_argument("--threads", type=minimum, default=4)
    architectural.add_argument("--cycles", type=minimum, default=8)
    architectural.add_argument("--repeats", type=minimum, default=3)
    architectural.add_argument("--output", help="new or empty results directory")
    build_parser = sub.add_parser("build", help="incrementally build the native comparison artifacts")
    build_parser.add_argument("--nginx", action="store_true", help="also fetch/build Nginx production/debug-palloc and wrk")
    build_parser.add_argument("--applications", action="store_true", help="also fetch/build pinned Redis and RocksDB")
    for command in (run, build_parser, architectural):
        command.add_argument("--jobs", type=minimum, default=default_jobs())
    screen_parser = sub.add_parser("screen", help="~2 min A/B of a candidate Rust library against a baseline on canary scenarios")
    screen_parser.add_argument("--baseline", default="HEAD", help="git revision (built and cached) or path to a Rust .so")
    screen_parser.add_argument("--candidate", help="Rust .so to test (default: build the working tree)")
    screen_parser.add_argument("--scenario", action="append", choices=sorted(NATIVE_SCENARIOS), help="override the canary set")
    screen_parser.add_argument("--rounds", type=lambda v: minimum(v, 3), default=5)
    screen_parser.add_argument("--runs", type=minimum, default=3, help="in-process runs per short scenario (median)")
    screen_parser.add_argument("--timed-secs", type=float, default=1.0, help="run time of xmalloc/larson workloads")
    screen_parser.add_argument("--jobs", type=minimum, default=default_jobs())
    sub.add_parser("list", help="list native and application scenarios; N uses the harness's host-derived thread count")
    report_parser = sub.add_parser("report", help="regenerate a saved comparison without running benchmarks")
    report_parser.add_argument("directory")
    for gate in GATES:
        sub.add_parser(gate, help=f"run the existing {gate} gate with its existing environment controls")
    args = parser.parse_args(argv)
    if args.command == "run" and args.scenario:
        args.scenario = list(dict.fromkeys(args.scenario))
    return args


def interrupt(signum: int, frame: object) -> None:
    raise KeyboardInterrupt(signal.Signals(signum).name)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    # Cancellation or a closed terminal must stop worker process groups like Ctrl-C does.
    for signum in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, interrupt)
    try:
        if args.command == "nginx":
            return nginx.run(args, sys.modules[__name__])
        if args.command == "webserver":
            return webserver.run(args, sys.modules[__name__])
        if args.command == "architecture":
            return architecture.run(args, sys.modules[__name__])
        if args.command == "run":
            return compare(args)
        if args.command == "screen":
            return screen(args)
        if args.command == "build":
            artifacts = build(args.jobs)
            if args.applications:
                artifacts["applications"] = applications.build(list(applications.SCENARIOS), artifacts, args.jobs,
                                                             sys.modules[__name__])
            if args.nginx:
                artifacts["nginx"] = nginx.build(artifacts, ("production", "debug-palloc"), args.jobs, sys.modules[__name__])
            print(json.dumps(artifacts, indent=2))
        elif args.command == "report":
            return 0 if report(repo_path(args.directory)) else 1
        elif args.command == "list":
            for scenario, metric in SCENARIOS.items():
                print(f"{scenario:20} {metric:30} {'quick' if scenario in QUICK else ''}")
            print("cache-scratchN and cache-thrashN require more than one available CPU.")
        else:
            log = ROOT / "target/bench-runs" / f"{args.command}-{uuid.uuid4().hex[:8]}.log"
            execute([str(ROOT / GATES[args.command])], log)
        return 0
    except BenchError as error:
        print(f"error: {error}", file=sys.stderr)
        return error.code
    except KeyboardInterrupt:
        return 130
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
