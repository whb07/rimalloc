# Performance Regression Worklog

## Goal

Fix the Rust allocator regressions observed in `mimalloc-bench` `redis` and
`larson-sized`, aiming to beat or reach parity with C mimalloc without weakening
the typed safety model or regressing other benchmark scenarios.

## Ground Rules

- Keep typed witnesses and validation guarantees intact.
- Do not replace typed APIs with broad raw-pointer fast paths unless invariants
  are already established and the helper remains private.
- Record failed or weak paths so they are not retried as standalone fixes.
- Refresh `target/bench-allocators/libmimalloc-rust.so` after release builds
  before benchmarking.
- Benchmark `redis` from both CSV relative time and `redis-*-out.txt`
  throughput/p50 because `bench.sh` derives Redis CSV time from requests/sec.

## Baseline Before Current Fix Attempt

Artifact:

```text
mimalloc-bench/out/bench/benchres-debug-redis-larson-r10-20260621-150211.csv
```

Scoped command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis larson-sized -r=10
```

Results:

| Test | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| `redis` relative time | 2.6900s | 2.8078s | Rust +4.38% |
| `redis` throughput | 743,613 req/s | 712,331 req/s | Rust -4.21% |
| `redis` p50 | 1.072ms | 1.136ms | Rust +5.97% |
| `larsonN-sized` relative time | 2.1118s | 2.1756s | Rust +3.02% |

## Paths Already Tried

### 1. ABI/Internal `mi_bin` Split

Implemented:

- Private `#[inline(always)] mi_bin_impl(size)`.
- Public ABI `mi_bin` and `_mi_bin` remain `#[inline(never)]`.
- Internal bin calculations call `mi_bin_impl`.

Result:

- Removed the obvious `mi_theap_queue_first_update_typed -> mi_bin` call.
- Not enough to close the larson regression.

Do not retry this as a standalone fix. It is a useful cleanup, but not the
dominant remaining issue.

### 2. Usable-Size Release Fast Path

Implemented:

- In non-debug, non-secure, non-guarded builds, `mi_route_usable_size_typed`
  avoids the full `mi_validate_pointer`/`PointerLookup` construction and uses
  `UserPtr` plus `mi_lookup_pointer_page`.
- Debug, secure, and guarded paths keep the full validation route.

Result:

- Helped the Redis-shaped hypothesis locally.
- Final 10-run Redis still did not reach parity.

Do not assume usable-size alone explains Redis.

### 3. Typed Realloc Validated Core

Implemented:

- Replaced `mi_theap_realloc_apply(req)` with
  `mi_theap_realloc_validated_core(theap, old, layout, zero_new_tail, ...)`.
- Kept `OldAllocation`, `LayoutRequest`, and typed validation boundaries.

Result:

- Reduced request aggregate passing on the hot path.
- Did not close the Redis gap.

Do not replace this with a broad raw `(*mut c_void, usize)` realloc helper
unless profiling proves the typed witnesses themselves remain non-zero-cost.

### 4. Raw Inner Direct-Page Queue Update

Implemented:

- In `mi_theap_queue_first_update_typed`, use raw direct-page array operations
  after typed queue/theap/block-size invariants are established.

Result:

- Final full 10-run did not improve larson.
- This path may be neutral or counterproductive; keep it under review and be
  ready to revert if later profiling does not support it.

## Final 10-Run After Spec Implementation

Artifact:

```text
mimalloc-bench/out/bench/benchres-final-redis-larson-r10-20260621-155359.csv
```

Allocator set confirmed by `bench.sh`:

```text
allocators: cmi rmi
tests     : redis larson-sized
```

Results:

| Test | C avg | Rust avg | Delta |
| --- | ---: | ---: | ---: |
| `redis` relative time | 2.6700s | 2.8254s | Rust +5.82% |
| `redis` throughput | 749,099 req/s | 709,285 req/s | Rust -5.31% |
| `redis` p50 | 1.069ms | 1.122ms | Rust +4.96% |
| `larsonN-sized` relative time | 2.0799s | 2.1859s | Rust +5.10% |

Median deltas:

| Test | Delta |
| --- | ---: |
| `redis` relative time | Rust +4.10% |
| `redis` throughput | Rust -3.94% |
| `larsonN-sized` relative time | Rust +4.64% |

Conclusion:

- The spec implementation did not meet the performance goal.
- Redis p50 improved relative to the original baseline, but average throughput
  was still worse and had one Rust outlier.
- Larson remained worse than the original baseline gap.

## Current Larson Counter Evidence

Command shape:

```sh
perf stat -r 5 -d -- env LD_PRELOAD=... ./larson-sized 5 8 1000 5000 100 4141 32
```

Artifacts:

```text
mimalloc-bench/out/bench/perf-goal-larson-cmi.stat
mimalloc-bench/out/bench/perf-goal-larson-rmi.stat
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 7.07859s | 7.108011s | Rust +0.42% |
| instructions | 465.53B | 487.23B | Rust +4.66% |
| branches | 56.49B | 66.22B | Rust +17.21% |
| branch misses | 685.75M | 701.24M | Rust +2.26% |
| cycles | 668.16B | 671.64B | Rust +0.52% |
| L1 load misses | 15.89B | 16.11B | Rust +1.41% |

Interpretation:

- Larson is still mainly extra Rust instruction and branch volume.
- Cache misses, context switches, page faults, and cycles are not the primary
  differentiators.
- The next investigation should focus on reducing generated control flow in:
  - `mi_page_queue_find_free_ex_typed`
  - `_mi_malloc_generic_typed`
  - page reclaim / abandoned-page paths if they show up in sample profiles

## Verification Status

Passed:

```sh
cargo fmt --check
cargo build --release --lib
cargo test --lib
```

Known clippy status:

```sh
cargo clippy --all-targets -- -D warnings
```

Fails on the repository-wide dead-code set. No `allow(dead_code)` was added.

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

Fails because `track-valgrind` and `track-asan` are mutually exclusive.

## Next Work Items

1. Record `perf report` samples for current Rust and C larson after the latest
   counter run.
2. Inspect generated assembly for `mi_page_queue_find_free_ex_typed` and
   `_mi_malloc_generic_typed`, comparing against C equivalents.
3. For Redis, repeat direct server-side `perf stat`/`perf record` to separate
   allocator cost from client outlier noise.
4. Only implement changes that reduce confirmed hot-path instructions/branches
   while preserving typed witnesses.
5. After each change, run a small focused benchmark before the full 10-run.
6. Before declaring success, run a broader mimalloc-bench subset to check for
   regressions outside `redis` and `larson-sized`.

## 2026-06-21 Larson Queue Fill Experiment

Observation:

- C `perf annotate` for `mi_page_queue_find_free_ex` showed the direct-page
  update loop compiled to paired vector stores.
- Rust `perf annotate` for `mi_theap_queue_first_update_typed` showed scalar
  direct-page stores and loop-control overhead.
- This was a code-shape issue inside a private helper after typed queue/theap
  invariants were already established.

Change tested:

- Replaced the scalar `for sz in start..=idx` direct-page assignment with a
  private `core::slice::from_raw_parts_mut(...).fill(page)` over
  `theap->pages_free_direct[start..=idx]`.

Generated-code result:

- `mi_theap_queue_first_update_typed` folded into
  `mi_theap_queue_first_update`.
- The fill loop now emits paired vector stores (`movdqu`) plus a scalar tail,
  much closer to C's generated shape.

Counter artifacts:

```text
mimalloc-bench/out/bench/perf-goal-fill-larson-cmi.stat
mimalloc-bench/out/bench/perf-goal-fill-larson-rmi.stat
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 7.075418s | 7.11468s | Rust +0.55% |
| instructions | 477.93B | 478.64B | Rust +0.15% |
| branches | 58.01B | 61.90B | Rust +6.70% |
| branch misses | 701.83M | 714.20M | Rust +1.76% |
| cycles | 672.62B | 663.82B | Rust -1.31% |
| L1 load misses | 16.34B | 16.46B | Rust +0.79% |

Compared to the prior Rust direct larson counter run:

| Metric | Before | After | Change |
| --- | ---: | ---: | ---: |
| instructions | 487.23B | 478.64B | -1.76% |
| branches | 66.22B | 61.90B | -6.52% |
| elapsed | 7.108011s | 7.11468s | +0.09% |

Interpretation:

- Keep this change for now: it removes real generated work without weakening
  typed invariants.
- It is not sufficient for parity by itself. Remaining larson gap is now mostly
  residual branch/control-flow overhead, not instruction volume.
- Do not rework this path again until another profile shows it is still hot
  after the broader fixes.

## 2026-06-21 Larson QueuedPage Witness Experiment

Observation:

- Post-fill `perf annotate` still showed Rust spending time in
  `mi_page_queue_find_free_ex_typed`.
- The hot path constructed `QueuedPage<NormalQueue>` with the fully validating
  constructor after `mi_page_queue_find_free_ex_typed` had already found the
  page by walking the same queue.
- That validation calls `mi_page_queue_contains`, which re-walks the queue and
  adds control flow that C does not have.

Change under test:

- Added `QueuedPage::<NormalQueue>::new_unchecked(active, queue)` with a narrow
  safety contract: the caller must already know the page is linked in the
  queue, not full, and block-size compatible.
- Used it only in `mi_page_queue_find_free_ex_typed` where the candidate page is
  produced by the local queue walk.
- Follow-up sample showed a second identical pattern in the private typed queue
  push helpers: after `mi_page_queue_push_internal` inserts the page, the helper
  used the fully validating constructor and re-walked the queue. The same
  checked-by-caller constructor is now used there too.

Safety position:

- This preserves the typed witness model. The witness is still required by
  `_mi_page_free_active_internal` and `mi_page_queue_move_to_front_typed`; only
  the redundant dynamic proof is skipped when the proof is already established
  by the algorithm.

Status:

- Initial build passed. First counter run did not improve elapsed time because
  one revalidation scan remained after the push. After removing the second scan,
  the direct larson 5-repeat `perf stat` run reached parity:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 7.07533s | 7.07492s | Rust -0.01% |
| instructions | 480.57B | 476.41B | Rust -0.87% |
| branches | 58.32B | 60.04B | Rust +2.94% |
| branch misses | 706.35M | 695.92M | Rust -1.48% |
| cycles | 663.37B | 669.29B | Rust +0.89% |
| L1 load misses | 16.49B | 16.36B | Rust -0.77% |

Artifacts:

```text
mimalloc-bench/out/bench/perf-goal-witness2-larson-cmi.stat
mimalloc-bench/out/bench/perf-goal-witness2-larson-rmi.stat
```

Keep this change. It preserves typed witnesses while removing redundant dynamic
proof work from a confirmed hot path.

