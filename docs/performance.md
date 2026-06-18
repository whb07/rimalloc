# Performance Policy

Allocator changes must preserve both hot-path behavior and whole-workload
performance relative to C mimalloc 3.3.2.

## Required Evidence

- Use `scripts/check-bench-smoke.sh` for fast iteration and for changes that
  may affect allocator hot paths.
- Use `scripts/check-bench-full.sh` before treating allocator-wide results as
  authoritative.
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

The native mimalloc-bench Criterion ports support:

- `MIMALLOC_BENCH_SCENARIOS`
- `MIMALLOC_BENCH_ALLOCATORS`
- `MIMALLOC_BENCH_SAMPLE_SIZE`
- `MIMALLOC_BENCH_MEASUREMENT_SECS`
- `MIMALLOC_BENCH_WARMUP_SECS`

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
