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

## 2026-06-22 Follow-Up Goal

Objective:

- Try the four identified follow-up areas:
  - deterministic hot symbol layout,
  - ABI wrapper audit,
  - validated hot-path flattening,
  - residual `glibc-thread` / `xmalloc-test` investigation.
- Keep only source changes that show repeatable benchmark improvement beyond
  ordinary noise and do not regress the existing Redis, larson, or RocksDB
  results.

Baseline setup:

```sh
cargo build --release --lib
cp target/release/libmechanical_mimalloc.so target/bench-allocators/libmimalloc-rust.so
cp target/release/libmechanical_mimalloc.so target/bench-allocators/libmimalloc-rust-baseline.so
```

Temporary allocator descriptors:

```text
mimalloc-bench/rust-baseline-vs-candidate.allocators
mimalloc-bench/c-rust-baseline-candidate.allocators
```

Allocator names:

```text
rbase = committed Rust baseline from f4d8f4d
rexp  = current experiment build
cmi   = C mimalloc
```

### 1. Deterministic Hot Symbol Layout

Experiment:

- Replaced the pre-`free` `.fill 0x30,1,0x90` pad with `.p2align 6`.
- Release build succeeded and the exported hot symbol addresses matched the
  saved Rust baseline exactly:

```text
0000000000016700 T operator delete[](void*)
00000000000167a0 T operator delete[](void*, unsigned long)
0000000000016700 T mi_free
0000000000016de0 T mi_free_size
0000000000016ce0 T mi_malloc
```

Focused benchmark:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators \
  rbase rexp redis larson-sized -r=5
```

Artifact:

```text
mimalloc-bench/out/bench/benchres-followup-p2align-layout-redis-larson-r5-20260622-202645.csv
```

Results versus saved Rust baseline:

| Test | Mean | Median | Trim1 | Trim2 |
| --- | ---: | ---: | ---: | ---: |
| `redis` | rexp +1.95% | rexp +0.75% | rexp +1.24% | rexp +0.75% |
| `larsonN-sized` | rexp +0.74% | rexp +1.35% | rexp +0.76% | rexp +1.35% |

Conclusion:

- Rejected. This did not produce a real improvement and may slightly regress
  the target scenarios.
- Restored the explicit `.fill 0x30` pad.
- Do not retry `.p2align 6` as a standalone deterministic-layout cleanup.

### 2. ABI Wrapper Audit

Benchmark dynamic imports:

```text
larson-sized:
  operator new[](unsigned long)
  operator delete[](void*, unsigned long)

xmalloc-test:
  malloc
  free

glibc-thread:
  malloc
  free

redis-server:
  malloc
  free
  calloc
  realloc
  malloc_usable_size
```

Rust and C both route non-primary delete/free wrappers through PLT jumps for
the comparable symbols, so the remaining wrapper shape is not a Rust-only
defect.

Flat `perf record` artifacts:

```text
mimalloc-bench/out/bench/perf-followup-abi-larson-rust-20260622.data
mimalloc-bench/out/bench/perf-followup-abi-larson-c-20260622.data
```

Top Rust allocator-side samples:

```text
8.02% operator new[](unsigned long)
6.97% operator delete[](void*)
5.19% mi_page_queue_find_free_ex_typed
3.52% _mi_malloc_generic_typed
2.54% operator delete[](void*, unsigned long)@plt
2.13% mi_theap_queue_first_update
```

Top C allocator-side samples:

```text
11.46% operator new[](unsigned long)
9.09% operator delete[](void*)
5.62% mi_page_queue_find_free_ex
2.58% _mi_malloc_generic
1.66% operator delete[](void*, unsigned long)@plt
1.51% mi_bin
0.97% mi_page_queue_remove
```

Conclusion:

- No source change accepted for the ABI wrapper item.
- The hot benchmark ABI surface is already covered by the existing fast
  `operator new[]`, `free`, and sized-delete stubs.
- Extra aligned/nothrow C++ wrapper stubs would not affect Redis,
  `xmalloc-test`, `glibc-thread`, or the current larson binary.
- Continue in the typed page allocation/search path instead of adding wrapper
  variants speculatively.

### 3. Validated Hot-Path Flattening

Experiment:

- `_mi_malloc_generic_typed` wrote `usable` from `mi_page_usable_block_size`
  before calling `_mi_page_malloc_zero_typed`.
- `_mi_page_malloc_zero_typed` already computes the same usable block size for
  tracking and zeroing work after popping the block.
- Tested passing `usable` into `_mi_page_malloc_zero_typed` and doing the
  write from the existing local `bsize`.

Build/code-shape notes:

- Release build succeeded.
- Primary hot exported symbols stayed at the same addresses as the saved
  Rust baseline:

```text
0000000000016700 T operator delete[](void*)
00000000000167a0 T operator delete[](void*, unsigned long)
0000000000016700 T mi_free
0000000000016de0 T mi_free_size
0000000000016ce0 T mi_malloc
```

Initial focused benchmark:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators \
  rbase rexp redis larson-sized xmalloc-test glibc-thread -r=5
```

Artifact:

```text
mimalloc-bench/out/bench/benchres-followup-usable-pass-r5-20260622-203254.csv
```

Initial results versus saved Rust baseline:

| Test | Mean | Median | Trim1 | Trim2 |
| --- | ---: | ---: | ---: | ---: |
| `larsonN-sized` | rexp -2.85% | rexp -3.70% | rexp -3.47% | rexp -3.70% |
| `redis` | rexp -0.30% | rexp -1.47% | rexp -0.99% | rexp -1.47% |
| `xmalloc-testN` | rexp -0.09% | rexp +0.47% | tied | rexp +0.47% |
| `glibc-thread` | rexp -0.48% | tied | rexp -0.24% | tied |

Longer validation:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators \
  rbase rexp redis larson-sized rocksdb xmalloc-test glibc-thread -r=10
```

Artifact:

```text
mimalloc-bench/out/bench/benchres-followup-usable-pass-r10-20260622-203646.csv
```

Ten-run results versus saved Rust baseline:

| Test | Paired faster | Mean | Median | Trim1 | Trim2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `larsonN-sized` | 7/10 | rexp -0.44% | rexp +0.80% | rexp -0.38% | rexp -0.42% |
| `redis` | 5/10 | rexp -0.08% | tied | rexp -0.10% | rexp -0.12% |
| `rocksdb` | 6/10 | rexp -0.74% | rexp -1.10% | rexp -0.86% | rexp -0.87% |
| `xmalloc-testN` | 4/10 | rexp +0.14% | rexp -0.23% | rexp +0.17% | rexp +0.08% |
| `glibc-thread` | 6/10 | rexp -0.24% | rexp -0.36% | rexp -0.24% | rexp -0.31% |

Decision:

- Rejected and reverted.
- The first five-run result looked promising, but the ten-run did not confirm a
  real larson improvement; the median moved the wrong way and the trimmed
  means were below 0.5%.
- Redis, RocksDB, `xmalloc-testN`, and `glibc-thread` stayed in a small noise
  band. RocksDB looked slightly better in this run, but the effect size was
  too small to accept a source change by itself.
- Do not retry this exact usable-size pass-through as a performance fix unless
  a future profile shows a stronger, isolated reason to revisit it.

### 4. Residual `glibc-thread` / `xmalloc-testN`

Benchmark observation:

- The rejected flattening experiment did not materially change either residual
  scenario.
- In the ten-run validation, `xmalloc-testN` was effectively noise:
  rexp +0.14% mean, rexp -0.23% median, rexp +0.17% trim1, rexp +0.08%
  trim2.
- `glibc-thread` was also effectively noise:
  rexp -0.24% mean, rexp -0.36% median, rexp -0.24% trim1, rexp -0.31%
  trim2.

Perf-counter check:

```sh
perf stat -r 5 -e task-clock,cycles,instructions,branches,branch-misses \
  env LD_PRELOAD=<allocator> ./xmalloc-test -w 32 -t 5 -s 64
perf stat -r 5 -e task-clock,cycles,instructions,branches,branch-misses \
  env LD_PRELOAD=<allocator> ./glibc-thread 32
```

Artifacts:

```text
mimalloc-bench/out/bench/perf-followup-residual-xmalloc-rust-20260622.stat
mimalloc-bench/out/bench/perf-followup-residual-xmalloc-c-20260622.stat
mimalloc-bench/out/bench/perf-followup-residual-glibc-thread-rust-20260622.stat
mimalloc-bench/out/bench/perf-followup-residual-glibc-thread-c-20260622.stat
```

Results:

| Test | Allocator | Elapsed | Task-clock | Cycles | Instructions | Branch misses |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `xmalloc-testN` | Rust | 5.01055s | 148.65B | 633.77B | 553.41B | 225.21M |
| `xmalloc-testN` | C | 5.01229s | 141.65B | 606.45B | 496.91B | 308.82M |
| `glibc-thread` | Rust | 2.002933s | 62.11B | 237.64B | 417.08B | 2526.56M |
| `glibc-thread` | C | 2.003175s | 63.51B | 241.41B | 422.10B | 2574.60M |

Conclusion:

- No source change accepted.
- `xmalloc-testN` shows Rust doing more instructions and branches than C, but
  elapsed time is tied over five perf repetitions and the benchmark ten-run is
  within noise. This is a possible future code-size/instruction-count cleanup,
  not a confirmed wall-time regression.
- `glibc-thread` is already at parity with C by elapsed time and slightly lower
  than C on task-clock, cycles, instructions, and branch misses in this run.
- Future residual work should start with allocator-side flat/callgraph profiles
  for `xmalloc-testN`, not with source rewrites, and should require a larger
  elapsed-time signal than the current sub-percent movements.

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

## 2026-06-21 Follow-Up Goal: Typed-Preserving Performance Candidates

Goal:

- Investigate the five remaining candidate areas from the full perf sweep:
  xmalloc free/collect, generic allocation queue search, fast malloc/free entry
  shape, aligned allocation, and page free-list extension.
- Preserve the typed allocator invariants. Do not bypass validation, remove
  witnesses, weaken wrappers, or paper over warnings.
- Accept only changes with repeatable, non-noise improvements and no material
  regressions across the relevant mimalloc-bench scenarios.

Baseline:

- Git HEAD at goal start: `530592d Document follow-up performance experiments`.
- Tracked working tree was clean; only pre-existing untracked analysis and
  benchmark artifacts were present.
- Rebuilt the current Rust allocator and copied it to both:
  - `target/bench-allocators/libmimalloc-rust.so`
  - `target/bench-allocators/libmimalloc-rust-baseline.so`

Perf sweep reference:

- Directory: `mimalloc-bench/out/bench/perf-all-rust-20260621-212327`
- Practical sweep skipped external/unavailable suites: `leanN`, `mathlib`,
  `linux`, `spec`, and `security`.

Initial candidate order:

1. Page free-list extension: try replacing the internal raw C-ABI wrapper call
   with direct typed helper calls from `mi_page_extend_free_typed`.
2. Xmalloc partial collect: inspect whether the local-to-available promotion can
   stay direct while preserving the `LocalPage`/free-list wrappers.
3. Generic queue search: compare Rust/C control flow before attempting any
   queue-state change, because heap collection may mutate queues.
4. Fast malloc/free entry shape: inspect codegen/perf before changing public
   ABI wrappers.
5. Aligned allocation: avoid repeating the rejected usable-size pass-through
   experiment unless new evidence points there.

### Candidate: Direct Typed Page Free-List Extension

Change tried:

- In `mi_page_extend_free_typed`, replace calls through the private raw
  `mi_page_free_list_extend`/`mi_page_free_list_extend_secure` wrappers with
  direct calls to `mi_page_free_list_extend_typed` and
  `mi_page_free_list_extend_secure_typed`.
- Constructed `BlockSize` once from the already-validated page block size.
- Removed the now-unused private secure raw wrapper rather than suppressing the
  resulting dead-code warning.

Type-system assessment:

- The attempted change preserved the existing `PagePtr` witness and introduced
  no new unchecked raw-page path beyond the already-required
  `BlockSize::new_unchecked(bsize)` conversion.
- No validation or witness wiring was weakened.

Correctness checks:

- `cargo build --release --lib` succeeded.
- `cargo test --lib free::tests::free_validation_covers_null_invalid_and_bad_alignment -- --exact --nocapture`
  passed in debug mode.
- `cargo test --release --lib` is not a clean gate today:
  - `core_types::scalar::tests::requested_size_padding_checks_overflow` and
    `core_types::scalar::tests::size_wrappers_return_typed_errors_for_checked_arithmetic`
    fail independently in release mode because overflow checks are optimized
    differently than the tests expect.
  - `free::tests::free_validation_covers_null_invalid_and_bad_alignment`
    segfaults independently in release mode but passes in debug mode.
  - These failures are not from the page free-list extension path, but they mean
    release unit tests cannot currently be used as a clean accept/reject gate for
    allocator performance experiments.

Focused 5-run benchmark:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test sh8bench rptest larson-sized glibc-thread -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-typed-extend-r5-20260621-214025.log
mimalloc-bench/out/bench/benchres-goal-typed-extend-r5-20260621-214025.csv
```

