# Larson Performance Regression Investigation

**Report timestamp:** 2026-08-08T22:51:26-04:00

**Investigation status:** Major regression commit confirmed; C++ and page-queue fixes validated; smaller residual gap remains open

**Initial benchmarked Rust source:** `8baba5eb6018ea8c56a47de6f1c20a7fa07f39a4`

**Page-queue follow-up baseline:** `c3ec76a7a6705be779ed9487cec98224e320a88c`

**Historical Rust source:** `c73267593a5f2ddf727b761ccd3705d9f22cd7a8`

**C baseline:** vendored mimalloc 3.3.2

**Host:** AMD Ryzen 9 5950X, 16 cores / 32 logical CPUs
**Related report:** `RUST_VS_C_MIMALLOC_BENCHMARK_2026-08-08.md`

## Purpose

The full ten-run Rust-vs-C `mimalloc-bench` pass found current Rust
`larsonN-sized` performance about 12.75% behind C. The earlier report correctly
stated that a same-time Rust-vs-C comparison alone could not prove a regression
over Rust history. This follow-up rebuilt a historical Rust revision and ran it
against current Rust and the same C artifact to determine whether Rust
performance changed.

This report preserves the measurements, confirmed mechanisms, caveats, and
follow-up changes. The initial historical-attribution phase changed no allocator
source; later sections document the validated C++ and page-queue fixes.

## Why commit `c732675` was selected

`c73267593a5f2ddf727b761ccd3705d9f22cd7a8`, dated
2026-06-16T07:58:02-04:00, was selected because the historical ten-run table in
`BENCHMARK_RESULTS.md` reported `larsonN-sized` only 1.10% behind C at that
revision. It therefore provides a relevant pre-regression reference rather
than an arbitrary previous commit.

A later known performance commit is also relevant:

```text
f4d8f4d7e44ffb9658ca050cbf1cc76afde9b7f0
Fix Redis and larson allocator regressions
```

That June 21 commit recorded Larson-sized parity after flattening hot realloc
and usable-size paths, improving page queue updates, and adding direct C++
sized-delete stubs. Those direct sized-delete stubs are still present in current
source, so simply losing the stub is not the apparent current problem.

## Artifact construction

The historical revision was checked out in a detached Git worktree under the
ignored `target/larson-investigation/` directory. It was compiled with the same
current Rust compiler and release profile used for the current artifact. The
old revision predates the current `interpose` Cargo feature; its release
`cdylib` configuration already exported allocator override symbols.

The current interpose artifact was built with:

```bash
cargo rustc --release --features interpose --lib -- \
  --crate-type cdylib -C link-arg=-lstdc++
```

The old artifact was built with:

```bash
cargo rustc --release --lib -- \
  --crate-type cdylib -C link-arg=-lstdc++
```

The Larson executable itself links `libstdc++.so.6` and imports sized array
allocation/deallocation symbols `_Znam` and `_ZdaPvm`. Consequently, the
current Rust artifact's explicit C++ runtime dependency is not expected to
explain the approximately 10% old/current Larson difference.

Artifact SHA-256 values:

```text
f150433fa5b0b2c3a6107dbbc684270184ace98e8a38101b12aaacc47d6d68eb  C mimalloc 3.3.2
4a41ae3f24f5ba713c8af9ea024968432c4adaeb293f20c3005b298ab497de99  Rust c732675
433befad287a04576316d1c1b6efbb8363946c8976ba4f16207422af9f08b4d4  Rust 8baba5e
```

All artifacts exported the required allocator and C++ symbols and passed an
`LD_PRELOAD` smoke test before measurement.

## Controlled historical/current comparison

The primary follow-up ran C, old Rust, and current Rust in one ten-repetition
`mimalloc-bench` invocation:

```bash
cd mimalloc-bench/out/bench
../../bench.sh --procs=32 -r=10 \
  --external=../../../target/larson-investigation/larson-three-way.allocators \
  larson-sized
```

The actual workload command was:

```bash
./larson-sized 5 8 1000 5000 100 4141 32
```

### Ten-run result

| Allocator | Mean | Median | CV | Difference from C |
|---|---:|---:|---:|---:|
| C mimalloc 3.3.2 | 2.0452s | 2.0435s | 1.73% | baseline |
| Rust `c732675` | 2.0695s | 2.0710s | 1.92% | 1.19% slower |
| Rust `8baba5e` | 2.2893s | 2.2895s | 1.78% | 11.94% slower |

Current Rust was **10.62% slower than old Rust** by mean. Every current-Rust
sample was 8.19% to 16.13% slower than its same-round C sample. Old Rust ranged
from 1.33% faster to 3.93% slower than C and was near parity in aggregate.

