# Running benchmarks

Use the benchmark entry point from the repository root:

```bash
./benchmarks/bench.py
```

This builds the existing Rust benchmark executable, Rust interposition library,
and vendored C mimalloc 3.3.2 library, compares four core workloads, and writes
a Rust/C report. Comparison requires Linux, Python 3.10 or newer, the pinned
Rust toolchain, CMake, C and C++ compilers, and `lscpu`. No Python packages are
required. Paths also work when invoking the script from another directory;
relative output and report paths start at the repository root.

## Commands

| Command | Purpose |
|---|---|
| `./benchmarks/bench.py` | Build, run the core Rust/C comparison, and report |
| `./benchmarks/bench.py build` | Incrementally build the comparison artifacts |
| `./benchmarks/bench.py build --nginx` | Prepare both Nginx builds and wrk without running load |
| `./benchmarks/bench.py build --applications` | Also fetch and build pinned Redis/RocksDB |
| `./benchmarks/bench.py run --full` | Run all native scenarios plus Redis and RocksDB |
| `./benchmarks/bench.py nginx` | Run Nginx HTTP/pool benchmarks; Rust/C, optional glibc |
| `./benchmarks/bench.py architecture` | Run all four C++ architectural probes against Rust/C |
| `./benchmarks/bench.py list` | List scenarios, metrics, and the core preset |
| `./benchmarks/bench.py report RUN_DIRECTORY` | Regenerate a saved comparison report |
| `./benchmarks/bench.py screen` | ~3 min A/B of the working tree's Rust library against `HEAD` |
| `./benchmarks/scoreboard.py RUN_DIR...` | Per-suite Rust/rival geomeans and tolerance breaches from saved runs |
| `./benchmarks/bench.py smoke` | Run the existing benchmark smoke gate |
| `./benchmarks/bench.py release` | Run the existing five-mode release gate |

Select cases and repeat measurements explicitly:

```bash
./benchmarks/bench.py run --scenario malloc-large --scenario glibc-thread --repeats 3
./benchmarks/bench.py run --full --samples 20 --measurement-secs 5 --warmup-secs 2
./benchmarks/bench.py run --scenario malloc-large --jobs 4 --output target/bench-runs/my-run
```

The default core preset is `glibc-thread`, `malloc-large`, `cache-scratch1`, and
`xmalloc-testN`. It uses ten samples, a three-second measurement target, a
one-second warmup, and one repetition. The measurement target is **not a
wall-clock limit**: fixed-duration scenarios need time for every sample. The
core comparison takes several minutes; all 21 native cases plus the two
application workloads can take much longer. More repetitions multiply that time. `--jobs` controls compilation
only; measurements always run sequentially. `--help` and `run --help` show the
complete interface.

## Rival allocators

