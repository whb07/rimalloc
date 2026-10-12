# Architectural allocator probes

`architectural_bench.cpp` implements four Linux C++20 probes. The same binary
loads either allocator library directly; harness containers and thread support
use the system allocator. No preload is needed. Run from the repository root:

```sh
./benchmarks/bench.py architecture
./benchmarks/bench.py architecture --scenario cross-thread --threads 8 --block-size 64
./benchmarks/bench.py architecture --scenario decommit-oscillation --bytes 134217728 --cycles 16
./benchmarks/bench.py architecture --repeats 5 --output target/bench-runs/architecture-five
./benchmarks/bench.py report target/bench-runs/architecture-five
```

Defaults: 64 MiB requested live payload, 4096-byte blocks, four workers, eight
cycles, three independent process repetitions. Rust/C order alternates by
repetition. Each scenario/library pair has a fresh process. Integer division
rounds payload down to whole blocks (and equal worker partitions for threaded
cases); reports record actual requested live bytes. `--threads` controls the
threaded cases; `--cycles` controls all except the fixed occupancy sweep.

- **Segment occupancy cliffs** (`segment-occupancy`): allocate and physically
  touch a private heap, enumerate its actual pages through `mi_heap_visit_blocks`,
  retain half and then a tenth of each page's blocks (rounded up), retain one
  block per page, then free everything. Record each free phase and forced heap
  collection separately. Finally allocate the same number of survivors packed
  into fresh pages after collection. Mimalloc 3 uses arena slices/pages rather
  than the older segment design, so no segment size or pointer masking is
  assumed. `occupied_pages` counts allocation pages, not OS pages. The packed
  comparison is in the same process after the sweep and is deliberately warm;
  it demonstrates retained footprint, not an independent timing comparison.
- **Cross-thread fan-in/fan-out** (`cross-thread`): persistent workers allocate
  equal pools that the main thread frees, then the main thread allocates pools
  that workers free concurrently. Barriers publish pointer ownership and keep
  owners alive throughout the cycles. Allocation and free phases are separate;
  times for concurrent phases include barrier dispatch/completion. Initial
  thread construction and payload verification are outside those timers.
- **Decommit oscillation** (`decommit-oscillation`): repeatedly allocate/touch,
  free, and forcibly collect the same-sized pool. Sets purge delay to zero and
  arena purge multiplier to one before allocation; this is an explicit purge
  pressure experiment. Compare purge cost, RSS drops, subsequent touch latency,
  and minor faults. It does not measure the default delayed-purge policy.
- **Thread churn** (`thread-churn`): create a new batch of owner threads every
  cycle, allocate/touch their pools, explicitly run `mi_thread_done`, and join.
  The main thread validates and frees surviving allocations after owner exit,
  then collects abandoned memory. Creation/allocation/teardown/join are timed
  together; owner-exit frees and collection get separate timers. This includes
  OS thread cost and is not an isolated `mi_thread_done` microbenchmark.

Every payload page is written, including the last page; the writes are checked
before frees, outside timed free phases. Allocation failures, corrupted
payloads, missing symbols, and a heap visitor count mismatch fail the worker.
The runner validates phase ordering and metrics and preserves failed or
interrupted results with an incomplete report.

Each phase emits JSON with wall time in nanoseconds, operation count, live
requested bytes, RSS, RSS delta, minor/major fault deltas, and occupied-page
count (`-1` when unavailable). RSS comes from `/proc/self/statm`, not an
allocator's estimate. It covers the whole process, including harness storage;
page fault deltas also include checkpoint bookkeeping. They are process
measurements, not exact allocator accounting. A collection call does not
promise every allocation page can be released. Baseline records effective
purge delay. Detailed JSON/CSV retain cycles separately so first-use behavior
is visible; the Markdown report compares phase medians across process
repetitions. No mixed time/memory score, latency percentile, or regression
threshold is inferred from these exploratory probes.

Sweep block sizes (e.g. 64, 4096, 65536), payload sizes and worker counts to
expose transitions. For timing comparisons, use an otherwise idle machine and
several repetitions; use identical CPU affinity and allocator environment for
both libraries. Retained RSS and refaults are often more revealing here than
raw allocation throughput.

For standalone debugging, compile and run the same source:

```sh
c++ -std=c++20 -O2 -Wall -Wextra -Werror -pthread \
  -I mimalloc-3.3.2/include benches/architectural_bench.cpp -ldl \
  -o target/architectural-bench
target/architectural-bench --library /absolute/path/libmimalloc.so \
  --scenario segment-occupancy --bytes 8388608 --block-size 4096 --threads 4 --cycles 2
```

The runner/report regressions are included in the standard benchmark Python
checks. To also compile the C++ probe and exercise all four against existing
library builds (including 64-byte and 64-KiB occupancy cases), run:

```sh
ARCHITECTURE_TEST_RUST_LIB=/absolute/path/libmechanical_mimalloc.so \
ARCHITECTURE_TEST_C_LIB=/absolute/path/libmimalloc.so \
  python3 -m unittest discover -s benchmarks -p 'test_*.py' -v
```

The real-library checks are skipped when these paths are absent. A completed
architecture run records usable library paths in its `manifest.json`.
