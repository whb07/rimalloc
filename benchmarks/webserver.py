"""TechEmpower axum json, plaintext, and fortunes tests under each allocator.

Run explicitly with `bench.py webserver`; it is not part of the default suites. The server
(benches/webserver/axum) uses the system allocator, so the allocator under test is injected with
LD_PRELOAD and confirmed in the server's memory map before any load is applied.
"""
import csv
import hashlib
import http.client
import json
import os
from pathlib import Path
import random
import resource
import shutil
import sqlite3
import statistics
import subprocess
import sys
import time

import applications
import nginx
import rivals
from common import file_sha256, write_json

ALLOCATORS = ("rust", "mimalloc", "glibc", "jemalloc", "tcmalloc")
# TechEmpower test names. Plaintext is pipelined 16 deep, as in the TechEmpower plaintext test.
SCENARIOS = {
    "json": {"binary": "axum", "path": "/json", "pipeline": 1, "accept": "application/json"},
    "plaintext": {"binary": "axum", "path": "/plaintext", "pipeline": 16, "accept": "text/plain"},
    "fortunes": {"binary": "axum-sqlite", "path": "/fortunes", "pipeline": 1, "accept": "text/html"},
}
SOURCE = Path(__file__).resolve().parents[1] / "benches/webserver"
FORTUNE_ROWS = 12  # fortune.sql rows; the server adds one more per request


def build(core, jobs, driver):
    """Build the axum binaries and wrk; returns paths, allocator libraries, and provenance."""
    files = sorted(p for p in SOURCE.rglob("*") if p.is_file() and "target" not in p.relative_to(SOURCE).parts)
    digest = hashlib.sha256(json.dumps(nginx.SOURCES["wrk"]).encode())
    for path in files:
        digest.update(str(path.relative_to(SOURCE)).encode() + b"\0" + path.read_bytes())
    cache = Path(core["cache"]) / "webserver" / digest.hexdigest()[:16]
    cache.mkdir(parents=True, exist_ok=True)
    # As in the TechEmpower axum image.
    environment = {**driver.without_preload(), "RUSTFLAGS": "-C target-cpu=native"}
    command = ["cargo", "build", "--release", "--locked", "--manifest-path", str(SOURCE / "axum/Cargo.toml"),
               "--target-dir", str(cache / "target")]
    driver.execute(command, cache / "axum-build.log", environment)
    wrk = cache / "wrk-source"
    sources = driver.ROOT / "target/bench-app-sources"
    applications.copy_tree(applications.source("wrk", sources, nginx.SOURCES["wrk"]), wrk)
    driver.execute(["make", "-C", str(wrk), "-j", str(jobs), "WITH_OPENSSL=/usr", "VER=4.2.0"], cache / "wrk-build.log")
    binaries = {name: str(cache / "target/release" / name) for name in ("axum", "axum-sqlite")}
    libraries = {"rust": core["rust_library"], "mimalloc": core["c_library"], "glibc": None}
    result = {"binaries": binaries, "wrk": str(wrk / "wrk"), "libraries": libraries, "cache": str(cache),
              "logs": ["axum-build.log", "wrk-build.log"], "rustflags": environment["RUSTFLAGS"],
              "commands": [command], "wrk_source": dict(zip(("version", "url", "sha256"), nginx.SOURCES["wrk"]))}
    result["sha256"] = {name: file_sha256(path) for name, path in {**binaries, "wrk": result["wrk"]}.items()}
    write_json(cache / "artifacts.json", result)
    return result


def create_database(path):
    connection = sqlite3.connect(path)
    try:
        connection.executescript((SOURCE / "fortune.sql").read_text())
        connection.commit()
        count = connection.execute("SELECT count(*) FROM fortune").fetchone()[0]
    finally:
        connection.close()
    if count != FORTUNE_ROWS:
        raise ValueError(f"fortune table has {count} rows, expected {FORTUNE_ROWS}")


