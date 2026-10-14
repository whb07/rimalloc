#!/usr/bin/env python3
"""BOLT post-link layout of the Rust interpose library (opt-in, `MIMALLOC_BENCH_BOLT=1`).

`bench.py` links the library with relocations and applies the checked-in branch profile
(`bolt-profile.fdata`). `python3 benchmarks/bolt.py profile` re-records that profile; rerun it after
allocator changes, since BOLT drops the profile of functions whose code no longer matches.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROFILE = Path(__file__).resolve().parent / "bolt-profile.fdata"
# BOLT needs the relocations to move functions; the code itself is unchanged.
LINK_FLAGS = "-Clink-arg=-Wl,--emit-relocs"
# Always pad hot functions to a cache line: by default BOLT pads only up to 32 bytes, which left
# the asm `malloc`/`free` at whatever offset preceding code produced.
OPTIONS = ["-reorder-blocks=ext-tsp", "-reorder-functions=cdsort", "-split-functions", "-split-all-cold",
           "-infer-stale-profile", "-align-functions=64", "-align-functions-max-bytes=63"]
# 11 of the 21 native scenarios; the other 10 stay out of the profile so they show whether
# the layout generalizes (PERFORMANCE_WORKLOG.md X27).
TRAINING = ("alloc-test1", "barnes", "cache-scratchN", "cache-thrash1", "cfrac", "glibc-thread", "larsonN",
            "malloc-large", "mleak10", "mstressN", "sh6benchN")


def tool(name: str) -> str:
    """`name` from PATH, else from the newest /usr/lib/llvm-*/bin (Ubuntu's bolt-N package)."""
    found = shutil.which(name)
    if found:
        return found
    versions = sorted(Path("/usr/lib").glob("llvm-*/bin/" + name),
                      key=lambda p: int(re.sub(r"\D", "", p.parent.parent.name) or 0))
    if not versions:
        raise FileNotFoundError(f"{name} not found; install LLVM BOLT (Ubuntu: bolt-18)")
    return str(versions[-1])


def version() -> str:
    output = subprocess.run([tool("llvm-bolt"), "--version"], capture_output=True, text=True, check=True).stdout
    return next((line.strip() for line in output.splitlines() if "version" in line.lower()), output.strip())


def optimize(library: Path, output: Path, log: Path, profile: Path = PROFILE) -> dict:
    """Write the BOLT layout of `library` to `output` and report how much of the profile matched."""
    result = subprocess.run([tool("llvm-bolt"), str(library), "-o", str(output), f"-data={profile}", *OPTIONS],
                            capture_output=True, text=True)
    log.write_text(result.stdout + result.stderr)
    if result.returncode != 0:
        raise RuntimeError(f"llvm-bolt failed; see {log}")
    text = result.stdout + result.stderr
    profiled = re.search(r"(\d+) out of (\d+) functions in the binary \(([\d.]+)%\) have non-empty execution", text)
    stale = re.search(r"(\d+) \([^)]*\) functions? ha(?:s|ve) invalid \(possibly stale\) profile", text)
    report = {"profiled_functions": int(profiled.group(1)) if profiled else 0,
              "stale_functions": int(stale.group(1)) if stale else 0, "log": str(log)}
    print(f"BOLT: {report['profiled_functions']} functions profiled, {report['stale_functions']} stale "
          f"(rerun `benchmarks/bolt.py profile` when this grows)", flush=True)
    return report


def profile(driver, jobs: int, output: Path) -> None:
    """Record LBR branch samples of the training scenarios on a relocation-linked library."""
    for name in ("perf2bolt", "merge-fdata"):
        tool(name)
    artifacts = driver.build(jobs)
    target = ROOT / "target/bolt-profile"
    environment = {**os.environ, "CARGO_TARGET_DIR": str(target),
                   "RUSTFLAGS": (os.environ.get("RUSTFLAGS", "") + " " + LINK_FLAGS).strip()}
    built = driver.execute(["cargo", "rustc", "--locked", "--release", "--features", "interpose", "--lib",
                            "--jobs", str(jobs), "--message-format=json-render-diagnostics", "--crate-type", "cdylib"],
                           target / "build.log", environment)
    library = driver.cargo_artifact(built, "mechanical_mimalloc", shared=True)
    with tempfile.TemporaryDirectory(prefix="bolt-profile-") as scratch:
        profiles = []
        for scenario in TRAINING:
            environment = {k: v for k, v in driver.without_preload().items() if not k.startswith("MIMALLOC_BENCH_")}
            environment.update(MIMALLOC_RUST_LIB=str(library), MIMALLOC_C_LIB=artifacts["c_library"],
                               MIMALLOC_BENCH_SKIP_BUILD="1", MIMALLOC_BENCH_SCENARIOS=scenario,
                               MIMALLOC_BENCH_ALLOCATORS=driver.ALLOCATORS[0], MIMALLOC_BENCH_QUICK_RUNS="1",
                               MIMALLOC_BENCH_TIMED_SECS="1")
            samples = Path(scratch) / f"{scenario}.data"
            subprocess.run(["perf", "record", "-q", "-j", "any,u", "-e", "cycles:u", "-c", "200003", "-o",
                            str(samples), "--", artifacts["executable"], "--bench", "mimalloc-bench-native"],
                           env=environment, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
            converted = Path(scratch) / f"{scenario}.fdata"
            subprocess.run([tool("perf2bolt"), "-p", str(samples), "-o", str(converted), str(library)],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            profiles.append(str(converted))
            print(f"profiled {scenario}", flush=True)
        merged = subprocess.run([tool("merge-fdata"), *profiles], capture_output=True, text=True, check=True).stdout
    output.write_text(merged)
    print(f"wrote {output}", flush=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    record = sub.add_parser("profile", help="re-record the branch profile from the training scenarios")
    record.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    record.add_argument("--output", type=Path, default=PROFILE)
    args = parser.parse_args(argv)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import bench
    profile(bench, args.jobs, args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
