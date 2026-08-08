#!/usr/bin/env python3
"""Run mimalloc-bench workloads under the memcpy/memmove size tracer."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import signal
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
BENCH_ROOT = ROOT / "mimalloc-bench"
BENCH_OUT = BENCH_ROOT / "out" / "bench"
TRACER_SOURCE = ROOT / "scripts" / "copy-size-tracer.c"

WORKLOADS = (
    "cfrac", "espresso", "barnes", "redis", "lean", "larson-sized",
    "mstress", "rptest", "gs", "lua", "alloc-test", "sh6bench",
    "sh8bench", "xmalloc-test", "cscratch", "glibc-simple",
    "glibc-thread", "rocksdb", "larson", "lean-mathlib", "linux",
    "malloc-large", "mleak", "rbstress", "cthrash", "z3", "spec",
    "spec-bench", "security",
)


def unavailable_reason(workload: str) -> str | None:
    extern = BENCH_ROOT / "extern"
    if workload in {"lean", "lean-mathlib"} and not (extern / "lean" / "bin" / "lean").exists():
        return "Lean toolchain is not installed"
    if workload == "linux" and not (extern / "linux-6.5.1").exists():
        return "Linux source tree is not installed"
    if workload in {"spec", "spec-bench"} and not (ROOT / "spec2017").exists():
        return "SPEC CPU 2017 is not installed"
    if workload == "spec-bench":
        return "suite has no standalone spec-bench command; select a SPEC benchmark explicitly"
    return None


def build_tracer(library: pathlib.Path) -> None:
    library.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            os.environ.get("CC", "cc"), "-shared", "-fPIC", "-O2",
            "-fno-builtin", "-Wall", "-Wextra", "-Werror", "-o",
            str(library), str(TRACER_SOURCE), "-ldl",
        ],
        check=True,
        cwd=ROOT,
    )


def build_rust_allocator(target_dir: pathlib.Path) -> pathlib.Path:
    subprocess.run(
        [
            "cargo", "rustc", "--features", "interpose", "--lib", "--release",
            "--", "--crate-type", "cdylib",
        ],
        check=True,
        cwd=ROOT,
        env={**os.environ, "CARGO_TARGET_DIR": str(target_dir)},
    )
    libraries = sorted((target_dir / "release" / "deps").glob("libmechanical_mimalloc-*.so"))
    if not libraries:
        raise RuntimeError("cargo did not produce the mechanical-mimalloc cdylib")
    return libraries[-1].resolve()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("workloads", nargs="*", choices=WORKLOADS)
    parser.add_argument("--output", type=pathlib.Path,
                        default=ROOT / "target" / "size-analysis")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--timeout", type=int, default=1800,
                        help="seconds allowed for each workload repetition")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 16))
    parser.add_argument("--allocator", type=pathlib.Path,
                        help="allocator interposition library; default builds current Rust source")
    parser.add_argument("--keep", action="store_true",
                        help="retain prior output instead of starting clean")
    args = parser.parse_args()

    selected = tuple(args.workloads) if args.workloads else WORKLOADS
    output = args.output.resolve()
    if output.exists() and not args.keep:
        shutil.rmtree(output)
    output.mkdir(parents=True, exist_ok=True)

    library = output / "libcopy-size-tracer.so"
    build_tracer(library)
    allocator = (args.allocator.resolve() if args.allocator is not None
                 else build_rust_allocator(output / "rust-target"))
    if not allocator.is_file():
        raise FileNotFoundError(f"allocator library does not exist: {allocator}")
    allocator_file = output / "copytrace.allocators"
    allocator_file.write_text(
        f"copytrace {library}:{allocator}:libstdc++.so.6\n", encoding="utf-8"
    )

    status_path = output / "status.jsonl"
    with status_path.open("a", encoding="utf-8") as status_file:
        for workload in selected:
            reason = unavailable_reason(workload)
            if reason is not None:
                record = {"workload": workload, "status": "unavailable", "reason": reason}
                status_file.write(json.dumps(record, sort_keys=True) + "\n")
                status_file.flush()
                print(f"UNAVAILABLE {workload}: {reason}", flush=True)
                continue

            for repeat in range(1, args.repeats + 1):
                run_dir = output / workload / f"run-{repeat}"
                run_dir.mkdir(parents=True, exist_ok=True)
                log_path = run_dir / "runner.log"
                env = os.environ.copy()
                env["COPYTRACE_OUTPUT_DIR"] = str(run_dir)
                command = [
                    "bash", "../../bench.sh", f"--external={allocator_file}",
                    f"-j={args.jobs}", workload,
                ]
                print(f"RUN {workload} {repeat}/{args.repeats}", flush=True)
                started = time.monotonic()
                status = "passed"
                returncode: int | None = None
                with log_path.open("w", encoding="utf-8") as log:
                    process = subprocess.Popen(
                        command, cwd=BENCH_OUT, env=env, stdout=log,
                        stderr=subprocess.STDOUT, start_new_session=True,
                    )
                    try:
                        returncode = process.wait(timeout=args.timeout)
                        if returncode != 0:
                            status = "failed"
                    except subprocess.TimeoutExpired:
                        status = "timed-out"
                        os.killpg(process.pid, signal.SIGTERM)
                        try:
                            process.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.wait()
                elapsed = time.monotonic() - started
                trace_files = len(list(run_dir.glob("copytrace-*.tsv")))
                record = {
                    "workload": workload,
                    "repeat": repeat,
                    "status": status,
                    "returncode": returncode,
                    "elapsed_seconds": round(elapsed, 3),
                    "trace_files": trace_files,
                    "log": str(log_path.relative_to(output)),
                }
                status_file.write(json.dumps(record, sort_keys=True) + "\n")
                status_file.flush()
                print(
                    f"{status.upper()} {workload} run {repeat}: "
                    f"{elapsed:.1f}s, {trace_files} trace files",
                    flush=True,
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