Negative time means the candidate is faster:

| Test | Mean | Median | Trim-1 | Paired Faster |
| --- | ---: | ---: | ---: | ---: |
| glibc-thread | rexp +1.20% | rexp +1.45% | rexp +1.28% | 0/5 |
| larsonN-sized | rexp +2.03% | rexp +3.25% | rexp +2.42% | 2/5 |
| rptestN | rexp +8.51% | rexp +9.23% | rexp +7.05% | 1/5 |
| sh8benchN | rexp +2.48% | tied | tied | 1/5 |
| xmalloc-testN | rexp +0.55% | rexp +0.46% | rexp +0.92% | 1/5 |

Decision:

- Rejected and reverted.
- This path did not produce a repeatable improvement and regressed protected
  scenarios, especially `rptestN`, `larsonN-sized`, and `glibc-thread`.
- Do not retry the direct wrapper-elision version of page free-list extension
  without fresh codegen evidence that explains the regression.

### Candidate: Direct Local-Free Promotion in Partial Collect

Change tried:

- In `_mi_page_free_collect_partly_typed`, when a remote collect moved blocks
  into `local_free` and `free` was still empty, directly promoted the non-empty
  `LocalFreeList` into `AvailableFreeList`.
- Used the existing `LocalPage`/`PagePtr` witness path and
  `PagePtr::set_free`/`PagePtr::set_local_free`; did not write raw page fields
  for the list heads directly.
- Kept the explicit `free_is_zero = false` update from the existing quick
  collect helper.

Type-system assessment:

- The attempted change preserved the existing page witness and typed free-list
  wrappers.
- It did skip the generic `mi_page_free_quick_collect_typed` helper after
  reproducing the same local preconditions at the call site, so the risk was
  code-shape and duplicated invariant maintenance, not weakened validation.

Correctness checks:

- `cargo build --release --lib` succeeded.
- No new compiler warnings were introduced beyond the existing 264 warnings.

Focused 5-run benchmark:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test sh8bench rptest larson-sized glibc-thread -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-partial-collect-r5-20260621-214856.log
mimalloc-bench/out/bench/benchres-goal-partial-collect-r5-20260621-214856.csv
```

Negative time means the candidate is faster:

| Test | Mean | Median | Trim-1 | Paired Faster |
| --- | ---: | ---: | ---: | ---: |
| glibc-thread | rexp +0.62% | rexp +0.72% | rexp +0.72% | 1/5 |
| larsonN-sized | rexp -1.35% | rexp +0.10% | rexp -0.59% | 3/5 |
| rptestN | rexp -3.18% | rexp -1.56% | rexp -2.77% | 4/5 |
| sh8benchN | rexp -4.80% | tied | rexp -1.37% | 2/5 |
| xmalloc-testN | rexp -1.11% | rexp -1.84% | rexp -1.54% | 4/5 |

Decision:

- Rejected and reverted.
- The target path improved `xmalloc-testN`, and `rptestN` also improved, but
  `glibc-thread` regressed consistently across mean, median, trim-1, and paired
  runs.
- Do not retry duplicating the quick-collect local promotion inline unless a
  later change removes the `glibc-thread` cost or codegen shows a way to keep
  the helper abstraction while eliminating the target overhead.

### Candidate: Direct Typed Queue First-Update Calls

Change tried:

- Added a private `mi_theap_queue_first_update_if_normal_typed` helper that
  takes `ThreadHeapLocal` and `PageQueuePtr`.
- Routed internal queue remove/push/enqueue call sites through that typed helper
  instead of the public raw `mi_theap_queue_first_update` shim.
- Preserved the existing `mi_theap_contains_queue` internal assertion and the
  `NormalPageQueue` check; this was a code-shape experiment, not a witness or
  validation weakening.
- Added a type-checked sentinel reference for the raw shim after it became
  internally unused, matching the existing raw-helper sentinel pattern in
  `src/page.rs`.

Type-system assessment:

- The attempted change kept the same typed heap and queue wrappers at internal
  call sites.
- It did not expose raw queue mutation or bypass queue membership invariants.

Correctness checks:

- Initial build introduced one additional dead-code warning because the raw
  shim was no longer referenced.
- Fixed that by adding a real type-checked sentinel reference, not by allowing
  dead code.
- `cargo build --release --lib` then succeeded with the existing 264 warnings.

Focused 5-run benchmark:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test sh8bench rptest larson-sized glibc-thread -r=5
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-typed-queue-update-r5-20260621-215818.log
mimalloc-bench/out/bench/benchres-goal-typed-queue-update-r5-20260621-215818.csv
```

Negative time means the candidate is faster:

| Test | Mean | Median | Trim-1 | Paired Faster |
| --- | ---: | ---: | ---: | ---: |
| glibc-thread | rexp -2.04% | rexp -1.62% | rexp -1.93% | 5/5 |
| larsonN-sized | rexp -0.89% | rexp -0.33% | rexp -0.08% | 4/5 |
| rptestN | rexp -1.28% | rexp -0.13% | rexp -1.00% | 3/5 |
| sh8benchN | rexp -7.69% | rexp -7.69% | rexp -8.86% | 4/5 |
| xmalloc-testN | rexp -1.56% | rexp -3.20% | rexp -2.28% | 4/5 |

Broad 10-run guardrail:

```sh
../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test sh8bench sh6bench glibc-simple glibc-thread alloc-test mstress barnes cfrac espresso -r=10
```

Artifacts:

```text
mimalloc-bench/out/bench/bench-goal-typed-queue-update-guard-r10-20260621-220402.log
mimalloc-bench/out/bench/benchres-goal-typed-queue-update-guard-r10-20260621-220402.csv
```

Note: `cache-scratch` and `cache-thrash` were passed to the harness but treated
as unknown options by this `bench.sh` invocation, so they were not included in
this guardrail.

Negative time means the candidate is faster:

| Test | Mean | Median | Trim-1 | Trim-2 | Paired Faster |
| --- | ---: | ---: | ---: | ---: | ---: |
| alloc-test1 | rexp -0.99% | rexp -0.95% | rexp -1.01% | rexp -0.95% | 6/10 |
| alloc-testN | rexp +1.36% | rexp +0.46% | rexp +1.48% | rexp +0.69% | 2/10 |
| barnes | rexp -1.02% | rexp -0.70% | rexp -1.16% | rexp -0.85% | 8/10 |
| cfrac | rexp +0.40% | rexp -0.18% | rexp +0.27% | rexp +0.12% | 3/10 |
| espresso | rexp +0.07% | rexp +0.17% | rexp +0.04% | rexp +0.06% | 3/10 |
| glibc-simple | rexp -1.44% | rexp -1.09% | rexp -1.44% | rexp -1.33% | 8/10 |
| glibc-thread | rexp -1.09% | rexp -0.60% | rexp -1.16% | rexp -0.95% | 10/10 |
| larsonN-sized | rexp -1.11% | rexp -0.81% | rexp -0.82% | rexp -0.87% | 6/10 |
| mstressN | rexp -0.12% | tied | rexp -0.08% | tied | 4/10 |
| redis | rexp +0.31% | rexp +1.14% | rexp +0.58% | rexp +0.76% | 3/10 |
| rocksdb | rexp -0.03% | rexp -0.28% | rexp -0.07% | rexp -0.19% | 4/10 |
| rptestN | rexp -1.20% | rexp -1.41% | rexp -1.20% | rexp -1.51% | 7/10 |
| sh6benchN | rexp -24.11% | rexp -28.57% | rexp -24.78% | rexp -25.00% | 10/10 |
| sh8benchN | rexp +3.75% | rexp +4.17% | rexp +3.65% | rexp +3.47% | 0/10 |
| xmalloc-testN | rexp -0.69% | rexp -0.69% | rexp -0.80% | rexp -0.92% | 7/10 |

Decision:

- Rejected and reverted.
- The focused benchmark was encouraging, and the broad guardrail confirmed
  improvements in `glibc-thread`, `larsonN-sized`, `rptestN`, `xmalloc-testN`,
  `sh6benchN`, and several non-target scenarios.
- The change still fails the goal because `sh8benchN` regressed in all 10
  paired guardrail runs, `alloc-testN` regressed after trimming, and Redis
  moved slower on median/trimmed means.
- Do not retry direct typed queue-first-update routing as a broad change unless
  a future profile isolates why `sh8benchN` loses. A narrower version would need
  to preserve the `glibc-thread`/`rptestN` wins while avoiding normal push/remove
  paths used by `sh8benchN`.

### Candidate: Fast Malloc/Free Entry Shape

Change tried:

- No source change was made.
- Audited the current release fast entry shape before attempting any patch,
  because prior work already tuned this area with layout padding and
  assembly-level tests.

Findings:

- `src/alloc_override.rs` already provides release `mi_c_malloc_fast` assembly
  entry points for the hot allocation/free override surface:
  - `malloc`, `mi_malloc`, and `__libc_malloc` share the direct-TLS small
    allocation fast path.
  - `calloc`, `mi_calloc`, and `__libc_calloc` share the direct-TLS small
    zero-allocation fast path.
  - C++ `operator new`/array-new use the same direct-TLS allocation shape.
  - `free`, `mi_free`, `__libc_free`, unsized C++ delete, and unsized array-delete
    share one assembly free fast path.
  - sized delete and aligned sized delete forward through the C-shaped
    `mi_free`/`mi_free_size_aligned` stubs.
