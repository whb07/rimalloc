"""Nginx application benchmarks; production and debug-palloc remain separate."""
import csv
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import random
import resource
import shlex
import shutil
import signal
import statistics
import struct
import subprocess
import sys
import time

import applications
from common import write_json

WORKLOADS = ("static", "churn", "headers", "proxy", "transfer", "burst-idle")
ALLOCATORS = ("rust", "mimalloc", "glibc")
SOURCES = {
    "nginx": ("1.28.0", "https://nginx.org/download/nginx-1.28.0.tar.gz", "c6b5c6b086c0df9d3ca3ff5e084c1d0ef909e6038279c71c1c3e985f576ff76a"),
    "wrk": ("4.2.0", "https://codeload.github.com/wg/wrk/tar.gz/refs/tags/4.2.0", "e255f696bff6e329f5d19091da6b06164b8d59d62cb9e673625bdcd27fe7bdad"),

}
WRAPPED = ("malloc", "calloc", "realloc", "free", "posix_memalign")


def build(artifacts, profiles, jobs, driver):
    sources = driver.ROOT / "target/bench-app-sources"
    local_sources = {name: applications.source(name, sources, SOURCES[name]) for name in ("nginx", "wrk")}
    implementation = driver.ROOT / "benches/nginx"
    key = hashlib.sha256(json.dumps(SOURCES, sort_keys=True).encode() + b"".join(
        p.read_bytes() for p in sorted(implementation.iterdir()) if p.suffix == ".c" or p.name == "config")).hexdigest()[:16]
    cache = Path(artifacts["cache"]) / "nginx" / key
    cache.mkdir(parents=True, exist_ok=True)
    commands = []
    def execute(command, name, cwd=None):
        commands.append({"command": command, "cwd": str(cwd or driver.ROOT)})
        driver.execute(command, cache / (name + ".log"), cwd=cwd)
    cc = shlex.split(os.environ.get("CC", "cc"))
    probe = cache / "allocator_probe.o"
    execute(cc + ["-O2", "-Wall", "-Wextra", "-Werror", "-c", str(implementation / "allocator_probe.c"), "-o", str(probe)], "probe-build")
    guard = cache / "allocator_guard.so"
    execute(cc + ["-O2", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", str(implementation / "allocator_guard.c"), "-ldl", "-o", str(guard)], "guard-build")
    binaries = {}
    for profile in profiles:
        for trace in (False, True):
            label = profile + ("-trace" if trace else "")
            local = cache / label
            applications.copy_tree(local_sources["nginx"], local)
            options = ["--without-http_rewrite_module", "--without-http_gzip_module", "--without-http_fastcgi_module",
                       "--without-http_uwsgi_module", "--without-http_scgi_module", f"--add-module={implementation}",
                       "--with-cc=" + os.environ.get("CC", "cc")]
            cflags = os.environ.get("CFLAGS", "") + (" -DNGX_DEBUG_PALLOC=1" if profile == "debug-palloc" else "")
            options += [f"--with-cc-opt={cflags}"]
            ldflags = os.environ.get("LDFLAGS", "")
            if trace: ldflags += " " + str(probe) + " " + " ".join(f"-Wl,--wrap={n}" for n in WRAPPED)
            options += [f"--with-ld-opt={ldflags}"]
            if not (local / "objs/Makefile").exists(): execute(["./configure", *options], label + "-configure", local)
            execute(["make", "-C", str(local), "-j", str(jobs)], label + "-build")
            binaries[label] = str(local / "objs/nginx")
    wrk = cache / "wrk-source"
    applications.copy_tree(local_sources["wrk"], wrk)
    execute(["make", "-C", str(wrk), "-j", str(jobs), "WITH_OPENSSL=/usr", "VER=4.2.0"], "wrk-build")
    libraries = {"rust": artifacts["rust_library"], "mimalloc": artifacts["c_library"], "glibc": None}
    paths = {**binaries, "wrk": str(wrk / "wrk"), "guard": str(guard)}
    result = {"binaries": binaries, "wrk": paths["wrk"], "guard": str(guard), "libraries": libraries,
              "sources": {n: dict(zip(("version", "url", "sha256"), spec)) for n, spec in SOURCES.items()},
              "commands": commands, "cache": str(cache), "openssl": driver.capture(["openssl", "version"])}
    result["sha256"] = {n: hashlib.sha256(Path(p).read_bytes()).hexdigest() for n, p in
                        {**paths, **{k: v for k, v in libraries.items() if v}}.items()}
    driver.write_json(cache / "artifacts.json", result)
    return result


def quote(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('$', '\\$') + '"'


def configuration(prefix, port, workers, cpus, data, upstream=None, churn=False, buffering="on"):
    affinity = " ".join(format(1 << cpus[i % len(cpus)], "b") for i in range(workers))
    locations = [f'location = /small {{ alias {quote(data / "small")}; }}']
    for size in (1024, 4096, 16384, 65536, 262144, 1048576):
        locations.append(f'location = /{size} {{ alias {quote(data / str(size))}; }}')
    if upstream is None:
        locations.append('location = /upload { bench_sink; }')
    else:
        for size in (1024, 16384, 65536, 262144):
            locations.append(f'location = /proxy/{size} {{ proxy_pass http://127.0.0.1:{upstream}/{size}; '
                             f'proxy_buffering {buffering}; proxy_buffer_size 8k; proxy_buffers 16 16k; '
                             'proxy_busy_buffers_size 32k; proxy_max_temp_file_size 0; }')
        locations.append(f'location = /upload {{ proxy_pass http://127.0.0.1:{upstream}/upload; proxy_request_buffering off; }}')
    # The upstream keeps all known upload sizes in memory; frontend streams at 64KiB.
    body_buffer = "2m" if upstream is None else "64k"
    user = "user root;" if os.geteuid() == 0 else ""
    return f'''{user}
worker_processes {workers};
worker_cpu_affinity {affinity};
worker_rlimit_nofile 65536;
pid {quote(prefix / 'nginx.pid')};
error_log {quote(prefix / 'error.log')} error;
events {{ worker_connections 16384; multi_accept on; }}
http {{
    access_log off; sendfile on; tcp_nopush on;
    open_file_cache max=64 inactive=60s;
    keepalive_timeout {0 if churn else 30}s; keepalive_requests 10000;
    client_header_buffer_size 1k; large_client_header_buffers 8 16k;
    client_body_buffer_size {body_buffer}; client_max_body_size 16m;
    client_body_temp_path {quote(prefix / 'body-temp')};
    proxy_temp_path {quote(prefix / 'proxy-temp')};
    server {{ listen 127.0.0.1:{port} reuseport; {' '.join(locations)} }}
}}
'''


def cases(workloads):
    result = []
    for workload in workloads:
        if workload == "proxy":
            result += [dict(workload=workload, label=f"proxy-{size}-{buffering}", path=f"/proxy/{size}", size=size,
                            buffering=buffering) for size in (1024, 16384, 65536, 262144) for buffering in ("on", "off")]
        elif workload == "transfer":
            result += [dict(workload=workload, label=f"{kind}-{size}", path="/upload" if kind == "upload" else f"/{size}",
                            size=size, upload=kind == "upload") for size in (4096, 65536, 1048576) for kind in ("upload", "download")]
        else:
            result.append(dict(workload=workload, label=workload, path="/small"))
    return result


def lua_script(case):
    if case["workload"] == "headers":
        setup = '\n'.join(f'wrk.headers["X-Payload-{i}"] = string.rep("x", {size})'
                          for i, size in enumerate((128, 512, 1024, 4096, 128, 512, 1024, 4096)))
    elif case.get("upload"):
        setup = f'wrk.method="POST"; wrk.body=string.rep("x", {case["size"]}); wrk.headers["Content-Type"]="application/octet-stream"'
    else:
        setup = ""
    # done writes machine-readable wrk histogram percentiles. Transport/status
    # errors are a hard failure, not silently counted as completed good requests.
    return setup + '''
done = function(summary, latency, requests)
  io.write(string.format('BENCH_JSON {"requests":%d,"duration_us":%d,"bytes":%d,"p50_us":%.3f,"p95_us":%.3f,"p99_us":%.3f,"errors":%d}\\n',
    summary.requests, summary.duration, summary.bytes, latency:percentile(50), latency:percentile(95), latency:percentile(99),
    summary.errors.connect + summary.errors.read + summary.errors.write + summary.errors.status + summary.errors.timeout))
end
'''


def parse_wrk(output):
    lines = [line.removeprefix("BENCH_JSON ") for line in output.splitlines() if line.startswith("BENCH_JSON ")]
    if len(lines) != 1:
        raise ValueError("wrk did not produce exactly one machine-readable result")
    result = json.loads(lines[0])
    for key in ("requests", "duration_us", "bytes", "errors"):
        if type(result[key]) is not int or result[key] < 0: raise ValueError(f"invalid wrk {key}")
    if result["errors"] or not result["requests"] or not result["duration_us"]:
        raise ValueError(f"wrk incomplete/failed HTTP traffic: {result}")
    for key in ("p50_us", "p95_us", "p99_us"):
        if type(result[key]) not in (int, float) or not math.isfinite(result[key]) or result[key] < 0:
            raise ValueError("invalid wrk latency")
    if not result["p50_us"] <= result["p95_us"] <= result["p99_us"]:
        raise ValueError("unordered wrk latency percentiles")
    result["requests_per_second"] = result["requests"] * 1e6 / result["duration_us"]
    return result


def free_port():
    import socket
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0)); return s.getsockname()[1]


def worker_pids(master):
    return [int(p) for p in Path(f"/proc/{master}/task/{master}/children").read_text().split()]


def memory_cpu(pids):
    rss = 0; cpu = 0; minor = 0; major = 0
    for pid in pids:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        rss += int(fields[21]) * os.sysconf("SC_PAGE_SIZE")
        cpu += (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")
        minor += int(fields[7]); major += int(fields[9])
    return {"rss_bytes": rss, "cpu_seconds": cpu, "minor_faults": minor, "major_faults": major}


def trace_snapshot(directory, pids):
    names = ("malloc_calls", "calloc_calls", "realloc_calls", "free_calls", "aligned_calls",
             "requested_bytes", "usable_bytes", "live_usable_bytes", "peak_live_usable_bytes")
    total = {key: 0 for key in names}; total["size_histogram"] = [0] * 32
    for pid in pids:
        path = directory / f"{pid}.bin"
        if not path.exists(): raise ValueError(f"missing worker allocator trace: {pid}")
        values = struct.unpack("=41Q", path.read_bytes())
        for key, value in zip(names, values): total[key] += value
        total["size_histogram"] = [a + b for a, b in zip(total["size_histogram"], values[9:])]
    return total


def start_nginx(binary, prefix, conf, environment, cpus):
    prefix.mkdir(parents=True, exist_ok=True)
    (prefix / "logs").mkdir(exist_ok=True)
    (prefix / "nginx.conf").write_text(conf)
    command = ["taskset", "-c", ",".join(map(str, cpus)), binary, "-p", str(prefix) + "/", "-c", str(prefix / "nginx.conf"),
               "-g", "daemon off; master_process on;"]
    log = (prefix / "master.log").open("w")
    process = subprocess.Popen(command, env=environment, stdout=log, stderr=subprocess.STDOUT)
    log.close()
    return process, command


def stop(process):
    if process.poll() is None:
        # Quit the master; it waits for its workers. TERM is the bounded fallback.
        process.send_signal(signal.SIGQUIT)
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            children = worker_pids(process.pid)
            process.terminate()
            try: process.wait(timeout=2)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
            for pid in children:
                try: os.kill(pid, signal.SIGKILL)
                except ProcessLookupError: pass


def http_check(port, case):
    client = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
    try:
        headers = {f"X-Payload-{i}": "x" * size for i, size in enumerate((128, 512, 1024, 4096, 128, 512, 1024, 4096))} if case["workload"] == "headers" else {}
        body = b"x" * case["size"] if case.get("upload") else None
        client.request("POST" if body else "GET", case["path"], body=body, headers=headers)
        response = client.getresponse(); data = response.read()
        if body:
            if response.status != 204 or response.getheader("X-Received-Bytes") != str(len(body)):
                raise ValueError("upstream did not consume the entire upload")
        elif response.status != 200 or len(data) != case.get("size", 12):
            raise ValueError(f"unexpected HTTP status/body: {response.status}, {len(data)}")
    finally: client.close()


def ready(process, port, case, workers):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if process.poll() is not None: raise ValueError(f"Nginx exited {process.returncode}")
        try:
            pids = worker_pids(process.pid)
            if len(pids) == workers:
                http_check(port, case); return pids
        except (OSError, http.client.HTTPException): pass
        time.sleep(.05)
    raise ValueError("Nginx readiness timed out")


def wrk_run(config, case, port, seconds, directory, name, monitored):
    lua = directory / "workload.lua"; lua.write_text(lua_script(case))
    command = ["taskset", "-c", ",".join(map(str, config["load_cpus"])), config["artifacts"]["wrk"],
               "-t", str(config["load_threads"]), "-c", str(config["connections"]), "-d", f"{seconds}s", "--latency", "-s", str(lua),
               f"http://127.0.0.1:{port}{case['path']}"]
    samples = []; start = time.monotonic()
    before = memory_cpu(monitored["server"])
    with (directory / (name + ".log")).open("w") as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                   env={k: v for k, v in os.environ.items() if k != "LD_PRELOAD"})
        try:
            deadline = start + seconds + 15
            while process.poll() is None:
                sample = {"seconds": time.monotonic() - start, **{role: memory_cpu(pids) for role, pids in monitored.items()}}
                try: sample["load"] = memory_cpu([process.pid])
                except FileNotFoundError: pass
                samples.append(sample)
                if time.monotonic() > deadline: raise TimeoutError("wrk timed out")
                time.sleep(min(1, max(.05, seconds / 4)))
            if process.returncode: raise ValueError(f"wrk exited {process.returncode}")
        finally:
            if process.poll() is None: process.kill(); process.wait()
    after = memory_cpu(monitored["server"])
    result = parse_wrk((directory / (name + ".log")).read_text())
    result["payload_bytes_per_second"] = result["requests_per_second"] * case.get("size", 12)
    result.update(command=command, resources=samples, worker_cpu_seconds=after["cpu_seconds"] - before["cpu_seconds"],
                  peak_worker_rss_bytes=max((s["server"]["rss_bytes"] for s in samples), default=after["rss_bytes"]),
                  cpu_seconds_per_request=(after["cpu_seconds"] - before["cpu_seconds"]) / result["requests"])
    return result


def server_environment(config, trace=None):
    environment = {k: v for k, v in os.environ.items() if k not in ("LD_PRELOAD", "NGINX_BENCH_TRACE")}
    library = config["artifacts"]["libraries"][config["allocator"]]
    preloads = ([library] if library else []) + ["libstdc++.so.6", config["artifacts"]["guard"]]
    environment.update(LD_PRELOAD=":".join(preloads), NGINX_BENCH_ALLOCATOR=library or "glibc")
    if trace:
        trace.mkdir(); environment["NGINX_BENCH_TRACE"] = str(trace)
    return environment


def worker(config, directory):
    directory.mkdir(parents=True, exist_ok=True)
    case = config["case"]
    # Nginx temp files and all listeners belong to this run; no existing service
    # is signalled or reconfigured. Files are readable by benchmark workers.
    data = directory / "data"; data.mkdir()
    (data / "small").write_bytes(b"hello world\n")
    for size in (1024, 4096, 16384, 65536, 262144, 1048576):
        (data / str(size)).write_bytes(b"x" * size)
    result = {"config": config, "case": case, "commands": [], "runs": [], "idle": [], "quality_flags": []}
    if config["workers"] > len(config["server_cpus"]):
        result["quality_flags"].append("Nginx workers exceed assigned server CPUs")
    if config["load_threads"] > len(config["load_cpus"]):
        result["quality_flags"].append("wrk threads exceed assigned generator CPUs")
    upstream_process = None
    try:
        upstream_port = None; monitored_upstream = {}
        if case["workload"] == "proxy" or case.get("upload"):
            upstream_port = free_port(); prefix = directory / "upstream"
            upstream_process, command = start_nginx(config["artifacts"]["binaries"]["production"], prefix,
                configuration(prefix, upstream_port, len(config["upstream_cpus"]), config["upstream_cpus"], data),
                {k: v for k, v in os.environ.items() if k not in ("LD_PRELOAD", "NGINX_BENCH_TRACE", "NGINX_BENCH_ALLOCATOR")}, config["upstream_cpus"])
            result["commands"].append(command)
            upstream_case = {**case, "path": "/upload" if case.get("upload") else f"/{case['size']}"}
            upstream_pids = ready(upstream_process, upstream_port, upstream_case, len(config["upstream_cpus"]))
            result["upstream_worker_pids"] = upstream_pids
            monitored_upstream = {"upstream": upstream_pids}
            headroom = wrk_run(config, upstream_case, upstream_port, config["preflight_secs"], directory, "upstream-capacity",
                               {"server": upstream_pids})
            result["upstream_capacity"] = headroom
        # An instrumented preflight is always independent of throughput timing.
        for diagnostic in (True, False):
            label = "diagnostic" if diagnostic else "timed"
            prefix = directory / label; port = free_port()
            trace = directory / "trace" if diagnostic else None
            env = server_environment(config, trace)
            binary = config["artifacts"]["binaries"][config["profile"] + ("-trace" if diagnostic else "")]
            conf = configuration(prefix, port, config["workers"], config["server_cpus"], data, upstream_port,
                                 case["workload"] == "churn", case.get("buffering", "on"))
            process, command = start_nginx(binary, prefix, conf, env, config["server_cpus"])
            result["commands"].append(command)
            try:
                pids = ready(process, port, case, config["workers"])
                result[label + "_worker_pids"] = pids
                monitored = {"server": pids, **monitored_upstream}
                if diagnostic:
                    before = trace_snapshot(trace, pids)
                    diagnostic_result = wrk_run(config, case, port, config["preflight_secs"], directory, "allocator-preflight", monitored)
                    after = trace_snapshot(trace, pids)
                    delta = {key: after[key] - before[key] for key in before if key not in
                             ("size_histogram", "live_usable_bytes", "peak_live_usable_bytes")}
                    delta["size_histogram"] = [a - b for a, b in zip(after["size_histogram"], before["size_histogram"])]
                    count = sum(delta[k] for k in ("malloc_calls", "calloc_calls", "realloc_calls", "aligned_calls"))
                    if count <= 0 or delta["requested_bytes"] <= 0:
                        raise ValueError("no meaningful Nginx allocator traffic in preflight")
                    result["allocator_traffic"] = {"deltas": delta, "after": after,
                        "requests": diagnostic_result["requests"], "allocations_per_request": count / diagnostic_result["requests"],
                        "requested_bytes_per_request": delta["requested_bytes"] / diagnostic_result["requests"],
                        "scope": "Nginx executable libc allocation sites; separate diagnostic run"}
                    result["diagnostic"] = diagnostic_result
                    if count / diagnostic_result["requests"] < .01:
                        result["quality_flags"].append("weak allocator intensity (<0.01 allocations/request)")
                else:
                    wrk_run(config, case, port, config["warmup_secs"], directory, "warmup", monitored)
                    cycles = config["burst_cycles"] if case["workload"] == "burst-idle" else 1
                    for cycle in range(cycles):
                        print(f"{config['profile']} / {config['allocator']} / {case['label']} cycle {cycle + 1}/{cycles}", flush=True)
                        measured = wrk_run(config, case, port, config["duration_secs"], directory, f"load-{cycle}", monitored)
                        result["runs"].append(measured)
                        if case["workload"] == "burst-idle":
                            idle = []; end = time.monotonic() + config["idle_secs"]
                            while True:
                                idle.append({"cycle": cycle, "seconds_remaining": max(0, end - time.monotonic()), **memory_cpu(pids)})
                                if time.monotonic() >= end: break
                                time.sleep(min(1, end - time.monotonic()))
                            result["idle"].append(idle)
            finally:
                stop(process)
            if process.returncode != 0: raise ValueError(f"Nginx did not exit cleanly: {process.returncode}")
        result["quality_flags"].append("local closed-loop wrk; not fixed-rate latency or proven sustainable throughput")
        if upstream_process:
            if result["upstream_capacity"]["requests_per_second"] < 1.25 * max(r["requests_per_second"] for r in result["runs"]):
                result["quality_flags"].append("upstream headroom below 25%; throughput may be upstream/client limited")
        result["status"] = "complete"
    except BaseException as error:
        result.update(status="failed", error=str(error)); raise
    finally:
        if upstream_process: stop(upstream_process)
        write_json(directory / "nginx.json", result)
        shutil.rmtree(data, ignore_errors=True)


def cpu_set(value):
    result = []
    for token in value.split(","):
        if "-" in token:
            a, b = map(int, token.split("-")); result.extend(range(a, b + 1))
        else: result.append(int(token))
    if not result or min(result) < 0: raise ValueError("invalid CPU set")
    return sorted(set(result))


def add_arguments(parser, positive, default_jobs):
    parser.add_argument("--workload", action="append", choices=WORKLOADS)
    parser.add_argument("--allocator", action="append", choices=ALLOCATORS, help="default: rust and mimalloc; glibc is optional")
    parser.add_argument("--profile", action="append", choices=("production", "debug-palloc"), help="default: both, reported separately")
    parser.add_argument("--workers", type=positive, default=2)
    parser.add_argument("--connections", type=positive, default=256)
    parser.add_argument("--load-threads", type=positive, default=2)
    parser.add_argument("--duration-secs", type=positive, default=30)
    parser.add_argument("--warmup-secs", type=positive, default=10)
    parser.add_argument("--preflight-secs", type=positive, default=3)
    parser.add_argument("--repeats", type=positive, default=3)
    parser.add_argument("--burst-cycles", type=positive, default=3)
    parser.add_argument("--idle-secs", type=positive, default=30)
    parser.add_argument("--server-cpus", help="comma-separated IDs/ranges; defaults to a disjoint local CPU set")
    parser.add_argument("--load-cpus")
    parser.add_argument("--upstream-cpus")
    parser.add_argument("--order-seed", type=int, default=12345)
    parser.add_argument("--jobs", type=positive, default=default_jobs)
    parser.add_argument("--output", help="new or empty results directory")


def read_result(path, config):
    result = json.loads(path.read_text())
    if result["status"] != "complete" or result["config"] != config: raise ValueError("wrong/incomplete Nginx measurement")
    expected = config["burst_cycles"] if config["case"]["workload"] == "burst-idle" else 1
    if len(result["runs"]) != expected: raise ValueError("missing Nginx cycles")
    for key in ("allocations_per_request", "requested_bytes_per_request"):
        value = result["allocator_traffic"][key]
        if type(value) not in (float, int) or not math.isfinite(value) or value <= 0:
            raise ValueError("missing/invalid allocator traffic")
    if config["case"]["workload"] == "burst-idle":
        if len(result["idle"]) != expected or any(not phase for phase in result["idle"]):
            raise ValueError("missing idle checkpoints")
        for phase in result["idle"]:
            if type(phase[-1]["rss_bytes"]) is not int or phase[-1]["rss_bytes"] < 0:
                raise ValueError("invalid idle RSS")
    for run in result["runs"]:
        for key in ("requests_per_second", "p99_us", "peak_worker_rss_bytes"):
            if type(run[key]) not in (float, int) or not math.isfinite(run[key]) or run[key] <= 0:
                raise ValueError("invalid Nginx measurement")
        value = run["cpu_seconds_per_request"]
        if type(value) not in (float, int) or not math.isfinite(value) or value < 0:
            raise ValueError("invalid Nginx worker CPU time")
        if run["errors"]: raise ValueError("Nginx HTTP errors")
    return result


def report(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    measurements = []; errors = []
    for job in manifest["jobs"]:
        try:
            if job.get("exit_status") != 0: raise ValueError("worker did not complete")
            result = read_result(directory / job["directory"] / "nginx.json", job["config"])
            measurements.append({"repeat": job["repeat"], **result})
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append(f"{job['directory']}: {error}")
    complete = manifest["status"] == "complete" and not errors and len(measurements) == manifest["expected_jobs"]
    rows = []
    for profile in manifest["profiles"]:
        for case in manifest["cases"]:
            for allocator in manifest["allocators"]:
                selected = [m for m in measurements if (m["config"]["profile"], m["case"]["label"], m["config"]["allocator"])
                            == (profile, case["label"], allocator)]
                if len(selected) != manifest["controls"]["repeats"]: continue
                rows.append({"profile": profile, "case": case["label"], "allocator": allocator,
                    "requests_per_second": statistics.median(statistics.median(r["requests_per_second"] for r in m["runs"]) for m in selected),
                    "payload_bytes_per_second": statistics.median(statistics.median(r.get("payload_bytes_per_second", r["requests_per_second"] * case.get("size", 12)) for r in m["runs"]) for m in selected),
                    "p99_us": statistics.median(statistics.median(r["p99_us"] for r in m["runs"]) for m in selected),
                    "peak_worker_rss_bytes": statistics.median(max(r["peak_worker_rss_bytes"] for r in m["runs"]) for m in selected),
                    "cpu_seconds_per_request": statistics.median(statistics.median(r["cpu_seconds_per_request"] for r in m["runs"]) for m in selected),
                    "allocations_per_request": statistics.median(m["allocator_traffic"]["allocations_per_request"] for m in selected),
                    "requested_bytes_per_request": statistics.median(m["allocator_traffic"]["requested_bytes_per_request"] for m in selected),
                    "idle_final_rss_bytes": statistics.median(m["idle"][-1][-1]["rss_bytes"] for m in selected) if case["workload"] == "burst-idle" else None,
                    "quality_flags": sorted({f for m in selected for f in m["quality_flags"]})})
    write_json(directory / "results.json", {"complete": complete, "summary": rows, "measurements": measurements, "errors": errors})
    fields = ("profile", "case", "allocator", "requests_per_second", "payload_bytes_per_second", "p99_us", "peak_worker_rss_bytes", "cpu_seconds_per_request",
              "allocations_per_request", "requested_bytes_per_request", "idle_final_rss_bytes", "quality_flags")
    with (directory / "results.csv").open("w", newline="") as out:
        writer = csv.DictWriter(out, fieldnames=fields); writer.writeheader(); writer.writerows(rows)
    lines = [f"# Nginx allocator comparison ({'complete' if complete else 'INCOMPLETE'})", "",
             "Independent run medians; HTTP p99 is wrk's closed-loop latency histogram, not allocator p99. "
             "Peak RSS sums worker RSS (shared pages can be counted more than once). "
             "Allocator traffic comes from a separate instrumented preflight. "
             "CPU time is whole-worker CPU time, not allocator-only cycles. "
             "No rankings are combined between builds; this local load generator does not establish maximum sustainable throughput."]
    for profile in manifest["profiles"]:
        lines += ["", f"## {profile}", "", "| Case | Allocator | Requests/s | HTTP p99 ms | Peak worker RSS MiB | Allocations/request | Requested bytes/request | Idle final RSS MiB |",
                  "|---|---|---:|---:|---:|---:|---:|---:|"]
        for row in rows:
            if row["profile"] == profile:
                idle = f"{row['idle_final_rss_bytes']/2**20:.2f}" if row["idle_final_rss_bytes"] is not None else "—"
                lines.append(f"| {row['case']} | {row['allocator']} | {row['requests_per_second']:.0f} | {row['p99_us']/1000:.3f} | "
                             f"{row['peak_worker_rss_bytes']/2**20:.2f} | {row['allocations_per_request']:.3f} | {row['requested_bytes_per_request']:.0f} | {idle} |")
    lines += ["", "## Interpretation flags", ""]
    for row in rows:
        for flag in row["quality_flags"]:
            lines.append(f"- {row['profile']}/{row['case']}/{row['allocator']}: {flag}")
    if manifest.get("error"): lines += ["", manifest["error"]]
    lines += ["", *errors] if errors else []
    (directory / "summary.md").write_text("\n".join(lines) + "\n")
    print(f"Report: {directory / 'summary.md'}", flush=True)
    return complete


def run(args, driver):
    allowed = sorted(os.sched_getaffinity(0))
    if len(allowed) < 3: raise driver.BenchError("local Nginx suite needs three disjoint CPU sets")
    physical = []; seen = set()
    for cpu in allowed:
        topology = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        identity = ((topology / "physical_package_id").read_text(), (topology / "core_id").read_text())
        if identity not in seen: physical.append(cpu); seen.add(identity)
    automatic = physical if len(physical) >= 3 else allowed
    width = len(automatic) // 3
    sets = {"server_cpus": cpu_set(args.server_cpus) if args.server_cpus else automatic[:width],
            "load_cpus": cpu_set(args.load_cpus) if args.load_cpus else automatic[width:2*width],
            "upstream_cpus": cpu_set(args.upstream_cpus) if args.upstream_cpus else automatic[2*width:3*width]}
    if any(set(cpus) - set(allowed) for cpus in sets.values()) or any(
        set(sets[a]) & set(sets[b]) for a, b in (("server_cpus", "load_cpus"), ("server_cpus", "upstream_cpus"), ("load_cpus", "upstream_cpus"))):
        raise driver.BenchError("CPU sets must be available and disjoint")
    if args.connections < args.load_threads: raise driver.BenchError("connections must be at least load threads")
    if resource.getrlimit(resource.RLIMIT_NOFILE)[0] < 65536:
        soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
        if hard < 65536: raise driver.BenchError("raise the file descriptor hard limit to at least 65536")
        resource.setrlimit(resource.RLIMIT_NOFILE, (65536, hard))
    directory = driver.run_directory(args.output, "nginx-")
    profiles = list(dict.fromkeys(args.profile or ("production", "debug-palloc")))
    allocators = list(dict.fromkeys(args.allocator or ("rust", "mimalloc")))
    selected_cases = cases(list(dict.fromkeys(args.workload or WORKLOADS)))
    controls = {k: getattr(args, k) for k in ("workers", "connections", "load_threads", "duration_secs", "warmup_secs",
                                            "preflight_secs", "repeats", "burst_cycles", "idle_secs", "order_seed")}
    controls.update(sets)
    manifest = driver.run_manifest(suite="nginx", controls=controls, profiles=profiles, allocators=allocators,
                                   cases=selected_cases, jobs=[],
                                   expected_jobs=len(profiles) * len(allocators) * len(selected_cases) * args.repeats)
    path = directory / "manifest.json"; driver.write_json(path, manifest)
    print(f"Results: {directory}; local CPU sets: {sets}; {manifest['expected_jobs']} independent runs", flush=True)
    with driver.run_lifecycle(path, manifest, manifest["jobs"]):
        core = driver.build(args.jobs); manifest["core_artifacts"] = core
        artifacts = build(core, list(dict.fromkeys(("production", *profiles))), args.jobs, driver)
        manifest["artifacts"] = artifacts
        for name in ("harness.log", "rust.log", "c-configure.log", "c-build.log"):
            shutil.copy2(Path(core["cache"]) / name, directory / ("core-" + name))
        for log in Path(artifacts["cache"]).glob("*.log"): shutil.copy2(log, directory / log.name)
        manifest["status"] = "running"; driver.write_json(path, manifest)
        generator = random.Random(args.order_seed)
        for repeat in range(1, args.repeats + 1):
            jobs = [(p, c, a) for p in profiles for c in selected_cases for a in allocators]
            generator.shuffle(jobs)
            for profile, case, allocator in jobs:
                relative = f"round-{repeat:03}/{profile}/{case['label']}/{allocator}"
                config = {**controls, "artifacts": artifacts, "profile": profile, "allocator": allocator, "case": case}
                config_path = directory / relative / "config.json"; config_path.parent.mkdir(parents=True)
                driver.write_json(config_path, config)
                command = [sys.executable, str(driver.ROOT / "benchmarks/nginx.py"), str(config_path), str(config_path.parent)]
                job = {"repeat": repeat, "directory": relative, "command": command, "config": config, "started_utc": driver.utc()}
                manifest["jobs"].append(job); driver.write_json(path, manifest)
                print(f"Round {repeat}/{args.repeats}: {profile} / {case['label']} / {allocator}", flush=True)
                driver.execute(command, config_path.parent / "worker.log", {k: v for k, v in os.environ.items() if k != "LD_PRELOAD"})
                read_result(config_path.parent / "nginx.json", config)
                job.update(exit_status=0, finished_utc=driver.utc()); driver.write_json(path, manifest)
    if manifest.get("error"): print(manifest["error"], file=sys.stderr)
    return 0 if report(directory) else manifest.get("exit_status", 1)


if __name__ == "__main__":
    try: worker(json.loads(Path(sys.argv[1]).read_text()), Path(sys.argv[2]))
    except Exception as error:
        print(f"Nginx benchmark: {error}", file=sys.stderr); sys.exit(1)
