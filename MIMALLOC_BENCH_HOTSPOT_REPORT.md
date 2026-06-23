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

## Actionable Performance Work Items

Work these as small, independently measured changes. For each item, capture a before/after baseline for at least `xmalloc-testN`, and reject changes that only move cost between symbols without reducing total runtime, instruction count, or allocator-module CPU time. The important signal from this pass is extra instruction/branch volume and store-heavy metadata traffic, not branch misprediction.

### 0. Establish the repeatable measurement gate before changing code

Priority: required before all tuning work

Source/command entry points:

- Benchmark binary: `benches/mimalloc_bench.rs`
- Primary scenario: `xmalloc-testN`
- Guard scenarios: `glibc-simple`, `larsonN`, `hotpath/single_reuse`, `workloads fragmentation`

Actions:

- Rebuild the exact release benchmark configuration before every experiment:

  ```bash
  cargo bench --features bench,compare --benches --no-run
  ```

- For codegen/callgraph work, also keep a frame-pointer build available:

  ```bash
  RUSTFLAGS="-C force-frame-pointers=yes" cargo bench --features bench,compare --benches --no-run
  ```

- Record, for Rust and C mimalloc 3.3.2, the same metrics used in this report: Criterion median, `perf stat` instructions/branches/cycles/cache misses, flat `perf report`, and AMDuProf hotspot time when needed.
- Use the report's current xmalloc signal as the regression target: Rust is about 2.5% slower than C in Criterion and retires about 2.7% more instructions and 5.17% more branches under `perf stat`.

Acceptance gate:

- Prefer changes that improve `xmalloc-testN` while staying neutral on `glibc-simple`, `larsonN`, `hotpath/single_reuse`, and fragmentation.
- Accept neutral runtime only if instructions, branches, `.text`, or code complexity clearly decrease.
- Run at minimum: `cargo fmt --check`, `cargo clippy --features bench,compare --benches --tests`, and `cargo test --features unstable-raw-internals`.

### 1. Reduce extra work in free-side remote collection

Priority: high

Hotspot evidence:

- `src/free.rs:1036` `mi_free_try_collect_mt_typed`
- `src/free.rs:877` `mi_free`
- Rust flat perf: `mi_free_try_collect_mt_typed` 12.59%; `mi_free` is the hottest AMDuProf allocator symbol at 20.62 s.
- C follows the same path, but Rust shows higher allocator-module CPU time and extra retired instructions/branches.

Concrete checks:

- Diff annotated assembly for Rust `mi_free_try_collect_mt_typed` against C `mi_free_try_collect_mt`.
- Identify wrapper traffic that survives optimization: repeated page/raw pointer conversions, witness reconstruction, optional-state checks, or helper calls inside the hot loop.
- Check whether typed/raw boundary helpers are actually inlined in release. Add or adjust `#[inline]` only where the assembly shows a real call or duplicated wrapper sequence.
- Look for invariants reloaded multiple times in one free-collection pass: page identity, owning heap/thread id, collect mode, and remote-list emptiness.
- Verify that `_mi_option_get_fast` or other option reads are not repeated inside a loop when the value is semantically stable for the duration of the call.

Candidate fixes to try one at a time:

- Carry an already-validated typed page/witness through the free-collection path instead of reconstructing it after every raw helper boundary.
- Hoist invariant reads out of the collection loop when doing so preserves current runtime option semantics.
- Collapse duplicate validation/count work only if the earlier validation dominates all later uses.
- Keep structural/type assertions at raw API boundaries; do not remove checks merely to match C instruction count.

Success criteria:

- Lower `xmalloc-testN` retired instructions and branches with no increase in branch misses.
- Lower or equal flat-perf share for `mi_free_try_collect_mt_typed` and no compensating increase in `_mi_page_free_collect_partly` or `_mi_page_free_collect`.

### 2. Tighten partial page-free collection list handling

Priority: high

Hotspot evidence:

- `src/page.rs:802` `_mi_page_free_collect_partly`
- `src/page.rs:817` `_mi_page_free_collect_partly_typed`
- Rust flat perf: 5.68%; AMDuProf: 11.51 s.
- IBS: low store miss rate compared with free-list extension, but non-trivial load latency share, so list traversal shape matters.