## 2026-06-21 Focused 10-Run After Queue Witness Fix

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis larson-sized -r=10
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-redis-larson-r10-20260621-180708.log
mimalloc-bench/out/bench/benchres-goal-redis-larson-r10-20260621-180708.csv
```

CSV results:

| Test | C avg | Rust avg | Delta | C median | Rust median | Median delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `redis` relative time | 2.6859s | 2.7556s | Rust +2.60% | 2.6700s | 2.7590s | Rust +3.33% |
| `larsonN-sized` relative time | 2.0661s | 2.0909s | Rust +1.20% | 2.0570s | 2.0940s | Rust +1.80% |

Redis last-10 output-file results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| throughput mean | 744,692 req/s | 725,772 req/s | Rust -2.54% |
| throughput median | 749,074 req/s | 724,638 req/s | Rust -3.26% |
| p50 mean | 1.0742ms | 1.1086ms | Rust +3.20% |
| p50 median | 1.067ms | 1.111ms | Rust +4.12% |

Interpretation:

- The queue witness fix improves larson diagnostics but does not satisfy the
  full benchmark goal by itself.
- Redis remains the primary blocker and needs server-side profiling.
- Do not spend more time on the queued-page constructor path unless a new
  profile shows it has become hot again.

## 2026-06-21 Redis Server-Side Realloc/Usable-Size Profile

Direct Redis server profiling:

- Started `redis-server` under `perf stat -d` with only `cmi` or `rmi`
  preloaded.
- Ran the benchmark client outside the allocator preload so the counters
  primarily cover Redis server work:

```sh
redis-benchmark -r 1000000 -n 100000 -q -P 16 lpush a 1 2 3 4 5 lrange a 1 5
```

Artifacts:

```text
mimalloc-bench/out/bench/redis-goal-cmi-run{1,2,3}.stat
mimalloc-bench/out/bench/redis-goal-rmi-run{1,2,3}.stat
mimalloc-bench/out/bench/redis-goal-cmi.out
mimalloc-bench/out/bench/redis-goal-rmi.out
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| throughput avg | 742,692 req/s | 728,215 req/s | Rust -1.95% |
| p50 avg | 1.079ms | 1.108ms | Rust +2.72% |
| server task-clock | 143.42M | 145.70M | Rust +1.59% |
| instructions | 1.7069B | 1.7636B | Rust +3.32% |
| cycles | 672.79M | 693.47M | Rust +3.07% |
| branches | 319.82M | 333.33M | Rust +4.22% |
| branch misses | 2.42M | 2.43M | Rust +0.35% |
| L1 loads | 415.30M | 426.64M | Rust +2.73% |
| L1 load misses | 11.70M | 13.41M | Rust +14.58% |

Longer Redis perf record:

```text
mimalloc-bench/out/bench/perf-goal-redis-long-cmi.data
mimalloc-bench/out/bench/perf-goal-report-redis-long-cmi.txt
mimalloc-bench/out/bench/perf-goal-redis-long-rmi.data
mimalloc-bench/out/bench/perf-goal-report-redis-long-rmi.txt
```

Client output from that longer run:

| Allocator | Throughput | p50 |
| --- | ---: | ---: |
| C mimalloc | 755,858 req/s | 1.063ms |
| Rust mimalloc | 729,395 req/s | 1.111ms |

Relevant C allocator symbols:

- `mi_theap_realloc`: 2.36%
- `malloc_usable_size`: 2.20%
- `mi_malloc`: 1.12%
- `ztryrealloc_usable`: 1.07%

Relevant Rust allocator symbols:

- `mi_malloc_usable_size`: 2.96%
- `mechanical_mimalloc::alloc::mi_theap_realloc_validated_core`: 2.36%
- `mechanical_mimalloc::alloc::mi_theap_realloc_layout`: 1.25%
- `realloc`: 1.00%
- `mi_malloc`: 0.81%

Assembly comparison:

- C `mi_theap_realloc` is flat: pointer lookup, one usable-size computation,
  resize-in-place check, then allocation/copy/free.
- Rust currently validates into `OldAllocation`, stores the typed witness on
  the stack in `mi_theap_realloc_layout`, then calls
  `mi_theap_realloc_validated_core`, which immediately destructures it again.
- For interior/aligned pointers, Rust also recomputes the block/offset while
  calculating usable size even though `mi_validate_pointer` already computed a
  `PointerLocation` containing that block and offset.

Next experiment:

- Preserve the typed `OldAllocation` boundary, but derive its usable-size field
  from the validated `PointerLocation` instead of calling `_mi_usable_size_typed`
  and repeating the interior-pointer lookup.
- Rebuild and measure Redis again before doing broader reshaping of the realloc
  call graph.

## 2026-06-21 Redis Realloc Shape Experiments

### Validated-location usable size

Change:

- Added `mi_usable_size_from_validated_location(page, location)`.
- `mi_realloc_old_allocation` now builds `OldAllocation.usable` from the
  `PointerLocation` already produced by validation.

Why:

- The prior generated `mi_theap_realloc_layout` path called
  `_mi_page_ptr_unalign` once while validating the pointer location, then could
  call it again while computing `_mi_usable_size_typed`.
- Reusing the `PointerLocation` keeps the type proof and removes the duplicate
  dynamic reconstruction.

Generated-code result:

- The interior/aligned realloc path dropped from two `_mi_page_ptr_unalign`
  calls to one.
- The first version still paid a subtract/cmov for zero-offset normal blocks.
  A follow-up fast return for `adjust == 0` removed that from the common path.

Short Redis counter result after the zero-offset refinement:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| throughput avg | 731,742 req/s | 710,957 req/s | Rust -2.84% |
| p50 avg | 1.092ms | 1.127ms | Rust +3.21% |
| task-clock | 142.63M | 146.29M | Rust +2.57% |
| instructions | 1.7070B | 1.7587B | Rust +3.03% |
| cycles | 674.58M | 679.89M | Rust +0.79% |
| branches | 322.26M | 332.81M | Rust +3.27% |
| L1 loads | 706.47M | 735.09M | Rust +4.05% |
| L1 load misses | 6.98M | 6.35M | Rust -9.11% |

Artifacts:

```text
mimalloc-bench/out/bench/redis-goal-zerooffset-cmi.out
mimalloc-bench/out/bench/redis-goal-zerooffset-rmi.out
mimalloc-bench/out/bench/redis-goal-zerooffset-cmi-run{1,2,3}.stat
mimalloc-bench/out/bench/redis-goal-zerooffset-rmi-run{1,2,3}.stat
```

Interpretation:

- Keep the validated-location helper: it removes a real redundant proof in the
  aligned/interior case and does not weaken the typed model.
- This alone is not enough for Redis. The remaining prominent issue is the
  typed realloc wrapper/core split.

### Typed realloc wrapper/core flattening

Change:

- Added `#[inline(always)]` to the private typed realloc layout/core boundary:
  `mi_theap_realloc_layout` and `mi_theap_realloc_validated_core`.

Why:

- Long Redis profile attributed time separately to
  `mi_theap_realloc_layout` and `mi_theap_realloc_validated_core`.