`run --rival glibc --rival jemalloc --rival tcmalloc` (any subset) and
`nginx --allocator jemalloc ...` add other allocators to the comparison.
jemalloc 5.3.0 and gperftools 2.16 (`tcmalloc_minimal`) build from pinned,
checksummed releases into the build cache (`benchmarks/rivals.py`); glibc is the
system allocator. Native workers `dlopen` each library `RTLD_LOCAL`, so rivals
link with `-Bsymbolic-functions` to keep their own internal `malloc`/`free`
calls (jemalloc's C++ operators, for example) inside the library, as when
they are the process allocator. jemalloc keeps its initial-exec TLS (disabling
it routes every TLS access through `__tls_get_addr`, about 24% slower, and
crashed forked Nginx workers); rival native workers set
`GLIBC_TUNABLES=glibc.rtld.optional_static_tls=65536` so it can be `dlopen`ed. For
glibc the malloc family comes from `libc.so.6` and the C++ array operators
from `libstdc++.so.6`. Application workers preload the rival (nothing for
glibc) and the ownership check verifies the `malloc` library; the mimalloc
ownership probe applies only to mimalloc builds. Reports add a median and
Rust-vs-rival column per rival and a native geomean per rival; the
architectural probes need mimalloc heap APIs and stay Rust vs C.

`run --allocator NAME` (repeatable; `rust`, `mimalloc`, `glibc`, `jemalloc`,
`tcmalloc`, as for `nginx` and `webserver`) measures only the named allocators
instead of Rust and C plus any `--rival`, for example
`run --scenario redis --scenario rocksdb --allocator rust` after a change to the
Rust library alone. The report then has a column per measured allocator, Rust
ratios only when Rust ran, and the Rust/C geomean only when both ran;
`scoreboard.py` combines such a run with earlier result directories for the
allocators it left out.

## Screening a change

`./benchmarks/bench.py screen` is the inner dev loop for allocator changes. It
builds the working tree's Rust library, builds the baseline from a git revision
(`--baseline`, default `HEAD`, cached by commit under `target/bench-screen/`) or
takes a `.so` path, and runs 11 fast canary scenarios in paired, interleaved,
fresh processes for 5 rounds. Short workloads take the median of 3 in-process
runs after a discarded warmup; `xmalloc-testN` and the Larson pair run for 1 s
(`--timed-secs`) instead of 5 s. Only Rust libraries are measured. It also
prints the `malloc`/`free` cache-line offsets and warns when they differ,
because fast-path-bound scenarios move from placement alone.

Verdicts: `faster`/`SLOWER` need a median move of at least 2% with all but one
round agreeing; `likely faster`/`likely slower` cover 0.5-2%. Calibrated
against held-out runs of known changes, the screen reproduced every effect of
2% or more (for example `xmalloc-testN` -7.7% vs -8.8% held-out, and the
`sh6benchN` +3.9% layout regression vs +2.6%), but single scenarios can read
up to about 1.5% from noise, so treat `likely` verdicts as prompts for a
targeted rerun. Confirm any change you keep with the full interleaved held-out
comparison before committing. The exit code is 1 if any scenario reads `SLOWER`.

## Scoreboard

`./benchmarks/scoreboard.py` reads saved `run` and `nginx` result directories
and reports, per suite (native scenarios, applications, Nginx) and per rival,
the geomean of Rust time / rival time (rival / Rust throughput for Nginx). A
suite is won when the geomean is below 1.0 and no scenario exceeds its
tolerance: 1.03 against C mimalloc, 1.10 against other allocators. Pass several
directories to combine runs: values are merged per scenario and allocator
(later directories win) before ratios are formed, so re-running only the
allocators that changed reuses earlier results for the rest. The exit code
is 0 only when every suite is won. A single breach is a prompt to confirm with
two interleaved rounds, not a verdict.

## Comparison behavior

The workloads and allocator implementations are unchanged. Each scenario and
allocator runs in a fresh process, avoiding retained allocator state from
earlier cases. Rust runs before C in the first repetition, with order reversed
in the second, alternating thereafter. `N` scenarios retain the harness's
host-derived worker counts; the runner adds no thread-count override.
`cache-scratchN` and `cache-thrashN` are unavailable on a single-CPU host and
are identified in full-run reports. An explicitly requested unavailable case
is an error.

The harness uses `bench`; the Rust shared library uses `interpose`. C uses
CMake Release with allocation overrides and secure/guarded modes disabled.
The runner obtains Rust artifacts from Cargo's JSON build messages and passes
both library paths explicitly to each worker. Incremental Cargo and CMake
builds run on every build/run invocation. Configuration-specific caches live
under `target/bench-build/`; previous reports remain separate. Standard build
environment variables such as `CC`, `CXX`, `RUSTFLAGS`, and `CARGO_BUILD_*`,
plus Cargo config files, are recorded and participate in cache selection. The
`nginx` and `architecture` suites build only the two shared libraries, not the
Criterion harness. Comparison selections and Criterion controls
come from the CLI, overriding inherited `MIMALLOC_BENCH_*` settings.

## BOLT layout (opt-in)

`MIMALLOC_BENCH_BOLT=1` applies BOLT post-link layout to the Rust library in
every command that builds it (`run`, `screen`, `build`, `nginx`,
`webserver`). The library is linked with `--emit-relocs`, then `llvm-bolt`
(found on `PATH` or under `/usr/lib/llvm-*/bin`; Ubuntu package `bolt-18`)
applies the checked-in branch profile `benchmarks/bolt-profile.fdata`. The
LLVM BOLT version, profile hash and options select a separate build cache, and
`artifacts.json` records how many functions the profile covered and how many
were stale. Re-record the profile after allocator changes with
`python3 benchmarks/bolt.py profile` (needs `perf` with branch records,
`perf2bolt` and `merge-fdata`); it trains on 11 native scenarios and leaves the
other 10 out (`PERFORMANCE_WORKLOG.md` X27). The C library is not BOLTed.

## Results

Each comparison creates a unique directory under `target/bench-runs/`, or uses
a new/empty directory provided with `--output`. A nonempty directory is
rejected. The runner prints progress, the results directory, and the final
report location.

- `summary.md`: scenario medians, Rust/C ratios, noise, and repetition details.
- `results.csv`: one aggregate row per fully measured scenario.
- `results.json`: aggregate results and individual repetition statistics.
- `manifest.json`: status, controls, commands, host/compiler details, dirty
  state, process layout, library paths, and artifact SHA-256 digests.
- Build/listing logs and per-worker logs and Criterion JSON.

For each allocator, aggregate medians are the median of its repetition
medians. Ratios divide Rust's aggregate median by C's; a negative percentage
means less time for Rust. Each row preserves its workload or normalized
operation metric. Criterion p95 describes batch measurements, not individual
allocation tail latency. Noise uses the existing summarizer's within-run
classification. One repetition remains exploratory evidence, and a suite
geomean does not predict application-wide performance.

Failed or interrupted runs retain their completed data and an **incomplete**
report. Missing or malformed expected measurements cause a nonzero exit and
suppress the overall geomean. Ctrl-C stops the active worker process group.
`report` only reads saved measurements and rewrites report files; it does not
build or execute workloads.

## Redis and RocksDB

Both are integrated into `run`, `list`, `report`, and `run --full`:

```bash
./benchmarks/bench.py run --scenario redis --scenario rocksdb
./benchmarks/bench.py run --scenario redis --scenario rocksdb --repeats 3
./benchmarks/bench.py build --applications --jobs 4
```

The first selected run downloads checksum-pinned Redis **6.2.7**, RocksDB
**10.10.1**, and gflags **2.2.2** sources. These application versions match the
upstream [mimalloc-bench setup](https://github.com/daanx/mimalloc-bench/blob/master/build-bench-env.sh).
This runner uses explicit workloads below, so results are not interchangeable
with historical `bench.sh` results. GNU make is the only additional system build
tool; gflags is built locally. No system packages or services are installed.
Source archives/extractions live in `target/bench-app-sources/`; compiler-specific
application binaries live in `target/bench-build/<configuration>/applications/`.
Initial RocksDB compilation takes several minutes. Later runs use incremental
Make/CMake builds. Build logs, source versions/checksums, executable checksums,
commands and runtime controls are saved with results.

Redis is compiled with `MALLOC=libc` and without bundled jemalloc. RocksDB
uses a portable Release build with jemalloc, compression libraries and liburing
disabled; compression is explicitly disabled at runtime. Each measured server
or `db_bench` process preloads the selected allocator, the C++ runtime (for both
allocators), and a small ownership checker. The checker verifies the library
providing `malloc` and that a libc `malloc` allocation is owned by mimalloc.
Workers fail if this proof is missing. Redis clients and the Python runner use
the system allocator. Inherited `LD_PRELOAD` is removed from every worker and
recorded in the manifest as `caller_ld_preload_removed`.

Defaults are ten samples per allocator, 1,000,000 operations, one application
thread and 256-byte values. `redis-benchmark` reports throughput from a
millisecond clock, so at 100,000 requests both allocators produced identical
rates; 1,000,000 gives about 0.2% resolution (CV 0.6-1.9%) and the two
applications together still take under a minute per allocator. Each sample uses a fresh application process; round
order alternates Rust/C as for native scenarios. Controls:

- `--app-operations`: Redis requests **per command**, RocksDB writes **per thread**.
- `--app-threads`: Redis client threads or RocksDB write workers. Redis server
  execution remains its default; this flag does not configure server I/O threads.
- `--app-value-size`: value size in bytes for both applications.
- `--app-timeout`: seconds per application command, default 120; readiness and
  shutdown also have bounded waits. Cleanup runs on client/database failures.

Redis uses a private Unix socket with TCP and persistence disabled. It warms
SET/GET with 1,000 requests each, flushes the data, then measures SET followed by
GET, with 50 connections and pipeline depth 16. Random keys use a keyspace equal
to the requested operation count; GET includes hits and misses after random SET.
Redis's client RNG is not seeded by this runner. Each sample records command
throughput, client latency percentiles and server memory INFO. The normalized
metric is the equal-weight mean of reciprocal SET and GET throughput, expressed
as nanoseconds per request; it is not individual request latency.

RocksDB runs fixed-seed `fillrandom` against a fresh directory for each sample,
with WAL enabled, synchronous writes disabled and no compression. It performs
one discarded full-sized warmup on a separate database. Its normalized metric
uses reciprocal aggregate operations/second, expressed as nanoseconds per
write. Generated databases are removed; logs and measurements remain. Disk
and page-cache behavior can materially affect results; database paths and
options are recorded, and the runner does not drop the OS page cache.

Criterion `--measurement-secs` and `--warmup-secs` govern native workloads only;
applications use the fixed counts and warmups above. Application normalized
sample medians appear alongside native results with distinct units, and the
report also shows separate Redis SET/GET and RocksDB throughput. Application p95
in the repetition table describes whole-sample variation; Redis client latency
p95 is reported separately. Applications are excluded from the native geomean.
Use larger operation counts and several repetitions for performance conclusions.
Existing smoke/release gates keep their reviewed workload sets.

For optional real-application regression checks, point `BENCH_APPLICATION_ARTIFACTS`
at a completed build's `applications/artifacts.json` and set
`ARCHITECTURE_TEST_RUST_LIB` / `ARCHITECTURE_TEST_C_LIB` to its library paths, then
run the Python test command below. These checks execute both applications with
both allocators and also prove that system malloc is rejected.

## Nginx

`./benchmarks/bench.py nginx` compares Rust mechanical_mimalloc and C mimalloc
with six HTTP workloads in separate production and debug-palloc profiles.
Select glibc explicitly with `--allocator glibc` alongside the Rust/C selections.
See [the Nginx benchmark guide](../benches/nginx/README.md) for CPU placement,
workload selection, instrumented allocator preflights, burst/idle measurements
and the local load generator's limits. This is a dedicated suite and is not
included in `run --full` or the release gates.

## Architectural probes

The C++20 workloads in [`benches/`](../benches/README.md) measure occupancy
cliffs, persistent cross-thread fan-in/fan-out, forced decommit/refill
oscillation, and thread churn with frees after owner exit. They use the same
Rust/C library builds and a separate C++ executable. Run them with
`./benchmarks/bench.py architecture`; use `architecture --help` for controls.
Phase reports retain timing, RSS, page faults and live bytes for each cycle.
`report` also regenerates these reports. Existing Criterion workloads and
release gates keep their existing behavior.

## Release gates and compatibility

`smoke` and `release` call the existing gate scripts, preserving their settings,
artifact modes, exit statuses, and validation. They retain their existing
environment controls and output directories under `target/bench-smoke/` and
`target/bench-full/`. For example:

```bash
CARGO_BUILD_JOBS=4 ./benchmarks/bench.py smoke
./benchmarks/bench.py release
```

The older scripts remain available for compatibility. The new isolated
comparison does not apply stored absolute-time thresholds or update reviewed
baselines. Keep its results separate from historical runs that measured
multiple scenarios in one process. Release methodology remains documented in
[the performance policy](../docs/performance.md).

Run the entry point's regression checks with:

```bash
python3 -m unittest discover -s benchmarks -p 'test_*.py' -v
```