- `src/free.rs` already defines the release forwarding wrappers
  `mi_free_size -> mi_free`, `mi_free_size_aligned -> mi_free_size`, and
  `mi_free_aligned -> mi_free`.
- The release artifact confirmed the expected public symbol grouping:
  `malloc`/`mi_malloc`, `calloc`/`mi_calloc`, `free`/`mi_free`, separate
  free-size/aligned stubs, and direct-TLS `mi_malloc_aligned`.
- `tests/release_fast_path_audit.rs` already guards this shape, including
  direct TLS, C-style free/delete aliases, forwarding stubs, entry `endbr64`,
  and layout-sensitive padding.

Type-system assessment:

- The existing assembly is intentionally limited to ABI override entry points
  where C signatures are required. Internal allocator invariants still cross
  through typed page/free-list/layout witnesses after the entry fast path.
- A new edit here would likely be either layout padding churn or broader raw
  ABI surface, not a typed-preserving improvement. That would not satisfy the
  goal without fresh profile evidence.

Decision:

- Audited and left unchanged.
- Do not retry malloc/free entry reshaping unless a new profile shows a concrete
  remaining entry wrapper sample. The current known fast entry mismatches have
  already been addressed and are covered by release audit tests.

### Candidate: Aligned Allocation Path

Change tried:

- No source change was made.
- Audited the aligned allocation path and the benchmark call surface instead of
  repeating the earlier usable-size/aligned pass-through experiments.

Findings:

- `rptest` calls `memalign` on Linux. That path routes through
  `mi_memalign -> mi_malloc_aligned`.
- The current release `mi_malloc_aligned` is already a naked direct-TLS wrapper
  that tail-jumps to `mi_theap_malloc_aligned`, matching the prior C-shape fix.
- `src/alloc_aligned.rs` keeps the important internal invariants typed:
  `LayoutRequest`, `Alignment`, `AlignmentOffset`, `ThreadHeapLocal`,
  `PaddedSize`, `ActivePage`, and `AvailablePage` are still used on the hot
  aligned allocation path.
- The aligned page allocation path already uses `_mi_page_malloc_zero_typed`
  and the release `_mi_padding_shrink` helper boundary is intentionally kept to
  match C.
- Perf report searches from the full sweep did not show new hot samples in
  less-common aligned variants such as `mi_zalloc_aligned` or
  `mi_calloc_aligned`. Adding direct-TLS wrappers there would be speculative and
  unlikely to affect the measured mimalloc-bench scenarios.

Type-system assessment:

- The only obvious way to shorten the aligned path further would be to bypass or
  duplicate typed layout/alignment checks. That would weaken the typed wiring or
  increase divergence from the audited path.
- The existing hot path already has the typed-preserving C-shape wrapper that
  was previously measured and accepted.

Decision:

- Audited and left unchanged.
- Do not retry aligned wrapper reshaping unless a new profile shows a specific
  aligned symbol as hot after `mi_malloc_aligned`'s direct-TLS wrapper. Future
  candidates should preserve `LayoutRequest`, `Alignment`, `AlignmentOffset`,
  and the page/free-list witnesses rather than introducing raw shortcuts.

### Guardrail: Direct Page-Cache Typed Boundary

Change kept:

- Restored the page queue direct-bin update boundary to typed wrappers without
  changing the generated release code:
  - `mi_theap_queue_first_update_typed` now derives the direct cache slot with
    `mi_direct_bin_from_size(size)` and carries it as `DirectBin`.
  - Queue bin comparisons now route through `BlockSize` and `Bin` instead of
    open-coded raw `mi_bin_impl(...)` calls at this boundary.
  - The direct page-cache fill now goes through
    `ThreadHeapPtr::fill_direct_pages(DirectBin, DirectBin, Option<PagePtr>)`.
- Updated release audit tests to match current source shape while preserving
  the stricter requirements:
  - direct cache fill must use `DirectBin`, `PagePtr`, and `ThreadHeapPtr`;
  - free validation must continue returning `PointerLookup` and `BlockForFree`;
  - realloc must build `OldAllocation` only after validation;
  - the fast entry shape remains direct-TLS and ABI-only.

Type-system assessment:

- This change does not weaken any witness wiring. The raw pointer slice fill is
  now encapsulated behind `ThreadHeapPtr::fill_direct_pages`, and callers must
  provide typed direct-bin bounds plus an optional typed page pointer.
- The boundary remains `unsafe` because it writes thread-heap metadata, but the
  public surface is narrower than the raw open-coded form it replaces.

Verification:

- `cargo fmt --check` passed.
- `git diff --check` passed.
- `cargo clippy --lib --features unstable-raw-internals -- -D clippy::all`
  passed.
- `cargo test --quiet --test release_fast_path_audit --features unstable-raw-internals`
  passed: 36 passed, 0 failed.
- `cargo build --release --lib` passed. It still reports the repository's
  existing translated dead-code-style rustc warnings; no lint suppression was
  added for this change.

10-run benchmark guardrail:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test sh8bench glibc-thread alloc-test -r=10`
- Allocators used: only `rbase` and `rexp`.
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal-typed-boundary-r10-20260621-225423.csv`
- Baseline and candidate libraries were byte-identical after the typed wrapper
  change:
  `43e8cca6dcf8ae9352556266c7081f7bfbda9485044f008d20f993dc4216c5db`
  for both `libmimalloc-rust-baseline.so` and `libmimalloc-rust.so`.
- The disassembly of `mechanical_mimalloc::page::mi_theap_queue_first_update`
  was identical. Any benchmark deltas below are therefore run noise, not
  generated-code differences from this source change.

| scenario | n | base median | candidate median | median delta | base mean | candidate mean | mean delta | candidate faster runs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| redis | 10/10 | 2.680s | 2.670s | -0.37% | 2.678s | 2.696s | +0.66% | 5/10 |
| rocksdb | 10/10 | 3.580s | 3.595s | +0.42% | 3.588s | 3.584s | -0.11% | 4/10 |
| larsonN-sized | 10/10 | 2.050s | 2.077s | +1.34% | 2.067s | 2.088s | +1.02% | 3/10 |
| rptestN | 10/10 | 1.515s | 1.562s | +3.07% | 1.499s | 1.546s | +3.19% | 4/10 |
| xmalloc-testN | 10/10 | 0.219s | 0.211s | -3.42% | 0.219s | 0.212s | -2.97% | 10/10 |
| sh8benchN | 10/10 | 0.240s | 0.240s | +0.00% | 0.244s | 0.237s | -2.87% | 5/10 |
| glibc-thread | 10/10 | 0.420s | 0.420s | +0.00% | 0.421s | 0.421s | +0.12% | 4/10 |
| alloc-test1 | 10/10 | 2.095s | 2.085s | -0.48% | 2.102s | 2.089s | -0.62% | 8/10 |
| alloc-testN | 10/10 | 2.190s | 2.205s | +0.68% | 2.225s | 2.226s | +0.04% | 2/10 |

Decision:

- Kept the typed boundary/audit fix because it restores the intended type
  wiring and compiles to the same release library as the saved baseline.
- No performance optimization candidate from this pass was accepted. The
  rejected candidates remain documented above so the same broad changes are not
  retried without new profile evidence.

## 2026-06-22 Follow-Up Goal: RSS/CPU Candidate Experiments

Goal:

- Evaluate the next performance candidates for RSS and CPU:
  free-list refill/page extension, cross-thread free and reclaim, branch
  layout/cold splitting, narrow queue-search/cache update, and Redis
  realloc/usable-size leftovers.
- Use appropriate diagnostics before changing source: `perf`, flamegraph tools,
  heap/RSS tools where useful, and generated assembly inspection.
- Keep only typed-preserving changes. Do not loosen validation, typestate,
  witness, or atomic wrapper wiring.
- Keep `cargo fmt`, clippy, targeted tests, and benchmark guardrails happy.
- If source or assembly becomes simpler without regressions, weigh the
  maintenance and type-safety tradeoff before deciding whether to keep it.

Tooling available on this host:

- `perf`
- `inferno-flamegraph`
- `heaptrack`
- `AMDuProfPcm`
- `AMDuProfCLI`
- `objdump` / `llvm-objdump`
- `rustfilt`

Baseline setup:

- Current tracked tree at the start of this goal includes the typed direct
  page-cache boundary/audit fix from the previous goal.
- The existing `mimalloc-bench/rust-baseline-vs-candidate.allocators` descriptor
  compares only `rbase` and `rexp`.

Baseline/profiling notes:

- Saved the current release allocator as both baseline and candidate before
  starting this follow-up pass:
  - `target/bench-allocators/libmimalloc-rust-baseline.so`
  - `target/bench-allocators/libmimalloc-rust.so`
- Both copies initially hashed to
  `43e8cca6dcf8ae9352556266c7081f7bfbda9485044f008d20f993dc4216c5db`.
- `perf stat -d -r 3` was run on the Rust baseline for the high-signal
  scenarios before source changes:

| scenario | elapsed | IPC | branch miss rate | L1d load miss rate | notes |
| --- | ---: | ---: | ---: | ---: | --- |
| xmalloc-testN | 5.025s | 0.91 | 0.18% | 4.03% | Hot allocator self-time includes collect/refill/reabandon. |
| glibc-thread | 2.026s | 1.77 | 5.01% | 7.43% | Branch-heavy free/delete and malloc entry paths dominate allocator time. |
| rptestN | 16.023s | 0.15 | 9.68% | 16.76% | Mostly workload `memset`; allocator symbols are not the main cost. |

`perf record` / flamegraph artifacts:

- `mimalloc-bench/out/bench/perf-goal2-xmalloc-rust-20260622.data`
- `mimalloc-bench/out/bench/perf-goal2-glibc-thread-rust-20260622.data`
- `mimalloc-bench/out/bench/perf-goal2-rptest-rust-20260622.data`
- Matching `.svg` flamegraphs and `perf-goal2-report-*.txt` reports are in
  the same directory.

Top-symbol findings:

- `xmalloc-testN` flat/self allocator time:
  - `mechanical_mimalloc::free::mi_free_try_collect_mt_typed`: 15.97%
  - `mechanical_mimalloc::page::mi_page_free_list_extend`: 7.18%
  - `mechanical_mimalloc::page::_mi_page_free_collect_partly`: 6.71%
  - `mechanical_mimalloc::arena::_mi_arenas_page_try_reabandon_to_mapped`: 4.30%
  - `mechanical_mimalloc::free::mi_abandoned_page_try_reclaim`: 2.59%
- `glibc-thread`:
  - benchmark loop dominates total runtime, but allocator self-time is mainly
    `mi_malloc`, `operator delete[]`, and `_mi_malloc_generic_typed`.
  - branch miss rate is materially higher than `xmalloc-testN`, so entry and
    rare-path layout remain worth testing.
- `rptestN`:
  - `__memset_avx2_unaligned_erms` accounts for most sampled time. Allocator
    source changes are unlikely to move this scenario unless a later profile
    shows a specific allocator symbol.