Concrete checks:

- Compare Rust and C traversal shape for the partial remote-free list: number of loads per node, branch structure, and whether head/tail/count are recomputed.
- Check whether Rust repeats list validation, count derivation, or head/tail discovery after the list has already been detached or proven.
- Inspect whether `Option`, enum, or witness wrappers introduce extra discriminant tests in the inner list walk.
- Confirm that atomic loads/exchanges around remote-free handoff match C semantics; do not weaken ordering unless there is a separate correctness proof.

Candidate fixes to try one at a time:

- Carry detached list metadata (`head`, optional `tail`, and count if already known) through the path instead of rediscovering it.
- Split a tiny typed inner helper if it lets LLVM keep list state in registers without reloading page fields.
- Remove redundant conversions between `*mut mi_block_t` and typed block wrappers inside the walk when the raw block list was already validated.

Success criteria:

- Fewer instructions in `_mi_page_free_collect_partly` and `mi_free_try_collect_mt_typed` combined.
- No increase in allocator crashes or invariant-test failures under `unstable-raw-internals`.

### 3. Improve full page-free collection early exits and force/non-force split

Priority: high

Hotspot evidence:

- `src/page.rs:772` `_mi_page_free_collect`
- Rust flat perf: 2.87%; AMDuProf: 4.09 s.
- CPI/IBS: very low IPC (0.2479) and high load L1D miss rate (25.25%).

Concrete checks:

- Separate the common non-force case from force collection in assembly and profiles.
- Count how often full collection is called when there is no remote list, no local-free transfer, or no useful work.
- Check whether page state fields are reloaded after calls that cannot modify them.
- Compare C's early-return order with Rust's order for empty lists, full pages, retired pages, and delayed-free state.

Candidate fixes to try one at a time:

- Add a cheap, semantics-preserving early return before expensive traversal when the page has no collectible free-list state.
- Split force and non-force helpers if the non-force hot path becomes smaller and the force path stays correct.
- Carry page state read once at entry into inner helpers only when no intervening mutation can invalidate it.

Success criteria:

- Fewer calls into `_mi_page_free_collect` per `xmalloc-testN` operation, or fewer instructions per call.
- No regression in abandoned-page reclaim counts or page-retirement behavior.

### 4. Optimize `mi_page_free_list_extend` store-heavy metadata construction

Priority: high

Hotspot evidence:

- `src/page.rs:1289` `mi_page_free_list_extend`
- `src/page.rs:1300` `mi_page_free_list_extend_typed`
- Rust flat perf: 6.62%; AMDuProf: 15.18 s.
- CPI/IBS: low IPC (0.4707) and very high store L1D miss rate (79.08%).

Concrete checks:

- Diff the Rust loop against C mimalloc 3.3.2 for block-link store order, loop-carried dependencies, unrolling, and register pressure.
- Confirm whether secure/non-secure branches compile away in the release configuration used by benchmarks.
- Inspect whether typed wrappers force extra address recomputation for each block.
- Check whether the loop repeatedly reloads `bsize`, page start/end, encoded-next metadata, or page flags.
- Verify that the generated code does not add avoidable bounds/provenance checks around raw block pointer arithmetic.

Candidate fixes to try one at a time:

- Hoist loop invariants (`bsize`, block count/limit, page-local constants, secure mode) outside the block-linking loop.
- Use a small typed inner loop that receives raw start/end/count after validation, so each iteration only computes the next block address and writes the next pointer.
- Consider separate secure and non-secure inner helpers if it removes a branch in the common benchmark build.
- Experiment with modest unrolling only after confirming the C loop is unrolled or that LLVM misses an obvious opportunity; reject if `.text` grows without runtime benefit.

Success criteria:

- Lower CPU time in `mi_page_free_list_extend` and no increase in `malloc`/`_mi_malloc_generic_typed` time.
- Same allocator correctness under small, medium, and large allocation sequence tests.

### 5. Reduce abandoned-page reclaim overhead on the xmalloc path

Priority: medium-high

Hotspot evidence:

- `src/free.rs:965` `mi_abandoned_page_try_reclaim`
- Rust flat perf: 2.74%; AMDuProf: 3.99 s.
- This sits in the same remote-free lifecycle cluster as the primary xmalloc gap.

Concrete checks:

- Count reclaim attempts, successes, and failures by reason: no abandoned page, already claimed, wrong heap/thread, busy wait, page no longer suitable, and arena/map failure.
- Compare Rust and C paths for repeated heap/page validation after a page has been claimed.
- Inspect whether the Rust path re-derives page ownership or heap identity in multiple helpers.
- Check whether failed reclaim attempts immediately fall through into reabandon work that could be avoided.

Candidate fixes to try one at a time:

- Carry the claimed-page witness and owning heap/thread evidence from the successful claim into follow-up helpers.
- Add a cheap failure reason path that returns before arena/page-map work when reclaim cannot proceed.
- Avoid rechecking abandoned state after an atomic claim unless another thread-visible mutation can occur at that exact point.

Success criteria:

- Lower reclaim attempts per successful reclaim, or lower instructions per reclaim attempt.
- No change in abandoned-page safety tests or stress behavior.

### 6. Continue simplifying reabandon-to-mapped-arena with typed witnesses

Priority: medium-high

Hotspot evidence:

- `src/arena.rs:3546` `_mi_arenas_page_try_reabandon_to_mapped_arena`
- Rust flat perf: 3.34%; AMDuProf: 3.41 s.
- C has the same path visible; Rust should focus on avoiding extra proof/re-derivation work, not changing ownership semantics.

Concrete checks:

- Locate repeated derivation of arena id, slice index, slice count, page map entry, and bitmap range.
- Confirm whether a validated `ArenaMemId` and `SliceRange` can be carried through bitmap and page-map operations without crossing an unvalidated raw boundary.
- Check if page-map registration/unregistration evidence is reconstructed after every helper call.
- Measure how often reabandon fails because of mapped-arena bitmap availability versus page state mismatch.

Candidate fixes to try one at a time:

- Pass a single validated `ArenaMemId`/`SliceRange` pair through the reabandon path.
- Share bitmap range checks between find/clear and page-map update helpers.
- Split failure cases so common no-space or state-mismatch failures return before expensive map work.

Success criteria:

- Lower instruction count in `_mi_arenas_page_try_reabandon_to_mapped_arena` with no correctness weakening in arena ownership and page-map tests.
- No increase in `pages_reabandon_full` or arena rollback/purge counters for the same workload.

### 7. Inspect page-queue search and candidate scanning

Priority: medium

Hotspot evidence:

- `src/page.rs:2876` `mi_page_queue_find_free_ex_typed`
- Rust flat perf: 1.57%.
- CPI profile: low IPC (0.4101), suggesting queue traversal or metadata dependency cost.

Concrete checks:

- Add bench-only counters for page queue searches, pages scanned per search, and success/failure reason.
- Compare Rust and C search order for page queues, especially size-class bin selection and full/retired page skipping.
- Inspect whether the Rust typed path reloads heap/page queue metadata for each candidate instead of carrying queue-local state.
- Check whether candidate rejection performs expensive checks before cheap checks.

Candidate fixes to try one at a time:

- Reorder candidate checks to put cheap/full-page/no-capacity tests before expensive validation when semantics allow.
- Carry queue and size-class metadata through candidate scanning instead of deriving it per page.
- Add a narrow helper for the common successful candidate case if the current helper is too branch-heavy.

Success criteria:

- Fewer scanned candidates per successful allocation or fewer instructions per candidate.
- No regression in fragmentation workload or page-retirement behavior.

### 8. Audit malloc/free wrapper and ABI-boundary overhead

Priority: medium

Hotspot evidence:

- `src/alloc.rs:734` `mi_malloc`
- `src/alloc_override.rs:518` `malloc`
- `src/free.rs:877` `mi_free`
- Rust AMDuProf: `malloc` 14.60 s and `mi_free` 20.62 s; flat perf also shows substantial time around `operator delete[](void*)` and `mi_malloc`.

Concrete checks:

