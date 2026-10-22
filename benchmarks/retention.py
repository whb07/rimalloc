"""X33 burst/idle/reburst observations with an external Linux memory sampler."""
import csv
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import statistics
import subprocess
import time

import architecture
from common import file_sha256, write_json

ACTIONS = ("idle", "owner-collect", "other-collect", "owner-exit", "empty")
PHASES = ("ready", "baseline", "filled", "freed", "before-intervention", "intervened",
          "settled", "refilled", "refreed", "final")
MEMORY = {"Rss": "rss_bytes", "Pss": "pss_bytes", "Anonymous": "anonymous_bytes",
          "AnonHugePages": "huge_bytes", "LazyFree": "lazy_free_bytes"}


def add_arguments(parser, minimum, jobs):
    parser.add_argument("--action", action="append", choices=ACTIONS)
    parser.add_argument("--bytes", type=minimum, default=64 * 1024 * 1024)
    parser.add_argument("--block-size", type=minimum, default=64)
    parser.add_argument("--idle-ms", type=minimum, default=10000)
    parser.add_argument("--sample-ms", type=minimum, default=100)
    parser.add_argument("--repeats", type=minimum, default=5)
    parser.add_argument("--worker-cpu", type=int, required=True)
    parser.add_argument("--sampler-cpu", type=int, required=True)
    parser.add_argument("--aa", action="store_true", help="A and B both use the same Rust library")
    parser.add_argument("--artifacts", help="reuse frozen artifacts from a previous retention manifest")
    parser.add_argument("--output", help="new or empty results directory")
    parser.add_argument("--jobs", type=minimum, default=jobs)


def sample(pid, kind):
    start = time.monotonic_ns()
    values = {}
    for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
        key, _, value = line.partition(":")
        if key in MEMORY:
            values[MEMORY[key]] = int(value.split()[0]) * 1024
    if set(values) != set(MEMORY.values()):
        raise ValueError("smaps_rollup lacks required memory fields")
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    end = time.monotonic_ns()
    return {"kind": kind, "sample_ns": (start + end) // 2, "read_ns": end - start,
            "minor_faults": int(fields[7]), "major_faults": int(fields[9]), **values}


def observe(command, directory, environment, idle_ms, sample_ms):
    directory.mkdir(parents=True)
    events, samples = [], []
    cpu_start = time.process_time_ns()
    with (directory / "stderr.log").open("w") as errors, \
            (directory / "stdout.jsonl").open("w") as output, \
            (directory / "samples.jsonl").open("w") as raw:
        with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                              stderr=errors, env=environment, start_new_session=True) as process:
            def capture(kind):
                row = sample(process.pid, kind)
                samples.append(row)
                raw.write(json.dumps(row) + "\n")
                raw.flush()
                return row

            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                next_sample = time.monotonic()
                deadline = next_sample + 3 * idle_ms / 1000 + 60
                pending = None
                try:
                    while process.poll() is None:
                        now = time.monotonic()
                        if now > deadline:
                            raise TimeoutError("retention worker exceeded its phase deadline")
                        if pending is not None and now >= pending[1]:
                            pending[0]["ack_ns"] = time.monotonic_ns()
                            process.stdin.write(b"\n")
                            process.stdin.flush()
                            pending = None
                        if events and now >= next_sample:
                            try:
                                capture("periodic")
                            except (FileNotFoundError, ProcessLookupError):
                                if events[-1]["phase"] != "final":
                                    raise
                                break
                            next_sample = time.monotonic() + sample_ms / 1000
                        for key, _ in selector.select(timeout=min(0.02, sample_ms / 1000)):
                            line = key.fileobj.readline()
                            if not line:
                                selector.unregister(key.fileobj)
                                continue
                            output.write(line.decode())
                            output.flush()
                            event = json.loads(line)
                            phase = event["phase"]
                            if len(events) >= len(PHASES) or phase != PHASES[len(events)]:
                                raise ValueError(f"unexpected phase: {phase}")
                            event["memory"] = capture(phase)
                            events.append(event)
                            pending = (event, time.monotonic() + (1 if phase == "baseline" else 0))
                    code = process.wait(timeout=5)
                    if code != 0 or len(events) != len(PHASES):
                        raise ValueError(f"worker exited {code} with {len(events)} phases; see stderr.log")
                except BaseException:
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                    raise
                finally:
                    write_json(directory / "events.json", events)
    return {"events": events, "samples": samples, "sampler_cpu_ns": time.process_time_ns() - cpu_start}