Assembly findings before prototypes:

- Rust and C `mi_page_free_list_extend` generate similarly compact loops. The
  source is not an obvious performance target unless a change makes the source
  simpler without changing codegen.
- C has a separate `mi_abandoned_page_try_reclaim.cold` symbol. The Rust build
  has `mi_abandoned_page_try_reclaim` as one ordinary function, called from the
  already-cold `mi_free_try_collect_mt_typed` path.
- Rust `mi_malloc`/`__libc_malloc` entry code is already close to the C entry
  shape, so the next low-risk candidate is cold-path function layout rather
  than bypassing typed validation or direct TLS wrappers.

Tooling decision:

- `perf`, `inferno-collapse-perf`, `inferno-flamegraph`, `objdump`,
  `llvm-objdump`, and `rustfilt` are suitable for this pass.
- `heaptrack` also interposes allocation, so it is not a clean diagnostic for
  the current `LD_PRELOAD` allocator comparisons. It should only be used on a
  separate non-interposed target or for qualitative investigation where the
  allocator-under-test result is not treated as benchmark evidence.

Prototype candidate queue:

1. Try a narrow cold/inline split for abandoned-page reclaim, matching the C
   layout signal while preserving `OwnedAbandonedPage` and
   `PageReclaimOnFree`.
2. If useful or codegen-simplifying, try the same for arena reabandon-to-mapped.
3. Re-check queue direct-cache update and free-list refill only if assembly or
   profiles show a concrete generated-code simplification opportunity.
4. Re-profile Redis/rocksdb/larson only after a candidate survives focused
   allocator benchmarks, so we do not chase noise.

### Candidate: Cold-Split Abandoned Reclaim

Prototype:

- Temporarily added `#[cold] #[inline(never)]` to
  `mi_abandoned_page_try_reclaim`.
- Type safety impact: none; it preserved `OwnedAbandonedPage` and
  `PageReclaimOnFree` exactly.

Assembly result:

- The reclaim body itself stayed the same.
- `mi_free_try_collect_mt_typed` moved the reclaim call behind the common
  returns and became slightly tighter in the normal path.

Focused 5-run benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test glibc-thread alloc-test -r=5`
- Allocators used: only `rbase` and `rexp`.
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal2-cold-reclaim-r5-20260622-081328.csv`

| scenario | n | base median | cand median | median delta | base mean | cand mean | mean delta | cand faster | RSS delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alloc-test1 | 5/5 | 2.100s | 2.100s | +0.00% | 2.108s | 2.104s | -0.19% | 2/5 | +0.14% |
| alloc-testN | 5/5 | 2.180s | 2.200s | +0.92% | 2.208s | 2.252s | +1.99% | 0/5 | -0.13% |
| glibc-thread | 5/5 | 0.418s | 0.417s | -0.24% | 0.419s | 0.420s | +0.33% | 2/5 | +0.09% |
| larsonN-sized | 5/5 | 2.102s | 2.093s | -0.43% | 2.096s | 2.092s | -0.19% | 2/5 | +0.20% |
| redis | 5/5 | 2.660s | 2.660s | +0.00% | 2.692s | 2.668s | -0.88% | 3/5 | +0.82% |
| rocksdb | 5/5 | 3.580s | 3.610s | +0.84% | 3.576s | 3.594s | +0.50% | 1/5 | -0.21% |
| rptestN | 5/5 | 1.538s | 1.490s | -3.12% | 1.499s | 1.490s | -0.64% | 2/5 | -0.88% |
| xmalloc-testN | 5/5 | 0.218s | 0.219s | +0.46% | 0.216s | 0.218s | +0.74% | 1/5 | -0.21% |

Decision:

- Rejected and reverted.
- Reason: modest source/assembly layout improvement did not translate into a
  broad win. `xmalloc-testN`, `rocksdb`, and `alloc-testN` regressed in the
  aggregate, and there was no RSS improvement.

### Candidate: Cold-Split Reabandon-To-Mapped Wrapper

Prototype:

- Temporarily added `#[cold] #[inline(never)]` to
  `mi_abandoned_page_try_reabandon_to_mapped`.
- Type safety impact: none; it preserved the `OwnedAbandonedPage` boundary.

Assembly result:

- `mi_free_try_collect_mt_typed` became much smaller because duplicated inline
  reabandon checks moved behind one typed helper call.
- Tradeoff: workloads that actually hit reabandon pay another call and lose the
  inline branch checks.

Focused 3-run benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test glibc-thread alloc-test -r=3`
- Allocators used: only `rbase` and `rexp`.
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal2-cold-reabandon-r3-20260622-081845.csv`

| scenario | n | base median | cand median | median delta | base mean | cand mean | mean delta | cand faster | RSS delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alloc-test1 | 3/3 | 2.100s | 2.100s | +0.00% | 2.113s | 2.093s | -0.95% | 2/3 | -1.04% |
| alloc-testN | 3/3 | 2.180s | 2.200s | +0.92% | 2.187s | 2.197s | +0.46% | 1/3 | -0.96% |
| glibc-thread | 3/3 | 0.433s | 0.439s | +1.39% | 0.432s | 0.435s | +0.54% | 1/3 | +0.77% |
| larsonN-sized | 3/3 | 2.110s | 2.191s | +3.84% | 2.095s | 2.161s | +3.17% | 1/3 | +1.28% |
| redis | 3/3 | 2.700s | 2.700s | +0.00% | 2.806s | 2.720s | -3.08% | 1/3 | +0.65% |
| rocksdb | 3/3 | 3.570s | 3.570s | +0.00% | 3.643s | 3.590s | -1.46% | 2/3 | -1.46% |
| rptestN | 3/3 | 1.515s | 1.573s | +3.83% | 1.547s | 1.567s | +1.34% | 1/3 | +0.29% |
| xmalloc-testN | 3/3 | 0.213s | 0.217s | +1.88% | 0.215s | 0.216s | +0.78% | 1/3 | -0.63% |

Decision:

- Rejected and reverted.
- Reason: although the caller assembly was cleaner, the benchmark filter showed
  clear regressions in `larsonN-sized`, `rptestN`, `xmalloc-testN`,
  `glibc-thread`, and `alloc-testN`.

### Candidate: Count-Based Free-List Refill Loop

Prototype:

- Temporarily rewrote `mi_page_free_list_extend_typed` to link `1..extend` and
  set the final block once, instead of looping through the last block and then
  overwriting its `next`.
- Type safety impact: none; it kept `PagePtr`, `BlockSize`, and `BlockPtr`
  wiring.

Assembly result:

- Rejected before benchmarking.
- The original pointer loop compiles to the compact C-shaped sequence with no
  callee-saved register saves.
- The count-based loop caused LLVM to generate a larger unrolled function with
  multiple callee-saved register pushes. Since `mi_page_free_list_extend` is
  hot in `xmalloc-testN`, this is a bad tradeoff despite simpler source.

Decision:

- Rejected and reverted.
- Do not retry count-based refill loops unless LLVM codegen changes or a future
  profile shows a different bottleneck inside the refill loop.

### Queue Search / Direct Cache Decision

Findings:

- `mi_theap_queue_first_update_typed` was already covered by the previous typed
  direct-cache boundary work and compiled to the same release library as the
  saved baseline.
- `mi_page_queue_find_free_ex_typed` keeps the same tail-recursive retry shape
  as C. The Rust optimizer already turns the retry into an in-function jump
  rather than an extra call frame.

Decision:

- No source change accepted.
- A manual loop rewrite would be speculative and likely increase the size of an
  already-large allocator fallback. Revisit only with a profile that points to
  the retry itself, not the broader queue search.

### Redis / RocksDB / Heap Diagnostics Decision

Findings:

- The rejected candidates were all run with Redis and RocksDB included in the
  allocator set, so these scenarios were not skipped.
- Redis results remained noisy and did not identify a new source-level
  candidate after the previous realloc/usable-size work.
- `heaptrack` is not appropriate as benchmark evidence for these
  `LD_PRELOAD` allocator comparisons because it interposes allocation itself.

Decision:

- No new Redis/RocksDB-specific source change accepted in this pass.
- Future Redis/RocksDB work should profile the server process directly, but the
  candidate must still be confirmed with normal non-interposed benchmark runs.

### Final Verification For This Follow-Up Goal

Final source state:

- No performance prototype from this follow-up goal was kept.
- The final release allocator copied back to
  `target/bench-allocators/libmimalloc-rust.so` is byte-identical to the saved
  baseline:
  `43e8cca6dcf8ae9352556266c7081f7bfbda9485044f008d20f993dc4216c5db`.
- The existing typed direct page-cache boundary/audit work from the previous
  goal remains in the tree.

Verification:

- `cargo fmt --check` passed.
- `git diff --check` passed.
- `cargo clippy --lib --features unstable-raw-internals -- -D clippy::all`
  passed.
- `cargo test --quiet --test release_fast_path_audit --features unstable-raw-internals`
  passed: 36 passed, 0 failed.
- `cargo test --quiet --features unstable-raw-internals` passed across the
  broader suite. Some tests intentionally print allocator error/assertion
  messages while validating failure behavior; the test result was still clean.

Outcome:

- The tried candidates produced either noisy/no real wins or direct regressions.
- Keeping the final source unchanged from the previous accepted typed-boundary
  state is the correct tradeoff for now: no type-system weakening, no benchmark
  regression accepted, and no speculative source cleanup that worsens assembly.

## 2026-06-22 Follow-Up Goal: Broad RSS/CPU Tuning

Goal:

- Improve mimalloc-bench performance by lowering RSS and/or reducing CPU work.
- Keep typed witnesses, validation, typestate, and atomic-wrapper assertions
  intact. No raw shortcut is acceptable unless the existing local invariants
  already prove it and the public/internal typed boundary remains stronger or
  equivalent.
- Do not accept source changes that materially regress other mimalloc-bench
  scenarios.
- Favor a neutral wall-time candidate only if it clearly simplifies source
  and/or lowers generated instruction count without weakening invariants.
- Use runtime tools before editing source: `perf`, Inferno flamegraphs,
  `heaptrack` where allocator interposition does not invalidate evidence,
  AMDuProf tooling where useful, and assembly inspection.
- Keep `cargo fmt`, targeted tests, clippy, and broader tests passing before
  each commit. Commit and push accepted batches after checks pass.

Starting point:

- Baseline commit pushed before this goal: `ab8568f`.
- Current accepted source state includes the typed direct page-cache boundary
  and audit update from the previous goal.
- Rebuilt the release library and copied it to both:
  - `target/bench-allocators/libmimalloc-rust.so`
  - `target/bench-allocators/libmimalloc-rust-baseline.so`
- Baseline, candidate, and release library hash:
  `43e8cca6dcf8ae9352556266c7081f7bfbda9485044f008d20f993dc4216c5db`.

Available tooling:

- `perf`
- `inferno-collapse-perf`
- `inferno-flamegraph`
- `heaptrack`
- `AMDuProfCLI`
- `AMDuProfPcm`
- `llvm-objdump`
- `rustfilt`

Initial focus:

1. Re-profile residual watch items from the previous pass:
   `xmalloc-testN`, `glibc-thread`, and related free/collect/refill paths.