def check_response(scenario, status, content_type, body):
    """Raise unless the body is the scenario's correct TechEmpower response."""
    if status != 200:
        raise ValueError(f"{scenario}: HTTP {status}")
    if scenario == "json":
        if not content_type.startswith("application/json") or json.loads(body) != {"message": "Hello, World!"}:
            raise ValueError(f"json: unexpected response {content_type} {body[:80]!r}")
    elif scenario == "plaintext":
        if body != b"Hello, World!":
            raise ValueError(f"plaintext: unexpected body {body[:80]!r}")
    else:
        text = body.decode()
        # Header row plus the stored rows plus the one added at request time; the script row
        # must arrive escaped.
        if (not content_type.startswith("text/html") or text.count("<tr>") != FORTUNE_ROWS + 2
                or "Additional fortune added at request time." not in text
                or "&lt;script&gt;alert(&quot;This should not be displayed" not in text):
            raise ValueError(f"fortunes: unexpected response {content_type} {text[:120]!r}")


def http_check(port, scenario):
    client = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
    try:
        client.request("GET", SCENARIOS[scenario]["path"])
        response = client.getresponse()
        check_response(scenario, response.status, response.getheader("Content-Type", ""), response.read())
    finally:
        client.close()


def check_allocator(pid, library, libraries):
    """The preloaded library must be mapped in the server, and no other allocator library."""
    mapped = {line.split(None, 5)[5] for line in Path(f"/proc/{pid}/maps").read_text().splitlines()
              if len(line.split(None, 5)) == 6}
    if library and os.path.realpath(library) not in mapped:
        raise ValueError(f"allocator library is not mapped in the server: {library}")
    others = [other for other in libraries if other and other != library and os.path.realpath(other) in mapped]
    if others:
        raise ValueError(f"another allocator library is mapped in the server: {others}")


def lua_script(depth):
    """TechEmpower's pipeline.lua, plus the Nginx runner's machine-readable summary."""
    pipeline = f'''
init = function(args)
  local r = {{}}
  for i = 1, {depth} do r[i] = wrk.format() end
  req = table.concat(r)
end
request = function() return req end
''' if depth > 1 else ""
    # Not an Nginx headers/upload workload, so this adds only the summary function.
    return pipeline + nginx.lua_script({"workload": "plain"})


def parse_wrk(output, pipeline):
    if pipeline == 1:
        return nginx.parse_wrk(output)
    # wrk's latency histogram is unreliable with pipelining (high percentiles read 0), so keep
    # throughput and error checks only, as TechEmpower does for plaintext.
    lines = [line.removeprefix("BENCH_JSON ") for line in output.splitlines() if line.startswith("BENCH_JSON ")]
    if len(lines) != 1:
        raise ValueError("wrk did not produce exactly one machine-readable result")
    result = json.loads(lines[0])
    if result["errors"] or result["requests"] <= 0 or result["duration_us"] <= 0:
        raise ValueError(f"wrk incomplete/failed HTTP traffic: {result}")
    result.update(p50_us=None, p95_us=None, p99_us=None,
                  requests_per_second=result["requests"] * 1e6 / result["duration_us"])
    return result


