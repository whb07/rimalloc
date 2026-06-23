# mimalloc-bench Hotspot Report

Generated: 2026-06-23

## Executive Summary

The current Rust allocator is broadly close to C mimalloc 3.3.2 on the Criterion native ports that completed in this pass. The clearest remaining CPU gap is `xmalloc-testN`: Rust measured about 2.5% slower than C mimalloc in Criterion and retired about 2.7% more instructions under `perf stat`. `glibc-simple` and `larsonN` were effectively neutral. The short `mleak10` RSS probe did not show a meaningful RSS gap.

The actionable hotspot cluster is not the raw smallest-object fast path in isolation. It is the remote/free collection and abandoned-page lifecycle path:

- `mi_free_try_collect_mt_typed`
- `_mi_page_free_collect_partly`
- `_mi_page_free_collect`
- `mi_page_free_list_extend`
- `mi_abandoned_page_try_reclaim`
- `_mi_arenas_page_try_reabandon_to_mapped_arena`
- `mi_page_queue_find_free_ex_typed`

AMDuProf and perf agree on this shape. IBS does not point to branch misprediction as the main bottleneck; the whole-process branch mispredict rate was under 0.3% in the IBS pass. Load miss cycles were also a small share of tag-to-retire time overall. The data instead suggests allocator metadata traffic, store-heavy paths, and cross-thread/free-list collection behavior are the next areas to inspect.

## Scope

Benchmarks used:

- `benches/mimalloc_bench.rs`: Rust allocator versus C mimalloc 3.3.2, loaded from `mimalloc-3.3.2`.
- `benches/hotpath.rs`: Rust allocator versus system allocator.
- `benches/workloads.rs`: Rust allocator versus system allocator.

Main artifacts:

- `target/perf-diagnostics-2026-06-23/mimalloc_bench_selected_measurements.txt`
- `target/perf-diagnostics-2026-06-23/perf_stat_*.txt`
- `target/perf-diagnostics-2026-06-23/perf_report_*_flat.txt`
- `target/perf-diagnostics-2026-06-23/amduprof/**/report.csv`
- `target/perf-diagnostics-2026-06-23/heaptrack_xmalloc_rust_print.txt`

Environment:

- Host: AMD Ryzen 9 5950X, 32 logical CPUs
- Kernel: Linux 6.17.0-35-generic
- `perf_event_paranoid=1`
- AMDuProfCLI: 5.2.606.0
- Release allocator shared object: `target/release/libmechanical_mimalloc.so`
- Release `.text`: 190,550 bytes

## Method Notes

Criterion binaries must be run with `--bench` for actual measurement. Direct execution without `--bench` only ran benchmark smoke tests.

The native Criterion scan was intentionally bounded with:

```bash
MIMALLOC_BENCH_SKIP_BUILD=1
MIMALLOC_BENCH_SAMPLE_SIZE=10
MIMALLOC_BENCH_MEASUREMENT_SECS=1
MIMALLOC_BENCH_WARMUP_SECS=1
```

Criterion still stretched expensive scenarios to their minimum useful sample time. `rptestN` estimated 160 seconds per allocator at minimum settings, so the broad scan was stopped there and profiler-mode runs were used instead.

For profiler runs, Criterion `--profile-time` was used to keep runtime bounded and disable statistical analysis:

```bash
./target/release/deps/mimalloc_bench-6daac1c8e7d89af6 --bench xmalloc-testN --profile-time 3
```

The high-rate xmalloc `perf record -F 999 --call-graph dwarf` runs lost too many samples on the 32-thread workload. They were repeated at `-F 99`, which still lost samples but was good enough for directional top-symbol evidence:

- Rust xmalloc lower-rate perf record: 21.62% lost samples
- C xmalloc lower-rate perf record: 27.52% lost samples
- Hotpath single-reuse perf record: 0 lost samples

AMDuProfCLI hotspot reports did not collect call stacks in the selected config, but the function/module attribution was useful and agreed with perf.

## Criterion Measurements

Selected native `mimalloc_bench` measurements:

| Scenario | Rust median | C mimalloc 3.3.2 median | Read |
| --- | ---: | ---: | --- |
| `glibc-simple` | 895.98 ms | 891.18 ms | Neutral; Rust about 0.5% slower |
| `xmalloc-testN` | 2.1676 ms, 461.33 Melem/s | 2.1154 ms, 472.73 Melem/s | Rust about 2.5% slower |
| `larsonN` | 2.8858 ms, 346.53 Melem/s | 2.8863 ms, 346.47 Melem/s | Neutral |

