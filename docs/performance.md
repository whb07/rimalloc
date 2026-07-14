# Performance Policy

Allocator changes must preserve both hot-path behavior and whole-workload
performance relative to C mimalloc 3.3.2.

## Required Evidence

- Use `scripts/check-bench-smoke.sh` for fast iteration and for changes that
  may affect allocator hot paths.
- Use `scripts/check-bench-full.sh` before treating allocator-wide results as
  authoritative.
- Before a release candidate, run `scripts/check-bench-full.sh` and preserve
  each mode's `manifest.json`, `environment.txt`, `commands.txt`, and
  `resource-results.jsonl`; preserve `criterion/` and `threshold-results.csv`
  for all four Criterion modes, their `benchmark-review.json` status, plus the
  default mode's `evidence-report.json`. Attach the generated
  `stages-0-3-evidence-<commit>.tar.zst`, its `.sha256` sidecar, and
  `evidence-index.json`; the index records the commit, host profiles, all five
  artifact modes, and assembly manifest. Update
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

The resource runner invokes `benchmark_probe` once per
allocator/scenario/repetition and writes `resource-results.jsonl`. Those records
contain process/resource consistency fields: start/end/peak RSS and commit,
logical live and peak-live requested bytes, allocator current/peak committed
bytes, combined page-fault deltas, normalized ratios, and timing variation.

The full gate separately runs `evidence_report`, preserves
`evidence-report.json`, and validates its schema. That report contains page
occupancy, page-search length, relaxed diagnostic counters such as
`remote_free_cas_retries`, separate minor/major fault deltas, and sampled
p99/p999 allocation and remote-free latency. `page_occupancy` is live binned
block bytes divided by the capacity of live small, medium, and large allocator
pages; huge allocations are excluded. `page_search_length` is candidates
examined per page search. On Linux, the report's RSS values are KiB from
`getrusage`; `live_requested_bytes` is tracked from the reporter's live set
instead of mimalloc's cumulative requested-byte statistic.

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
The full release gate enforces reviewed thresholds and preserves results in
distinct `page-batch-plain` and `page-batch-secure` artifact modes.

The native mimalloc-bench Criterion ports support:

- `MIMALLOC_BENCH_SCENARIOS`
- `MIMALLOC_BENCH_ALLOCATORS`
- `MIMALLOC_BENCH_SAMPLE_SIZE`
- `MIMALLOC_BENCH_MEASUREMENT_SECS`
- `MIMALLOC_BENCH_WARMUP_SECS`

The full `workloads` run also preserves measured persistent fan-in, fan-out,
cyclic transfer, owner-exit, thread-churn, and oversubscribed ownership
topologies in the default-mode Criterion artifact. These are performance
scenarios in addition to the matching correctness tests.

`scripts/check-bench-full.sh` keeps release-candidate measurements separate by
artifact mode:

- Default Rust library mode uses `target/bench-full/default-rust`.
- Feature-linked interposition mode uses
  `target/bench-full/interpose-feature-linked`. This executable directly calls
  the Rust facade, raw API, and system allocator while the interposition symbols
  are linked into the process; it is feature-build evidence, not preload evidence.
- Deployed interposition mode uses `target/bench-full/ld-preload-interpose`. It
  compiles a neutral C probe that does not link this crate, builds the release
  interposition cdylib, and reaches it through `LD_PRELOAD` and libc allocation
  calls. Its allocator label is `preloaded-libc`; no independent system control
  is claimed inside the preloaded process.
- Plain page-batch diagnostics use `target/bench-full/page-batch-plain`.
- Secure encoded-link page-batch diagnostics use
  `target/bench-full/page-batch-secure`.

Keep those target directories separate, or override
`MIMALLOC_BENCH_DEFAULT_TARGET_DIR`, `MIMALLOC_BENCH_INTERPOSE_TARGET_DIR`, and
`MIMALLOC_BENCH_PRELOAD_TARGET_DIR`,
`MIMALLOC_BENCH_PAGE_BATCH_PLAIN_TARGET_DIR`, and
`MIMALLOC_BENCH_PAGE_BATCH_SECURE_TARGET_DIR` with distinct paths. Release
reports must label each section with the artifact mode and feature set.
Rust-vs-C ratios come from the native scenario benchmarks in the default Rust
library mode, while linked-feature, deployed-preload, and diagnostic page-batch
evidence are reported separately.

`benchmarks/thresholds.json` schema 2 keys each baseline by artifact mode and
feature set and gates both sample median and p95. The allocator-wide release
baseline is the median of five clean-run medians and the median of five
clean-run p95 values from one commit and one documented host profile. Generate
a proposal with `scripts/run-benchmark-baseline.sh`; promotion requires manual
outlier review and the explicit `--write-reviewed-baseline` capture flag.
The full gate and baseline runner use identical default controls: 100 samples,
10 seconds, and a 1-second warmup for hotpath measurements; 20 samples,
5 seconds, and a 2-second warmup for the longer native workloads. Keep these
defaults aligned because Criterion's native flat-sampling batch size changes
with the controls and is part of the benchmark methodology.
Both runners prebuild the thresholded benchmark executables and apply a
recorded 60-second cooldown before each sustained native group. The baseline
runner prebuilds all five clean artifacts before recording its five
fresh-process hotpath replicas contiguously; long native and resource workloads
must not be interleaved between microbenchmark replicas. This bounds unrelated
host phase drift without discarding individual measurements or selecting
favorable runs.
The selected single-thread hotpath threshold probe is additionally pinned to
logical CPU 0 in both runners to prevent cross-CCD migration. The full hotpath
matrix and every multithreaded workload remain unpinned.
Page-batch baselines use one clean 100-sample Criterion artifact for each
feature-keyed mode and are proposed or promoted with
`scripts/capture-page-batch-baseline.py`; the source commit and sample count are
recorded per threshold. Schema-1 parsing is retained for one migration cycle
only.

The per-entry median and p95 limits are hard infrastructure ceilings intended
to tolerate host noise. They do not replace the stricter investigation budgets
below. `scripts/check-bench-thresholds.py` records both decisions in
`threshold-results.csv` and writes `benchmark-review.json`; the selected
current/baseline median geomean above 1.01, or an individual result above its
2%/3% budget, produces a mandatory `review-required` status even when the hard
ceiling still passes.

Every full-run manifest records the resolved sample size, measurement and
warmup durations, selected native scenarios and allocators, resource operation
count, and repeat count under `controls`. `commands.txt` repeats those resolved
values in the actual command lines.

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

`benchmarks/resource-thresholds.json` applies scenario-specific hard ceilings
to peak committed/logical bytes, end-minus-start committed bytes, peak
RSS/logical bytes, and page faults per million operations for both `alloc_free`
and `remote_free_1x1`. The full gate applies these to every required allocator
surface and to `preloaded-libc`; resource validation is therefore a memory and
fault regression gate, not only a schema/noise check.

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