def wrk_run(config, port, seconds, directory, name, pid):
    scenario = SCENARIOS[config["scenario"]]
    lua = directory / f"{name}.lua"
    lua.write_text(lua_script(scenario["pipeline"]))
    command = ["taskset", "-c", ",".join(map(str, config["load_cpus"])), config["artifacts"]["wrk"],
               "-t", str(config["load_threads"]), "-c", str(config["connections"]), "-d", f"{seconds}s", "--latency",
               "-H", f"Accept: {scenario['accept']}", "-H", "Connection: keep-alive", "-s", str(lua),
               f"http://127.0.0.1:{port}{scenario['path']}"]
    before = nginx.memory_cpu([pid]); samples = []
    start = time.monotonic()
    with (directory / f"{name}.log").open("w") as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                   env={k: v for k, v in os.environ.items() if k != "LD_PRELOAD"})
        try:
            while process.poll() is None:
                samples.append({"seconds": time.monotonic() - start, **nginx.memory_cpu([pid])})
                if time.monotonic() > start + seconds + 15:
                    raise TimeoutError("wrk timed out")
                time.sleep(1)
            if process.returncode:
                raise ValueError(f"wrk exited {process.returncode}")
        finally:
            if process.poll() is None:
                process.kill(); process.wait()
    after = nginx.memory_cpu([pid])
    result = parse_wrk((directory / f"{name}.log").read_text(), scenario["pipeline"])
    result.update(command=command, resources=samples,
                  peak_server_pss_bytes=max((s["pss_bytes"] for s in samples), default=after["pss_bytes"]),
                  cpu_seconds_per_request=(after["cpu_seconds"] - before["cpu_seconds"]) / result["requests"])
    return result


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill(); process.wait()


def measure(config, directory):
    """One allocator and scenario: start the server, verify it, warm up, then measure."""
    directory.mkdir(parents=True, exist_ok=True)
    scenario = config["scenario"]
    library = config["artifacts"]["libraries"][config["allocator"]]
    result = {"config": config, "quality_flags": []}
    if config["load_threads"] > len(config["load_cpus"]):
        result["quality_flags"].append("wrk threads exceed assigned generator CPUs")
    environment = {k: v for k, v in os.environ.items() if k not in ("LD_PRELOAD", "PORT", "SQLITE_PATH")}
    if library:
        environment["LD_PRELOAD"] = library
    if scenario == "fortunes":
        environment["SQLITE_PATH"] = config["database"]
    binary = config["artifacts"]["binaries"][SCENARIOS[scenario]["binary"]]
    for attempt in range(3):
        port = nginx.free_port()
        command = ["taskset", "-c", ",".join(map(str, config["server_cpus"])), binary]
        log = (directory / "server.log").open("w")
        process = subprocess.Popen(command, env={**environment, "PORT": str(port)}, stdout=log, stderr=subprocess.STDOUT)
        log.close()
        try:
            deadline = time.monotonic() + 15; last = None
            while True:
                if process.poll() is not None:
                    raise ValueError(f"server exited {process.returncode}: {(directory / 'server.log').read_text()[-500:]}")
                try:
                    http_check(port, scenario); break
                except (OSError, http.client.HTTPException) as error:
                    last = error
                if time.monotonic() > deadline:
                    raise ValueError(f"server readiness timed out; last check: {last}")
                time.sleep(.05)
            break
        except ValueError:
            stop(process)
            if attempt == 2 or "Address already in use" not in (directory / "server.log").read_text():
                raise
    try:
        check_allocator(process.pid, library, config['artifacts']['libraries'].values())
        threads = int(next(line for line in Path(f"/proc/{process.pid}/status").read_text().splitlines()
                           if line.startswith("Threads:")).split()[1])
        result.update(command=command, server_pid=process.pid, server_threads=threads)
        if config["warmup_secs"]:
            result["warmup"] = wrk_run(config, port, config["warmup_secs"], directory, "warmup", process.pid)
        print(f"{config['allocator']} / {scenario}: {config['duration_secs']} s", flush=True)
        result["run"] = wrk_run(config, port, config["duration_secs"], directory, "load", process.pid)
        http_check(port, scenario)  # still serving correct responses after the load
        result["status"] = "complete"
    except BaseException as error:
        result.update(status="failed", error=str(error)); raise
    finally:
        stop(process)
        write_json(directory / "webserver.json", result)
    return result


