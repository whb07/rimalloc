"""Pinned Redis/RocksDB builds and isolated application benchmark workers."""
import contextlib
import csv
import fcntl
import io
import json
import math
import os
from pathlib import Path
import re
import shlex
import shutil
import socket
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request

from common import file_sha256, write_json

SCENARIOS = {"redis": "per-redis-request", "rocksdb": "per-rocksdb-write"}
SOURCES = {
    "redis": ("6.2.7", "https://download.redis.io/releases/redis-6.2.7.tar.gz",
              "b7a79cc3b46d3c6eb52fa37dde34a4a60824079ebdfb3abfbbfa035947c55319"),
    "rocksdb": ("10.10.1", "https://codeload.github.com/facebook/rocksdb/tar.gz/refs/tags/v10.10.1",
                "df2ff348f3fac8578fd4b727eee7267aaf90cd403c99b55e898d1db63fa8cff5"),
    "gflags": ("2.2.2", "https://codeload.github.com/gflags/gflags/tar.gz/refs/tags/v2.2.2",
               "34af2f15cf7367513b352bdcd2493ab14ce43692d2dcd9dfc499492966c64dcf"),
}
ROCKS_OPTIONS = {
    "CMAKE_BUILD_TYPE": "Release", "PORTABLE": "1", "WITH_TESTS": "OFF", "WITH_TOOLS": "OFF",
    "WITH_BENCHMARK_TOOLS": "ON", "ROCKSDB_BUILD_SHARED": "OFF", "WITH_GFLAGS": "ON",
    "WITH_JEMALLOC": "OFF", "WITH_LIBURING": "OFF", "WITH_SNAPPY": "OFF", "WITH_ZLIB": "OFF",
    "WITH_BZ2": "OFF", "WITH_LZ4": "OFF", "WITH_ZSTD": "OFF", "FAIL_ON_WARNINGS": "OFF",
}