2. Use focused direct counters and profiles before choosing any source edit.
3. Treat Redis, larson-sized, and RocksDB as guardrails unless fresh profiles
   identify a new concrete hot-path mismatch.

### Candidate 1: Fast MemKind Guard In Reabandon Probe

Profile setup:

- Direct `perf stat -r 10 -d` on:
  - `env LD_PRELOAD=.../libmimalloc-rust-baseline.so ./xmalloc-test -w 32 -t 5 -s 64`
  - `env LD_PRELOAD=.../libmimalloc-rust.so ./xmalloc-test -w 32 -t 5 -s 64`
  - `env LD_PRELOAD=.../libmimalloc-rust-baseline.so ./glibc-thread 32`
  - `env LD_PRELOAD=.../libmimalloc-rust.so ./glibc-thread 32`
- `perf record -F 999 -g` plus flat reports for C and Rust `xmalloc-testN`.

Baseline profile observations:

- `xmalloc-testN` direct Rust vs C remained wall-time tied, but Rust showed a
  meaningful instruction/control-flow gap:
  - Rust: 544.18B instructions, 137.85B branches, 624.46B cycles.
  - C: 500.94B instructions, 123.73B branches, 594.67B cycles.
- `glibc-thread` was already effectively parity:
  - Rust: 416.60B instructions, 52.51B branches, 240.72B cycles.
  - C: 415.64B instructions, 52.44B branches, 239.75B cycles.
- Flat `xmalloc-testN` Rust self time pointed at
  `mi_free_try_collect_mt_typed` and the free/collect/reabandon path.

Source/codegen rationale:

- The Rust caller guard in `mi_abandoned_page_try_reabandon_to_mapped` used
  `MemId::view((*page).memid)` and matched `MemIdView::Arena(_)`.
- Upstream C uses a cheap caller prefilter:
  `page->memid.memkind != MI_MEM_ARENA || mi_page_is_abandoned_mapped(page)`.
- The Rust arena helper still validates the typed arena payload before mutating
  arena state via `_mi_arenas_page_try_reabandon_to_mapped_internal`, so the
  prototype only changed the hot caller prefilter and did not remove the final
  typed guard.

Prototype:

```rust
if (*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA
    || mi_page_is_abandoned_mapped(page)
{
    return false;
}
```

Assembly effect:

- `.text` shrank by 96 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,502 bytes
- `mi_free_try_collect_mt_typed` no longer emitted the caller-side arena pointer
  null check and slice-range validation sequence before the reabandon call.
- Candidate hash: `3fc36ad8f012d3c43f65ef491c962fa0fef630c15c3e93b7a9927b5123076a6a`.

Focused counter results:

| Scenario | Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: | ---: |
| `xmalloc-testN` | elapsed | 5.012289s | 5.011590s | tied |
| `xmalloc-testN` | instructions | 561.53B | 529.75B | -5.66% |
| `xmalloc-testN` | branches | 142.06B | 134.16B | -5.56% |
| `xmalloc-testN` | cycles | 627.92B | 616.54B | -1.81% |
| `glibc-thread` | elapsed | 2.003218s | 2.002886s | tied |
| `glibc-thread` | instructions | 413.96B | 417.33B | +0.82% |
| `glibc-thread` | branches | 52.19B | 52.61B | +0.80% |
| `glibc-thread` | cycles | 240.23B | 242.10B | +0.78% |

Guardrail benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test glibc-thread alloc-test -r=5`
- Artifacts:
  - `mimalloc-bench/out/bench/bench-goal4-memkind-guard-r5-20260622.log`
  - `mimalloc-bench/out/bench/benchres-goal4-memkind-guard-r5-20260622.csv`

| Scenario | Time mean | Time median | Time trim-1 | Paired wins | RSS mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `alloc-test1` | -1.14% | -0.48% | -0.95% | 5/5 | +0.06% |
| `alloc-testN` | +0.18% | +0.45% | +0.45% | 1/5 | -2.04% |
| `glibc-thread` | +0.42% | +0.24% | +0.00% | 2/5 | +0.04% |
| `larsonN-sized` | -0.32% | -1.16% | -0.95% | 3/5 | -0.42% |
| `redis` | +0.89% | +0.00% | +0.74% | 2/5 | +0.32% |
| `rocksdb` | -0.45% | +0.00% | -0.19% | 2/5 | +0.57% |
| `rptestN` | -0.91% | -4.19% | -3.58% | 3/5 | +0.74% |
| `xmalloc-testN` | +1.51% | +3.32% | +2.05% | 1/5 | -1.64% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifacts:
  - `mimalloc-bench/out/bench/bench-goal4-memkind-xmalloc-r20-20260622.log`
  - `mimalloc-bench/out/bench/benchres-goal4-memkind-xmalloc-r20-20260622.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | +0.54% |
| Median time | +1.40% |
| Trim-1 time | +0.57% |
| Paired wins | 7/20 |
| Mean RSS | -1.53% |
| Mean user time | -1.06% |

Decision:

- Rejected and reverted.
- The instruction-count and RSS improvements are real, and the source is simpler,
  but the target `bench.sh` `xmalloc-testN` relative time regressed in the
  stronger 20-run validation.
- This does not satisfy the "do not regress other scenarios" constraint, and the
  final typed caller guard remains unchanged.

### Tooling Notes: AMDuProf And Heaptrack

AMDuProf:

- `AMDuProfCLI collect --config tbp` works in this environment.
- Collection command:
  `AMDuProfCLI collect --config tbp -o amduprof-goal4-xmalloc-rust-tbp /usr/bin/env LD_PRELOAD=.../libmimalloc-rust.so ./xmalloc-test -w 32 -t 5 -s 64`
- Report artifact:
  `mimalloc-bench/out/bench/amduprof-goal4-xmalloc-rust-tbp/AMDuProf-env-TBP_Jun-22-2026_16-52-36/report.csv`
- The report warned about kallsyms mismatch, but the user-space hotspot summary
  was still useful.
- AMDuProf agreed with `perf` on the `xmalloc-testN` hot path:
  `mi_free`, `mi_free_try_collect_mt_typed`, `mi_page_free_list_extend`,
  `_mi_page_free_collect_partly`, `_mi_arenas_page_try_reabandon_to_mapped`,
  and `mi_abandoned_page_try_reclaim`.

Heaptrack:

- Short diagnostic command:
  `heaptrack --record-only -o heaptrack-goal4-xmalloc-rust.gz /usr/bin/env LD_PRELOAD=.../libmimalloc-rust.so ./xmalloc-test -w 32 -t 1 -s 64`
- Artifact:
  `mimalloc-bench/out/bench/heaptrack-goal4-xmalloc-rust.gz.zst`
- Result: heaptrack reported only 225 allocations, mostly libc locale/env setup.
  It does not observe the allocator-under-test traffic usefully when mimalloc is
  itself injected with `LD_PRELOAD`.
- Decision: do not use heaptrack output as benchmark evidence for the preloaded
  allocator scenarios; keep it only as a diagnostic limitation note.

### Candidate 2: Cold-Split Thread-Free Corruption Reports

Rationale:

- `_mi_page_free_collect_partly` is hot in `xmalloc-testN`.
- Its normal path included stack reservation and formatted error-message setup
  for two corruption branches that should be unreachable in valid benchmark
  execution.
- Prototype moved the two `_mi_error_message_fmt` calls into private
  `#[cold] #[inline(never)]` helpers while preserving the same corruption
  checks and messages.

Codegen effect:

- `.text` shrank by 128 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,470 bytes
- `_mi_page_free_collect_partly` no longer reserved stack space in the normal
  path and moved the two format setup paths behind calls to cold helpers.
- Candidate hash: `f510dbb34ad232b35f9fa85f46421f79e5401077111aaace849768a7243d029e`.

Focused `perf stat -r 10 -d` on `xmalloc-testN`:

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.013590s | 5.010392s | tied |
| task-clock | 147.91B | 145.11B | -1.90% |
| instructions | 552.72B | 543.46B | -1.67% |
| cycles | 624.15B | 609.18B | -2.40% |
| branches | 139.70B | 138.33B | -0.99% |
| branch misses | 200.02M | 220.20M | +10.09% |
| L1 loads | 157.85B | 155.99B | -1.18% |
| L1 load misses | 6.32B | 6.37B | +0.82% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifacts:
  - `mimalloc-bench/out/bench/bench-goal5-cold-error-xmalloc-r20-20260622.log`
  - `mimalloc-bench/out/bench/benchres-goal5-cold-error-xmalloc-r20-20260622.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | +0.77% |
| Median time | +1.18% |
| Trim-1 time | +0.80% |
| Paired wins | 6/20 |
| Mean RSS | -0.48% |
| Mean user time | -2.27% |
| Mean system time | +23.60% |

Decision:

- Rejected and reverted.
- The candidate reduced generated code size and retired work under `perf`, but
  the benchmark harness showed a repeatable relative-time regression and a large
  system-time increase.
- This reinforces the current rule for `xmalloc-testN`: instruction/RSS wins
  alone are not enough if `bench.sh` relative time moves the wrong way.

### Candidate 3: Defer Reclaim-On-Free Option Read

Rationale:

- `mi_free_try_collect_mt_typed` read
  `mi_option_page_reclaim_on_free` before checking whether the page became fully
  free.
- `_mi_option_get_fast` is a noinline fast-load boundary. If the page is freed
  immediately, the reclaim option is unused.
- Prototype moved the `PageReclaimOnFree` construction after
  `mi_abandoned_page_try_free(page)`, preserving the typed wrapper and all
  reclaim/type assertions.

Codegen effect:

- `.text` shrank by 16 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,582 bytes
- The immediate-free path skipped the option call.
- The non-empty path gained a `used == 0` branch before the option read, so this
  candidate depends on the frequency of the immediate-free exit.
- Candidate hash: `ae493abfc4b8924b786cc521c06736783b7c0e4b2c0f59f6985d2d3316208939`.

Focused `perf stat -r 5 -d` on `xmalloc-testN`:

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.010382s | 5.013120s | tied |
| task-clock | 147.85B | 147.85B | +0.00% |
| instructions | 549.72B | 549.65B | -0.01% |
| cycles | 625.30B | 620.73B | -0.73% |
| branches | 139.08B | 138.02B | -0.76% |
| branch misses | 197.66M | 196.71M | -0.48% |
| L1 loads | 156.88B | 158.46B | +1.00% |
| L1 load misses | 6.28B | 6.32B | +0.64% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifacts:
  - `mimalloc-bench/out/bench/bench-goal6-option-defer-xmalloc-r20-20260622.log`
  - `mimalloc-bench/out/bench/benchres-goal6-option-defer-xmalloc-r20-20260622.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | +0.84% |
| Median time | +1.40% |
| Trim-1 time | +0.91% |
| Paired wins | 7/20 |
| Mean RSS | +0.99% |
| Mean user time | -0.78% |
| Mean system time | +6.28% |

Decision:

- Rejected and reverted.
- The branch/cycle counter movement was not enough to offset the added
  non-empty-path branch in the benchmark harness.