def report(directory, manifest=None):
    manifest = manifest or json.loads((directory / "manifest.json").read_text())
    measurements = []
    for job in manifest["jobs"]:
        path = directory / job["directory"] / "webserver.json"
        if path.exists():
            result = json.loads(path.read_text())
            if result.get("status") == "complete":
                measurements.append(result)
    rows = []
    for scenario in manifest["scenarios"]:
        for allocator in manifest["allocators"]:
            runs = [m["run"] for m in measurements if (m["config"]["scenario"], m["config"]["allocator"]) == (scenario, allocator)]
            if len(runs) != manifest["controls"]["repeats"]:
                continue
            rows.append({"scenario": scenario, "allocator": allocator,
                         "requests_per_second": statistics.median(r["requests_per_second"] for r in runs),
                         "p99_us": None if runs[0]["p99_us"] is None else statistics.median(r["p99_us"] for r in runs),
                         "peak_server_pss_bytes": statistics.median(r["peak_server_pss_bytes"] for r in runs),
                         "cpu_seconds_per_request": statistics.median(r["cpu_seconds_per_request"] for r in runs)})
    complete = manifest["status"] == "complete" and len(rows) == len(manifest["scenarios"]) * len(manifest["allocators"])
    with (directory / "results.csv").open("w", newline="") as out:
        writer = csv.DictWriter(out, fieldnames=list(rows[0]) if rows else ["scenario"])
        writer.writeheader(); writer.writerows(rows)
    lines = [f"# TechEmpower axum webserver ({'complete' if complete else 'INCOMPLETE'})", "",
             f"Median of {manifest['controls']['repeats']} run(s) of {manifest['controls']['duration_secs']} s each. "
             "Throughput ratio is allocator/Rust; below 1.0 favours Rust. Server PSS is sampled once a second. "
             "Local closed-loop wrk; not fixed-rate latency. Plaintext is pipelined 16 deep, where wrk latency is unreliable.", "",
             "| Scenario | Allocator | Requests/s | vs Rust | HTTP p99 ms | Peak server PSS MiB | Server CPU us/request |",
             "|---|---|---:|---:|---:|---:|---:|"]
    for row in rows:
        rust = next((r["requests_per_second"] for r in rows if r["scenario"] == row["scenario"] and r["allocator"] == "rust"), None)
        ratio = f"{row['requests_per_second'] / rust:.4f}" if rust else "—"
        lines.append(f"| {row['scenario']} | {row['allocator']} | {row['requests_per_second']:.0f} | {ratio} | "
                     f"{'—' if row['p99_us'] is None else format(row['p99_us'] / 1000, '.3f')} | {row['peak_server_pss_bytes'] / 2**20:.2f} | "
                     f"{row['cpu_seconds_per_request'] * 1e6:.2f} |")
    (directory / "summary.md").write_text("\n".join(lines) + "\n")
    print(f"Report: {directory / 'summary.md'}", flush=True)
    return complete


def add_arguments(parser, positive, default_jobs):
    parser.add_argument("--allocator", action="append", choices=ALLOCATORS, help="repeat for more; default: rust and mimalloc")
    parser.add_argument("--scenario", action="append", choices=tuple(SCENARIOS), help="repeat for more; default: all three")
    parser.add_argument("--duration-secs", type=positive, default=30)
    parser.add_argument("--warmup-secs", type=int, default=5)
    parser.add_argument("--connections", type=positive, default=256)
    parser.add_argument("--load-threads", type=positive, help="default: one per load CPU")
    parser.add_argument("--repeats", type=positive, default=1)
    parser.add_argument("--server-cpus", help="comma-separated IDs/ranges; default: half the physical cores")
    parser.add_argument("--load-cpus", help="default: the other half")
    parser.add_argument("--order-seed", type=int, default=12345)
    parser.add_argument("--jobs", type=positive, default=default_jobs)
    parser.add_argument("--output", help="new or empty results directory")