`xmalloc-testN` is the best immediate optimization target from this pass because it has a repeatable Rust-vs-C CPU gap and a clear hotspot cluster.

## perf stat

### xmalloc-testN: Rust vs C mimalloc

Command shape:

```bash
perf stat -r 3 -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses,page-faults,context-switches,cpu-migrations -- \
  env MIMALLOC_BENCH_SKIP_BUILD=1 MIMALLOC_BENCH_SCENARIOS=xmalloc-testN MIMALLOC_BENCH_ALLOCATORS=rust \
  ./target/release/deps/mimalloc_bench-6daac1c8e7d89af6 --bench xmalloc-testN --profile-time 3
```

| Counter | Rust | C mimalloc 3.3.2 | Rust delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.012172 s | 5.013374 s | neutral |
| task-clock | 152.709 B | 150.688 B | +1.34% |
| CPUs utilized | 30.468 | 30.057 | +1.37% |
| cycles | 649.916 B | 645.020 B | +0.76% |
| instructions | 490.147 B | 477.379 B | +2.67% |
| branches | 120.175 B | 114.270 B | +5.17% |
| branch misses | 235.689 M | 255.501 M | -7.75% |
| cache misses | 995.394 M | 988.279 M | +0.72% |
| page faults | 21,655 | 23,399 | -7.45% |

Interpretation: the Rust gap is mostly instruction/branch volume, not branch-miss volume. Cache miss behavior is close. This supports looking for extra Rust-side work in the same logical paths rather than changing memory policy first.

### glibc-simple: Rust vs C mimalloc

| Counter | Rust | C mimalloc 3.3.2 | Read |
| --- | ---: | ---: | --- |
| elapsed | 4.52355 s | 4.50223 s | neutral |
| cycles | 21.593 B | 21.537 B | neutral |
| instructions | 70.747 B | 70.675 B | neutral |
| branches | 10.070 B | 10.057 B | neutral |
| cache misses | 21.739 M | 23.800 M | Rust lower |

Interpretation: no strong CPU hotspot unique to Rust here.

### hotpath single_reuse/64: Mechanical vs System

| Counter | Mechanical | System | Read |
| --- | ---: | ---: | --- |
| elapsed | 4.99255 s | 5.00355 s | neutral |
| cycles | 23.906 B | 23.818 B | neutral |
| instructions | 82.572 B | 82.647 B | neutral |
| branch misses | 17.030 M | 31.335 M | Mechanical lower |

`perf record` for the mechanical run showed:

- `mi_malloc`: 38.50%
- `operator delete[](void*)`: 37.76%
- Criterion bencher loop: 21.84%
- `_mi_malloc_generic_typed`: 1.12%

Interpretation: the isolated 64-byte reuse loop is not currently exposing the same Rust-vs-C gap as `xmalloc-testN`; it is mainly validating that the direct hot path remains tight.

### workloads fragmentation: Mechanical vs System

| Counter | Mechanical | System | Read |
| --- | ---: | ---: | --- |
| elapsed | 5.0203 s | 5.00483 s | neutral |
| cycles | 24.333 B | 24.102 B | neutral |
| instructions | 53.996 B | 54.512 B | Mechanical slightly lower |
| branch misses | 221.570 M | 224.008 M | neutral |
| cache misses | 803.018 M | 808.817 M | neutral |

Interpretation: this high-level fragmentation workload is cache-missy for both allocators, but this pass did not show a mechanical-specific regression.

## perf record Hotspots

### xmalloc-testN Rust allocator

Lower-rate flat report, `perf_xmalloc_rust_f99.data`:

| Symbol | Overhead |
| --- | ---: |
| `mimalloc_bench::xmalloc_test::allocator_thread` | 31.85% |
| `mechanical_mimalloc::free::mi_free_try_collect_mt_typed` | 12.59% |
| `operator delete[](void*)` | 11.84% |
| `mi_malloc` | 8.22% |
| `mechanical_mimalloc::page::mi_page_free_list_extend` | 6.62% |
| `mechanical_mimalloc::page::_mi_page_free_collect_partly` | 5.68% |
| `mechanical_mimalloc::arena::_mi_arenas_page_try_reabandon_to_mapped_arena` | 3.34% |
| `mechanical_mimalloc::page::_mi_page_free_collect` | 2.87% |
| `mechanical_mimalloc::free::mi_abandoned_page_try_reclaim` | 2.74% |
| `mimalloc_bench::xmalloc_test::releaser_thread` | 2.43% |
| `mechanical_mimalloc::page::mi_page_queue_find_free_ex_typed` | 1.57% |
| `mechanical_mimalloc::page::_mi_malloc_generic_typed` | 1.00% |
| `mechanical_mimalloc::options::_mi_option_get_fast` | 0.63% |