@contextlib.contextmanager
def locked(path):
    """Serialize concurrent runs that share a download or copy destination."""
    with path.open("w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        yield


def source(name, root, spec=None):
    version, url, digest = spec or SOURCES[name]
    root.mkdir(parents=True, exist_ok=True)
    with locked(root / f".{name}.lock"):
        return fetch(name, root, version, url, digest)


def fetch(name, root, version, url, digest):
    extension = ".tar.bz2" if url.endswith(".tar.bz2") else ".tar.gz"
    archive = root / f"{name}-{version}{extension}"
    if not archive.exists():
        print(f"Downloading {name} {version}: {url}", flush=True)
        temporary = archive.with_suffix(".download")
        try:
            with urllib.request.urlopen(url, timeout=60) as response, temporary.open("wb") as output:
                shutil.copyfileobj(response, output)
            if file_sha256(temporary) != digest:
                raise ValueError(f"source checksum mismatch: {name}")
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    elif file_sha256(archive) != digest:
        raise ValueError(f"source checksum mismatch: {archive}; remove it and retry")
    directory = root / f"{name}-{version}"
    if not directory.exists():
        # The archive bytes are checked against the pinned digest before extraction.
        # Extract beside the target and rename so an interrupted run never leaves a partial tree.
        staging = Path(tempfile.mkdtemp(dir=root, prefix=f".{directory.name}-"))
        try:
            with tarfile.open(archive) as package:
                package.extractall(staging)
            (staging / directory.name).rename(directory)
        finally:
            shutil.rmtree(staging, ignore_errors=True)
    return directory


def copy_tree(source_directory, destination):
    """Copy a source tree once, renaming into place so an interrupted copy is never reused."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    with locked(destination.with_name(f".{destination.name}.lock")):
        if destination.exists():
            return
        staging = destination.with_name(f".{destination.name}.partial")
        shutil.rmtree(staging, ignore_errors=True)
        shutil.copytree(source_directory, staging)
        staging.rename(destination)


def build(selected, artifacts, jobs, driver):
    if shutil.which("make") is None:
        raise driver.BenchError("application benchmarks require GNU make")
    cache = Path(artifacts["cache"]) / "applications"
    cache.mkdir(parents=True, exist_ok=True)
    sources = driver.ROOT / "target/bench-app-sources"
    result = {"sources": {}, "build_commands": [], "rocksdb_options": ROCKS_OPTIONS}
    def execute(command, log):
        result["build_commands"].append(command)
        driver.execute(command, cache / log)
    for name in selected:
        version, url, digest = SOURCES[name]
        result["sources"][name] = {"version": version, "url": url, "sha256": digest}
        directory = source(name, sources)
        if name == "redis":
            local = cache / f"redis-{version}"
            copy_tree(directory, local)
            execute(["make", "-C", str(local / "src"), "-j", str(jobs), "MALLOC=libc", "USE_JEMALLOC=no",
                     "BUILD_TLS=no", "OPTIMIZATION=-O2", "redis-server", "redis-benchmark"], "redis-build.log")
            result.update(redis_server=str(local / "src/redis-server"), redis_benchmark=str(local / "src/redis-benchmark"))
        else:
            gflags_source = source("gflags", sources)
            result["sources"]["gflags"] = dict(zip(("version", "url", "sha256"), SOURCES["gflags"]))
            prefix = cache / "gflags-install"
            execute(["cmake", "-S", str(gflags_source), "-B", str(cache / "gflags"),
                     "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF", "-DBUILD_TESTING=OFF",
                     "-DREGISTER_BUILD_DIR=OFF", "-DREGISTER_INSTALL_PREFIX=OFF",
                     f"-DCMAKE_INSTALL_PREFIX={prefix}"], "gflags-configure.log")
            execute(["cmake", "--build", str(cache / "gflags"), "--parallel", str(jobs)], "gflags-build.log")
            execute(["cmake", "--install", str(cache / "gflags")], "gflags-install.log")
            execute(["cmake", "-S", str(directory), "-B", str(cache / "rocksdb"),
                     f"-DCMAKE_PREFIX_PATH={prefix}"] + [f"-D{k}={v}" for k, v in ROCKS_OPTIONS.items()],
                    "rocksdb-configure.log")
            execute(["cmake", "--build", str(cache / "rocksdb"), "--target", "db_bench", "--parallel", str(jobs)],
                    "rocksdb-build.log")
            result["rocksdb"] = str(cache / "rocksdb/db_bench")
    check = cache / "allocator-check.so"
    execute(shlex.split(os.environ.get("CC", "cc")) + ["-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror",
            str(driver.ROOT / "benchmarks/allocator-check.c"), "-ldl", "-o", str(check)], "check-build.log")
    result["allocator_check"] = str(check)
    result["sha256"] = {k: file_sha256(v) for k, v in result.items()
                        if k in ("redis_server", "redis_benchmark", "rocksdb", "allocator_check")}
    driver.write_json(cache / "artifacts.json", result)
    return result


def parse_redis(output):
    rows = list(csv.DictReader(io.StringIO(output)))
    if [r.get("test") for r in rows] != ["SET", "GET"]:
        raise ValueError("Redis did not report exactly SET and GET")
    result = []
    for row in rows:
        values = {key: float(row[key]) for key in ("rps", "avg_latency_ms", "p50_latency_ms", "p95_latency_ms", "p99_latency_ms")}
        if math.isinf(values["rps"]):
            raise ValueError("redis-benchmark finished too quickly to measure throughput; increase --app-operations")
        if not all(math.isfinite(v) and v >= 0 for v in values.values()) or values["rps"] <= 0:
            raise ValueError("invalid Redis throughput/latency")
        result.append({"command": row["test"], **values})
    return result


def parse_rocksdb(output):
    matches = re.findall(r"^fillrandom\s+:\s+([\d.]+) micros/op\s+([\d.]+) ops/sec", output, re.MULTILINE)
    if len(matches) != 1:
        raise ValueError("RocksDB did not report exactly one fillrandom result")
    micros, rps = map(float, matches[0])
    if not all(math.isfinite(v) and v > 0 for v in (micros, rps)):
        raise ValueError("invalid RocksDB throughput")
    return {"command": "fillrandom", "micros_per_op": micros, "rps": rps}


def request(sock_path, *args):
    data = f"*{len(args)}\r\n".encode()
    for arg in args:
        raw = str(arg).encode()
        data += f"${len(raw)}\r\n".encode() + raw + b"\r\n"
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(2)
        client.connect(str(sock_path))
        client.sendall(data)
        reply = client.makefile("rb").readline()
    if not reply or reply.startswith(b"-"):
        raise ValueError(f"Redis command failed: {reply!r}")
    return reply


def run_command(command, log, environment, timeout, commands):
    commands.append({"command": command, "log": str(log), "preloaded": bool(environment.get("LD_PRELOAD"))})
    with log.open("w") as output:
        subprocess.run(command, env=environment, stdout=output, stderr=subprocess.STDOUT, timeout=timeout, check=True)
    return log.read_text()


def confirmed(marker, pid):
    if str(pid) not in marker.read_text().splitlines():
        raise ValueError("application did not confirm allocator ownership")


def redis_sample(config, directory, environment, commands):
    # Short socket paths avoid sockaddr_un's 108-byte limit, even in long output paths.
    with tempfile.TemporaryDirectory(prefix="mi-redis-") as name:
        sock = Path(name) / "redis.sock"
        command = [config["apps"]["redis_server"], "--port", "0", "--unixsocket", str(sock),
                   "--unixsocketperm", "700", "--save", "", "--appendonly", "no",
                   "--daemonize", "no", "--protected-mode", "yes", "--dir", name]
        commands.append({"command": command, "log": str(directory / "server.log"), "preloaded": True})
        with (directory / "server.log").open("w") as log:
            server = subprocess.Popen(command, env=environment, stdout=log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + min(15, config["timeout"])
                while True:
                    if server.poll() is not None:
                        raise ValueError(f"Redis exited {server.returncode}; see {directory / 'server.log'}")
                    try:
                        if request(sock, "PING") == b"+PONG\r\n":
                            break
                    except (OSError, ValueError):
                        pass
                    if time.monotonic() >= deadline:
                        raise TimeoutError("Redis readiness timed out")
                    time.sleep(0.02)
                confirmed(Path(environment["BENCH_ALLOCATOR_CHECK"]), server.pid)
                client_env = {k: v for k, v in environment.items() if k not in
                              ("LD_PRELOAD", "BENCH_ALLOCATOR_CHECK", "BENCH_ALLOCATOR_EXPECTED")}
                # Warm the command paths, then discard warmup data. Persistence is disabled.
                base = [config["apps"]["redis_benchmark"], "-s", str(sock), "-t", "set,get", "--csv",
                        "-d", str(config["value_size"]), "-r", str(config["operations"]), "-c", "50", "-P", "16",
                        "--threads", str(config["threads"])]
                run_command(base + ["-n", "1000"], directory / "warmup.log", client_env, config["timeout"], commands)
                request(sock, "FLUSHALL")
                output = run_command(base + ["-n", str(config["operations"])], directory / "client.log",
                                     client_env, config["timeout"], commands)
                components = parse_redis(output)
                info = request_info(sock)
                return {"normalized_ns": statistics.mean(1e9 / c["rps"] for c in components),
                        "components": components, "server_info": info, "allocator_pid": server.pid, "allocator_verified": True}
            finally:
                if server.poll() is None:
                    server.terminate()
                    try:
                        server.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        server.kill(); server.wait()
                if server.returncode not in (0, -15) and sys.exc_info()[0] is None:
                    raise ValueError(f"Redis shutdown failed: {server.returncode}")


def request_info(sock):
    # Read a bulk INFO response without truncating it to the first protocol line.
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(2); client.connect(str(sock))
        client.sendall(b"*2\r\n$4\r\nINFO\r\n$6\r\nmemory\r\n")
        with client.makefile("rb") as stream:
            header = stream.readline()
            if not header.startswith(b"$"):
                raise ValueError("invalid Redis INFO response")
            data = stream.read(int(header[1:]))
    info = dict(line.split(":", 1) for line in data.decode().splitlines() if ":" in line)
    return {k: info[k] for k in ("used_memory", "used_memory_rss", "used_memory_peak", "mem_allocator")}


def rocksdb_sample(config, directory, environment, commands):
    database = directory / "db"
    command = [config["apps"]["rocksdb"], "--benchmarks=fillrandom", f"--num={config['operations']}",
               f"--value_size={config['value_size']}", f"--threads={config['threads']}", "--seed=12345",
               "--compression_type=none", "--disable_wal=0", "--sync=0", "--use_existing_db=0", f"--db={database}"]
    try:
        output = run_command(command, directory / "db_bench.log", environment, config["timeout"], commands)
        component = parse_rocksdb(output)
        markers = Path(environment["BENCH_ALLOCATOR_CHECK"]).read_text().splitlines()
        if len(markers) != 1:
            raise ValueError("RocksDB did not confirm exactly one allocator-owned process")
        return {"normalized_ns": 1e9 / component["rps"], "components": [component], "allocator_pid": int(markers[0]), "allocator_verified": True}
    finally:
        shutil.rmtree(database, ignore_errors=True)


def worker(config, directory):
    directory.mkdir(parents=True, exist_ok=True)
    environment = {k: v for k, v in os.environ.items() if k != "LD_PRELOAD"}
    environment.update(LD_PRELOAD=f"{config['library']}:libstdc++.so.6:{config['apps']['allocator_check']}",
                       BENCH_ALLOCATOR_EXPECTED=config["library"])
    result = {"schema": 1, "scenario": config["scenario"], "controls": config, "samples": [], "commands": []}
    try:
        for index in range(config["samples"] + (config["scenario"] == "rocksdb")):
            warmup = config["scenario"] == "rocksdb" and index == 0
            sample_dir = directory / ("warmup" if warmup else f"sample-{index:03}")
            sample_dir.mkdir()
            environment["BENCH_ALLOCATOR_CHECK"] = str(sample_dir / "allocator-owned.txt")
            sample = (redis_sample if config["scenario"] == "redis" else rocksdb_sample)(
                config, sample_dir, environment, result["commands"])
            if not warmup:
                result["samples"].append(sample)
            write_json(directory / "application.json", result)
    finally:
        write_json(directory / "application.json", result)


def measurement(path, scenario, samples):
    result = json.loads(path.read_text())
    if result["scenario"] != scenario or len(result["samples"]) != samples:
        raise ValueError("wrong application scenario or sample count")
    for sample in result["samples"]:
        if sample["allocator_verified"] is not True or type(sample["allocator_pid"]) is not int or sample["allocator_pid"] <= 0:
            raise ValueError("application allocator ownership not verified")
        expected = ["SET", "GET"] if scenario == "redis" else ["fillrandom"]
        if [c["command"] for c in sample["components"]] != expected:
            raise ValueError("wrong application components")
        for component in sample["components"]:
            rate = component["rps"]
            if type(rate) not in (float, int) or not math.isfinite(rate) or rate <= 0:
                raise ValueError("invalid application throughput")
            if scenario == "redis":
                for key in ("avg_latency_ms", "p50_latency_ms", "p95_latency_ms", "p99_latency_ms"):
                    value = component[key]
                    if type(value) not in (float, int) or not math.isfinite(value) or value < 0:
                        raise ValueError("invalid Redis latency")
        derived = statistics.mean(1e9 / c["rps"] for c in sample["components"])
        if type(sample["normalized_ns"]) not in (float, int) or not math.isclose(sample["normalized_ns"], derived, rel_tol=1e-12):
            raise ValueError("application normalized time does not match throughput")
    values = [r["normalized_ns"] for r in result["samples"]]
    if not all(type(v) in (int, float) and math.isfinite(v) and v > 0 for v in values):
        raise ValueError("invalid application timing")
    ordered = sorted(values)
    median = statistics.median(values)
    p95 = ordered[math.ceil(0.95 * len(ordered)) - 1]
    spread = (p95 - ordered[0]) / median * 100
    return {"median": median, "mean": statistics.mean(values), "p95": p95,
            "noisy": spread > 10, "spread_pct": spread, "application": result}


def report_lines(measurements):
    apps = [m for m in measurements if "application" in m]
    if not apps:
        return []
    lines = ["", "## Application measurements", "",
             "Redis's normalized time averages the reciprocal SET and GET throughputs with equal request counts. "
             "RocksDB uses reciprocal aggregate fillrandom throughput. Applications are excluded from the native geomean. "
             "Application p95 in the repetition table describes whole-sample throughput variation. "
             "Redis client latency percentiles are shown separately below. Fixed work counts apply; "
             "Criterion measurement/warmup seconds do not apply to applications.", "",
             "| Scenario / command | Round | Allocator | Median ops/s | Redis client p95 ms |",
             "|---|---:|---|---:|---:|"]
    for m in apps:
        samples = m["application"]["samples"]
        for command in ("SET", "GET") if m["scenario"] == "redis" else ("fillrandom",):
            components = [next(c for c in sample["components"] if c["command"] == command) for sample in samples]
            rps = statistics.median(c["rps"] for c in components)
            p95 = f"{statistics.median(c['p95_latency_ms'] for c in components):.3f}" if command != "fillrandom" else "—"
            lines.append(f"| {m['scenario']} / {command} | {m['repeat']} | {m['allocator']} | {rps:.2f} | {p95} |")
    return lines


if __name__ == "__main__":
    try:
        config = json.loads(Path(sys.argv[1]).read_text())
        worker(config, Path(sys.argv[2]))
    except Exception as error:
        print(f"application benchmark: {error}", file=sys.stderr)
        sys.exit(1)