def cpu_sets(args, allowed):
    physical = []; seen = set()
    for cpu in allowed:
        topology = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        identity = ((topology / "physical_package_id").read_text(), (topology / "core_id").read_text())
        if identity not in seen:
            physical.append(cpu); seen.add(identity)
    automatic = physical if len(physical) >= 2 else allowed
    half = len(automatic) // 2
    return {"server_cpus": nginx.cpu_set(args.server_cpus) if args.server_cpus else automatic[:half],
            "load_cpus": nginx.cpu_set(args.load_cpus) if args.load_cpus else automatic[half:2 * half]}


def run(args, driver):
    allowed = sorted(os.sched_getaffinity(0))
    if len(allowed) < 2:
        raise driver.BenchError("the webserver benchmark needs two disjoint CPU sets")
    sets = cpu_sets(args, allowed)
    if any(not cpus or set(cpus) - set(allowed) for cpus in sets.values()) or set(sets["server_cpus"]) & set(sets["load_cpus"]):
        raise driver.BenchError("CPU sets must be available, nonempty, and disjoint")
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    if soft < 65536:
        if hard < 65536:
            raise driver.BenchError("raise the file descriptor hard limit to at least 65536")
        resource.setrlimit(resource.RLIMIT_NOFILE, (65536, hard))
    directory = driver.run_directory(args.output, "webserver-")
    allocators = list(dict.fromkeys(args.allocator or ("rust", "mimalloc")))
    scenarios = list(dict.fromkeys(args.scenario or SCENARIOS))
    controls = {"duration_secs": args.duration_secs, "warmup_secs": args.warmup_secs, "connections": args.connections,
                "load_threads": args.load_threads or len(sets["load_cpus"]), "repeats": args.repeats,
                "order_seed": args.order_seed, **sets}
    if controls["connections"] < controls["load_threads"]:
        raise driver.BenchError("connections must be at least load threads")
    manifest = driver.run_manifest(suite="webserver", controls=controls, allocators=allocators, scenarios=scenarios,
                                   jobs=[], expected_jobs=len(allocators) * len(scenarios) * args.repeats)
    path = directory / "manifest.json"; driver.write_json(path, manifest)
    print(f"Results: {directory}; CPU sets: {sets}; {manifest['expected_jobs']} runs of {args.duration_secs} s", flush=True)
    with driver.run_lifecycle(path, manifest, manifest["jobs"]):
        core = driver.build(args.jobs, harness=False); manifest["core_artifacts"] = core
        artifacts = build(core, args.jobs, driver)
        built = [a for a in allocators if a in ("jemalloc", "tcmalloc")]
        if built:
            manifest["rival_artifacts"] = rivals.build(built, core, args.jobs, driver)
            artifacts["libraries"].update(manifest["rival_artifacts"]["libraries"])
        manifest["artifacts"] = artifacts
        driver.copy_build_logs(core, directory, "core-")
        for log in artifacts["logs"]:
            shutil.copy2(Path(artifacts["cache"]) / log, directory / log)
        database = directory / "fortune.db"
        create_database(database)
        runtime = {key: artifacts[key] for key in ("binaries", "wrk", "libraries")}
        manifest["status"] = "running"; driver.write_json(path, manifest)
        generator = random.Random(args.order_seed)
        for repeat in range(1, args.repeats + 1):
            jobs = [(s, a) for s in scenarios for a in allocators]
            generator.shuffle(jobs)
            for scenario, allocator in jobs:
                relative = f"round-{repeat:03}/{scenario}/{allocator}"
                config = {**controls, "artifacts": runtime, "scenario": scenario, "allocator": allocator,
                          "database": str(database)}
                job = {"repeat": repeat, "directory": relative, "started_utc": driver.utc()}
                manifest["jobs"].append(job); driver.write_json(path, manifest)
                print(f"Round {repeat}/{args.repeats}: {scenario} / {allocator}", flush=True)
                measure(config, directory / relative)
                job.update(exit_status=0, finished_utc=driver.utc()); driver.write_json(path, manifest)
    if manifest.get("error"):
        print(manifest["error"], file=sys.stderr)
    return 0 if report(directory, manifest) else manifest.get("exit_status", 1)