### xmalloc-testN C mimalloc 3.3.2

Lower-rate flat report, `perf_xmalloc_c_f99.data`:

| Symbol | Overhead |
| --- | ---: |
| `mimalloc_bench::xmalloc_test::allocator_thread` | 29.07% |
| `mi_free_try_collect_mt` | 11.80% |
| `operator delete[](void*)` | 10.05% |
| `mi_malloc` | 8.16% |
| `mi_page_free_list_extend` | 7.48% |
| `mimalloc_bench::xmalloc_test::releaser_thread` | 6.02% |
| `_mi_page_free_collect_partly` | 5.32% |
| `_mi_arenas_page_try_reabandon_to_mapped` | 4.10% |
| `_mi_page_free_collect` | 3.09% |

Interpretation: Rust and C hit the same algorithmic paths. The Rust-side difference is not a missing fast path; it is extra cost inside the same free-collection/reclaim/reabandon cluster.

## AMDuProfCLI

### Hotspots

Rust xmalloc hotspot profile:

- Profile duration: 5.573 seconds
- Threads: 65
- Process CPU time: 152.12 seconds
- Module CPU time:
  - `libmechanical_mimalloc.so`: 103.17 seconds
  - benchmark binary: 45.57 seconds
  - libc: 3.24 seconds

Rust hottest allocator functions:

| Function | CPU time |
| --- | ---: |
| `mi_free` | 20.62 s |
| `mi_free_try_collect_mt_typed` | 18.57 s |
| `mi_page_free_list_extend` | 15.18 s |
| `malloc` | 14.60 s |
| `_mi_page_free_collect_partly` | 11.51 s |
| `_mi_page_free_collect` | 4.09 s |
| `mi_abandoned_page_try_reclaim` | 3.99 s |
| `_mi_arenas_page_try_reabandon_to_mapped_arena` | 3.41 s |

C mimalloc hotspot profile:

- Profile duration: 5.572 seconds
- Threads: 65
- Process CPU time: 150.02 seconds
- Module CPU time:
  - `libmimalloc.so.3.3`: 95.65 seconds
  - benchmark binary: 49.61 seconds
  - libc: 4.62 seconds

C hottest allocator functions:

| Function | CPU time |
| --- | ---: |
| `operator delete(void*)` | 19.48 s |
| `__libc_malloc` | 14.57 s |
| `mi_page_free_list_extend` | 14.47 s |
| `mi_free_try_collect_mt` | 13.48 s |
| `_mi_page_free_collect_partly` | 11.94 s |
| `_mi_page_free_collect` | 4.46 s |
| `mi_abandoned_page_try_reclaim` | 4.14 s |
| `_mi_arenas_page_try_reabandon_to_mapped` | 2.95 s |

Read: AMDuProf reinforces the perf finding. Rust spends more allocator-module CPU time on the same workload. The most interesting Rust-over-C differences are `mi_free_try_collect_mt_typed` and the wrapper/free boundary around `mi_free`/`operator delete`.

### CPI

Rust xmalloc CPI profile:

- Whole process IPC: 0.828
- `libmechanical_mimalloc.so` IPC: 1.0097
- benchmark binary IPC: 0.3734

Allocator functions with notable low IPC:

| Function | IPC | CPI |
| --- | ---: | ---: |
| `_mi_page_free_collect` | 0.2479 | 4.0341 |
| `mi_page_queue_find_free_ex_typed` | 0.4101 | 2.4382 |
| `_mi_malloc_generic_typed` | 0.3853 | 2.5951 |
| `mi_page_free_list_extend` | 0.4707 | 2.1247 |
| `malloc` | 0.4395 | 2.2751 |
| `mi_free_try_collect_mt_typed` | 1.2227 | 0.8179 |
| `mi_free` | 1.3432 | 0.7445 |

Read: some slow-path/page-search/page-extension code is low IPC, while the free collection entry itself is not especially low IPC. This points to memory dependency and metadata traversal inside specific subpaths, not just all free-side code.

### IBS

Rust xmalloc IBS process summary:

- Branch operations: 17.88% of IBS ops
- Load/store operations: 45.81% of IBS ops
- Branch mispredict rate: 0.296%
- Load L1D miss rate: 1.84%
- Store L1D miss rate: 12.99%
- Load L1D miss latency cycles: 0.697% of tag-to-retire cycles

Rust `libmechanical_mimalloc.so` IBS module summary:

- Branch operations: 23.19% of IBS ops
- Load/store operations: 35.40% of IBS ops
- Branch mispredict rate: 0.272%
- Load L1D miss rate: 2.32%
- Store L1D miss rate: 26.83%
- Load L1D miss latency cycles: 0.837% of tag-to-retire cycles

Selected function-level IBS observations:

- `mi_page_free_list_extend`: store L1D miss rate 79.08%, load miss latency only 0.028% of tag-to-retire cycles.
- `mi_free`: store L1D miss rate 62.66%, load miss latency 1.17% of tag-to-retire cycles.
- `_mi_page_free_collect_partly`: store L1D miss rate 3.43%, load miss latency 2.73% of tag-to-retire cycles.
- `_mi_page_free_collect`: load L1D miss rate 25.25%, load miss latency 1.06% of tag-to-retire cycles.
- `mi_bbitmap_try_find_and_clear`: high local miss ratios in a small sample, but only 1,723 IBS ops.

Read: the profile is not branch-prediction dominated. Store-heavy metadata updates and page/free-list memory traffic are more plausible than branch tuning as the next target.

## RSS and Heap Diagnostics

Short `mleak10` `/usr/bin/time -v` profile-mode run:

| Allocator | Max RSS | Elapsed | Read |
| --- | ---: | ---: | --- |
| Rust | 8,544 KB | 1.92 s | neutral |
| C mimalloc 3.3.2 | 8,460 KB | 1.91 s | neutral |

This pass did not expose a meaningful RSS difference.

`heaptrack` was run on `xmalloc-testN` with the Rust allocator. It reported only 1,207 allocation calls, mostly from Criterion/clap/tinytemplate setup and pthread/TLS setup. That is expected: `mimalloc_bench` drives the allocator through explicit function handles, so heaptrack mostly sees harness/global-allocation activity rather than the allocator traffic being benchmarked. It is useful for harness noise checks, not for allocator hot-path attribution here.

## Existing Allocator Diagnostics

The allocator already exposes general stats and process diagnostics:

- `mi_stats_get`
- `mi_heap_stats_get`
- `mi_subproc_stats_get`
- `mi_subproc_stats_get_exclusive`
- `mi_stats_print_out`
- `mi_thread_stats_print_out`
- `mi_process_info`
- `mi_process_info_print_out`
- `mi_stats_get_json`
- `mi_heap_stats_get_json`
- `mi_subproc_stats_get_json`
- `mi_stats_get_bin_size`

The `mi_stats_t` surface already includes useful fields for future benchmark-mode diagnostics:

- `reserved`, `committed`, `page_committed`
- `pages`, `pages_abandoned`
- `mmap_calls`, `commit_calls`, `reset_calls`, `purge_calls`
- `arena_count`, `arena_rollback_count`, `arena_purges`
- `pages_extended`, `pages_retire`
- `page_searches`, `page_searches_count`
- `pages_reclaim_on_alloc`, `pages_reclaim_on_free`
- `pages_reabandon_full`, `pages_unabandon_busy_wait`
- `malloc_bins`, `page_bins`, `chunk_bins`

Gap: these are lifecycle and memory-policy counters, not detailed hot-path attribution counters. For the current xmalloc hotspot, useful bench-only counters would be:

- remote-free collection attempts
- remote-free blocks collected per attempt
- `_mi_page_free_collect` calls and force/non-force split
- `_mi_page_free_collect_partly` calls and list lengths
- abandoned-page reclaim attempts/success/fail by reason
- reabandon-to-mapped-arena attempts/success/fail by reason
- free-list extension calls and blocks extended
- page queue searches and candidates scanned
- arena bitmap find/clear attempts and failures

These should be compile-time gated or bench-only to avoid perturbing release fast paths.

## Candidate Work Items

### 1. Compare Rust and C codegen for free collection

Priority: high

Target functions:

- `mi_free_try_collect_mt_typed`
- `_mi_page_free_collect_partly`
- `_mi_page_free_collect`
- `mi_page_thread_collect_to_local_typed`