- Confirm whether wrapper layers around `malloc`/`mi_malloc` and `mi_free`/delete are simple tail calls or contain repeated heap lookup, option lookup, size-class logic, or typed/raw conversion.
- Compare exported Rust wrapper assembly to C mimalloc exported wrapper assembly.
- Check if symbol boundaries prevent inlining of tiny helpers that C inlines into the public entry points.
- Separate benchmark harness/operator-delete time from allocator time before attributing changes to the allocator.

Candidate fixes to try one at a time:

- Inline tiny Rust-only helpers that appear as real calls in hot public wrappers.
- Avoid duplicate heap lookup between public wrapper and generic allocation helper when the helper can accept already-read heap evidence.
- Keep exported ABI and public semantics unchanged.

Success criteria:

- Lower allocator-module CPU time without merely moving samples from `mi_free`/`malloc` into a different wrapper symbol.
- No change in exported symbol behavior or C ABI compatibility.

### 9. Remove avoidable hot-path option reads

Priority: medium-low

Hotspot evidence:

- `src/options.rs:1343` `_mi_option_get_fast`
- Rust flat perf: 0.63%, small but visible.
- Extra option reads can also inflate instruction/branch counts inside larger hot functions.

Concrete checks:

- List every `_mi_option_get_fast` call reachable from `mi_free_try_collect_mt_typed`, `_mi_page_free_collect`, `mi_page_free_list_extend`, `_mi_malloc_generic_typed`, and reabandon/reclaim.
- Identify calls inside loops or repeated calls within one public allocation/free operation.
- Verify which options are compile-time constants, process-stable after init, or dynamically mutable.

Candidate fixes to try one at a time:

- Hoist process-stable option reads out of loops.
- Pass already-read option values into inner helpers if the value cannot legally change during the operation.
- Do not cache dynamically mutable options across API calls unless C mimalloc has equivalent behavior.

Success criteria:

- `_mi_option_get_fast` disappears or shrinks in flat perf for `xmalloc-testN` without changing option semantics.
- Instructions/branches drop in the caller that previously repeated the read.

### 10. Treat arena bitmap tuning as exploratory until counters prove it matters

Priority: low-medium

Hotspot evidence:

- `src/bitmap.rs:2210` `mi_bbitmap_try_find_and_clear`
- IBS showed high local miss ratios in a small sample, but only 1,723 IBS ops, so this is not yet a top proven bottleneck.

Concrete checks:

- Instrument bitmap find/clear attempts, failures, range length, and retry count under bench-only diagnostics.
- Compare bit-scan and clear loop shape against C only for workloads where the counter shows frequent use.
- Check whether reabandon failures repeatedly scan the same unavailable bitmap range.

Candidate fixes to try one at a time:

- Share already-validated bitmap range data from reabandon instead of recomputing it.
- Fast-return on impossible or empty ranges before entering generic bitmap scanning.
- Avoid broader bitmap refactors unless counters show this path grows beyond noise level.

Success criteria:

- Reduced reabandon CPU time or bitmap retry count on `xmalloc-testN`.
- No change in arena allocation correctness.

### 11. Add bench-only diagnostics for the hotspot cluster

Priority: medium, but useful before changing complex lifecycle code

Target counters:

- Remote-free collection attempts and blocks collected per attempt.
- `_mi_page_free_collect` calls split by force/non-force and by early-exit reason.
- `_mi_page_free_collect_partly` calls, detached list lengths, and count/tail rediscovery events.
- `mi_page_free_list_extend` calls, blocks extended, secure/non-secure path count, and average extend size.
- Abandoned-page reclaim attempts/success/fail by reason.
- Reabandon-to-mapped-arena attempts/success/fail by reason.
- Page queue searches, candidates scanned, and successful candidate position.
- Arena bitmap find/clear attempts, failures, and retry counts.
- Hot-path option reads by option id, if this can be done without adding default-build overhead.

Implementation requirements:

- Gate all counters behind a feature or private benchmark cfg, for example `bench-diagnostics`.
- Ensure the default release build has no extra atomics, branches, or counter storage on hot paths.
- Use atomics only where cross-thread updates are necessary; prefer per-thread or per-benchmark accumulation where possible.
- Emit CSV or JSON from `mimalloc_bench` so a before/after run can be diffed with the perf data.

