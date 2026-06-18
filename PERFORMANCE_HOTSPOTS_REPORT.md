# Performance Hotspots Report

Date: 2026-06-18

This report summarizes a local `perf`/flamegraph pass intended to identify
where the team should research additional performance without weakening
correctness or increasing RSS.

## Method

Bench binaries were built once and profiled directly, so compile time was not
part of the samples:

```sh
cargo bench --features bench --bench hotpath --no-run
cargo bench --features bench --bench mimalloc_bench --no-run
```

Representative runs:

```sh
perf stat -d -r 3 -- \
  target/release/deps/hotpath-... \
  --bench hotpath/paired_alloc_free/mechanical_mimalloc/16 --quiet

perf stat -d -r 3 -- \
  target/release/deps/mimalloc_bench-... \
  --bench mimalloc-bench-native --quiet
```

Sampled profiles:

```sh
perf record -F 999 -g --call-graph dwarf,16384 \
  -o target/perf-hotspots/hotpath_paired_16.data -- \
  env MIMALLOC_HOTPATH_SAMPLE_SIZE=10 \
      MIMALLOC_HOTPATH_MEASUREMENT_SECS=3 \
      MIMALLOC_HOTPATH_WARMUP_SECS=1 \
      MIMALLOC_HOTPATH_NRESAMPLES=1000 \
      target/release/deps/hotpath-... \
      --bench hotpath/paired_alloc_free/mechanical_mimalloc/16 --quiet

perf record -F 49 -g --call-graph dwarf,8192 \
  -o target/perf-hotspots/native_cache_scratch.data -- \
  env MIMALLOC_BENCH_SCENARIOS=cache-scratchN \
      MIMALLOC_BENCH_ALLOCATORS=rust \
      MIMALLOC_BENCH_SAMPLE_SIZE=10 \
      MIMALLOC_BENCH_MEASUREMENT_SECS=3 \
      MIMALLOC_BENCH_WARMUP_SECS=1 \
      target/release/deps/mimalloc_bench-... \
      --bench mimalloc-bench-native --quiet
```

Flamegraphs were rendered with:

```sh
perf script -i target/perf-hotspots/<profile>.data \
  | rustfilt \
  | inferno-collapse-perf \
  > target/perf-hotspots/reports/<profile>.folded

inferno-flamegraph target/perf-hotspots/reports/<profile>.folded \
  > target/perf-hotspots/reports/<profile>.svg
```

Generated local artifacts:

- `target/perf-hotspots/reports/hotpath_paired_16.svg`
- `target/perf-hotspots/reports/native_cache_scratch.svg`
- `target/perf-hotspots/reports/*.report.txt`
- `target/perf-hotspots/*.stat`

`perf_event_paranoid=1` allowed user-space profiling. Kernel symbols were
restricted, so kernel samples are not useful in this run.

## Confidence Notes

- `hotpath_paired_16`: useful but not perfect; `perf record` reported 10.68%
  lost samples at high frequency.
- `native_cache_scratch`: good profile; only four lost samples.
- `native_glibc_thread` and `native_xmalloc`: useful for coarse attribution
  only. The workloads are highly parallel and DWARF call-graph capture dropped
  many samples. Treat exact percentages from those two profiles as directional,
  not authoritative.

## Counter Summary

Hotpath paired alloc/free:

- 4.18 s elapsed across three `perf stat` repeats.
- 3.90 instructions per cycle.
- 0.07% branch misses.
- 6.23% L1 data-load miss rate.

Interpretation: the tight single-threaded fast path is not branch-mispredict
limited. Any win here probably comes from fewer instructions, shorter exported
call chains, or better codegen around the block pop path.

Selected native workload aggregate:

- 87.2 s elapsed across three repeats.
- 1.00 instructions per cycle.
- 1.69% branch misses.
- 4.11% L1 data-load miss rate.
- About 29 CPUs utilized.

Interpretation: the broader threaded workload is contention, synchronization,
and memory-system sensitive. Wins here are more likely in cross-thread free,
abandoned-page reclaim, queue scanning, and page/free-list refill behavior than
in the tiny local alloc/free pair alone.

## Hotspots To Research

### 1. Allocation Fast Path

Primary symbols:

- `mechanical_mimalloc::alloc::_mi_theap_malloc_zero`
- `mechanical_mimalloc::alloc::_mi_page_malloc_zero_typed`

Source:

- `src/alloc.rs`: `_mi_page_malloc_zero_typed`
- `src/alloc.rs`: `_mi_theap_malloc_zero_ex`
- `src/alloc.rs`: `_mi_theap_malloc_zero`

Why it matters:

The hotpath profile put nearly all allocator-attributed samples under
`_mi_theap_malloc_zero`, with the tight benchmark otherwise mostly inside the
Criterion loop. The counter profile also shows high IPC and very low branch
misses, so this path is already efficient but still the most important place to
look for instruction-count savings.

Research ideas:

- Compare the emitted assembly for `_mi_theap_malloc_zero_ex`,
  `mi_theap_malloc_small_zero_nonnull`, and `_mi_page_malloc_zero_typed`
  against upstream C for the 16/64-byte path.
- Verify that strict-type wrappers in `_mi_page_malloc_zero_typed` compile to
  exactly the same load/store sequence as the raw C path, especially the
  `AvailableFreeList::pop` path.
- Investigate whether internal `#[inline(never)]` boundaries retained for
  audit/export shape are costing the hot path when called through Rust
  benchmark shims. Do not remove symbol-retention boundaries without preserving
  ABI and audit checks.
- Check whether repeated `mi_page_usable_block_size(page)` and block-size
  calculations in the fast path can be proven redundant by the compiler or
  safely hoisted without changing stats, padding, or usable-size behavior.

Correctness/RSS guardrails:

- Preserve C ABI behavior, null/error behavior, padding behavior, stats
  behavior, and thread-local heap initialization behavior.
- Do not change page sizing, extend counts, or collection thresholds as part of
  this research track.

### 2. Free-List Refill And Page Extension

Primary symbols from xmalloc directional profile:

- `mechanical_mimalloc::page::mi_page_free_list_extend`
- `mechanical_mimalloc::page::mi_page_extend_free`
- `mechanical_mimalloc::page::_mi_malloc_generic_typed`

Source:

- `src/page.rs`: `mi_page_free_list_extend`
- `src/page.rs`: `mi_page_extend_free`
- `src/page.rs`: `_mi_malloc_generic_typed`

Why it matters:

`xmalloc-testN` exposed refill and extension work after the benchmark threads
had churned through local free lists. This is not the tightest path, but it is
visible in free-heavy and refill-heavy workloads.

Research ideas:

- Compare the refill loop in `mi_page_free_list_extend` against upstream C for
  instruction count and stores per block.
- Check whether block-link helper calls inline completely in release builds.
- Investigate a specialized non-secure/default path for `MI_SECURE < 3` only if
  the generated code still preserves secure and guarded feature behavior.
- Measure refill batch sizes before changing them. Larger batches can improve
  throughput but may increase retained free memory or RSS; that tradeoff is not
  acceptable without explicit evidence.

Correctness/RSS guardrails:

- Do not increase page capacity, reserved slices, refill batch size, or page
  retention without RSS evidence.
- Preserve encoded free-list behavior under secure mode.

### 3. Cross-Thread Free, Partial Collection, And Reclaim

Primary symbols from xmalloc directional profile:

- `mechanical_mimalloc::free::mi_free_try_collect_mt`
- `mechanical_mimalloc::page::_mi_page_free_collect_partly`
- `mechanical_mimalloc::page::_mi_page_free_collect`
- `mechanical_mimalloc::free::mi_abandoned_page_try_reclaim`
- `mechanical_mimalloc::arena::_mi_arenas_page_try_reabandon_to_mapped`

Source:

- `src/free.rs`: `mi_free_try_collect_mt`
- `src/free.rs`: `mi_abandoned_page_try_reclaim`
- `src/page.rs`: `_mi_page_free_collect_partly`
- `src/page.rs`: `_mi_page_free_collect`
- `src/arena.rs`: `_mi_arenas_page_try_reabandon_to_mapped`

Why it matters:

The broad threaded workload has much lower IPC and visible allocator samples in
collection/reclaim paths. These paths are also correctness-sensitive because
they publish and consume page ownership, local free lists, and abandoned-page
state.

Research ideas:

- Audit repeated page-state predicates in `mi_free_try_collect_mt`,
  `mi_abandoned_page_try_reclaim`, and `_mi_arenas_page_try_reabandon_to_mapped`
  for redundant loads that can be safely cached inside a single unsafe block.
- Compare the typed atomic wrapper codegen for thread-free collection against
  upstream C acquire/release operation sequences.
- Investigate whether option reads in reclaim paths, especially
  `_mi_option_get_fast`, are already compiled to the expected fast load.