This is strong evidence that Rust Larson performance changed between the two
source revisions. The exact 10.62% magnitude remains subject to allocator-order
bias because the suite always ran C first, old Rust second, and current Rust
third. A balanced confirmation is still required before treating the exact
percentage as final.

## Milestone sweep

Selected source milestones were built with the same toolchain and tested in a
three-repetition sweep:

| Revision | Relevant change | Mean | Difference from C |
|---|---|---:|---:|
| C mimalloc 3.3.2 | baseline | 2.0500s | baseline |
| `c732675` | historical baseline | 2.0713s | 1.04% slower |
| `f4d8f4d` | known Redis/Larson fix | 2.0613s | 0.55% slower |
| `f39a11b` | restore release `mi_free` fast route | 2.0947s | 2.18% slower |
| `297dca4` | contain internal macro exports | 2.1723s | 5.97% slower |
| `ff1ff0f` | typed block-size reads in free paths | 2.1870s | 6.68% slower |
| `75c48c1` | preserve abandoned page collection proofs | 2.3673s | 15.48% slower |
| `deb8c8c` | page-key generation gating | 2.3663s | 15.43% slower |
| `86b1cb96` | benchmark review/ceiling changes | 2.3610s | 15.17% slower |
| current | `8baba5e` artifact | 2.2857s | 11.50% slower |

This sweep suggests two phases:

1. an incremental loss between the June 21 fix and early July, reaching roughly
   6%;
2. a larger loss around the July 12–13 page ownership and batch work, followed
   by a partial recovery before current source.

### Milestone caveat

The allocator file listed revisions in chronological order, and `bench.sh`
always ran them in that order. Larson saturates all logical CPUs, so thermal,
frequency, and position effects can bias later entries. The progression is
useful for selecting commit ranges, but these exact percentages must not be
used as a final per-commit attribution.

## Page-batch commit sweep

The largest milestone jump overlapped these changes:

```text
ef12bae  Strengthen allocator evidence layer
178c6dd  Make validation capabilities linear
1bf3e55  Make page capabilities linear
7b399ba  Separate published and detached remote ownership
ba52d13  Introduce page-homogeneous batch primitives
5041c40  Integrate linear page batch collection
f67670c  Complete page batch correctness evidence
75c48c1  Preserve abandoned page collection proofs
```

A five-repetition fixed-order sweep around this range produced:

| Revision | Relevant state | Mean | Difference from C |
|---|---|---:|---:|
| C mimalloc 3.3.2 | baseline | 2.0830s | baseline |
| `ff1ff0f` | pre-page-batch reference | 2.1678s | 4.07% slower |
| `e80ba10` | immediately before page-batch commits | 2.2438s | 7.72% slower |
| `ba52d13` | batch primitives introduced | 2.2224s | 6.69% slower |
| `5041c40` | batch collection integrated | 2.3438s | 12.52% slower |
| `f67670c` | correctness evidence completed | 2.3398s | 12.33% slower |
| `75c48c1` | abandoned collection proofs | 2.3580s | 13.20% slower |
| current | `8baba5e` artifact | 2.3208s | 11.42% slower |

The apparent `ba52d13` to `5041c40` change was confounded by fixed allocator
order. A later direct reversed-order experiment removed the per-node membership
check from `scan_detached_chain()` in a disposable build. It changed unsized
Larson by +0.20% and sized Larson by -0.13%, both noise-level. The experiment
was not landed because it weakened validation and provided no performance
benefit. Page-batch membership validation is therefore not a demonstrated
cause of the Larson gap.

## Validated C++ allocation cause

**Follow-up timestamp:** 2026-08-08T23:47:41-04:00

The Criterion Larson port and upstream Larson executables call the Itanium C++
ABI symbols `_Znam`, `_ZdaPv`, and `_ZdaPvm`. Binary inspection and `dladdr()`
confirmed that both allocator libraries own all three symbols. The earlier
hypothesis that Rust delete calls fell through `libstdc++` was false.

The Rust `_Znam` implementation was nevertheless compiled at effective `-O0`:
`build.rs` compiled `src/cxx_new_shim.cpp` without an optimization flag. The
resulting release entrypoint spilled its argument, called an out-of-line
`mi_cxx_new_impl`, and then called optimized Rust `mi_malloc`. C mimalloc kept
its small-allocation path directly in `_Znam`.

The major transition was isolated to:

```text
4cbb441a34be0ae3ae667bbf884bbb06dff4e174
2026-06-30T01:20:05-04:00
Add release C++ artifact checks
```