def window_metrics(samples, metric, baseline, initial, start, end):
    rows = sorted((r for r in samples if start <= r["sample_ns"] <= end), key=lambda r: r["sample_ns"])
    if not rows:
        raise ValueError("observation window has no samples")
    # Include window edges, using the nearest observed value at each edge.
    points = [(start, initial), *[(r["sample_ns"], r[metric]) for r in rows], (end, rows[-1][metric])]
    integral = sum((b[0] - a[0]) / 1e9 * (max(0, a[1] - baseline) + max(0, b[1] - baseline)) / 2
                   for a, b in zip(points, points[1:]))
    result = {"excess_byte_seconds": integral}
    duration = (end - start) / 1e9
    for seconds in (0, 1, 5, 10):
        if seconds <= round(duration):
            closest = min(rows, key=lambda r: abs(r["sample_ns"] - min(start + seconds * 1e9, end)))
            result[f"at_{seconds}s_bytes"] = closest[metric]
            result[f"at_{seconds}s_sample_seconds"] = (closest["sample_ns"] - start) / 1e9
    periodic = [r for r in rows if r["kind"] == "periodic"]
    excess = initial - baseline
    for name, fraction in (("t50_seconds", 0.5), ("t90_seconds", 0.1)):
        crossing = None
        # Sub-page changes cannot support a meaningful decay estimate.
        if excess > os.sysconf("SC_PAGE_SIZE"):
            threshold = baseline + fraction * excess
            for i in range(len(periodic) - 2):
                if all(r[metric] <= threshold for r in periodic[i:i + 3]):
                    crossing = (periodic[i]["sample_ns"] - start) / 1e9
                    break
        result[name] = crossing
        result[name.replace("_seconds", "_status")] = (
            "not-informative" if excess <= os.sysconf("SC_PAGE_SIZE") else
            "right-censored" if crossing is None else "observed")
    return result


def summarize(observation, payload, idle_ms):
    events, samples = observation["events"], observation["samples"]
    if [e["phase"] for e in events] != list(PHASES):
        raise ValueError("incomplete phase sequence")
    phases = {e["phase"]: e for e in events}
    for event in events:
        expected = payload if event["phase"] in ("filled", "refilled") else 0
        if event["live_bytes"] != expected:
            raise ValueError("logical payload differs from the workload contract")
    if phases["before-intervention"]["monotonic_ns"] - phases["freed"]["monotonic_ns"] < idle_ms * 0.99e6:
        raise ValueError("passive window ended early")
    result = {"fill_ms": phases["filled"]["work_ns"] / 1e6,
              "refill_ms": phases["refilled"]["work_ns"] / 1e6,
              "refill_total_ms": phases["refilled"]["elapsed_ns"] / 1e6,
              "intervention_ms": phases["intervened"]["elapsed_ns"] / 1e6,
              "refill_minor_faults": phases["refilled"]["minor_faults"] - phases["settled"]["minor_faults"],
              "refill_major_faults": phases["refilled"]["major_faults"] - phases["settled"]["major_faults"],
              "sampler_cpu_ms": observation["sampler_cpu_ns"] / 1e6}
    baseline_rows = [r for r in samples if phases["baseline"]["monotonic_ns"] <= r["sample_ns"]
                     <= phases["baseline"]["ack_ns"]]
    if not baseline_rows:
        raise ValueError("missing baseline observations")
    for metric in MEMORY.values():
        baseline = statistics.median(r[metric] for r in baseline_rows)
        result[f"baseline_{metric}"] = baseline
        for phase in ("filled", "freed", "before-intervention", "intervened", "settled", "refilled", "final"):
            result[f"{phase}_{metric}"] = phases[phase]["memory"][metric]
            result[f"{phase}_excess_{metric}"] = phases[phase]["memory"][metric] - baseline
        if metric not in ("rss_bytes", "pss_bytes"):
            continue
        for label, begin, finish in (("passive", "freed", "before-intervention"),
                                     ("assisted", "before-intervention", "settled")):
            start, end = phases[begin]["memory"]["sample_ns"], phases[finish]["memory"]["sample_ns"]
            values = window_metrics(samples, metric, baseline, phases[begin]["memory"][metric], start, end)
            result.update({f"{label}_{metric}_{key}": value for key, value in values.items()})
    return result


def host_state():
    paths = ("/proc/meminfo", "/proc/loadavg", "/proc/pressure/cpu", "/proc/pressure/memory", "/proc/stat",
             "/sys/kernel/mm/transparent_hugepage/enabled", "/sys/kernel/mm/transparent_hugepage/defrag")
    return {path: Path(path).read_text() for path in paths}