- The generated wrapper stored `OldAllocation` on the stack and immediately
  called the core, which destructured it again.
- Inlining lets LLVM keep the typed facts in registers while preserving the
  `OldAllocation`, `LayoutRequest`, and `ThreadHeapLocal` proof types in source.

Generated-code result:

- `nm -C target/release/libmechanical_mimalloc.so` no longer shows
  `mechanical_mimalloc::alloc::mi_theap_realloc_layout` or
  `mechanical_mimalloc::alloc::mi_theap_realloc_validated_core`.
- `mi_realloc` is now a flat path: pointer lookup, validated usable-size
  derivation, resize-in-place check, allocation/copy/free.

Short Redis counter result:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| throughput avg | 742,692 req/s | 733,532 req/s | Rust -1.23% |
| p50 avg | 1.082ms | 1.090ms | Rust +0.74% |
| task-clock | 140.71M | 142.61M | Rust +1.35% |
| instructions | 1.7248B | 1.7173B | Rust -0.43% |
| cycles | 666.37M | 666.46M | Rust +0.01% |
| branches | 319.93M | 326.92M | Rust +2.18% |
| L1 loads | 714.50M | 714.42M | Rust -0.01% |
| L1 load misses | 6.71M | 6.86M | Rust +2.22% |

Artifacts:

```text
mimalloc-bench/out/bench/redis-goal-inline-cmi.out
mimalloc-bench/out/bench/redis-goal-inline-rmi.out
mimalloc-bench/out/bench/redis-goal-inline-cmi-run{1,2,3}.stat
mimalloc-bench/out/bench/redis-goal-inline-rmi-run{1,2,3}.stat
```

Interpretation:

- Keep for now. This is the first Redis change that moves server-side CPU
  counters close to parity without weakening typed validation.
- Throughput is still slightly behind in the short sample, so do not call Redis
  solved until the real 10-run harness confirms parity or identifies the next
  blocker.

## 2026-06-21 Focused 10-Run After Redis Realloc Flattening

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis larson-sized -r=10
```

Allocator/test scope confirmed by harness output:

```text
allocators:  cmi rmi
tests     :  redis larson-sized
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-inline-redis-larson-r10-20260621-182406.log
mimalloc-bench/out/bench/benchres-goal-inline-redis-larson-r10-20260621-182406.csv
```

CSV results:

| Test | C avg | Rust avg | Delta | C median | Rust median | Median delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `redis` relative time | 2.6939s | 2.6695s | Rust -0.91% | 2.6800s | 2.6500s | Rust -1.12% |
| `larsonN-sized` relative time | 2.0748s | 2.0950s | Rust +0.97% | 2.0665s | 2.0830s | Rust +0.80% |

Redis last-10 output-file results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| throughput mean | 742,820 req/s | 749,639 req/s | Rust +0.92% |
| throughput median | 746,269 req/s | 754,728 req/s | Rust +1.13% |
| p50 mean | 1.0718ms | 1.0638ms | Rust -0.75% |
| p50 median | 1.071ms | 1.055ms | Rust -1.49% |

Improvement vs the previous focused 10-run
(`benchres-goal-redis-larson-r10-20260621-180708.csv`):

| Test | Previous median delta | Current median delta | Change |
| --- | ---: | ---: | ---: |
| `redis` relative time | Rust +3.33% | Rust -1.12% | +4.45 points |
| `larsonN-sized` relative time | Rust +1.80% | Rust +0.80% | +1.00 point |

Interpretation:

- Redis now meets the goal in the focused 10-run: Rust beats C on relative time,
  throughput, and p50 latency.
- Larson improved but is not yet at parity in the harness. The remaining gap is
  below 1%, so the next step is a dedicated larson run/profile to determine
  whether this is stable signal or benchmark variance.

## 2026-06-21 Dedicated Larson 20-Run Check

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=20
```

Allocator/test scope confirmed by harness output:

```text
allocators:  cmi rmi
tests     :  larson-sized
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-inline-larson-r20-20260621-182921.log
mimalloc-bench/out/bench/benchres-goal-inline-larson-r20-20260621-182921.csv
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.09455s | 2.10420s | Rust +0.46% |
| median relative time | 2.09200s | 2.10650s | Rust +0.69% |
| trimmed mean, drop 2 min/max | 2.09300s | 2.10381s | Rust +0.52% |
| min | 2.005s | 2.015s | Rust +0.50% |
| max | 2.209s | 2.192s | Rust -0.77% |

Interpretation:

- The mixed 10-run larson gap was not only a single outlier; a dedicated 20-run
  still shows a stable sub-1% Rust slowdown.
- This is small, but the stated goal is parity or better. Continue profiling
  larson before declaring completion.

## 2026-06-21 Rejected Larson Bin ABI Call Experiment

Hypothesis:

- Internal direct-bin maintenance might be adding front-end pressure because
  `mi_bin_impl` is inlined at each call site. Calling the no-inline exported
  `mi_bin` wrapper from the internal queue update paths might reduce code size
  enough to help larson.

Temporary change tested:

- Routed `mi_bin_from_raw_size` and the neighboring-bin scan inside
  `mi_theap_queue_first_update_typed` through exported `mi_bin`.

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=10
```

Allocator/test scope confirmed by harness output:

```text
allocators:  cmi rmi
tests     :  larson-sized
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-bin-call-larson-r10-20260621-183835.log
mimalloc-bench/out/bench/benchres-goal-bin-call-larson-r10-20260621-183835.csv
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.06850s | 2.09120s | Rust +1.10% |
| median relative time | 2.07450s | 2.09300s | Rust +0.89% |
| min | 1.994s | 2.020s | Rust +1.30% |
| max | 2.116s | 2.140s | Rust +1.13% |

Decision:

- Rejected and reverted. This is worse than the prior dedicated 20-run
  baseline, where Rust was +0.46% mean / +0.69% median.
- Do not retry the no-inline `mi_bin` internal-call path unless fresh profiles
  show instruction-cache/code-size pressure as the dominant limiter.

## 2026-06-21 Larson Sized-Delete ABI Stub Experiment

Profile/static finding:

- In the Rust DSO, sized C++ delete symbols (`_ZdlPvm`, `_ZdaPvm`) compiled as
  Rust wrappers in `mi_c_malloc_fast` mode. The hot array-sized delete path was:

```text
_ZdaPvm -> GOT mi_free_size -> PLT mi_free
```

- The C mimalloc DSO has `_ZdaPvm`/`_ZdlPvm` as one tiny ABI stub that tail-jumps
  directly to `mi_free@plt`.
- This matched the larson profile, where Rust had visible samples attributed to
  `mi_free_size` but C did not have the extra wrapper hop.

Change tested:

- For `all(not(windows), mi_c_malloc_fast)`, define sized C++ delete stubs in
  assembly:
  - `_ZdlPvm` and `_ZdaPvm` tail-jump to `mi_free`.
  - `_ZdlPvmSt11align_val_t` and `_ZdaPvmSt11align_val_t` tail-jump to
    `mi_free_size_aligned`.
- Gate the Rust wrapper functions out only for `mi_c_malloc_fast`; non-fast/debug
  builds keep the existing Rust wrappers and debug-sized validation behavior.

Static verification:

```text
00000000000166d0 <operator delete[](void*, unsigned long)>:
   endbr64
   jmp mi_free@plt
```

Focused benchmark command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=10
```

Allocator/test scope confirmed by harness output:

```text
allocators:  cmi rmi
tests     :  larson-sized
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-sized-delete-larson-r10-20260621-184602.log
mimalloc-bench/out/bench/benchres-goal-sized-delete-larson-r10-20260621-184602.csv
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.09350s | 2.08880s | Rust -0.23% |
| median relative time | 2.08550s | 2.09200s | Rust +0.31% |
| trimmed mean, drop 1 min/max | 2.09213s | 2.09238s | Rust +0.01% |
| min | 2.027s | 1.993s | Rust -1.68% |
| max | 2.171s | 2.156s | Rust -0.69% |

Interpretation:

- This is a profiling-supported improvement and reaches practical parity in the
  10-run larson result.
- Continue with a longer larson run to check whether the parity signal holds.

Longer validation command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=20
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-sized-delete-larson-r20-20260621-184906.log
mimalloc-bench/out/bench/benchres-goal-sized-delete-larson-r20-20260621-184906.csv
```

20-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.08380s | 2.10635s | Rust +1.08% |
| median relative time | 2.07900s | 2.10050s | Rust +1.03% |
| trimmed mean, drop 1 min/max | 2.08189s | 2.10722s | Rust +1.22% |
| trimmed mean, drop 2 min/max | 2.07969s | 2.10725s | Rust +1.33% |

Comparison to prior 20-run baseline
(`benchres-goal-inline-larson-r20-20260621-182921.csv`):

| Metric | Prior delta | This variant delta |
| --- | ---: | ---: |
| mean | Rust +0.46% | Rust +1.08% |
| median | Rust +0.69% | Rust +1.03% |
| trimmed mean, drop 2 min/max | Rust +0.52% | Rust +1.33% |

Decision:

- Do not keep this first placement. The ABI path is cleaner, but inserting the
  separate assembly block before the hot fast-free/malloc block shifted code
  layout and did not validate in the stronger run.
- If retrying this idea, keep `free`/`malloc` layout stable by placing the sized
  delete stubs inside the existing fast-override assembly block after the `free`
  aliases and reducing the padding by the inserted stub bytes.

## 2026-06-21 Larson Sized-Delete Layout-Preserving Variant

Follow-up change:

- Moved the sized C++ delete stubs into the existing fast-override assembly block
  after the `free` aliases.
- Reduced the padding before `malloc` from `0x540` to `0x520` to keep the
  free-to-malloc spacing stable after adding two 16-byte stub slots.

Static verification:

```text
0000000000016770 <operator delete[](void*, unsigned long)>:
   endbr64
   jmp mi_free@plt
```

Focused benchmark command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=10
```

Allocator/test scope confirmed by harness output:

```text
allocators:  cmi rmi
tests     :  larson-sized
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-sized-delete-layout-larson-r10-20260621-185611.log
mimalloc-bench/out/bench/benchres-goal-sized-delete-layout-larson-r10-20260621-185611.csv
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.07250s | 2.06440s | Rust -0.39% |
| median relative time | 2.07100s | 2.05600s | Rust -0.72% |
| trimmed mean, drop 1 min/max | 2.07388s | 2.06513s | Rust -0.42% |
| min | 2.008s | 2.015s | Rust +0.35% |
| max | 2.126s | 2.108s | Rust -0.85% |

Interpretation:

- This is the first larson-focused result that beats C across mean, median, and
  trimmed mean.
- Needs 20-run validation before accepting because the prior first-placement
  variant looked good at 10 runs but failed at 20.

20-run validation command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=20
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-sized-delete-layout-larson-r20-20260621-185912.log
mimalloc-bench/out/bench/benchres-goal-sized-delete-layout-larson-r20-20260621-185912.csv
```

20-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.10720s | 2.07575s | Rust -1.49% |
| median relative time | 2.10450s | 2.06200s | Rust -2.02% |
| trimmed mean, drop 1 min/max | 2.10667s | 2.07839s | Rust -1.34% |
| trimmed mean, drop 2 min/max | 2.10613s | 2.07663s | Rust -1.40% |
| min | 2.039s | 1.925s | Rust -5.59% |
| max | 2.185s | 2.179s | Rust -0.27% |

Comparison to prior accepted larson 20-run
(`benchres-goal-inline-larson-r20-20260621-182921.csv`):

| Metric | Prior delta | Current delta | Change |
| --- | ---: | ---: | ---: |
| mean | Rust +0.46% | Rust -1.49% | +1.95 points |
| median | Rust +0.69% | Rust -2.02% | +2.71 points |
| trimmed mean, drop 2 min/max | Rust +0.52% | Rust -1.40% | +1.92 points |

Decision:

- Keep this variant. It removes the extra sized-delete wrapper hop while
  preserving the hot fast-override layout better than the first placement.
- This satisfies the larson goal in the dedicated 20-run: Rust now beats C
  across mean, median, and trimmed means.

## 2026-06-21 Redis Regression Check After Layout-Preserving Variant

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis -r=10
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-final-redis-r10-20260621-190443.log
mimalloc-bench/out/bench/benchres-goal-final-redis-r10-20260621-190443.csv
```