That commit changed the C++ archive to `+whole-archive` and disabled the prior
fast assembly `_Znam` whenever the C++ shim was linked. A direct parent/commit
comparison with six samples across reversed orders measured:

| Benchmark | Before `4cbb441` | After `4cbb441` | Change |
|---|---:|---:|---:|
| `larsonN` | 2.0662s | 2.1998s | 6.47% slower |
| `larsonN-sized` | 2.0500s | 2.1813s | 6.41% slower |

Compiling only the C++ shim at `-O3` recovered 4.72% on unsized Larson and
4.15% on sized Larson relative to the old `-O0` artifact. The production fix
also links `libstdc++` into Linux C++ artifacts so standalone `LD_PRELOAD` no
longer fails with unresolved C++ symbols.

## Semantic fast-new follow-up

An additional fast-new path was implemented and retained after validation. It
uses the existing assembly small-allocation success path for `_Znwm`/`_Znam`
and tail-calls an optimized C++ slow helper only on failure. This preserves
`std::new_handler` and `std::bad_alloc` behavior while removing the common-path
PLT call into `mi_malloc`.

The candidate passed:

- C++11, C++14, C++17, and C++20 operator-new/delete parity;
- dynamic and static release artifact C++ tests;
- standalone preload and C++ symbol-ownership checks.

A ten-sample reversed-order comparison against the `-O3`-only artifact showed:

| Benchmark | Fast-new change |
|---|---:|
| `larsonN` | 1.31% faster |
| `larsonN-sized` | 0.47% faster |

A separate ten-run C comparison measured fast-new Rust 5.23% slower on
`larsonN` and 4.83% slower on `larsonN-sized`. The remaining difference is no
longer explained by missing C++ symbols or an unoptimized C++ object.

## Residual-gap evidence

Before the page-queue follow-up, throughput-normalized `perf stat -r 3`
measurements found fast-new Rust still used approximately 5–6% more
cycles/instructions per operation and 13–14% more
branches per operation than C, with a much smaller cache-miss difference.
`perf record` showed the same broad allocator paths in both libraries. Rust's
`collect_page_free_lists` received 4.39% of flat samples versus 1.70% for C's
`_mi_page_free_collect`, but removing the suspected per-node membership checks
did not improve Larson. Further collection changes require a new measured
hypothesis and must preserve the existing corruption checks.

The source-level missing `#[inline(always)]` annotations and extra abandoned
checks are not immediate fixes: release interposition uses handwritten assembly
for the common local and remote free paths, bypassing those typed branches and
helpers. Atomic ordering already matches C and must not be weakened.

## Page-queue control-flow follow-up

**Follow-up timestamp:** 2026-08-09T09:42:50-04:00

Branch-event profiling confirmed that page collection and queue search account for
a substantial part of Rust's remaining control-flow excess. Per-operation branch
sampling attributed about 4.9% of Rust branches to `collect_page_free_lists`
versus 1.1% to C's `_mi_page_free_collect`. Rust's typed queue search accounted
for about 6.9% versus 6.4% for C, while also containing generic queue-transition
work that C attributed to separate helpers.

Three source-shape candidates were rejected before the production candidate:

- forcing the 464-byte direct-cache updater inline improved sized Larson by
  1.47% but slowed unsized Larson by 0.76%;
- simplifying queue classification was 0.6% slower unsized and 0.4% slower
  sized;
- replacing division with a power-of-two membership mask increased code size
  and was 0.74% slower unsized and 1.47% slower sized.

The retained candidate optimizes the general operation that moves a selected
page to the front of its existing normal queue. The old typed implementation
performed a full remove followed by a full insertion. This needlessly cleared
the already-clear full-queue flag twice with locked atomic operations,
decremented and restored heap/queue counts, and cleared and rebuilt links
through generic queue transitions. `QueuedPage<NormalQueue>`
already proves membership and non-full state, so the new implementation relinks
the node in place while leaving counts and flags unchanged.

The release queue-search function changed from three locked atomic instructions
to none. A focused invariant test covers moving both a middle and tail page and
checks the complete link order, queue count, heap page count, and owner/flag
words.

A ten-sample reversed-order baseline/candidate comparison measured:

| Benchmark | Baseline | In-place relink | Change |
|---|---:|---:|---:|
| `larsonN` | 2.1633s | 2.1181s | 2.09% faster |
| `larsonN-sized` | 2.1530s | 2.0981s | 2.55% faster |

Two-sample reversed-order `perf stat` measurements also showed the expected
reduction in retired work:

| Benchmark | Instructions/op | Branches/op | Branch misses/op |
|---|---:|---:|---:|
| `larsonN` | 1.24% lower | 1.90% lower | 0.10% lower |
| `larsonN-sized` | 1.22% lower | 1.84% lower | 0.12% lower |

A final two-sample reversed-order C/Rust counter comparison quantified the
remaining per-operation excess after the relink:

| Benchmark | Cycles | Instructions | Branches | Branch misses |
|---|---:|---:|---:|---:|
| `larsonN` | 1.94% higher | 4.94% higher | 11.77% higher | 18.73% higher |
| `larsonN-sized` | 0.13% higher | 4.32% higher | 11.24% higher | 18.30% higher |

A broader four-sample balanced screen covered Redis, mstress, rptest,
alloc-test, sh6bench, sh8bench, xmalloc-test, glibc-simple, and glibc-thread.
No consistent regression appeared; aggregate changes ranged from 2.88% faster
to 0.58% slower, with the two small slow readings inside those benchmarks'
run-to-run variation. A complete `allt` pass in both allocator orders likewise
found no repeatable regression across the remaining real-world and stress
workloads.

Finally, a direct ten-sample reversed-order C comparison measured:

| Benchmark | C mimalloc 3.3.2 | Rust with relink | Remaining gap |
|---|---:|---:|---:|
| `larsonN` | 2.0613s | 2.1359s | 3.62% slower |
| `larsonN-sized` | 2.0743s | 2.1112s | 1.78% slower |

This is a general queue-maintenance improvement rather than a Larson-specific
policy change. No validation, atomic ordering, allocation policy, or benchmark
behavior was weakened.

## C++ sized-delete findings

`larson-sized` imports `_Znam` and `_ZdaPvm`. Rust and C both own `_ZdaPvm` and
use the same general direct-jump shape into `mi_free`; both ignore the size in
release mode. Adding delete overloads to `src/cxx_new_shim.cpp` would duplicate
symbols already supplied by `src/raw/macros.rs` and is not a valid fix.

## Canceled broad balanced experiment

An earlier ten-round, ten-allocator cyclic experiment was canceled during round
five and deliberately produced no aggregate. None of its partial data was used
for the validated C++ commit or fix measurements above; those used smaller
reversed-order comparisons.

## Recommended next investigation

1. Profile the remaining branch/instruction excess with the final relink
   artifact, separating initial cross-thread turnover from steady-state
   worker-local traffic.
2. Focus the next measured hypothesis on collection work that can be removed by
   an existing ownership invariant, rather than deleting corruption checks.
3. Preserve remote-list validation and current atomic ordering unless a new
   invariant and correctness evidence replace them.
4. Validate every candidate with reversed allocator order, C++ semantic tests,
   the full Rust verification commands, and the complete Rust-vs-C suite.

## Current conclusion

The major Larson regression is attributed to `4cbb441`: forcing an unoptimized
C++ operator-new shim into release artifacts and disabling the fast `_Znam`
path cost about 6.4%. Compiling the shim with `-O3` and restoring a semantic
fast success path recovered most of that loss. Replacing generic normal-queue
remove/reinsert work with an invariant-preserving in-place relink recovered a
further 2.09% on unsized Larson and 2.55% on sized Larson while improving the
broader workload screen. Delete-symbol ownership, release `mi_free_size`
behavior, atomic ordering, and page-batch membership checks are not causes. The
remaining measured Rust gap is 3.62% unsized and 1.78% sized and is still
associated with extra instruction and branch volume in allocator
slow/collection paths.

## Local evidence

Generated local evidence at report time:

- `larson-three-way-r10-20260808.log` — primary C/old/current ten-run result;
- `larson-milestones-r3-20260808.log` — coarse revision milestone sweep;
- `larson-page-batch-r5-20260808.log` — page-batch commit-range sweep;
- `target/larson-investigation/commit-4cbb-order-*.csv` — direct regression-commit A/B;
- `target/larson-investigation/cxx-order-*.csv` — C++ `-O0`/`-O3` A/B;
- `target/larson-investigation/o3-fast-order-*.csv` — optimized/fast-new A/B;
- `target/larson-investigation/page-batch-order-*.csv` — discarded membership-check experiment;
- `target/larson-investigation/perf2-*` and `perf-record-*` — residual counter/profile evidence;
- `target/queue-investigation/` — queue candidate A/B logs, branch profiles,
  counter data, complete-suite screens, and disposable artifacts;
- `target/larson-investigation/` — ignored allocator artifacts and allocator-list files.

The logs and build outputs are untracked local evidence. This tracked report
preserves their methodology and summarized values.