- Since both time and RSS moved the wrong way in the dedicated run, this is not
  a neutral simplification candidate.

### Candidate 4: Defer Thread-Collect Used-Count Read

Rationale:

- `mi_page_thread_collect_to_local_typed` loaded the page `used` count before
  walking the detached remote-free list.
- `used` is only needed after the capacity corruption check. Moving the load to
  the second corruption check narrows the source-level live range and matches
  the C shape more closely.
- The change preserves the same `OwnedAbandonedPage`, `RemoteFreeList`,
  `LocalFreeList`, `PageCapacity`, and `PageUsed` typed wrappers and keeps both
  corruption checks unchanged.

Codegen/performance effect:

- No runtime performance claim.
- Full `.text` section was byte-identical between baseline and candidate:
  `6c0228c11e541361e13fc948441c6b604ce591a068c113f4d4961233a2abbbcd`.
- `mechanical_mimalloc::page::_mi_page_free_collect_partly` disassembly was
  unchanged.
- Candidate library hash:
  `2eb9c59071b38c2c8b3a81b904e09239f8aa350b379b04f198dbf95cee10ae67`.

Verification:

- `cargo fmt --check` passed.
- `cargo build --release --lib` passed with the existing translated dead-code
  warnings.
- `cargo clippy --lib --features unstable-raw-internals -- -D clippy::all`
  passed.
- `cargo test --quiet --test release_fast_path_audit --features unstable-raw-internals`
  passed: 36 passed, 0 failed.
- `cargo test --quiet --features unstable-raw-internals` passed across the
  broader suite. Some tests intentionally print allocator error/assertion
  messages while validating failure behavior.

Decision:

- Accepted as a neutral source cleanup, not as a measured RSS/CPU win.
- The source is slightly closer to the upstream C ordering, and the generated
  release `.text` is unchanged, so there is no expected mimalloc-bench
  regression.

### Candidate 5: Remove Cold Attribute From Collect-MT Helper

Rationale:

- `mi_free_try_collect_mt_typed` is the top Rust allocator hotspot in
  `xmalloc-testN`, but it was marked `#[cold]`.
- Upstream C marks this helper noinline but not cold.
- Prototype removed `#[cold]` from both the typed helper and the raw ABI wrapper,
  while keeping `#[inline(never)]`, the typed `OwnedAbandonedPage`/
  `RemoteFreeList` boundary, and all assertions/checks.

Codegen effect:

- `.text` grew by 32 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,630 bytes
- Candidate hash: `6b2c3024defd4fb590f612eef41ca9a3c33ca47a6afda599530b994e04568784`.

Focused `perf stat -r 5 -d`:

| Scenario | Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: | ---: |
| `xmalloc-testN` | elapsed | 5.019900s | 5.011215s | -0.17% |
| `xmalloc-testN` | instructions | 548.77B | 550.30B | +0.28% |
| `xmalloc-testN` | cycles | 622.29B | 620.15B | -0.34% |
| `xmalloc-testN` | branches | 138.88B | 139.07B | +0.13% |
| `glibc-thread` | elapsed | 2.003032s | 2.003051s | tied |
| `glibc-thread` | instructions | 420.20B | 411.99B | -1.95% |
| `glibc-thread` | cycles | 244.05B | 238.44B | -2.30% |
| `glibc-thread` | branches | 52.96B | 51.93B | -1.95% |

Guardrail benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test glibc-thread alloc-test -r=5`
- Artifacts:
  - `mimalloc-bench/out/bench/bench-goal7-no-cold-collect-mt-guard-r5-20260622.log`
  - `mimalloc-bench/out/bench/benchres-goal7-no-cold-collect-mt-guard-r5-20260622.csv`

| Scenario | Time mean | Time median | Time trim-1 | Paired wins | RSS mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `alloc-test1` | -0.38% | -0.48% | -0.48% | 2/5 | +0.21% |
| `alloc-testN` | +0.73% | +0.45% | +0.76% | 1/5 | +3.27% |
| `glibc-thread` | +0.14% | +0.24% | +0.16% | 1/5 | -1.73% |
| `larsonN-sized` | +1.13% | +1.97% | +1.37% | 1/5 | +0.08% |
| `redis` | +0.60% | +0.00% | +0.00% | 2/5 | -0.11% |
| `rocksdb` | +0.06% | -0.28% | -0.09% | 1/5 | -0.11% |
| `rptestN` | -1.82% | -5.24% | -2.55% | 4/5 | +0.59% |
| `xmalloc-testN` | -1.74% | -1.83% | -1.99% | 4/5 | +6.41% |

Decision:

- Rejected and reverted.
- The focused perf counters and target `xmalloc-testN`/`rptestN` harness rows
  were promising, but the candidate regressed `larsonN-sized`, `alloc-testN`,
  `glibc-thread`, and Redis time in the guardrail, and increased RSS on
  `xmalloc-testN` and `alloc-testN`.
- This does not satisfy the no-regression constraint.

### Candidate 6: Non-Null Partly Collect Typed Head

Rationale:

- `_mi_page_free_collect_partly` is a hot helper under `xmalloc-testN`.
- The raw ABI wrapper must keep accepting a nullable `head`, but the hot
  `mi_free_try_collect_mt_typed` caller has already proven a non-null
  `BlockPtr` through `RemoteFreeList::head()`.
- Prototype changed `_mi_page_free_collect_partly_typed` to take `BlockPtr`
  directly and routed the hot caller to the typed helper, while keeping the raw
  null-tolerant wrapper and preserving the `OwnedAbandonedPage`,
  `RemoteFreeList`, `LocalPage`, and `BlockPtr` witness checks.

Codegen effect:

- `.text` shrank by 48 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,550 bytes
- Candidate `.text` hash:
  `7cd0ca9321da3fd8bc804f4eaeceb2976db2b816dd152b075126c3bc03f374c2`.
- Candidate allocator hash:
  `e7606c232872546ece5762029d67b235aa733a3818e144bcaf73a2eb35b94088`.
- Tradeoff: `_mi_page_free_collect_partly` disappeared as a separate release
  symbol and its body was inlined into `mi_free_try_collect_mt_typed`, growing
  that helper from 399 to 631 bytes. The total `.text` still shrank because the
  raw wrapper and duplicate head handling were removed from the hot path.

Focused `perf stat -r 5 -d` on `xmalloc-testN`:

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.014630s | 5.008349s | -0.13% |
| task-clock | 149.03B | 140.70B | -5.59% |
| instructions | 560.94B | 515.97B | -8.02% |
| cycles | 631.39B | 596.32B | -5.56% |
| branches | 141.88B | 128.97B | -9.10% |
| branch misses | 196.25M | 257.57M | +31.24% |
| L1 loads | 160.28B | 144.51B | -9.84% |
| L1 load misses | 6.40B | 6.35B | -0.80% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal8-nonnull-partly-xmalloc-r20-20260622.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | +0.60% |
| Median time | -0.23% |
| Trim-1 time | +0.59% |
| Paired wins | 6/20 |
| Mean RSS | -13.04% |
| Mean user time | -4.63% |
| Mean system time | +55.45% |
| Mean page reclaims | -0.21% |

Decision:

- Rejected and reverted.
- The instruction, user-time, and RSS reductions are real enough to note, but
  the dedicated `xmalloc-testN` harness regressed mean and trimmed time and only
  won 6 of 20 paired runs.
- The large system-time increase is a bad tradeoff and likely explains why the
  counter win did not become a reliable wall-time win.
- No broad guardrail run was done because the target scenario already failed
  the no-regression bar.

### Candidate 7: Non-Null Partly Collect Typed Head, Out-of-Line

Rationale:

- Candidate 6 showed that removing the duplicate null-head handling could reduce
  instructions and RSS, but inlining the partial collect body into
  `mi_free_try_collect_mt_typed` grew the hot helper and failed target
  wall-time validation.
- Prototype kept the raw nullable ABI wrapper, changed the typed helper to take
  a non-null `BlockPtr`, routed the hot caller directly to that typed helper,
  and added `#[inline(never)]` to keep the partial-collect body out of
  `mi_free_try_collect_mt_typed`.
- The typed witness boundary was not weakened: the raw wrapper still checked
  null before constructing `BlockPtr`, and the hot caller already had a
  non-null `BlockPtr` from `RemoteFreeList::head()`.

Codegen effect:

- `.text` size was unchanged from the accepted baseline:
  - baseline: 190,598 bytes
  - candidate: 190,598 bytes
- Candidate `.text` hash:
  `4e60cc12005fed733a1fe312a1612747ac924a55d52bb8a8582c8d41b42208c0`.
- Candidate allocator hash:
  `d68328c88a9536ccf564ea3ae5e25eb00911a34c5ae1fbd03a21b3d8d3683836`.
- `mi_free_try_collect_mt_typed` stayed at 399 bytes instead of growing to the
  631-byte Candidate 6 shape.
- `_mi_page_free_collect_partly_typed` remained a separate 276-byte symbol.

Focused `perf stat -r 5 -d` on `xmalloc-testN`:

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.010800s | 5.011107s | +0.01% |
| task-clock | 147.10B | 148.90B | +1.23% |
| instructions | 541.55B | 560.76B | +3.55% |
| cycles | 623.28B | 626.59B | +0.53% |
| branches | 136.99B | 141.99B | +3.65% |
| branch misses | 205.61M | 194.59M | -5.36% |
| L1 loads | 154.71B | 160.18B | +3.53% |
| L1 load misses | 6.68B | 6.40B | -4.18% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal9-nonnull-partly-noinline-xmalloc-r20-20260622.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | +0.26% |
| Median time | +0.24% |
| Trim-1 time | +0.31% |
| Paired time wins | 9/20 |
| Mean RSS | +0.97% |
| Median RSS | +0.15% |
| Trim-1 RSS | +0.75% |
| Mean user time | -0.06% |
| Mean system time | +4.07% |
| Mean page reclaims | +2.63% |

Decision:

- Rejected and reverted.
- Keeping the typed helper out-of-line fixed Candidate 6's hot-helper size
  growth, but it also removed the instruction/RSS advantage that made the
  earlier prototype worth measuring.
- The target harness regressed mean, median, and trimmed time, and RSS also
  regressed, so this does not satisfy the performance or no-regression bar.
- No broad guardrail run was done because the target scenario already failed.

### Candidate 8: Source-Queue Typed First-Update Routing

Rationale:

- An earlier broad queue-first-update candidate improved several scenarios but
  regressed `sh8benchN`, `alloc-testN`, and Redis.
- Prototype tried the narrower version suggested by that result: route only
  source-queue first updates after removing a first page through a typed helper,
  while leaving push and target-enqueue first updates on the existing raw shim.
- The helper preserved the same `mi_theap_contains_queue` assertion, accepted
  `ThreadHeapLocal` and `PageQueuePtr` witnesses, and only called
  `mi_theap_queue_first_update_typed` after proving `NormalPageQueue`.

Codegen effect:

- `.text` shrank by 96 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,502 bytes
- Candidate `.text` hash:
  `7eed7de3c80555e46a9f386c00d7febc10e6a23e8cf6ad1f7bd57c97547d629e`.
