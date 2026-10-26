# Performance Hotspot Report

Date: 2026-06-18 21:09 EDT
Commit profiled: `fc790aa`
Host: Linux `6.17.0-23-generic`, x86_64

## Scope

This pass used `perf` against focused Criterion benchmark binaries, not a full
release benchmark aggregate. The goal was to identify where the team should
research performance wins without weakening correctness checks or increasing RSS.

The latest authoritative regression data remains `BENCHMARK_RESULTS.md`: the
current 10-sample aggregate showed no confirmed hotpath regression and a native
scenario geomean Rust/C median ratio of `0.9972` (`-0.28%`, Rust slightly
faster overall). No allocator hotpath code was changed during this profiling
pass.

## Commands

```bash
cargo bench --features bench --bench hotpath --no-run
cargo bench --features bench --bench mimalloc_bench --no-run

env MIMALLOC_HOTPATH_SAMPLE_SIZE=10 \
  MIMALLOC_HOTPATH_MEASUREMENT_SECS=1 \
  MIMALLOC_HOTPATH_WARMUP_SECS=1 \
  MIMALLOC_HOTPATH_NRESAMPLES=1000 \
  perf record -F 999 -g \
  --output target/perf/hotpath-paired-16.data -- \
  target/release/deps/hotpath-2d764e6f6cbe18dd \
  --profile-time 5 hotpath/paired_alloc_free/rimalloc/16

env MIMALLOC_BENCH_SCENARIOS=glibc-thread \
  MIMALLOC_BENCH_ALLOCATORS=rust \
  MIMALLOC_BENCH_SAMPLE_SIZE=10 \
  MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
  MIMALLOC_BENCH_WARMUP_SECS=1 \
  perf record -F 999 -g \
  --output target/perf/glibc-thread-rust.data -- \
  target/release/deps/mimalloc_bench-d35cafa9d69668da \
  --profile-time 8 \
  mimalloc-bench-native/glibc-thread-per-1M-malloc-free-pairs/rust

env MIMALLOC_BENCH_SCENARIOS=cache-scratchN \
  MIMALLOC_BENCH_ALLOCATORS=rust \
  MIMALLOC_BENCH_SAMPLE_SIZE=10 \
  MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
  MIMALLOC_BENCH_WARMUP_SECS=1 \
  MIMALLOC_BENCH_SKIP_BUILD=1 \
  perf record -F 999 -g \
  --output target/perf/cache-scratchN-rust.data -- \
  target/release/deps/mimalloc_bench-d35cafa9d69668da \
  --profile-time 8 \
  mimalloc-bench-native/cache-scratchN-per-workload/rust

env MIMALLOC_BENCH_SCENARIOS=glibc-thread \
  MIMALLOC_BENCH_ALLOCATORS=rust \
  MIMALLOC_BENCH_SKIP_BUILD=1 \
  perf stat -e cycles,instructions,branches,branch-misses,cache-misses -- \
  target/release/deps/mimalloc_bench-d35cafa9d69668da \
  --profile-time 3 \
  mimalloc-bench-native/glibc-thread-per-1M-malloc-free-pairs/rust
```

`perf` warned that kernel address maps were restricted, so kernel samples are
not symbolized. User-space allocator symbols are still usable.

## Findings

### 1. `glibc-thread` is the best current optimization probe

`target/perf/glibc-thread-rust.data` captured about 64k samples. Top relevant
symbols:

| Symbol | Self | Children |
|---|---:|---:|
| `mimalloc_bench::glibc_thread::malloc_benchmark_loop` | 77.66% | 88.76% |
| `operator delete[](void*)` | 8.49% | 26.96% |
| `mi_malloc` | 8.09% | 21.36% |
| `_mi_malloc_generic_typed` | 0.68% | 1.76% |
| `_mi_page_retire_internal` | 0.10% | 0.53% |
| `mi_bin` | 0.32% | 0.47% |
| `mi_page_free_list_extend` | 0.00% | 0.22% |

`perf stat` for the same scenario:

| Counter | Value |
|---|---:|
| cycles | 238,326,593,997 |
| instructions | 347,182,946,338 |
| IPC | 1.46 |
| branches | 55,150,314,767 |
| branch misses | 2,641,491,187 |
| branch miss rate | 4.79% |
| cache misses | 1,157,892,358 |

Interpretation: the useful allocator signal is in the exported `mi_malloc` and
`operator delete[]` paths plus their inlined helper code. The 4.79% branch miss
rate makes branch layout/cold splitting a plausible research target.

### 2. `cache-scratchN` is workload-dominated, not allocator-dominated

`target/perf/cache-scratchN-rust.data` captured about 3.2k samples. The flat
profile is essentially all `mimalloc_bench::cache_workloads::scratch_worker`.

`perf stat`:

