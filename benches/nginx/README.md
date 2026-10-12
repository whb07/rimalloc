# Nginx allocator benchmark

Run through the repository's existing entry point:

```sh
# Prepare builds without starting HTTP workloads.
./benchmarks/bench.py build --nginx --jobs 4

# Focused run: Rust mechanical_mimalloc versus vendored C mimalloc 3.3.2.
./benchmarks/bench.py nginx --workload static --workload headers

# Add glibc explicitly. These are the only supported allocators.
./benchmarks/bench.py nginx --allocator rust --allocator mimalloc --allocator glibc --workload churn

# Proxy body-size and buffering sweep.
./benchmarks/bench.py nginx --workload proxy --workers 4 --connections 1024

# Longer retention experiment: 30s bursts, 30s idle, 30 cycles.
./benchmarks/bench.py nginx --workload burst-idle --duration-secs 30 --idle-secs 30 --burst-cycles 30

# More substantial measurements, on explicitly separated CPU sets.
./benchmarks/bench.py nginx --workload headers --workload proxy \
  --workers 8 --connections 1024 --load-threads 8 \
  --server-cpus 0-7 --load-cpus 8-15 --upstream-cpus 16-23 \
  --warmup-secs 60 --duration-secs 120 --repeats 5

./benchmarks/bench.py report target/bench-runs/NGINX_RUN
```

`nginx --help` lists the controls. The default compares **only Rust and C
mimalloc**, runs both production and `NGX_DEBUG_PALLOC=1` profiles, and uses two
workers, 256 connections, two wrk threads, 10 seconds warmup, 30 seconds timed
load, three repetitions, and a three-second allocator preflight. Burst/idle
uses three cycles and 30-second idle phases. All six workloads expand to **18
cases**, so the default complete sweep is a multi-hour experiment. Use selected
workloads and short durations for functional validation; one-second samples
are not performance evidence. Nginx stays separate from `run --full` and from
Criterion/application geomeans.

## Builds and CPU placement

The first run downloads checksum-pinned **Nginx 1.28.0** and **wrk 4.2.0**.
It reuses the same Rust interpose and vendored C Release libraries as the other
application benchmarks. It builds Nginx once per profile/configuration and
switches allocators with `LD_PRELOAD`; glibc has no allocator preload. All three
load the C++ runtime equally. The initial guard verifies the library owning
`malloc`, `calloc`, `realloc`, `free`, `posix_memalign` and `malloc_usable_size`,
checks realloc preservation and aligned allocation, and checks mimalloc
ownership. Nginx workers inherit the verified mappings after fork. No dynamic
third-party Nginx modules are enabled.