Reason: Rust xmalloc spends substantial time in this cluster, and `perf stat` shows extra instructions/branches rather than branch misses. This is the best candidate for a neutral-or-better simplification that reduces instruction count without weakening type checks.

What to look for:

- extra wrapper traffic that survives optimization
- repeated raw/witness conversions inside loops
- duplicated list validation or count work
- missed inlining at typed/raw boundaries
- unnecessary option reads or reloads after invariants are known

Risk: high if we remove checks. Keep type assertions and structural checks; prefer carrying validated witnesses through the path when it removes repeated proof work.

### 2. Inspect `mi_page_free_list_extend` store-heavy behavior

Priority: high

Reason: `mi_page_free_list_extend` is a top hotspot in both perf and AMDuProf. CPI/IBS shows low IPC and high store miss rates. C is not free of this cost, but Rust remains slightly behind overall.

What to look for:

- loop shape versus C mimalloc 3.3.2
- block-link store order and aliasing assumptions
- whether typed wrappers inhibit vectorization/unrolling or register allocation
- whether secure/non-secure branches compile away as expected in release

Risk: medium. This path is core allocator metadata construction. Any change needs fuzz allocation sequence tests and benchmark confirmation across small, medium, and large paths.

### 3. Revisit abandoned-page reclaim and reabandon path after the recent witness improvement

Priority: medium

Reason: `_mi_arenas_page_try_reabandon_to_mapped_arena` remains visible at about 3% to 4% in xmalloc. The recent witness-carrying change reduced instructions in previous measurements, but this path is still part of the remaining hotspot cluster.

What to look for:

- repeated arena/slice count derivation
- bitmap checks that can share a validated `SliceRange`
- page map registration/unregistration evidence
- opportunities to carry typed `ArenaMemId` and `SliceRange` deeper without weakening raw-boundary validation

Risk: medium. This path touches correctness-sensitive arena ownership and abandoned-page state.

### 4. Add bench-only internal counters around the hotspot cluster

Priority: medium

Reason: perf and AMDuProf identify hot functions, but not all branch reasons. Existing `mi_stats_t` counters are useful but too coarse for this xmalloc gap.

Recommended shape:

- behind a cargo feature such as `bench-diagnostics` or a private cfg used only by benchmark builds
- atomics only where needed for cross-thread scenarios
- no release fast-path cost when disabled
- report as CSV or JSON from `mimalloc_bench`

Risk: low if fully gated. High if counters leak into default release builds.

### 5. Keep RSS work secondary until a stronger RSS scenario appears

Priority: low for this pass

Reason: `mleak10` showed only an 84 KB RSS difference, and fragmentation had neutral perf counters. RSS work should continue with longer resident-set sampling or scenario-specific `/proc/<pid>/smaps_rollup` collection, but this pass did not identify a concrete RSS regression.

## Suggested Next Profiling Pass

1. Rebuild with frame pointers for cleaner callgraphs:

```bash
RUSTFLAGS="-C force-frame-pointers=yes" cargo bench --features bench,compare --benches --no-run
```

2. Re-run `xmalloc-testN` Rust/C with lower-rate perf callgraphs:

```bash
perf record -F 99 -g --call-graph fp -o target/perf-diagnostics/xmalloc-rust-fp.data -- \
  env MIMALLOC_BENCH_SKIP_BUILD=1 MIMALLOC_BENCH_SCENARIOS=xmalloc-testN MIMALLOC_BENCH_ALLOCATORS=rust \
  ./target/release/deps/mimalloc_bench-* --bench xmalloc-testN --profile-time 5
```

3. Diff annotated assembly for the Rust and C versions of:

- `mi_free_try_collect_mt`
- `_mi_page_free_collect_partly`
- `mi_page_free_list_extend`
- `_mi_arenas_page_try_reabandon_to_mapped`

4. If code changes are attempted, accept candidates only if one of these holds:

- Criterion/perf shows a real improvement on `xmalloc-testN` without regression in `glibc-simple`, `larsonN`, `hotpath/single_reuse`, and fragmentation.
- Runtime is neutral but `.text` or instruction count drops and the code is simpler.
- The change improves diagnostics while remaining fully gated out of release fast paths.

## Current State

The effort should be left focused on CPU, not RSS. The current best target is the xmalloc remote-free/abandoned-page lifecycle path. The most defensible next code changes would be small, typed-witness-preserving simplifications in free collection or page free-list extension, validated against C mimalloc 3.3.2 and the existing Criterion scenarios.