| Counter | Value |
|---|---:|
| cycles | 11,928,541,465 |
| instructions | 24,042,518,035 |
| IPC | 2.02 |
| branches | 4,008,582,392 |
| branch misses | 1,055,343 |
| branch miss rate | 0.03% |
| cache misses | 951,594 |

Interpretation: this remains a useful benchmark for end-to-end A/B comparison,
but it is a poor profiler for allocator internals. The worker loop hides the
allocator symbols through inlining and scenario work.

### 3. Short hotpath and `malloc-large` profiles need better harness support

The focused 16-byte paired alloc/free profile captured only 10 samples, and
`malloc-large` captured only 32 samples. They are too short for strong ranking.
The low-confidence `malloc-large` sample did point at kernel time and
`mi_page_free_list_extend`, which matches the expected page extension/commit
path, but this should not drive changes without a dedicated profiling harness.

## Research Targets

1. Export-boundary fast path cost.

   The native benchmark intentionally calls through the cdylib ABI so the Rust
   and C allocators are compared fairly. That makes `mi_malloc` and
   `operator delete[]` real costs for C ABI users, but not necessarily for
   in-crate Rust callers. Research whether internal Rust hotpath benchmarks
   should also measure direct non-dylib entry points so we can separate ABI cost
   from allocator algorithm cost. Do not weaken or hide the exported ABI path.

2. Branch layout in `mi_malloc` and `operator delete[]`.

   The 4.79% branch miss rate in `glibc-thread` is the clearest CPU-side signal.
   Candidate areas:

   - `src/alloc.rs`: `mi_malloc`, `mi_theap_malloc_zero_nonnull`,
     `mi_theap_malloc_small_zero_nonnull`, and `_mi_page_malloc_zero_typed`.
   - `src/free.rs`: exported delete/free paths and abandoned-page fallbacks.
   - `src/page.rs`: `_mi_malloc_generic_typed`, `_mi_page_retire_internal`,
     `mi_page_free_list_extend`, and `mi_bin`.

   Safe research direction: move rare initialization, guarded, abandoned,
   collect, and OOM branches into colder `#[cold] #[inline(never)]` helpers
   where assembly confirms the fast path becomes shorter. Preserve all checks
   and failure behavior.

3. Verify typed wrapper zero-cost claims on the hot path.

   The typed migration has added wrappers around free-list and page metadata
   operations. The benchmark results are currently positive overall, but the
   next performance pass should inspect generated assembly for:

   - `_mi_page_malloc_zero_typed` free-list pop.
   - `AtomicThreadFree` operations in free/collect paths.
   - `mi_bin_typed` and `LayoutRequest`/`PaddedSize` construction at fast-path
     boundaries.

   Only replace wrappers if assembly shows real extra instructions or branches.
   Keep the typed boundary if LLVM already erases it.

4. Generic allocation collection cadence.

   `_mi_malloc_generic_typed` increments `generic_count` and periodically runs
   deferred free and retired-page collection. It is visible but small in the
   `glibc-thread` profile. Research whether the threshold/counter layout causes
   avoidable branch misses. Do not change collection frequency without checking
   RSS and abandoned-page behavior.

5. Page extension and large allocation path.

   The low-confidence `malloc-large` profile points toward kernel time and
   `mi_page_free_list_extend`. Before changing anything here, add a profiling
   harness that loops large/page-extension operations long enough to capture
   stable samples. Any optimization must be paired with RSS/commit checks,
   because larger batching can improve CPU time while increasing retained
   committed memory.

6. Benchmark profiler fidelity.

   Add profiler-specific Criterion filters or small standalone bench binaries
   that run one allocator operation pattern for a fixed wall-clock duration.
   The normal Criterion harness is good for regression detection, but several
   scenarios are too short or too workload-heavy for actionable `perf` samples.

## Recommended Next Steps

1. Add a `profile-hotpath` bench mode that loops `mi_malloc`/`mi_free`,
   `mi_malloc_small`, and C++ delete paths for 5-10 seconds without Criterion
   setup overhead.
2. Capture assembly for the typed wrappers listed above and compare against a
   raw-pointer equivalent on the same commit.
3. Prototype cold splitting for rare branches in `alloc.rs` and `free.rs`.
4. Validate any prototype with:
   - `scripts/check-lints.sh`
   - `scripts/check-bench-smoke.sh`
   - focused `glibc-thread` and `hotpath/paired_alloc_free/16` Criterion reruns
   - RSS/commit stats from the native scenarios before running a full aggregate.

## Bottom Line

No regression was found in the existing benchmark record, and no allocator
hotpath changes were made in this pass. The most promising performance research
area is branch layout and rare-path splitting around exported allocation/free
entry points. The next most important work is improving profiler fidelity so the
team can distinguish ABI overhead, benchmark workload overhead, and allocator
algorithm overhead before changing correctness-sensitive code.