- Candidate allocator hash:
  `10e4eca5a0ea1bb4359e7bb43ca606fdfbf27e36df6c7fb77ef636081bf8ae5c`.
- Hot free/collect sizes stayed stable:
  - `mi_free_try_collect_mt_typed`: 399 bytes
  - `mi_page_free_list_extend`: 90 bytes
  - `mi_theap_queue_first_update`: 464 bytes

Focused guardrail benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test sh8bench glibc-thread alloc-test -r=5`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal10-queue-source-update-r5-20260622.csv`

| Scenario | Time mean | Time median | Time trim-1 | Paired wins | RSS mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `alloc-test1` | -0.29% | +0.00% | -0.16% | 2/5 | +0.26% |
| `alloc-testN` | +0.09% | +0.45% | +0.15% | 1/5 | -1.87% |
| `glibc-thread` | -0.05% | +0.95% | +0.71% | 2/5 | -0.40% |
| `larsonN-sized` | +0.74% | +1.37% | +0.72% | 2/5 | -1.02% |
| `redis` | +1.49% | +0.00% | +0.73% | 2/5 | -0.42% |
| `rocksdb` | -0.74% | -0.85% | -0.95% | 3/5 | -0.52% |
| `rptestN` | -2.35% | -7.15% | -4.23% | 2/5 | -0.57% |
| `sh8benchN` | -5.38% | +4.17% | -3.90% | 2/5 | +1.19% |
| `xmalloc-testN` | +1.30% | +2.34% | +1.40% | 0/5 | -7.92% |

Decision:

- Rejected and reverted.
- The `.text` shrink and RSS reductions on several scenarios were not enough:
  `xmalloc-testN` regressed in every paired run, and the candidate also
  regressed `redis`, `larsonN-sized`, `alloc-testN`, and median/trimmed
  `glibc-thread` time.
- The narrower routing did not preserve the broad candidate's useful wins
  without the prior regressions, so this queue-update direction remains closed
  unless a future profile isolates a more specific call site.

### Candidate 9: Refill Last-Block Link Skip

Rationale:

- A fresh current-profile pass on `xmalloc-testN` still showed the same hot set:
  `mi_free_try_collect_mt_typed`, `mi_page_free_list_extend`,
  `_mi_page_free_collect_partly`, reabandon/reclaim helpers, and `mi_malloc`.
- The already-tried collect/reclaim/queue directions either regressed wall time
  or guardrails. The remaining concrete C/Rust mismatch was the hot refill
  helper: C's `mi_page_free_list_extend` was 74 bytes, while Rust's typed
  wrapper compiled to 90 bytes.
- Prototype changed the refill loop from `while block <= last` to
  `while block < last`, then set `last.next` once. This avoids writing the
  last block's next pointer to the one-past block and immediately overwriting
  it with the old free-list head.
- The typed boundary was unchanged: the path still used `PagePtr`,
  `BlockSize`, and `BlockPtr::set_next`; no raw free-list writes or assertion
  weakening were introduced.

Codegen effect:

- `.text` size was unchanged from the accepted baseline:
  - baseline: 190,598 bytes
  - candidate: 190,598 bytes
- Candidate `.text` hash:
  `e24d613e19c4a462ddd0bea9cbe19a8720b781c4b315ede4c65daff511b8c2f6`.
- Candidate allocator hash:
  `e7502f9daef17e64e09e8d2516a56fe69358c8ff60406d870d6c3d27c4efe427`.
- `mi_page_free_list_extend` remained a 90-byte symbol. The branch condition
  changed, but the generated function did not shrink.

Fresh current-profile artifacts:

- `mimalloc-bench/out/bench/perf-goal11-xmalloc-current-rust-20260622-210944.stat`
- `mimalloc-bench/out/bench/perf-goal11-xmalloc-current-rust-20260622-211026.data`
- Matching flat/children reports:
  `perf-goal11-xmalloc-current-rust-20260622-211026.flat.txt` and
  `perf-goal11-xmalloc-current-rust-20260622-211026.children.txt`.

Focused `perf stat -r 5 -d` on `xmalloc-testN`:

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.00968 | 5.01072 | +0.02% |
| task-clock | 147.22B | 146.19B | -0.70% |
| instructions | 545.03B | 535.13B | -1.82% |
| cycles | 628.36B | 624.79B | -0.57% |
| branches | 137.78B | 135.27B | -1.82% |
| branch misses | 205.84M | 212.81M | +3.39% |
| L1 loads | 155.64B | 152.44B | -2.06% |
| L1 load misses | 6.25B | 6.20B | -0.94% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal11-refill-last-skip-xmalloc-r20-20260622-211705.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | -0.60% |
| Median time | -1.82% |
| Trim-1 time | -0.64% |
| Paired wins | 11/20 |
| Mean RSS | +3.19% |
| Median RSS | +2.37% |
| Trim-1 RSS | +2.36% |
| Mean user time | -0.08% |
| Mean system time | +1.81% |
| Mean page reclaims | +0.32% |

Focused guardrail benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test sh8bench glibc-thread alloc-test -r=5`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal11-refill-last-skip-guard-r5-20260622-212458.csv`

| Scenario | Time mean | Time median | Time trim-1 | Paired wins | RSS mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `redis` | -0.59% | -1.49% | -1.00% | 3/5 | +0.41% |
| `rocksdb` | -0.46% | -0.29% | -0.38% | 4/5 | +0.20% |
| `larsonN-sized` | -0.81% | -0.10% | -0.51% | 3/5 | +0.09% |
| `rptestN` | +7.60% | +4.42% | +7.20% | 1/5 | +0.21% |
| `xmalloc-testN` | -2.47% | -3.64% | -3.63% | 4/5 | +4.28% |
| `sh8benchN` | +0.79% | +0.00% | +0.00% | 1/5 | +0.10% |
| `glibc-thread` | +0.38% | +0.00% | -0.08% | 1/5 | -1.79% |
| `alloc-test1` | +0.00% | +0.48% | +0.16% | 1/5 | -0.05% |
| `alloc-testN` | +0.64% | +0.92% | +0.76% | 0/5 | -1.30% |

Decision:

- Rejected and reverted.
- The target `xmalloc-testN` wall-time result was mildly positive and perf
  showed fewer retired instructions, but RSS regressed materially on the target
  run and guardrail.
- The guardrail failed the no-regression bar: `rptestN` regressed strongly,
  `alloc-testN` lost every paired run, and `sh8benchN`, `glibc-thread`, and
  `alloc-test1` also moved the wrong way on time aggregates.
- This refill-loop direction should not be retried without new codegen evidence
  that actually shrinks the hot refill helper or changes its generated loop
  shape materially.

### Candidate 10: Out-of-Line Reclaim Queue-Length Check

Rationale:

- A C/Rust code-size comparison showed Rust's
  `mi_abandoned_page_try_reclaim` at 0x1e4 bytes versus C's 0x17e-byte hot
  symbol plus a 0x17-byte cold fragment.
- The mismatch came partly from Rust inlining the bin/queue lookup through
  `mi_page_queue_len_is_atmost`, while C calls `_mi_bin` from the reclaim
  queue-limit path.
- Prototype added `#[inline(never)]` to `mi_page_queue_len_is_atmost` only.
  The function kept its existing `ThreadHeapLocal`, `BlockSize`, and
  `PageQueueCount` typed parameters and its existing queue/count assertions;
  no witness or assertion was weakened.

Codegen effect:

- `.text` size was unchanged from the accepted baseline:
  - baseline: 190,598 bytes
  - candidate: 190,598 bytes
- Candidate `.text` hash:
  `cc0658075c21ab44a3088e8eae6d12baa7a3de21bc90346140174d679a7235a3`.
- Candidate allocator hash:
  `290c90c5de31a1ac1aea7f1ceee145ff292daaefc7294526f36ebe3d79a264b6`.
- `mi_abandoned_page_try_reclaim` shrank from 0x1e4 bytes to 0x16d bytes.
- The new out-of-line `mi_page_queue_len_is_atmost` symbol was 0x71 bytes.
- The combined helper plus reclaim body was therefore 6 bytes smaller, but
  section size did not shrink due to alignment/layout effects.

Focused `perf stat -r 5 -d` on `xmalloc-testN`:

- Baseline artifact:
  `mimalloc-bench/out/bench/perf-goal12-reclaim-queue-noinline-baseline-r5-20260622-213150.stat`
- Candidate artifact:
  `mimalloc-bench/out/bench/perf-goal12-reclaim-queue-noinline-candidate-r5-20260622-213221.stat`

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.00977 | 5.01335 | +0.07% |
| task-clock | 148.34B | 150.54B | +1.48% |
| instructions | 547.35B | 562.62B | +2.79% |
| cycles | 632.92B | 635.73B | +0.44% |
| branches | 138.55B | 141.83B | +2.37% |
| branch misses | 198.76M | 182.85M | -8.00% |
| L1 loads | 156.23B | 160.52B | +2.74% |
| L1 load misses | 6.326B | 6.330B | +0.05% |

Decision:

- Rejected and reverted.
- Although the reclaim symbol became smaller, the extra call made the target
  CPU counters worse: retired instructions, task-clock, cycles, branches, and
  L1 loads all increased on the direct `xmalloc-testN` perf pass.
- The branch-miss reduction was not enough to offset the broader CPU cost, and
  the unchanged `.text` size meant there was no neutral code-size/simplification
  reason to keep the attribute.

### Candidate 11: Raw Static Error Messages In Partial Collect

Rationale:

- `_mi_page_free_collect_partly` is still a persistent `xmalloc-testN` hot
  helper.
- Its typed Rust body preserved C's two corruption checks in
  `mi_page_thread_collect_to_local_typed`, but reported the static corruption
  strings through `_mi_error_message_fmt(format_args!(...))`.
- C reports the same two static strings through `_mi_error_message`, avoiding
  per-call `fmt::Arguments` setup in the partial-collect symbol.
- Prototype changed only those two static messages to `_mi_error_message` with
  the same `EFAULT` errno. The corruption checks, returns, error handler path,
  and typed `RemoteFreeList`/`LocalFreeList` invariants were unchanged.

Codegen effect:

- `.text` shrank by 192 bytes:
  - baseline: 190,598 bytes
  - candidate: 190,406 bytes
- Candidate `.text` hash:
  `b9ad602db874ffd77c5c9e1a76693fbac46470d1f6c53c80275e7671e145e27c`.
- Candidate allocator hash:
  `d015a5d91266b51a3bb892f2b92d292e7eb25d55b897c785a8711ed07edf3a5c`.
- `_mi_page_free_collect_partly` shrank from 0x114 bytes to 0xde bytes.
- `mi_free_try_collect_mt_typed` stayed at 0x18f bytes.
- `mi_page_free_list_extend` stayed at 0x5a bytes.

Focused `perf stat -r 5 -d` on `xmalloc-testN`:

- Baseline artifact:
  `mimalloc-bench/out/bench/perf-goal13-static-error-baseline-r5-20260622-213818.stat`
- Candidate artifact:
  `mimalloc-bench/out/bench/perf-goal13-static-error-candidate-r5-20260622-213849.stat`