Results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| relative time mean | 2.68780s | 2.71380s | Rust +0.97% |
| relative time median | 2.68000s | 2.69000s | Rust +0.37% |
| relative time trimmed mean, drop 1 min/max | 2.68488s | 2.69738s | Rust +0.47% |
| ops/sec mean | 744,261 | 737,331 | Rust -0.93% |
| ops/sec median | 746,269 | 743,505 | Rust -0.37% |
| ops/sec trimmed mean, drop 1 min/max | 744,978 | 741,472 | Rust -0.47% |

Decision:

- Do not accept the layout-preserving variant as-is. Redis is only slightly
  slower, but the goal explicitly says not to regress the other benchmark
  scenarios.
- Static inspection showed that removing the Rust sized-delete wrappers shifted
  the fast override block by `0x50` bytes even though relative free-to-malloc
  spacing was preserved. Try one more variant that restores the pre-stub hot
  symbol addresses.

## 2026-06-21 Address-Compensated Sized-Delete Variant

Change:

- Added `0x50` bytes of padding before the `mi_c_malloc_fast` override assembly
  so the hot fast-path addresses returned to the pre-stub layout while keeping
  the one-hop sized-delete stubs.

Static verification:

```text
0000000000016720 <operator delete[](void*)>:
00000000000167c0 <operator delete[](void*, unsigned long)>:
0000000000016d00 <__libc_malloc>:
0000000000016e00 <mi_free_size>:
```

Combined focused benchmark command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis larson-sized -r=10
```

Allocator/test scope confirmed by harness output:

```text
allocators:  cmi rmi
tests     :  redis larson-sized
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-layout-comp-redis-larson-r10-20260621-190723.log
mimalloc-bench/out/bench/benchres-goal-layout-comp-redis-larson-r10-20260621-190723.csv
```

Results:

| Test / metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| Redis relative time mean | 2.69560s | 2.69000s | Rust -0.21% |
| Redis relative time median | 2.70000s | 2.68000s | Rust -0.74% |
| Redis relative time trimmed mean | 2.69463s | 2.68500s | Rust -0.36% |
| Redis ops/sec mean | 742,124 | 743,643 | Rust +0.20% |
| Redis ops/sec median | 740,741 | 746,269 | Rust +0.75% |
| Redis ops/sec trimmed mean | 742,307 | 744,928 | Rust +0.35% |
| Larson relative time mean | 2.10640s | 2.07260s | Rust -1.61% |
| Larson relative time median | 2.10650s | 2.07250s | Rust -1.61% |
| Larson relative time trimmed mean | 2.10988s | 2.07125s | Rust -1.83% |

Decision:

- Keep testing this variant. It restores the Redis win and preserves the larson
  win in a combined 10-run.
- Run a dedicated larson 20-run on this exact build because the previous larson
  20-run was for the pre-compensation layout.

Dedicated larson 20-run command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=20
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-layout-comp-larson-r20-20260621-191217.log
mimalloc-bench/out/bench/benchres-goal-layout-comp-larson-r20-20260621-191217.csv
```

20-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.07710s | 2.09480s | Rust +0.85% |
| median relative time | 2.08050s | 2.09700s | Rust +0.79% |
| trimmed mean, drop 1 min/max | 2.07683s | 2.09156s | Rust +0.71% |
| trimmed mean, drop 2 min/max | 2.07588s | 2.08881s | Rust +0.62% |

Decision:

- Rejected. The address compensation fixed Redis in the combined 10-run, but it
  lost the larson win in the stronger dedicated 20-run and is worse than the old
  accepted baseline.
- Revert the `0x50` pre-padding and return to the non-compensated sized-delete
  layout, which had the strong larson 20-run. Re-run Redis on that layout to
  determine whether the prior Redis slowdown was noise or a real tradeoff.

## 2026-06-21 Non-Compensated Redis 20-Run Recheck

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis -r=20
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-noncomp-redis-r20-20260621-191814.log
mimalloc-bench/out/bench/benchres-goal-noncomp-redis-r20-20260621-191814.csv
```

20-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| relative time mean | 2.66785s | 2.69280s | Rust +0.94% |
| relative time median | 2.66000s | 2.68000s | Rust +0.75% |
| relative time trimmed mean, drop 2 min/max | 2.66500s | 2.68481s | Rust +0.74% |
| ops/sec mean | 749,743 | 742,983 | Rust -0.90% |
| ops/sec median | 751,880 | 746,269 | Rust -0.75% |
| ops/sec trimmed mean, drop 2 min/max | 750,514 | 745,000 | Rust -0.73% |

Decision:

- The Redis loss on the non-compensated sized-delete layout is real enough to
  reject accepting that layout as-is.
- We now have two failed extremes:
  - no pre-padding: larson wins, Redis loses.
  - `0x50` pre-padding: Redis wins in 10-run, larson loses in 20-run.
- Next step is a small padding sweep between those extremes, using short
  combined Redis + larson runs to find a candidate that keeps both workloads at
  parity before spending on longer validation.

## 2026-06-21 Padding Sweep: `0x20` Pre-Padding

Change under test:

- Keep the fast C++ sized-delete assembly stubs.
- Insert `.fill 0x20,1,0x90` before the fast malloc/free override assembly.
- Keep the internal post-stub `.fill 0x520,1,0x90`.

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis larson-sized -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad20-redis-larson-r5-20260621-192356.log
mimalloc-bench/out/bench/benchres-goal-pad20-redis-larson-r5-20260621-192356.csv
```

