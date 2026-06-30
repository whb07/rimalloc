# Performance Policy

Allocator changes must preserve both hot-path behavior and whole-workload
performance relative to C mimalloc 3.3.2.

## Required Evidence

- Use `scripts/check-bench-smoke.sh` for fast iteration and for changes that
  may affect allocator hot paths.
- Use `scripts/check-bench-full.sh` before treating allocator-wide results as
  authoritative.
- Before a release candidate, run `scripts/check-bench-full.sh`, capture
  `scripts/env-info.sh`, preserve the default
  `target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv` and
  interpose `target/bench-full/interpose/criterion/benchmark-thresholds-full.csv`
  threshold reports, and update `BENCHMARK_RESULTS.md` with the exact command
  lines, feature flags, artifact modes, Rust/C ratios, and noisy benchmark notes
  for the release commit.
- Capture `scripts/env-info.sh` output with any committed full benchmark report.
- Record benchmark evidence in `BENCHMARK_RESULTS.md`, including commit hash,
  allocator versions, feature flags, command lines, and noisy-benchmark notes.
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

Do not compare Criterion's saved baseline across those two modes. Criterion
benchmark IDs do not encode Cargo features, so use a fresh `CARGO_TARGET_DIR`,
clear `target/criterion`, or save separate external reports when switching
between default and interposition measurements.

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

## Regression Budgets

Treat any of these as requiring investigation before merge or release:

- Hotpath Criterion probe regresses by more than 2% and Criterion reports the
  change outside the configured noise threshold.
- A native ported scenario regresses by more than 3% in a repeat run.
- Overall geomean across the selected allocator workload set regresses by more
  than 1%.
- Any low-level allocator path shows a new allocation, syscall, lock, panic,
  or formatting dependency in a documented no-allocation critical path.

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
