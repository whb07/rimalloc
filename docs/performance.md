# Performance Policy

Allocator changes must preserve both hot-path behavior and whole-workload
performance relative to C mimalloc 3.3.2.

## Required Evidence

- Use `scripts/check-bench-smoke.sh` for fast iteration and for changes that
  may affect allocator hot paths.
- Use `scripts/check-bench-full.sh` before treating allocator-wide results as
  authoritative.
- Before a release candidate, run `scripts/check-bench-full.sh` and preserve
  each mode's `manifest.json`, `environment.txt`, `commands.txt`, `criterion/`,
  `resource-results.jsonl`, and `threshold-results.csv`. Update
  `BENCHMARK_RESULTS.md` with the exact command
  lines, feature flags, artifact modes, Rust/C ratios, and noisy benchmark notes
  for the release commit.
- Capture `scripts/env-info.sh` output with any committed full benchmark report.
- Record benchmark evidence in `BENCHMARK_RESULTS.md`, including commit hash,
  allocator versions, feature flags, command lines, and noisy-benchmark notes.
- Publish a per-release performance summary in `BENCHMARK_RESULTS.md` or an
  attached report. Include geomean Rust/C ratios, hot-path deltas against the
  previous release evidence, release-blocking regressions, noisy watch items,
  and links to raw Criterion summaries or threshold CSV artifacts.
- Use `scripts/summarize-criterion.py` to summarize Criterion JSON into
  mean/median/min/p95/max, noisy/stable classification, and Rust/C geomean
  ratios. Prefer exact `--id-file` inputs for release reports so stale
  historical Criterion directories cannot leak into the aggregate.

## Fast Iteration

Quick runs are acceptable while developing:

- Start with short Criterion settings and the changed scenario set.
- Escalate from quick samples to longer runs when confidence intervals overlap,
  results conflict with prior evidence, or the scenario is known noisy.
- Skip benchmark reruns for documentation, packaging, lint-only, and export-only
  changes when generated allocator logic and hot-path control flow do not move.

The hotpath Criterion harness supports:

- `MIMALLOC_HOTPATH_SAMPLE_SIZE`
- `MIMALLOC_HOTPATH_MEASUREMENT_SECS`
- `MIMALLOC_HOTPATH_WARMUP_SECS`
- `MIMALLOC_HOTPATH_NRESAMPLES`
- `MIMALLOC_BENCH_ASSERT_NO_HARNESS_ALLOC=1` for a diagnostic allocation-count pass

The harness reads the allocation-check switch once while configuring each
benchmark group. Normal measurements do not inspect the process environment or
snapshot diagnostic counters inside timed iterations. The smoke gate enables
the check explicitly; full performance runs leave it disabled.

## Evidence-layer realism

Run the public Rust facade and raw C ABI as separate Criterion groups:

```sh
cargo bench --features bench --bench evidence
```

The `workloads`, `hotpath`, and `evidence` harnesses preallocate pointer slots,
layouts, random schedules, and persistent worker threads before Criterion enters
the timed closure. Allocator operations and synchronization intrinsic to a
concurrent workload remain timed; harness construction does not.

Capture fresh-process allocator and operating-system evidence with:

```sh
scripts/run-bench-resources.py --artifact-dir target/bench-full/default-rust \
  --artifact-mode default-rust --feature bench
```

The runner invokes `benchmark_probe` once per allocator/scenario/repetition and
emits JSON Lines. Records include start/end/peak RSS and commit, logical live
and peak-live requested bytes, allocator current/peak committed bytes, page
occupancy, page-search length, remote CAS retries, minor/major faults, and
sampled p99/p999 latency where applicable. `page_occupancy` is live binned block bytes
divided by the capacity of live small, medium, and large allocator pages; huge
allocations are excluded. `page_search_length` is candidates examined per page
search. Tail latency is sampled separately for paired facade allocation/free
and remote free. On Linux, RSS values are KiB and come from `getrusage`; both
the process-lifetime peak and the delta from the pre-workload peak are emitted.
`live_requested_bytes` is tracked by the reporter's live allocation set instead
of mimalloc's cumulative requested-byte statistic. Minor and major page faults
are workload deltas. Diagnostic counters, including
`remote_free_cas_retries`, are benchmark-only and use relaxed atomics.

The randomized C/Rust differential trace includes ordinary allocation,
alignment, realloc, heap allocation, owner-exit remote free, heap destruction,
and forced collection transitions:

```sh
scripts/check-differential-trace.py
```

## Build Mode Comparability

Hotpath results are only comparable when the Cargo feature set and artifact mode
match. The default command:

```sh
cargo bench --features "std bench" --bench hotpath
```

measures the default Rust library build. The C ABI/interposition hot path should
be measured explicitly:

```sh
cargo bench --features "std bench interpose" --bench hotpath
```

Do not compare Criterion's saved baseline across those two modes. Use the
artifact-specific clean directories created by `run-bench-clean.sh`; never
copy or reuse `criterion/` data between default and interposition measurements.

Stage 3 page-batch slow paths use opaque fixtures whose list setup is excluded
from timing. Run both default and secure encoded-link modes:

```sh
cargo bench --features "bench bench-diagnostics" --bench page_batch
cargo bench --features "bench bench-diagnostics secure" --bench page_batch
```

The suite covers forced page-free collection, local-to-available and detached
remote collection at lengths 1, 8, 32, and 128, and empty/nonempty destination
lists. The forced-collector case includes the feature's fixed relaxed-counter
update, so compare it only against runs with the same feature set; production
fast-path equivalence is established separately by release assembly capture.

The native mimalloc-bench Criterion ports support:

- `MIMALLOC_BENCH_SCENARIOS`
- `MIMALLOC_BENCH_ALLOCATORS`
- `MIMALLOC_BENCH_SAMPLE_SIZE`
- `MIMALLOC_BENCH_MEASUREMENT_SECS`
- `MIMALLOC_BENCH_WARMUP_SECS`

`scripts/check-bench-full.sh` keeps release-candidate measurements separate by
artifact mode:

- Default Rust library mode uses `target/bench-full/default-rust`.
- `interpose` mode uses `target/bench-full/interpose`.

Keep those target directories separate, or override
`MIMALLOC_BENCH_DEFAULT_TARGET_DIR` and `MIMALLOC_BENCH_INTERPOSE_TARGET_DIR`
with two different paths. Release reports must label each section with the
artifact mode and feature set. Rust-vs-C ratios come from the native scenario
benchmarks in the default Rust library mode, while interpose hot-path evidence
is reported separately.

`benchmarks/thresholds.json` schema 2 keys each baseline by artifact mode and
feature set and gates both sample median and p95. A release baseline is the
median of five clean-run medians and the median of five clean-run p95 values
from one commit and one documented host profile. Generate a proposal with
`scripts/run-benchmark-baseline.sh`; promotion requires manual outlier review
and the explicit `--write-reviewed-baseline` capture flag. Schema-1 parsing is
retained for one migration cycle only.

## Regression Budgets

Treat any of these as requiring investigation before merge or release:

- Hotpath Criterion probe regresses by more than 2% and Criterion reports the
  change outside the configured noise threshold.
- A native ported scenario regresses by more than 3% in a repeat run.
- Overall geomean across the selected allocator workload set regresses by more
  than 1%.
- Any low-level allocator path shows a new allocation, syscall, lock, panic,
  or formatting dependency in a documented no-allocation critical path.
- A new strict internal wrapper changes release assembly for free-list pop,
  local free, remote free, small page lookup, or another tracked hot path by
  adding a branch, call, memory operation, lock prefix, stack spill, formatting,
  allocation, panic path, bounds-check slow path, or wrapper-constructor traffic
  without a measured justification.

Small single-run deltas are not authoritative when the scenario is explicitly
classified noisy. Re-run noisy cases with more samples or a longer measurement
window before changing code or claiming a regression.

## Scenario Classification

- Hotpath probes: `hotpath/paired_alloc_free`, `hotpath/free_only`,
  `hotpath/lifo_stack`, and `hotpath/single_reuse`.
- Allocator micro-workloads: `glibc-thread`, `malloc-large`,
  `xmalloc-testN`, `cache-scratch*`, and `cache-thrash*`.
- Application-like workloads: `cfrac`, `barnes`, `alloc-test*`, `larson*`,
  `rptest*`, `sh*bench*`, and any future redis/rocksdb-style integration.

Espresso is intentionally not part of the current required Rust-port benchmark
set.

## Acceptance

A performance-sensitive change is acceptable only when:

1. The relevant hotpath or workload subset is measured at the changed commit.
2. Any noisy result is either resolved by a longer repeat or explicitly marked
   as a watch item.
3. No hotpath regression or overall-score regression exceeds the budgets above.
4. The report preserves raw command lines so the run can be repeated.