Success criteria:

- Diagnostics build explains which branch/reason dominates each hotspot.
- Default release `.text` and hot-path assembly are unchanged when diagnostics are disabled.

### 12. Keep RSS and heap-policy work secondary unless a reproducible memory regression appears

Priority: low for this pass

Evidence:

- `mleak10` max RSS differed by only 84 KB in the short run.
- Fragmentation perf counters were neutral between mechanical and system allocator.
- Heaptrack mostly observed benchmark harness allocations, not allocator-internal benchmark traffic.

Actions only if a new RSS issue is reported:

- Build a scenario-specific resident-set sampler using `/proc/<pid>/smaps_rollup` or repeated `status` RSS sampling.
- Compare Rust and C over longer runs and record committed/reserved/page counters from existing stats APIs.
- Correlate RSS differences with page retirement, abandoned pages, purges, and arena rollback counters before changing memory policy.

Success criteria:

- A repeatable RSS delta that is larger than measurement noise and attributable to allocator policy, not harness setup.

## Action Pass: 2026-06-23

Scope: execute every actionable item above, keep only changes with a clear
measured benefit or a defensible neutral simplification, and preserve the strict
typed checks/assertions around allocator state transitions.

### Summary decision

Kept two changes:

- Replaced the single-use `ForceCollect`/`LocalFreeList::collect_into_available`
  helper in full page free collection with the direct C-shaped splice in
  `_mi_page_free_collect`.
- Added `bench-diagnostics`, a feature-gated diagnostics build that emits
  JSONL hotspot counters from `mimalloc_bench` when
  `MIMALLOC_BENCH_DIAGNOSTICS` is set.

Rejected/reverted one source experiment:

- A direct rewrite of `_mi_page_free_collect_partly_typed` was reverted because
  it erased the isolated instruction/branch improvement seen from the full
  collection change.

The retained runtime change should be treated as a neutral CPU result with a
small source/code-size win, not as a proven wall-time speedup. The isolated perf
run after the direct collection change showed fewer retired instructions and
branches, but the final fresh plain release run was within noise and did not
reproduce that instruction reduction. The reason to keep it is that it removes
a dead single-use abstraction, preserves the same assertions and semantics, and
reduces default `.text`.

### Default-build measurements

Artifacts are under `target/perf-action-2026-06-23/`.

| Measurement | Baseline | Final/accepted | Read |
| --- | ---: | ---: | --- |
| Plain release `.text` | 190550 bytes | 190486 bytes | -64 bytes |
| Plain release total size | 346356 bytes | 346356 bytes | unchanged |
| Diagnostics symbols in plain release | none | none | compiled out |
| Isolated xmalloc instructions | 499.218B | 488.940B | -2.06% in `accepted_direct_collect_perf_xmalloc_rust.txt` |
| Isolated xmalloc branches | 122.463B | 119.697B | -2.26% in `accepted_direct_collect_perf_xmalloc_rust.txt` |
| Final fresh xmalloc elapsed | 5.0103s | 5.0132s | neutral/noisy in `final_default_perf_xmalloc_rust.txt` |
| Final fresh xmalloc instructions | 499.218B | 501.315B | neutral/noisy, no retained CPU-speed claim |

Guard probes did not show a clear regression in the checked scenarios:

| Scenario | Artifact | Result |
| --- | --- | --- |
| `glibc-simple` Rust | `accepted_perf_glibc_simple_rust.txt` | elapsed and instructions lower than the earlier diagnostic run |
| `larsonN` Rust | `accepted_perf_larsonN_rust.txt` | smoke guard completed without correctness issues |
| `hotpath_single_reuse_64` | `accepted_perf_hotpath_single_reuse_64_mech.txt` | effectively neutral versus earlier diagnostic data |
| `workload_fragmentation` | `accepted_perf_workload_fragmentation_mech.txt` | runtime neutral; instruction count noisy |

### Diagnostics result

`bench-diagnostics` is compiled out of normal builds. A separate feature build
verified the intended symbols:

```text
mi_bench_diagnostics_json
mi_bench_diagnostics_reset
mi_bench_diagnostics_snapshot
```

Feature-build size:

```text
.text 192614
Total 350751
```

Smoke JSONL from `xmalloc-testN` showed the dominant branch cluster clearly:

| Counter | Value |
| --- | ---: |
| `page_free_collect_partly_calls` | 117,938,526 |
| `page_free_collect_partly_detached` | 117,810,374 |
| `abandoned_reclaim_attempts` | 117,820,053 |
| `abandoned_reclaim_fail_thread_uninit` | 117,284,410 |
| `abandoned_reclaim_fail_queue_limit` | 535,641 |
| `option_reads_total` | 119,726,561 |
| `option_reads_page_reclaim_on_free` | 117,938,712 |
| `page_queue_searches` | 1,906,540 |
| `bitmap_find_clear_attempts` | 118,830 |

This confirms that future tuning should focus on the repeated partial-collect,
abandoned-reclaim, and option-read cluster before touching low-evidence bitmap
or RSS policy paths.

### Actionable-item disposition

| Item | Disposition |
| --- | --- |
| 0. Establish baseline gate | Done. Captured Criterion, perf stat, guard probes, `.text`, and diagnostics artifacts. |
| 1. Free collection directness | Kept only the full-collection direct C-shaped splice. Assertions remain in place; no type-system checks were weakened. |
| 2. Partial collection wrapper overhead | Tried and reverted. No retained benefit. |
| 3. Remove single-use helper if neutral | Done. Removed `ForceCollect` and `collect_into_available`; default `.text` shrank 64 bytes. |
| 4. Free-list extension | Inspected and instrumented. No safe runtime edit kept. |
| 5. Abandoned reclaim | Inspected and instrumented by reason. No semantic edit kept. |
| 6. Reabandon-to-mapped | Inspected and instrumented by reason. No semantic edit kept. |
| 7. Page queue search | Inspected and instrumented. No source edit kept beyond counters. |
| 8. Wrapper/ABI shape | Inspected hot symbol sizes and assembly. No ABI/type weakening kept. |
| 9. Option reads | Instrumented by option id. No caching kept because options are mutable API state. |
| 10. Bitmap tuning | Instrumented find/clear attempts. No tuning kept; current evidence remains low priority. |
| 11. Bench diagnostics | Implemented and smoke-tested. Default build has no extra symbols or counter storage. |
| 12. RSS policy | No change. Existing evidence did not show a reproducible RSS regression. |

### Verification

```text
cargo fmt --check
git diff --check
cargo clippy --features bench,compare --benches --tests
cargo test --features unstable-raw-internals
cargo bench --features bench,compare,bench-diagnostics --benches --no-run
CARGO_TARGET_DIR=target/bench-diagnostics-check cargo build --release --features bench-diagnostics
```

## Branchless-Code Follow-Up - 2026-06-23

A focused branchless-code pass did not retain any source change.

Summary:

- `xmalloc-testN` branch misses remained low at about 0.21%, so branch
  prediction itself is still not the primary bottleneck.
- `mi_page_free_list_extend` is a top branch-miss symbol, but annotation shows
  the samples on the refill loop/backedge rather than a removable value-choice
  branch.
- `_mi_page_free_collect`, `mi_free_try_collect_mt_typed`, and
  `mi_page_queue_find_free_ex_typed` mostly branch around side effects
  (collection, CAS retry, reclaim, queue mutation, or freeing). These are poor
  branchless rewrite targets without a semantic restructuring.
- `mi_page_queue_find_free_ex_typed` already uses `cmov` for the candidate-page
  selection in release codegen.

Rejected branchless candidate:

- Rewriting the small-bin return in `mi_bin_impl` with `.max(1)` removed a
  small inlined branch and shrank `.text` by 96 bytes, but guardrails regressed:
  `glibc-thread` +33.83%, `alloc-test1` +14.49%, `alloc-testN` +18.24%, and
  `sh8benchN` +6.17% mean time in the focused reject probe.
- Decision: reverted; no retained code change.