| Metric | Baseline | Candidate | Delta |
| --- | ---: | ---: | ---: |
| elapsed | 5.01481 | 5.01146 | -0.07% |
| task-clock | 148.09B | 147.90B | -0.13% |
| instructions | 550.27B | 543.70B | -1.19% |
| cycles | 630.73B | 626.15B | -0.73% |
| branches | 139.22B | 138.53B | -0.49% |
| branch misses | 201.66M | 200.45M | -0.60% |
| L1 loads | 157.20B | 156.72B | -0.31% |
| L1 load misses | 6.350B | 6.257B | -1.47% |

Dedicated `xmalloc-testN` validation:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp xmalloc-test -r=20`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal13-static-error-xmalloc-r20-20260622-214301.csv`

| Metric | Candidate delta |
| --- | ---: |
| Mean time | -0.34% |
| Median time | -0.23% |
| Trim-1 time | -0.38% |
| Paired wins | 10/20 |
| Mean RSS | +0.68% |
| Median RSS | +1.51% |
| Trim-1 RSS | +0.96% |
| Mean user time | +0.47% |
| Mean system time | -2.67% |
| Mean page reclaims | +4.04% |

Focused guardrail benchmark:

- Command:
  `../../bench.sh --external=../../rust-baseline-vs-candidate.allocators rbase rexp redis rocksdb larson-sized rptest xmalloc-test sh8bench glibc-thread alloc-test -r=5`
- Artifact:
  `mimalloc-bench/out/bench/benchres-goal13-static-error-guard-r5-20260622-215046.csv`

| Scenario | Time mean | Time median | Time trim-1 | Paired wins | RSS mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| `redis` | +1.36% | +0.76% | +0.50% | 1/5 | +0.56% |
| `rocksdb` | -0.28% | +0.29% | +0.29% | 2/5 | +4.65% |
| `larsonN-sized` | +1.64% | +1.13% | +1.21% | 1/5 | +1.14% |
| `rptestN` | -7.22% | -7.98% | -7.22% | 3/5 | +7.80% |
| `xmalloc-testN` | +0.65% | +0.00% | +1.09% | 1/5 | +0.97% |
| `sh8benchN` | +3.25% | +0.00% | +1.35% | 2/5 | -0.66% |
| `glibc-thread` | -0.05% | +0.00% | -0.08% | 1/5 | -0.48% |
| `alloc-test1` | -0.86% | -0.96% | -0.64% | 4/5 | +0.32% |
| `alloc-testN` | -0.45% | +0.00% | +0.30% | 2/5 | -3.19% |

Decision:

- Rejected and reverted.
- This was the strongest neutral-code-simplification candidate so far: it
  preserved the corruption checks and reduced both `.text` and direct
  `xmalloc-testN` CPU counters.
- The guardrail did not meet the no-regression bar. Redis, `larsonN-sized`,
  `xmalloc-testN`, and `sh8benchN` all regressed on mean/trimmed time, and
  RocksDB plus `rptestN` showed unacceptable RSS increases.
- The static-error-message cleanup should only be reconsidered if a future
  layout-preserving variant can keep the 192-byte shrink without moving the
  guardrail scenarios in the wrong direction.

## 2026-06-23 Continuation: State Recheck And Neutral Wrapper Cleanup

Resumed state:

- The tracked tree was clean at resume, with only pre-existing untracked
  scratch/benchmark files under the workspace.
- There is no tracked `WORKLOG.md`; this file remains the canonical performance
  worklog.
- The current accepted release allocator `.text` section is unchanged from the
  saved Rust baseline:
  - size: 190,598 bytes
  - `.text` hash:
    `6c0228c11e541361e13fc948441c6b604ce591a068c113f4d4961233a2abbbcd`
  - allocator hash before this source cleanup:
    `2eb9c59071b38c2c8b3a81b904e09239f8aa350b379b04f198dbf95cee10ae67`

Redis profile recheck:

- Re-read the existing direct Redis server-side `perf report` artifacts:
  - `mimalloc-bench/out/bench/perf-goal-report-redis-long-cmi.txt`
  - `mimalloc-bench/out/bench/perf-goal-report-redis-long-rmi.txt`
- The profile state still matches the earlier Redis conclusion:
  - libc/Redis `memmove`, ziplist, command parsing, and reply handling dominate
    the total profile.
  - Rust allocator samples are mainly `mi_malloc_usable_size`,
    `mi_theap_realloc_validated_core`, `mi_theap_realloc_layout`, `realloc`,
    and `mi_malloc`.
  - C allocator samples are flatter around `mi_theap_realloc`,
    `malloc_usable_size`, `mi_malloc`, and `operator delete[]`.
- The existing accepted Redis work already targeted that mismatch by keeping
  the typed `OldAllocation`/`LayoutRequest` boundary while deriving usable size
  from validated pointer-location data and flattening the private realloc
  layout/core boundary. There was no new Redis-specific source candidate from
  this re-read alone.

### Candidate 12: Nested Medium Reclaim Gate

Prototype:

- Changed the reclaim-on-free gate in `mi_free_try_collect_mt_typed` from one
  compound condition to a nested medium-size branch:

```rust
if (*page_raw).block_size <= MI_MEDIUM_MAX_OBJ_SIZE {
    if reclaim_on_free.is_enabled() && mi_abandoned_page_try_reclaim(page, reclaim_on_free) {
        return;
    }
}
```

Type-safety impact:

- None. The same `OwnedAbandonedPage` and `PageReclaimOnFree` typed values
  reached `mi_abandoned_page_try_reclaim`, and all existing reclaim assertions
  and checks stayed in place.

Codegen result:

- Rejected before runtime benchmarking.
- Release `.text` stayed byte-identical to the accepted baseline:
  `6c0228c11e541361e13fc948441c6b604ce591a068c113f4d4961233a2abbbcd`.
- Release allocator hash stayed
  `2eb9c59071b38c2c8b3a81b904e09239f8aa350b379b04f198dbf95cee10ae67`.
- Hot symbol sizes and `mi_free_try_collect_mt_typed` disassembly were
  unchanged.

Decision:

- Rejected and reverted.
- The source was not materially simpler, produced no instruction/code-size
  change, and therefore had no neutral simplification value.

### Candidate 13: Free-List Raw Head Accessors

Rationale:

- Several hot page collection paths repeatedly converted typed
  `AvailableFreeList` and `LocalFreeList` wrappers back to raw nullable
  `mi_block_t*` values by open-coding:
  `list.0.map(BlockPtr::as_ptr).unwrap_or(ptr::null_mut())`.
- The conversion is a boundary operation owned by the typed wrapper itself.
  Centralizing it clarifies that the raw pointer is only the storage format of
  `page.free` / `page.local_free`; it does not bypass or weaken the wrapper.

Change:

- Added `#[inline(always)] as_raw()` to:
  - `AvailableFreeList`
  - `LocalFreeList`
- Routed existing typed free/local-list store sites through those methods:
  - `PagePtr::set_free`
  - `PagePtr::set_local_free`
  - `mi_page_thread_collect_to_local_typed`
  - `mi_page_free_quick_collect_typed`
  - `_mi_page_free_collect`
- `LocalFreeList::push` now uses `self.as_raw()` for the previous local-list
  head.

Type-safety impact:

- Neutral to slightly positive for maintainability. The wrappers still expose
  typed `Option<BlockPtr>` internally, and the new method is only the explicit
  storage conversion for page metadata fields.
- No validation, witness construction, corruption check, typestate boundary, or
  atomic-wrapper assertion was removed or weakened.

Codegen result:

- Release build succeeded.
- Release `.text` stayed byte-identical to the accepted baseline:
  - size: 190,598 bytes
  - `.text` hash:
    `6c0228c11e541361e13fc948441c6b604ce591a068c113f4d4961233a2abbbcd`
- Hot symbol sizes stayed unchanged:
  - `mi_malloc`: 0x51
  - `mi_free`: 0x9e
  - `mi_free_size`: 0x9
  - `mi_free_try_collect_mt_typed`: 0x18f
  - `_mi_page_free_collect_partly`: 0x114
  - `_mi_page_free_collect`: 0x18b
  - `mi_page_free_list_extend`: 0x5a
  - `mi_page_queue_find_free_ex_typed`: 0x373
  - `_mi_malloc_generic_typed`: 0x4b4

Decision:

- Accepted as a neutral source cleanup, not as a measured RSS/CPU win.
- No mimalloc-bench runtime claim is made because the release `.text` is
  byte-identical. The maintenance win is that raw nullable free-list storage
  conversion now belongs to the typed free-list wrappers instead of being
  repeated at page call sites.

### Candidate 14: Collect-MT Cached Block Size

Rationale:

- `mi_free_try_collect_mt_typed` read `page->block_size` for the small-page
  collect branch and then read it again for the medium-page reclaim gate.
- A prototype cached the value in a local `block_size` after remote-free
  validation, before the small/large collect branch.
- The goal was to reduce repeated metadata loads in a hot free-side helper
  without changing the typed `OwnedAbandonedPage`, `RemoteFreeList`, or
  `PageReclaimOnFree` boundaries.

Type-safety impact:

- No witness construction, validation, corruption check, typestate boundary, or
  atomic-wrapper assertion was removed or weakened.
- The behavioral assumption was that the intervening collect paths do not
  mutate `page->block_size`; this matches the page metadata model, but the
  runtime result did not justify keeping the source change.

Codegen result:

- Release build succeeded.
- Release `.text` shrank from 190,598 bytes to 190,582 bytes.
- `mi_free_try_collect_mt_typed` shrank from 0x18f to 0x17d bytes.
- The generated code traded the second memory load for a callee-saved `r15`
  value in the function prologue/body.

Focused measurement:

- Direct `perf stat -d -r 5` on `xmalloc-test -w 32 -t 5 -s 64`:
  - elapsed: +0.04%
  - task-clock: -1.42%
  - instructions: -0.47%
  - cycles: -1.78%
  - branches: -2.85%
  - branch-misses: +8.35%
  - L1 load misses: +5.87%
- Dedicated `xmalloc-testN` r20 artifact:
  `mimalloc-bench/out/bench/benchres-goal14-block-size-local-xmalloc-r20-20260622-221436.csv`
  - elapsed: +0.28%
  - RSS: -3.25%
  - user time: -1.59%
  - sys time: +17.68%

Guardrail measurement:

- Guardrail r5 artifact:
  `mimalloc-bench/out/bench/benchres-goal14-block-size-local-guard-r5-20260622-222219.csv`
- Average candidate deltas versus the accepted baseline:
  - `xmalloc-testN`: elapsed -0.37%, RSS -2.35%, user -1.01%, sys +15.53%
  - `rptestN`: elapsed +5.85%, RSS -2.79%, user +9.44%, sys +3.20%
  - `sh8benchN`: elapsed +5.04%, RSS +0.41%, user +2.25%, sys +2.20%
  - `redis`: elapsed +0.73%, RSS +0.08%, user tied, sys +6.25%
  - `alloc-testN`: elapsed +0.55%, RSS +0.70%, user +0.53%, sys +21.05%

Decision:

- Rejected and reverted.
- The small xmalloc RSS/user-time signal and code-size reduction were not enough
  to offset broader guardrail regressions, especially `rptestN` and
  `sh8benchN`.