Each profile has a separate diagnostic executable linked with allocation
counter wrappers. **Timed binaries have no counter wrappers.** Only the
counter builds link `allocator_probe.c`; all configurations include the small
`ngx_http_bench_sink_module.c` upstream handler. Production and debug-palloc
results are reported in separate tables and are never combined into a ranking.
The debug build bypasses Nginx small-object pooling and is diagnostic evidence,
not normal production behavior. See the [Nginx development guide](https://nginx.org/en/docs/dev/development_guide.html).

Nginx is a minimal HTTP build: TLS, gzip, rewrite/regex and unused FastCGI,
uwsgi and SCGI modules are disabled. The tiny response is a cached 12-byte
static file, not a rewrite-module `return` directive. wrk uses its bundled
LuaJIT and the host's OpenSSL. Requirements beyond the existing runner include
GNU make, `taskset`, `unzip`, an OpenSSL development installation and the
`openssl` command. No services or system packages are installed. Sources,
binaries, configurations, compiler flags, checksums and build logs are recorded;
artifacts stay under `target/`.

The runner requires at least three available CPUs. By default it selects one
hardware thread per physical core, divides them into disjoint server, wrk and
upstream CPU sets, and pins Nginx workers and both generator/upstream processes.
With fewer than three physical cores it falls back to disjoint logical CPUs;
those can still share physical cores. Override all three CPU sets when you need
specific placement. Oversubscribing Nginx workers or wrk threads relative to
the assigned CPU sets is flagged in reports. Worker counts and connection counts are scalar controls;
repeat commands to sweep 1/2/4/8/16 workers or 256/1024/4096/16384 connections.
Nginx's connection limit includes upstream sockets; the configured 16,384
connections per worker is not a promise of that many proxied clients per worker.
The runner requires a descriptor hard limit of at least 65,536 and raises its
own soft limit when needed.

## Workloads

- **Static 200** (`static`): cached `hello world\n` with keepalive and sendfile.
  This measures baseline request processing and pool backing allocations.
- **Connection churn** (`churn`): the same endpoint with keepalive timeout zero.
  Transport or HTTP errors fail the run. Ephemeral ports/TIME_WAIT and generator
  capacity can limit this workload; the runner does not change kernel settings.
- **Large headers** (`headers`): eight headers alternating 128/512/1024/4096 bytes.
  Configured large-header buffers accept the requests; an initial correctness
  request checks the actual endpoint. These are pool/buffer operations, not
  eight independent system allocations of the header sizes.
- **Reverse proxy** (`proxy`): deterministic 1/16/64/256-KiB upstream files,
  each with proxy buffering on and off while keeping other settings fixed.
  The upstream is a separate glibc Nginx instance pinned to its own CPUs. A
  direct wrk run measures upstream headroom. Less than 25% headroom is flagged;
  it may indicate an upstream **or generator** bottleneck, not an allocator
  regression. Proxy buffer configuration matches the proposed 8k/16x16k setup;
  proxy temp-file spilling is disabled for this bounded-body experiment.
- **Upload/download** (`transfer`): separate POST uploads and static downloads
  at 4 KiB, 64 KiB and 1 MiB. Frontend uploads stream to the isolated upstream.
  The sink responds with 204 **only after reading the complete body** and
  validates its length, returning `X-Received-Bytes`. Correctness checks assert
  that value. Upstream uses a 2-MiB body buffer so it can consume these known
  body sizes without becoming a disk-write benchmark.
- **Burst and idle** (`burst-idle`): repeated tiny-response load and idle periods
  in the same timed Nginx instance. Each load has its own metrics and each idle
  phase records worker RSS/CPU/fault checkpoints, including the final idle
  checkpoint. Use longer cycles for retention conclusions. No forced allocator
  collection is injected.

Each allocator/profile/case/repetition uses a fresh Nginx instance. The complete
configuration order is shuffled reproducibly with `--order-seed`, with order
recorded in the manifest. Ports are selected locally and every listener binds
loopback. Upstream, client and target use separate processes; only the target
gets the selected allocator. Graceful shutdown is bounded and has a kill
fallback. Generated payload files are removed; diagnostic traces, HTTP logs,
configuration files and resource samples remain. Existing Nginx instances are
never signalled. Graceful worker recycling under active connections remains a
future seventh workload.

## Measurements and limits

The diagnostic preflight records Nginx executable allocation sites:
`malloc`, `calloc`, `realloc`, `free` and `posix_memalign`; requested/usable byte
totals; tracked live/peak usable bytes; and logarithmic allocation-size buckets.
`free_calls` counts non-null frees; zero-pointer frees are ignored.
These use link-time `--wrap`, so hidden allocations within libc/shared
libraries are **outside the counter scope**. Workers inherit tracked master
allocation state at fork. Report live/peak values as scoped tracking, not exact
process heap accounting. Histogram bucket `i` covers `[2^i, 2^(i+1)-1]`, with
zero-size requests in bucket zero and sizes >=2^31 in the last bucket.

The runner measures worker-counter deltas around the diagnostic HTTP run and
normalizes allocations and requested bytes by completed requests. A preflight
with no allocation traffic fails; density below 0.01 allocations/request is
flagged as weak sensitivity. Diagnostic throughput and counter overhead are
not mixed with timed results. Baseline static serving can be valid HTTP evidence
without being sensitive to every allocator mechanism.

Timed runs preserve completed requests, throughput, known payload byte rate,
wrk p50/p95/p99 latency, whole-worker CPU seconds per request, worker and
upstream RSS/CPU/page-fault samples, and local generator resource samples.
Resource samples occur once per second (more often for short validation runs).
RSS sums worker RSS and can double-count shared pages. Peak RSS is a sampled
peak; it can miss shorter transients. HTTP latency is wrk's closed-loop histogram,
not allocator operation latency or fixed-arrival-rate latency. Repetition
aggregates use independent run medians; burst cycles remain available in JSON.

Allocator-only CPU cycles, allocator operation latency distributions, exact
commit/decommit/purge counts, remote frees and contention counters are **not
measured** by this portable integration. Worker CPU time and OS page faults do
not substitute for those internal metrics. Use a separate profiler to attribute
mechanisms, and use the architectural cross-thread probes for remote-free
behavior. The report makes no maximum-sustainable-throughput claim.

This implementation is a **local, closed-loop validation suite**. Separate CPU
sets reduce interference but do not equal an independent load-generator host.
Every result carries that limitation, and insufficient upstream headroom is
flagged separately. For external-load experiments, reuse the saved Nginx config
and Lua workload on a dedicated generator host and independently establish
headroom and fixed-load latency; remote orchestration is not implemented here.

Saved `manifest.json`, `nginx.json` per worker, raw wrk logs, binary counter
files, `results.json`, `results.csv`, and `summary.md` make the runs reviewable.
Failures and interruptions retain an incomplete report; missing or malformed
workers cannot produce a complete result. `bench.py report` reruns no workloads.

Run the runner tests with:

```sh
python3 -m unittest discover -s benchmarks -p 'test_*.py' -v
```

Set `BENCH_NGINX_ARTIFACTS` to the build cache's `artifacts.json` to include the
real Nginx preload guard regression. The full short smoke sweep used during
implementation is reproducible with:

```sh
./benchmarks/bench.py nginx --workers 1 --connections 32 --load-threads 1 \
  --duration-secs 1 --warmup-secs 1 --preflight-secs 1 --repeats 1 \
  --burst-cycles 2 --idle-secs 1 --jobs 4
```