5-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| Redis relative time mean | 2.67600s | 2.68000s | Rust +0.15% |
| Redis relative time median | 2.66000s | 2.68000s | Rust +0.75% |
| Redis relative time trimmed mean | 2.67333s | 2.68000s | Rust +0.25% |
| Larson relative time mean | 2.08820s | 2.13920s | Rust +2.44% |
| Larson relative time median | 2.05200s | 2.14700s | Rust +4.63% |
| Larson relative time trimmed mean | 2.07200s | 2.14467s | Rust +3.51% |

Decision:

- Rejected. Redis is close enough to keep investigating nearby layouts, but
  larson regresses sharply even in the short run.
- Next candidate: `.fill 0x30,1,0x90` pre-padding.

## 2026-06-21 Padding Sweep: `0x30` Pre-Padding

Change under test:

- Keep the fast C++ sized-delete assembly stubs.
- Insert `.fill 0x30,1,0x90` before the fast malloc/free override assembly.
- Keep the internal post-stub `.fill 0x520,1,0x90`.

Static symbols after build:

```text
mi_free / operator delete[](void*)        0x16700
operator delete[](void*, unsigned long)  0x167a0
mi_malloc                                0x16ce0
mi_free_size                             0x16de0
```

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis larson-sized -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-redis-larson-r5-20260621-192645.log
mimalloc-bench/out/bench/benchres-goal-pad30-redis-larson-r5-20260621-192645.csv
```

5-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| Redis relative time mean | 2.69200s | 2.68400s | Rust -0.30% |
| Redis relative time median | 2.70000s | 2.68000s | Rust -0.74% |
| Redis relative time trimmed mean | 2.70000s | 2.68000s | Rust -0.74% |
| Larson relative time mean | 2.07960s | 2.06600s | Rust -0.65% |
| Larson relative time median | 2.06600s | 2.08400s | Rust +0.87% |
| Larson relative time trimmed mean | 2.08000s | 2.07467s | Rust -0.26% |

Decision:

- Keep testing. This is the first midpoint candidate where Redis is better on
  all aggregate views and larson is at least competitive on mean/trimmed mean.
- Run a dedicated larson 20-run first because larson has been the less stable
  signal and previous candidates failed there after promising short runs.

Dedicated larson 20-run command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi larson-sized -r=20
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-larson-r20-20260621-192922.log
mimalloc-bench/out/bench/benchres-goal-pad30-larson-r20-20260621-192922.csv
```

20-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean relative time | 2.10090s | 2.08270s | Rust -0.87% |
| median relative time | 2.09250s | 2.08050s | Rust -0.57% |
| trimmed mean, drop 1 min/max | 2.09850s | 2.08217s | Rust -0.78% |
| trimmed mean, drop 2 min/max | 2.09419s | 2.08031s | Rust -0.66% |

Decision:

- `0x30` survives larson validation. Unlike the `0x50` compensated layout, it
  remains faster than C after a 20-run, including median and trim-2.
- Next validation: Redis 20-run on the exact same build.

Dedicated Redis 20-run command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi redis -r=20
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-redis-r20-20260621-193431.log
mimalloc-bench/out/bench/benchres-goal-pad30-redis-r20-20260621-193431.csv
```

20-run relative-time results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean | 2.69975s | 2.68290s | Rust -0.62% |
| median | 2.68000s | 2.68000s | tied |
| trimmed mean, drop 1 min/max | 2.68428s | 2.67989s | Rust -0.16% |
| trimmed mean, drop 2 min/max | 2.67494s | 2.67744s | Rust +0.09% |

20-run ops/sec results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| mean | 741,702 | 745,570 | Rust +0.52% |
| median | 746,269 | 746,269 | tied |
| trimmed mean, drop 1 min/max | 745,394 | 746,355 | Rust +0.13% |
| trimmed mean, drop 2 min/max | 747,755 | 747,011 | Rust -0.10% |

Decision:

- Treat as Redis parity, not a clean win. Median is tied and trim-2 is within
  0.1% either way.
- Combined with the larson 20-run win, `0x30` is the current best candidate.
- Next validation: RocksDB guardrail with only `cmi` and `rmi`.

RocksDB guardrail command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi rocksdb -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-rocksdb-r5-20260621-193837.log
mimalloc-bench/out/bench/benchres-goal-pad30-rocksdb-r5-20260621-193837.csv
```

5-run results:

| Metric | C | Rust | Delta |
| --- | ---: | ---: | ---: |
| time mean | 3.572s | 3.572s | tied |
| time median | 3.570s | 3.570s | tied |
| time trimmed mean | 3.573s | 3.570s | Rust -0.09% |
| RSS mean | 149,722 | 121,646 | Rust -18.75% |
| RSS median | 149,708 | 121,452 | Rust -18.87% |
| RSS trimmed mean | 149,725 | 121,365 | Rust -18.94% |

Decision:

- No RocksDB time regression observed in this guardrail run.
- Rust uses materially less RSS in this scenario.
- The `0x30` layout remains the current candidate: larson improves, Redis is
  parity, RocksDB is parity with lower RSS.

## 2026-06-21 Verification Notes For `0x30` Candidate

Commands:

```sh
cargo fmt --check
cargo test --lib
cargo clippy --all-targets -- -D warnings
```

Results:

- `cargo fmt --check` passed.
- `cargo test --lib` passed: 152 tests.
- `cargo clippy --all-targets -- -D warnings` failed on the existing
  repo-wide `dead_code` surface: 250 lib errors and 248 lib-test errors. I did
  not suppress these with `allow(dead_code)`.

Representative clippy failures:

```text
src/alloc_aligned.rs:947  mi_theap_realloc_zero_aligned_at is never used
src/alloc_override.rs:9   MI_OSX_IS_INTERPOSED is never used
src/atomic.rs:16          MI_ATOMIC_H is never used
src/page.rs:108           mi_bin is never used
src/types.rs:178          mi_page_kind_e is never used
```