- Profile cross-thread free with lower-rate `perf record --call-graph fp` if
  frame pointers are enabled in a future profiling build; the current DWARF
  profiles dropped too many samples for fine-grained ranking.

Correctness/RSS guardrails:

- Do not weaken acquire/release ordering.
- Do not delay abandoned-page reclaim or reabandon more aggressively unless RSS
  and latency are measured together.
- Keep `BENCHMARK_RESULTS.md` watch items and RSS/commit evidence tied to any
  change in reclaim policy.

### 4. Page Queue Search

Primary symbol:

- `mechanical_mimalloc::page::mi_page_queue_find_free_ex`

Source:

- `src/page.rs`: `mi_page_queue_find_free_ex`

Why it matters:

Queue search is not the top symbol, but it appears in threaded/free-heavy
profiles and can affect both throughput and RSS. It is a tempting optimization
target, but also a risky one.

Research ideas:

- Add instrumentation-only counters for queue scan length under
  `mimalloc-bench-native` scenarios, then remove or feature-gate the
  instrumentation before merging performance changes.
- Compare full-page retention behavior against upstream C before changing
  `page_full_retain` behavior.
- Use per-scenario RSS/commit measurements if changing search or retention
  policy.

Correctness/RSS guardrails:

- Avoid making queues retain more partially free pages just to improve a single
  throughput benchmark.
- Preserve full/unfull typestate checks and page ownership invariants.

### 5. Arena Fresh Allocation And Reabandon

Primary symbols:

- `mechanical_mimalloc::arena::mi_arenas_page_alloc_fresh`
- `mechanical_mimalloc::arena::_mi_arenas_page_try_reabandon_to_mapped`

Source:

- `src/arena.rs`: `mi_arenas_page_alloc_fresh`
- `src/arena.rs`: `_mi_arenas_page_try_reabandon_to_mapped`

Why it matters:

Arena work is visible in xmalloc-style profiles but lower priority than the
local allocation path and cross-thread free paths. Changes here can easily
affect RSS, commit, purge, and abandonment behavior.

Research ideas:

- Compare arena fresh allocation and reabandon control flow against upstream C.
- Check whether strict slice-count and memid wrappers optimize away in the
  fresh-allocation path.
- Keep any research tied to `scripts/check-layout.sh`,
  `scripts/check-behavioral-parity.py`, and RSS/commit evidence.

Correctness/RSS guardrails:

- Do not increase arena retention or reduce purge opportunities without a
  measured RSS improvement elsewhere.

## Non-Targets From This Profiling Pass

`cache-scratchN` is not a good allocator-optimization target based on this
profile. The clean cache-scratch profile was dominated by:

- `mimalloc_bench::cache_workloads::scratch_worker`
- `libm` `exp`/`__ieee754_exp_fma`

Allocator symbols did not show up meaningfully in that run. Treat
cache-scratchN as an end-to-end workload/noise watch item rather than a direct
guide for allocator internals.

## Suggested Next Experiments

1. Rebuild a profiling-only bench target with frame pointers, then rerun
   `glibc-thread` and `xmalloc-testN` at low sample frequency to reduce DWARF
   unwind overhead and sample loss.
2. Produce side-by-side assembly for Rust vs C on:
   `_mi_theap_malloc_zero_ex`, `_mi_page_malloc_zero_typed`,
   `mi_page_free_list_extend`, `_mi_page_free_collect_partly`, and
   `mi_free_try_collect_mt`.
3. Add temporary scan/refill/reclaim counters for local experiments only:
   queue scan length, refill batch length, thread-free collect length,
   abandoned reclaim attempts/successes, and arena reabandon attempts/successes.
4. For any proposed code change, run:
   - focused hotpath Criterion with 10 samples,
   - affected native scenario with 3 to 5 samples first,
   - full aggregate only once the signal is clear,
   - RSS/commit capture if page retention, reclaim, arena, purge, or refill
     policy changes.

## Priority List

1. Allocation fast path codegen and wrapper zero-cost audit.
2. Cross-thread free and partial collection codegen/load reduction.
3. Free-list refill loop and page extension instruction count.
4. Page queue search length measurement before any policy change.
5. Arena fresh/reabandon codegen, only after higher-confidence items are
   exhausted.

The safest expected wins are instruction-count/codegen wins that preserve the
existing page sizing, collection thresholds, memory ordering, and retention
policy. Avoid throughput-only changes that retain more memory or defer reclaim
without explicit RSS evidence.