def report(directory, manifest=None):
    manifest = manifest or json.loads((directory / "manifest.json").read_text())
    results = []
    for worker in manifest["workers"]:
        if worker.get("exit_status") == 0:
            result = json.loads((directory / worker["directory"] / "result.json").read_text())
            results.append({k: worker[k] for k in ("label", "action", "repeat")} | result)
    complete = manifest["status"] == "complete" and len(results) == manifest["expected_workers"]
    write_json(directory / "results.json", {"complete": complete, "measurements": results})
    if results:
        with (directory / "results.csv").open("w", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=list(results[0]))
            writer.writeheader()
            writer.writerows(results)
    print(f"Observation data: {directory / 'results.csv'} ({'complete' if complete else 'incomplete'})", flush=True)
    return complete


def run(args, driver):
    affinity = os.sched_getaffinity(0)
    if args.bytes // args.block_size < 1 or not {args.worker_cpu, args.sampler_cpu} <= affinity:
        raise driver.BenchError("need at least one block and allowed worker/sampler CPUs")
    directory = driver.run_directory(args.output, "retention-")
    actions = list(dict.fromkeys(args.action or ACTIONS[:-1]))
    labels = ("A", "B") if args.aa else ("rust", "c")
    controls = {k: getattr(args, k) for k in ("bytes", "block_size", "idle_ms", "sample_ms", "repeats",
                                            "worker_cpu", "sampler_cpu", "aa")}
    manifest = driver.run_manifest(suite="retention", actions=actions, controls=controls, workers=[],
                                   expected_workers=len(actions) * len(labels) * args.repeats,
                                   entrypoint="explicit mi_* calls into release interpose libraries; system-allocator harness")
    path = directory / "manifest.json"
    driver.write_json(path, manifest)
    (directory / "working-tree.patch").write_text(driver.capture(["git", "diff", "HEAD", "--binary"]))
    print(f"Results: {directory}", flush=True)
    with driver.run_lifecycle(path, manifest, manifest["workers"]):
        if args.artifacts:
            artifacts = json.loads(Path(args.artifacts).read_text())["artifacts"]
        else:
            artifacts = driver.build(args.jobs, harness=False)
            executable, command = architecture.build_probe(artifacts, directory, driver)
            artifacts["architecture_executable"] = str(executable)
            artifacts["sha256"]["architecture_executable"] = file_sha256(executable)
            manifest["cpp_build_command"] = command
            driver.copy_build_logs(artifacts, directory)
        frozen = directory / "artifacts"
        frozen.mkdir()
        for key, name in (("rust_library", "rust.so"), ("c_library", "c.so"), ("architecture_executable", "probe")):
            source = Path(artifacts[key])
            if file_sha256(source) != artifacts["sha256"][key]:
                raise ValueError(f"artifact changed: {source}")
            shutil.copy2(source, frozen / name)
            artifacts[key] = str(frozen / name)
        for source in ("benches/architectural_bench.cpp", "benchmarks/retention.py"):
            shutil.copy2(driver.ROOT / source, frozen / Path(source).name)
        manifest.update(artifacts=artifacts, host_before=host_state(), status="running")
        driver.write_json(path, manifest)
        environment = driver.without_preload()
        os.sched_setaffinity(0, {args.sampler_cpu})
        for repeat in range(1, args.repeats + 1):
            order = labels if repeat % 2 else labels[::-1]
            action_order = actions if repeat % 2 else actions[::-1]
            for action in action_order:
                for label in order:
                    library = artifacts["c_library" if label == "c" else "rust_library"]
                    relative = f"round-{repeat:03}/{action}/{label}"
                    command = ["taskset", "-c", str(args.worker_cpu), artifacts["architecture_executable"],
                               "--library", library, "--scenario", "burst-idle", "--action", action,
                               "--bytes", str(args.bytes), "--block-size", str(args.block_size),
                               "--threads", "1", "--idle-ms", str(args.idle_ms)]
                    worker = dict(directory=relative, command=command, label=label, action=action,
                                  repeat=repeat, started_utc=driver.utc(), host_before=host_state())
                    manifest["workers"].append(worker)
                    driver.write_json(path, manifest)
                    print(f"Round {repeat}/{args.repeats}: {action} / {label}", flush=True)
                    observation = observe(command, directory / relative, environment, args.idle_ms, args.sample_ms)
                    payload = 0 if action == "empty" else args.bytes // args.block_size * args.block_size
                    result = summarize(observation, payload, args.idle_ms)
                    driver.write_json(directory / relative / "result.json", result)
                    worker.update(exit_status=0, finished_utc=driver.utc(), host_after=host_state())
                    driver.write_json(path, manifest)
        manifest["host_after"] = host_state()
        for key, digest in artifacts["sha256"].items():
            if file_sha256(artifacts[key]) != digest:
                raise ValueError(f"artifact changed while measuring: {key}")
    os.sched_setaffinity(0, affinity)
    complete = report(directory, manifest)
    if manifest.get("error"):
        print(manifest["error"], flush=True)
    return 0 if complete else manifest.get("exit_status", 1)