Decision:

- The performance candidate passes formatting, tests, and release build.
- Clippy is still not clean. Fixing it properly requires deciding which
  translation constants/wrappers should become cfg-gated, private, exported,
  or removed; blanket `allow(dead_code)` remains intentionally rejected.

## 2026-06-21 Broader Guardrail Smoke

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi allt -r=1
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-allt-r1-20260621-194135.log
mimalloc-bench/out/bench/benchres-goal-pad30-allt-r1-20260621-194135.csv
```

One-pass time deltas:

| Test | Delta |
| --- | ---: |
| cfrac | Rust +1.48% |
| espresso | Rust +0.70% |
| barnes | Rust -1.41% |
| redis | tied |
| larsonN-sized | Rust -2.33% |
| mstressN | Rust +1.23% |
| rptestN | Rust -1.69% |
| gs | tied |
| lua | tied |
| alloc-test1 | Rust -0.96% |
| alloc-testN | tied |
| sh6benchN | Rust -33.33% |
| sh8benchN | tied |
| xmalloc-testN | Rust +3.29% |
| cache-scratch1 | Rust -1.19% |
| cache-scratchN | tied |
| glibc-simple | Rust +2.19% |
| glibc-thread | Rust +1.39% |
| rocksdb | Rust +0.56% |

Decision:

- No catastrophic broad regression appeared.
- Re-run the single-pass slow cases above roughly 1% because one-pass results
  are too noisy for a decision.

Focused follow-up command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi cfrac mstress xmalloc-test glibc-simple glibc-thread -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-guardrail-slowcases-r5-20260621-194355.log
mimalloc-bench/out/bench/benchres-goal-pad30-guardrail-slowcases-r5-20260621-194355.csv
```

5-run follow-up:

| Test | Mean Delta | Median Delta | Trimmed Mean Delta | RSS Mean Delta |
| --- | ---: | ---: | ---: | ---: |
| cfrac | Rust +0.22% | tied | Rust +0.12% | Rust +4.73% |
| mstressN | Rust -0.61% | Rust -0.61% | Rust -0.61% | Rust -1.12% |
| xmalloc-testN | Rust +0.28% | Rust +1.41% | Rust +0.47% | Rust +7.33% |
| glibc-simple | Rust +0.15% | Rust -0.72% | Rust -0.48% | Rust +3.83% |
| glibc-thread | Rust +0.76% | Rust +0.96% | Rust +0.88% | Rust -0.42% |

Decision:

- The broad guardrail does not show a material non-target regression from the
  `0x30` candidate.
- Residual watch items:
  - `glibc-thread` is consistently but mildly slower in this 5-run.
  - `xmalloc-testN` is near parity by mean/trimmed mean but has a slower median
    and higher RSS.
- These residuals are much smaller than the original Redis/larson regression
  and should be tracked in a separate follow-up if the project wants strict
  parity on every mimalloc-bench row.

## 2026-06-21 Rest-Of-Suite 10-Run Guardrail

Command:

```sh
../../bench.sh --external=../../rust-vs-c.allocators cmi rmi allt no-redis no-larson-sized no-rocksdb -r=10
```

Scope:

- Only `cmi` and `rmi`.
- Excluded Redis, larson-sized, and RocksDB because they already had focused
  validation runs for this candidate.

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-pad30-rest-allt-r10-20260621-200037.log
mimalloc-bench/out/bench/benchres-goal-pad30-rest-allt-r10-20260621-200037.csv
```

10-run deltas. Negative time means Rust is faster:

| Test | Mean | Median | Trim-2 | RSS mean |
| --- | ---: | ---: | ---: | ---: |
| cfrac | Rust -0.47% | Rust -0.55% | Rust -0.49% | Rust +2.93% |
| espresso | Rust +0.17% | tied | Rust +0.06% | Rust +2.62% |
| barnes | Rust -0.37% | Rust -0.70% | Rust -0.55% | Rust +0.20% |
| mstressN | Rust +0.68% | Rust +0.31% | Rust +0.21% | Rust -0.45% |
| rptestN | Rust -2.75% | Rust -2.92% | Rust -1.61% | Rust +1.34% |
| gs | tied | tied | tied | Rust -0.28% |
| lua | tied | tied | tied | Rust -2.20% |
| alloc-test1 | Rust -0.53% | Rust -0.24% | Rust -0.48% | Rust +0.44% |
| alloc-testN | Rust -1.21% | tied | Rust +0.08% | Rust -0.39% |
| sh6benchN | Rust -27.59% | Rust -31.03% | Rust -28.74% | Rust -0.42% |
| sh8benchN | Rust +2.10% | tied | tied | Rust +0.17% |
| xmalloc-testN | Rust -1.38% | Rust -2.28% | Rust -1.83% | Rust +6.57% |
| cache-scratch1 | Rust -0.71% | Rust -1.19% | Rust -0.79% | Rust +0.83% |
| cache-scratchN | Rust +3.23% | tied | tied | Rust +1.06% |
| glibc-simple | Rust -0.36% | Rust -0.37% | Rust -0.24% | Rust +0.34% |
| glibc-thread | Rust +0.50% | Rust +0.60% | Rust +0.44% | Rust -0.53% |

Decision:

- The 10-run rest-of-suite guardrail does not show a material regression.
- Trim-2 Rust-slower cases are small:
  - `glibc-thread`: +0.44%.
  - `mstressN`: +0.21%.
  - `alloc-testN`: +0.08%.
  - `espresso`: +0.06%.
- Several non-target scenarios improved by trim-2, notably `sh6benchN`,
  `xmalloc-testN`, `rptestN`, `cache-scratch1`, `barnes`, `cfrac`,
  `alloc-test1`, and `glibc-simple`.
- `sh6benchN`, `sh8benchN`, and `cache-scratchN` have very small absolute
  elapsed times, so their large percentage swings should be interpreted with
  that quantization in mind.
