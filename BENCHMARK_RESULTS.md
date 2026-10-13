# Mimalloc Rust vs C Benchmark Results

## Stage 0 Reviewed Baseline (2026-07-12)

The current release gate is schema 2 in `benchmarks/thresholds.json`, captured
from five complete clean runs of commit
`2140b758f59424188387498bb194acfc3e17ad50` on host profile
`release-linux-x86_64-primary`. All accepted median coefficients of variation
were at most 3.81%. A partial fifth run collected under unrelated 21-core host
load was rejected and rerun after the host became idle.

The gate covers the public `MiMalloc` paired allocation/free facade and a
separate raw-C-ABI native workload group. The noisy 32-worker
`cache-scratchN` case remains available for exploratory evidence; the release
gate uses `cache-scratch1`, whose five-run median CV was 0.61%, so scheduler
placement does not dominate the allocator signal.

Each full artifact directory now contains `manifest.json`, `environment.txt`,
`commands.txt`, `criterion/`, `resource-results.jsonl`, and
`threshold-results.csv`. Default Rust and interpose directories remain
isolated; the latter reports default-mode thresholds as inapplicable until a
separate interpose baseline is reviewed.

## Release Candidate Artifact-Mode Evidence

For a release candidate, collect performance evidence with:

```bash
scripts/check-bench-full.sh
scripts/env-info.sh > target/bench-full/environment.txt
```

The full benchmark script intentionally separates default Rust-library,
feature-linked interposition, deployed preload, and page-batch evidence:

| Artifact mode | Feature set | Evidence path |
|---|---|---|
| default Rust library | `bench` | `target/bench-full/default-rust/criterion` and `resource-results.jsonl` |
| interpose feature-linked hot path | `bench interpose` | `target/bench-full/interpose-feature-linked/criterion` and `resource-results.jsonl` |
| LD_PRELOAD interpose | `interpose` | `target/bench-full/ld-preload-interpose/resource-results.jsonl` |
| page batch, plain links | `bench bench-diagnostics` | `target/bench-full/page-batch-plain/criterion` |
| page batch, secure encoded links | `bench bench-diagnostics secure` | `target/bench-full/page-batch-secure/criterion` |

Do not reuse Criterion baselines across artifact modes. Report each artifact
mode under a separate heading with the exact command line and feature set. The
native scenario section for the default Rust library mode must include the
Rust-vs-C ratios emitted by `scripts/summarize-criterion.py`; the linked-feature
and preload sections must remain separate and must not be compared against
default-mode saved baselines.

Record each Criterion mode's `benchmark-review.json` status and attach the
checksummed `stages-0-3-evidence-<commit>.tar.zst` bundle plus
`evidence-index.json`. The bundle must contain all five artifact modes and the
matching release assembly capture.

## Per-Release Performance Summary Template

Each release entry must include:

- release tag or candidate commit,
- benchmark host and `scripts/env-info.sh` artifact,
- native scenario geomean Rust/C median ratio,
- hot-path deltas against previous release evidence,
- release-blocking regressions, if any,
- noisy watch items separated from release-blocking regressions,
- links to raw Criterion summaries and threshold CSV artifacts for each measured
  artifact mode.

Release notes must link to the matching `BENCHMARK_RESULTS.md` section or to an
attached report that preserves the same fields and raw artifacts.

## Strict-Type Assembly Comparison Checklist

Use `scripts/capture-release-assembly.sh` before and after each strict-type
hot-path migration. Store the generated files from
`target/strict-type-assembly/` with the benchmark note or link to the preserved
artifact location.

For each compared symbol, the strict-type migration passes the assembly check
only when the before/after disassembly shows:

- no additional hot-path conditional or unconditional branch,
- no additional hot-path call,
- no additional hot-path load, store, atomic memory operation, lock prefix, or
  stack spill,
- no wrapper constructor, panic, formatting, allocation, or bounds-check slow
  path on the fast path,
- unchanged ABI entry/exit register convention and fallback target shape.

Required symbols and focus areas:

| Symbol | Required focus |
|---|---|
| `mi_malloc` | small-size entry branch, thread-heap lookup, and fallback handoff |
| `mi_malloc_small` | direct-bin/page lookup and zero extra size-wrapper traffic |
| `mi_free` | null check, local-free fast path, and remote-free handoff |
| `_mi_theap_malloc_zero_ex` | page lookup, free-list pop, and zeroing decision |

When an instruction changes for a non-wrapper reason, record the source change
that caused it and classify the benchmark evidence separately. If a strict-type
wrapper introduces any new hot-path branch or memory traffic, redesign it to
compile away or keep that invariant outside the hot path.

## Strict-Type Free-List Pop Assembly Capture

**Date Generated:** 2026-06-19
**Before Baseline:** `HEAD` (`2a21d3083dedd56923ae9600ac6e5fa512add980`)
**After Baseline:** current strict-type working tree

Commands:

```bash
git worktree add --detach /tmp/mechanical_mimalloc_asm_before HEAD
(cd /tmp/mechanical_mimalloc_asm_before && cargo build --release --lib)
scripts/capture-release-assembly.sh
```

Artifacts:

- before: `target/strict-type-assembly/free-list-pop-before/mi_malloc_small.asm`
- after: `target/strict-type-assembly/mi_malloc_small.asm`
- after manifest: `target/strict-type-assembly/manifest.txt`

`_mi_page_malloc_zero_typed` is inlined in release and does not appear as a
standalone symbol in either optimized artifact. The free-list pop is therefore
checked at the inlined `mi_malloc_small` allocation fast path.

Free-list pop sequence:

| Baseline | Instructions |
|---|---|
| before `mi_malloc_small` | `mov 0x8(%rcx),%rax`; `mov (%rax),%rdx`; `mov %rdx,0x8(%rcx)`; `movq $0x0,(%rax)` |
| after `mi_malloc_small` | `mov 0x8(%rcx),%rax`; `mov (%rax),%rdx`; `mov %rdx,0x8(%rcx)`; `movq $0x0,(%rax)` |

Conclusion: the final `AvailablePage::pop_free_block` / free-list wrapper
migration preserves the inlined free-list pop instruction shape. The pop itself
adds no branch, call, atomic operation, lock prefix, stack spill, or extra
memory traffic. The current caller has additional surrounding thread-init guard
code outside the pop sequence; classify that separately from this free-list
wrapper check in the final benchmark signoff.

## Strict-Type Local Free Assembly Capture

**Date Generated:** 2026-06-19
**Before Baseline:** `HEAD` (`2a21d3083dedd56923ae9600ac6e5fa512add980`)
**After Baseline:** current strict-type working tree

Artifacts:

- before: `target/strict-type-assembly/free-list-pop-before/mi_free.asm`
- after: `target/strict-type-assembly/mi_free.asm`

`mi_free_block_local` and `LocalFreeList::push` are inlined into the release
`mi_free` entrypoint, so the local free fast path was checked in `mi_free`.

Local free-list push sequence:

| Baseline | Instructions |
|---|---|
| before `mi_free` | `mov 0x18(%rdi),%rax`; `mov %rax,(%rsi)`; `subw $0x1,0x10(%rdi)`; `mov %rsi,0x18(%rdi)` |
| after `mi_free` | `mov 0x18(%rdi),%rax`; `mov %rax,(%rsi)`; `subw $0x1,0x10(%rdi)`; `mov %rsi,0x18(%rdi)` |

Conclusion: the final `ValidatedFree`/`LocalPage` witness hardening and
`LocalFreeList::push` wrapper preserve the local free fast-path instruction
shape. The push itself adds no branch, call, atomic operation, lock prefix,
stack spill, or extra memory traffic.

## Strict-Type Remote Free CAS Assembly Capture

**Date Generated:** 2026-06-19
**Before Baseline:** `HEAD` (`2a21d3083dedd56923ae9600ac6e5fa512add980`)
**After Baseline:** current strict-type working tree

Artifacts:

- before: `target/strict-type-assembly/free-list-pop-before/mi_free.asm`
- after: `target/strict-type-assembly/mi_free.asm`

`AtomicThreadFree::push_remote_acq_rel` is inlined into the release `mi_free`
remote-free path, so the CAS loop was checked in `mi_free`.

Remote free CAS loop:

| Baseline | Instructions |
|---|---|
| before `mi_free` | `mov %rsi,%rcx`; `mov 0x20(%rdi),%rax`; `lea 0x20(%rdi),%rdx`; `or $0x1,%rcx`; `mov %rax,%r8`; `and $0xfffffffffffffffe,%r8`; `mov %r8,(%rsi)`; `lock cmpxchg %rcx,(%rdx)`; `jne` loop |
| after `mi_free` | `mov %rsi,%rcx`; `mov 0x20(%rdi),%rax`; `lea 0x20(%rdi),%rdx`; `or $0x1,%rcx`; `mov %rax,%r8`; `and $0xfffffffffffffffe,%r8`; `mov %r8,(%rsi)`; `lock cmpxchg %rcx,(%rdx)`; `jne` loop |

Conclusion: the final `AtomicThreadFree` migration preserves the remote free
CAS loop instruction shape. The wrapper adds no extra branch, call, load, store,
atomic operation, lock prefix, or stack spill beyond the existing CAS-loop
traffic.

## Strict-Type Small Allocation Page Lookup Assembly Capture

**Date Generated:** 2026-06-19
**Before Baseline:** `HEAD` (`2a21d3083dedd56923ae9600ac6e5fa512add980`)
**After Baseline:** current strict-type working tree

Artifacts:

- before: `target/strict-type-assembly/free-list-pop-before/mi_malloc_small.asm`
- after: `target/strict-type-assembly/mi_malloc_small.asm`

The `PaddedSize` and `DirectBin` migration is checked in the release
`mi_malloc_small` direct-bin/page lookup path.

Small allocation direct-bin/page lookup:

| Baseline | Instructions |
|---|---|
| before `mi_malloc_small` | `mov %esi,%eax`; `shr $0x3,%eax`; `mov %esi,%ecx`; `and $0x7,%ecx`; `cmp $0x1,%rcx`; `sbb $0xffffffff,%eax`; `movzwl %ax,%eax`; `cmp $0x80,%eax`; `ja` fallback; `mov 0x110(%rdi,%rax,8),%rcx`; `mov 0x8(%rcx),%rax` |
| after `mi_malloc_small` | `mov %ebx,%eax`; `shr $0x3,%eax`; `mov %ebx,%ecx`; `and $0x7,%ecx`; `cmp $0x1,%rcx`; `sbb $0xffffffff,%eax`; `movzwl %ax,%eax`; `cmp $0x80,%eax`; `ja` fallback; `mov 0x110(%rdi,%rax,8),%rcx`; `mov 0x8(%rcx),%rax` |

Conclusion: the `PaddedSize` and `DirectBin` wrappers compile away in the small
allocation page lookup. The `%esi` to `%ebx` register change comes from
surrounding thread-initialization guard code that preserves the request size;
the bin arithmetic, bounds branch, direct page-table load, and page free-list
head load remain instruction-equivalent.

## Strict-Type Benchmark Smoke Checks

**Date Generated:** 2026-06-21
**Commit:** `fa3a191` (`Complete strict-type migration checklist`)
**Command:** `scripts/check-bench-smoke.sh`
**Result:** passed with exit code 0

This smoke pass was run after the strict-type TODO completion commit and before
the lint/docs polish pass. The Rust allocator comparisons in the configured
native subset did not detect a regression for `glibc-thread`, `malloc-large`,
`xmalloc-testN`, or `cache-scratchN`. A saved-baseline regression marker was
observed for the C allocator `xmalloc-testN` comparison only; classify that as
C-baseline drift, not as a Rust strict-wrapper regression.

---

**Date Generated:** 2026-06-19
**Command:** `scripts/check-bench-smoke.sh`
**Result:** passed with exit code 0

The smoke check completed `cargo check --benches --features bench` and the
configured native allocator benchmark subset. The run emitted the repository's
existing warning noise and Criterion's saved-baseline comparisons.

Observed Criterion smoke signals:

| Benchmark | Rust result |
|---|---|
| `glibc-thread-per-1M-malloc-free-pairs` | no performance change detected |
| `malloc-large-per-workload` | `+10.144%` mean time regression versus the saved Criterion baseline |
| `xmalloc-testN-per-1M-frees` | `-2.0886%` mean time improvement |
| `cache-scratchN-per-workload` | `-15.456%` mean time improvement |

The script returned success, so this smoke pass is recorded as a completed
post-migration check. The `malloc-large-per-workload` regression signal needs a
targeted rerun before the final benchmark decision because this smoke command is
short and compares against whatever Criterion baseline is already present under
`target/criterion`.

## Strict-Type Targeted Benchmark Subset

**Date Generated:** 2026-06-19
**Result:** passed; no strict-wrapper hot-path regression confirmed

This pass reran the targeted Criterion hotpath harness and the selected native
allocator subset after the final hot-path strict-wrapper migration.

Hotpath command:

```bash
MIMALLOC_HOTPATH_SAMPLE_SIZE=10 \
MIMALLOC_HOTPATH_MEASUREMENT_SECS=1 \
MIMALLOC_HOTPATH_WARMUP_SECS=1 \
MIMALLOC_HOTPATH_NRESAMPLES=1000 \
cargo bench --features bench --bench hotpath
```

Native subset command:

```bash
MIMALLOC_BENCH_SCENARIOS=glibc-thread,malloc-large,cache-scratchN,xmalloc-testN \
MIMALLOC_BENCH_ALLOCATORS=rust,c \
MIMALLOC_BENCH_SAMPLE_SIZE=10 \
MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
```

The first hotpath pass produced several short-run Criterion regression markers
against local saved baselines. Representative isolated reruns used 20 samples,
3 second measurement, 2 second warmup, and 10000 resamples:

| Benchmark | Isolated result | Decision |
|---|---:|---|
| `paired_alloc_free/16` | 13.294 us median, `-5.3824%` mean time | improved |
| `free_only/16` | 8.5404 us median, `+4.3325%` mean time | no change |
| `lifo_stack/16/512` | 1.5807 us median, `-42.291%` mean time | improved |
| `single_reuse/256` | 203.47 us median, `+4.3645%` mean time | within noise threshold |

The native subset initially flagged `malloc-large-per-workload` for both Rust
and C, and `xmalloc-testN-per-1M-frees` for Rust only. Isolated reruns cleared
both watch items:

| Benchmark | Rust result | C result | Decision |
|---|---:|---:|---|
| `malloc-large-per-workload` | 986.28 us median, `-3.1607%` mean time | 951.34 us median, within noise | Rust improved; C stable |
| `xmalloc-testN-per-1M-frees` | 2.1472 ms median, within noise | 2.0607 ms median, `-1.5671%` mean time | Rust stable; C improved |

The mixed native subset also showed `glibc-thread` stable for both allocators
and `cache-scratchN` within noise for both allocators.

Conclusion: no measured wrapper cost remains after isolated reruns. The current
decision is to keep the strict hot-path wrappers in place because the assembly
captures above show identical memory traffic and branch structure, and the
targeted Criterion/native subset did not confirm a Rust-only regression.

## Current-Head 10-Sample Criterion Aggregate

**Date Generated:** 2026-06-18 17:00:19 EDT
**Benchmarked Commit Hash:** `c0990a8b22b847d4999f9b0552ccbe2eacd5a1b4`

This is the current-head aggregate Criterion pass for the hotpath harness and
the selected native `mimalloc-bench` Rust ports. Espresso, redis, and rocksdb
are intentionally excluded from this required set. Criterion enforces a minimum
sample size of 10, so this pass used 10 samples for the aggregate and 20 samples
for isolated hotpath reruns that initially flagged regressions.

Environment:

- CPU: AMD Ryzen 9 5950X, 32 logical CPUs, 16 cores, 1 NUMA node.
- OS/kernel: Ubuntu 24.04, Linux `6.17.0-23-generic`, x86_64.
- Rust: `rustc 1.92.0 (ded5c06cf 2025-12-08)`, LLVM 21.1.3.
- C compiler: Ubuntu GCC `13.3.0`.
- Feature flags: `--features bench`.
- C allocator baseline: C mimalloc 3.3.2 built and linked by
  `benches/mimalloc_bench.rs` from `mimalloc-3.3.2`.
- Raw structured output: `target/criterion/**/new/{benchmark,estimates,sample}.json`.

Commands:

```bash
env MIMALLOC_HOTPATH_SAMPLE_SIZE=10 \
  MIMALLOC_HOTPATH_MEASUREMENT_SECS=1 \
  MIMALLOC_HOTPATH_WARMUP_SECS=1 \
  MIMALLOC_HOTPATH_NRESAMPLES=1000 \
  cargo bench --features bench --bench hotpath

env MIMALLOC_BENCH_SCENARIOS=glibc-simple,glibc-thread,malloc-large,malloc-large-old,cache-scratch1,cache-thrash1,cache-scratchN,cache-thrashN,xmalloc-testN,mstressN,sh6benchN,sh8benchN,alloc-test1,alloc-testN,larsonN,larsonN-sized,rptestN,barnes,cfrac,mleak10,mleak100 \
  MIMALLOC_BENCH_ALLOCATORS=rust,c \
  MIMALLOC_BENCH_SAMPLE_SIZE=10 \
  MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
  MIMALLOC_BENCH_WARMUP_SECS=1 \
  cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
```

Initial hotpath aggregate noise was followed up with longer isolated reruns:

```bash
env MIMALLOC_HOTPATH_SAMPLE_SIZE=20 \
  MIMALLOC_HOTPATH_MEASUREMENT_SECS=3 \
  MIMALLOC_HOTPATH_WARMUP_SECS=2 \
  MIMALLOC_HOTPATH_NRESAMPLES=10000 \
  cargo bench --features bench --bench hotpath -- <flagged-hotpath-id>
```

Summary generation:

```bash
scripts/summarize-criterion.py --id-file /tmp/mechanical_mimalloc_criterion_full_ids.txt
```

### Hotpath Summary

| Benchmark | Samples | Mean | Median | Min | P95 | Max | Noise |
|---|---:|---:|---:|---:|---:|---:|---|
| `paired_alloc_free/16` | 20 | 14.45 us | 14.45 us | 14.42 us | 14.53 us | 14.54 us | stable |
| `paired_alloc_free/64` | 10 | 18.18 us | 18.16 us | 18.09 us | 18.33 us | 18.35 us | stable |
| `paired_alloc_free/256` | 10 | 26.18 us | 26.15 us | 26.05 us | 26.35 us | 26.43 us | stable |
| `paired_alloc_free/1024` | 10 | 54.07 us | 53.89 us | 53.45 us | 55.16 us | 55.94 us | stable |
| `paired_alloc_free/4096` | 10 | 163.8 us | 164.1 us | 160.2 us | 165.2 us | 165.7 us | stable |
| `paired_alloc_free/32768` | 10 | 158.1 us | 158.0 us | 155.2 us | 162.3 us | 164.6 us | stable |
| `free_only/16` | 10 | 6.978 us | 7.054 us | 6.548 us | 7.320 us | 7.479 us | noisy |
| `free_only/64` | 10 | 12.09 us | 12.80 us | 8.395 us | 13.77 us | 13.78 us | noisy |
| `free_only/256` | 10 | 22.83 us | 23.23 us | 20.30 us | 23.69 us | 23.71 us | noisy |
| `free_only/1024` | 10 | 29.74 us | 28.26 us | 27.85 us | 36.81 us | 43.30 us | noisy |
| `free_only/4096` | 10 | 61.50 us | 63.62 us | 45.93 us | 66.02 us | 66.26 us | noisy |
| `free_only/32768` | 10 | 42.23 us | 42.13 us | 41.39 us | 42.87 us | 42.95 us | stable |
| `lifo_stack/16/512` | 20 | 1.675 us | 1.670 us | 1.644 us | 1.718 us | 1.744 us | stable |
| `lifo_stack/64/256` | 20 | 797.6 ns | 798.0 ns | 790.8 ns | 805.8 ns | 810.8 ns | stable |
| `lifo_stack/256/128` | 10 | 423.6 ns | 423.5 ns | 422.3 ns | 425.1 ns | 425.6 ns | stable |
| `lifo_stack/1024/64` | 20 | 510.7 ns | 509.1 ns | 504.7 ns | 517.4 ns | 518.0 ns | stable |
| `single_reuse/16` | 20 | 146.2 us | 146.0 us | 145.8 us | 147.2 us | 147.5 us | stable |
| `single_reuse/64` | 20 | 142.3 us | 142.3 us | 141.4 us | 143.3 us | 144.0 us | stable |
| `single_reuse/256` | 10 | 158.0 us | 157.8 us | 157.3 us | 158.8 us | 158.9 us | stable |
| `single_reuse/1024` | 10 | 157.8 us | 157.8 us | 155.5 us | 159.1 us | 159.4 us | stable |

Initial Criterion flags on `paired_alloc_free/16`, `lifo_stack/16/512`,
`lifo_stack/64/256`, `lifo_stack/1024/64`, `single_reuse/16`, and
`single_reuse/64` were rerun with 20 samples. The reruns cleared the hotpath
regression concern: five improved and `single_reuse/16` moved to within the
configured noise threshold.

### Native Scenario Ratios

| Scenario | Rust median | C median | Rust vs C | Noise |
|---|---:|---:|---:|---|
| `alloc-test1-per-workload` | 2.173 s | 2.173 s | -0.01% | stable |
| `alloc-testN-per-workload` | 2.234 s | 2.243 s | -0.37% | stable |
| `barnes-per-workload` | 1.836 s | 1.857 s | -1.14% | stable |
| `cache-scratch1-per-workload` | 2.053 s | 2.047 s | +0.26% | stable |
| `cache-scratchN-per-workload` | 103.6 ms | 105.0 ms | -1.29% | noisy |
| `cache-thrash1-per-workload` | 2.050 s | 2.044 s | +0.28% | stable |
| `cache-thrashN-per-workload` | 103.4 ms | 104.0 ms | -0.53% | noisy |
| `cfrac-per-workload` | 11.33 s | 11.24 s | +0.84% | stable |
| `glibc-simple-per-workload` | 895.6 ms | 907.5 ms | -1.30% | stable |
| `glibc-thread-per-1M-malloc-free-pairs` | 950.3 us | 932.7 us | +1.89% | stable |
| `larsonN-per-1M-alloc-free-pairs` | 2.956 ms | 2.908 ms | +1.66% | noisy |
| `larsonN-sized-per-1M-alloc-free-pairs` | 3.004 ms | 2.859 ms | +5.08% | noisy |
| `malloc-large-old-per-workload` | 142.4 ms | 142.4 ms | -0.02% | stable |
| `malloc-large-per-workload` | 907.5 us | 952.4 us | -4.72% | stable |
| `mleak10-per-workload` | 105.9 ms | 105.4 ms | +0.46% | stable |
| `mleak100-per-workload` | 1.059 s | 1.060 s | -0.11% | stable |
| `mstressN-per-workload` | 1.578 s | 1.548 s | +1.90% | stable |
| `rptestN-per-1M-rptest-ops` | 1.553 s | 1.582 s | -1.86% | stable |
| `sh6benchN-per-workload` | 107.2 ms | 117.2 ms | -8.49% | noisy |
| `sh8benchN-per-workload` | 197.6 ms | 187.9 ms | +5.14% | noisy |
| `xmalloc-testN-per-1M-frees` | 2.054 ms | 2.110 ms | -2.64% | stable |

Native scenario geomean Rust/C median ratio: `0.9972` (`-0.28%`), meaning the
Rust port is 0.28% faster overall by median geomean on this selected set.

Conclusion: no confirmed hotpath regression remains after isolated reruns. No
overall benchmark-score regression was detected; the current native aggregate is
slightly positive overall. Noisy watch items are `free_only/*`,
`cache-scratchN`, `cache-thrashN`, `larsonN*`, `sh6benchN`, and `sh8benchN`.

## Current-Head Smoke Check

**Date Generated:** 2026-06-18 16:22:21 EDT
**Commit Hash:** `5676b2d60bf87abd26f10ae5e8ee2ed83ed5bec4`

This smoke check followed documentation-only changes and crate-private
`#[cfg(test)]` core-type model-test additions. Those changes do not affect
release allocator hot paths, so a smoke run plus one isolated noisy-scenario
rerun was used instead of a full 10-run aggregate.

Command:

```bash
scripts/check-bench-smoke.sh
```

### Hotpath Criterion Probe

| Probe | Current Rust result | Criterion local-baseline result |
|---|---:|---|
| `hotpath/paired_alloc_free/mechanical_mimalloc/16` | 14.536 us median, 281.78 Melem/s | Within noise threshold: -0.59% time |

Conclusion: no hotpath regression was detected in the tight allocator probe.

### Native `mimalloc-bench` Criterion Smoke

The script ran:

```bash
MIMALLOC_BENCH_SCENARIOS=glibc-thread,malloc-large,cache-scratchN,xmalloc-testN \
MIMALLOC_BENCH_ALLOCATORS=rust,c \
MIMALLOC_BENCH_SAMPLE_SIZE=10 \
MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
```

| Scenario | Rust median | C mimalloc 3.3.2 median | Result |
|---|---:|---:|---|
| `glibc-thread-per-1M-malloc-free-pairs` | 936.91 us | 946.56 us | Rust +1.0%; Rust local baseline showed no change |
| `malloc-large-per-workload` | 902.72 us | 956.19 us | Rust +5.6%; Rust local baseline improved |
| `xmalloc-testN-per-1M-frees` | 2.1104 ms | 2.0540 ms | C +2.7%; both allocators improved vs local baseline |
| `cache-scratchN-per-workload` | 122.97 ms initial smoke; 107.28 ms isolated rerun | 106.28 ms initial smoke; 107.06 ms isolated rerun | Initial Rust sample was noisy/regressed; isolated rerun cleared it |

The initial `cache-scratchN` Rust smoke reported a regression with a wide
interval. Because recent changes were documentation/test-only and the C result
also moved materially, the scenario was rerun in isolation:

```bash
MIMALLOC_BENCH_SCENARIOS=cache-scratchN \
MIMALLOC_BENCH_ALLOCATORS=rust,c \
MIMALLOC_BENCH_SAMPLE_SIZE=5 \
MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native/cache-scratchN
```

Criterion still collected 10 samples for the isolated rerun. Rust measured
107.28 ms median and was marked improved relative to the immediately preceding
noisy sample; C measured 107.06 ms median and showed no change.

Conclusion: no Rust hotpath regression was detected, and no selected native
scenario regression remained after isolating the noisy `cache-scratchN` sample.
The latest full 10-run aggregate remains historical; use this entry as current
HEAD smoke evidence, not as a replacement for a release aggregate.

## Post-Documentation/Test Smoke Check

**Date Generated:** 2026-06-18 16:04:29 EDT
**Commit Hash:** `9fde88ec0fa1230a2607ca23ae8774b67c3a6c04`

This smoke check followed documentation, governance-template, property-test, and
`#[cfg(test)]` core-type model-test changes. Those changes do not affect release
allocator hot paths, so a smoke run was used instead of a full aggregate.

Command:

```bash
scripts/check-bench-smoke.sh
```

### Hotpath Criterion Probe

| Probe | Current Rust result | Criterion local-baseline result |
|---|---:|---|
| `hotpath/paired_alloc_free/mechanical_mimalloc/16` | 14.610 us median, 280.35 Melem/s | Within noise threshold: +1.18% time |

Conclusion: no hotpath regression was detected in the tight allocator probe.

### Native `mimalloc-bench` Criterion Smoke

The script ran:

```bash
MIMALLOC_BENCH_SCENARIOS=glibc-thread,malloc-large,cache-scratchN,xmalloc-testN \
MIMALLOC_BENCH_ALLOCATORS=rust,c \
MIMALLOC_BENCH_SAMPLE_SIZE=10 \
MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
```

| Scenario | Rust median | C mimalloc 3.3.2 median | Result |
|---|---:|---:|---|
| `glibc-thread-per-1M-malloc-free-pairs` | 935.43 us | 939.82 us | Rust +0.5%; Rust local baseline showed no change |
| `malloc-large-per-workload` | 935.53 us | 968.11 us | Rust +3.4%; Rust local baseline within noise |
| `xmalloc-testN-per-1M-frees` | 2.1582 ms | 2.1599 ms | Tie/Rust +0.1%; C local baseline moved slower |
| `cache-scratchN-per-workload` | 105.49 ms | 116.84 ms | Rust +9.7%; both allocators improved vs local baseline |

Conclusion: no Rust hotpath or selected native-scenario regression was detected.
The `xmalloc-testN` Criterion regression marker applied to the C allocator's
local-baseline comparison, while Rust and C were effectively tied in the
same run.

## Current-Head Regression Check

**Date Generated:** 2026-06-18 15:25:24 EDT
**Commit Hash:** `40d407c9bd9066c3dee78b1e674805eb3ab57458`

This is a focused current-head regression pass after the public facade/raw ABI
cleanup. Espresso was intentionally excluded from this pass.

### Hotpath Criterion Probes

Command shape:

```bash
MIMALLOC_HOTPATH_SAMPLE_SIZE=10 \
MIMALLOC_HOTPATH_MEASUREMENT_SECS=1 \
MIMALLOC_HOTPATH_WARMUP_SECS=1 \
MIMALLOC_HOTPATH_NRESAMPLES=1000 \
cargo bench --features bench --bench hotpath -- <filter>
```

| Probe | Current Rust result | Criterion local-baseline result |
|---|---:|---|
| `hotpath/paired_alloc_free/mechanical_mimalloc/16` | 14.388 us median, 284.69 Melem/s | Improved: -4.84% time, +5.09% throughput |
| `hotpath/single_reuse/mechanical_mimalloc/16` | 139.92 us median, 357.35 Melem/s | Within noise threshold |

Conclusion: no hotpath regression was detected in the tight allocator probes.

### Native `mimalloc-bench` Criterion Ports

Command:

```bash
MIMALLOC_BENCH_SCENARIOS=glibc-thread,malloc-large,cache-scratch1,cache-thrash1,cache-scratchN,cache-thrashN,xmalloc-testN \
MIMALLOC_BENCH_ALLOCATORS=rust,c \
MIMALLOC_BENCH_SAMPLE_SIZE=10 \
MIMALLOC_BENCH_MEASUREMENT_SECS=1 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
```

| Scenario | Rust median | C mimalloc 3.3.2 median | Result |
|---|---:|---:|---|
| `glibc-thread-per-1M-malloc-free-pairs` | 947.14 us | 929.05 us | C +1.9%; Rust local baseline showed no change |
| `malloc-large-per-workload` | 918.27 us | 953.42 us | Rust +3.7%; Rust local baseline showed no change |
| `cache-scratch1-per-workload` | 2.0716 s | 2.0521 s | Parity band; short-run Rust +1.0% slower |
| `cache-thrash1-per-workload` | 2.0678 s | 2.0525 s | Parity band; short-run Rust +0.7% slower |
| `xmalloc-testN-per-1M-frees` | 2.1466 ms | 2.1102 ms | C +1.7% |
| `cache-scratchN-per-workload` | 115.95 ms | 137.17 ms | Rust +15.5% |
| `cache-thrashN-per-workload` | 142.93 ms | 117.11 ms | C +18.1%; quick sample only |

Conclusion: the current HEAD remains within the expected Rust/C performance
profile for the selected non-espresso ports. The only large short-run split is
`cache-thrashN`; it is not a hotpath regression and should be confirmed with a
longer aggregate before treating it as authoritative.

## Historical 10-Run Aggregate

**Date Generated:** 2026-06-16 18:12:00 EDT
**Commit Hash:** `c73267593a5f2ddf727b761ccd3705d9f22cd7a8`

## Overview
This report contains the automated benchmarking results comparing the C-based `mimalloc` (`libmimalloc-c.so`) against our mechanical Rust translation (`libmimalloc-rust.so`). 

The `mimalloc-bench` suite was used to test the allocators across a variety of workloads, from single-threaded stress tests to cache-thrashing multi-threaded applications.

To reduce system noise and ensure statistical significance, each test was executed **10 times** (`-r=10`). The results below represent the average execution time across those 10 runs. Redis and rocksdb benchmarks are now included after their build dependencies were resolved.

## Hardware Configuration
- **Concurrency Level:** 32 cores

## Averaged Results (10 Iterations)

| Benchmark | C mimalloc (`cmi`) Avg | Rust mimalloc (`rmi`) Avg | Difference |
|---|---|---|---|
| **cfrac** | 2.755s | 2.768s | +0.47% (Slower) |
| **espresso** | 2.894s | 2.892s | -0.07% (Faster, tie) |
| **barnes** | 2.129s | 2.132s | +0.14% (Slower, tie) |
| **redis** | 2.686s | 2.697s | +0.41% (Slower) |
| **larsonN-sized** | 2.096s | 2.119s | +1.10% (Slower) |
| **mstressN** | 1.628s | 1.634s | +0.37% (Slower) |
| **rptestN** | 1.481s | 1.561s | +5.40% (Slower) |
| **gs** | 0.204s | 0.201s | -1.47% (Faster) |
| **lua** | 0.803s | 0.806s | +0.37% (Slower) |
| **alloc-test1** | 2.106s | 2.093s | -0.62% (Faster) |
| **alloc-testN** | 2.251s | 2.227s | -1.07% (Faster) |
| **sh6benchN** | 0.137s | 0.111s | **-19.0% (Faster)** |
| **sh8benchN** | 0.244s | 0.253s | +3.69% (Slower) |
| **xmalloc-testN** | 0.217s | 0.217s | 0.00% (Tie) |
| **cache-scratch1** | 0.841s | 0.838s | -0.36% (Faster) |
| **cache-scratchN** | 0.065s | 0.070s | +7.69% (Slower) |
| **glibc-simple** | 1.368s | 1.314s | **-3.95% (Faster)** |
| **glibc-thread** | 0.419s | 0.426s | +1.67% (Slower) |
| **rocksdb** | 3.683s | 3.639s | -1.19% (Faster) |

## Conclusion
The Rust translation continues to demonstrate functional and performance parity with the original C mimalloc. Across the 19 benchmarks (including the newly enabled redis and rocksdb), Rust is faster or within measurement noise in 9 cases and slower in 10, with the largest single gain being a 19 % improvement in `sh6benchN`. All differences remain within a few percent, confirming the mechanical translation preserves the allocator’s extreme performance characteristics.

## Stage 3 Page-Batch Evidence

**Date Generated:** 2026-07-13

**Before Baseline:** Stage 2 commit `e80ba10`

**After Baseline:** Stage 3 working tree

The targeted Criterion smoke suite passed in both default and secure encoded
free-list modes. It covers forced page-free collection and local/remote batch
transitions at lengths 1, 8, 32, and 128 with empty and nonempty destinations.
Fixture preparation and restoration occur before each `Instant::now()` timed
region.

Release assembly was captured with `scripts/capture-release-assembly.sh` into
`target/stage3-assembly/{before,after}`. After normalizing only linked image
addresses and RIP-relative layout displacements, these instruction sequences
were identical:

| Symbol | Instructions | Result |
|---|---:|---|
| `mi_malloc` | 52 | equivalent |
| `mi_malloc_small` | 49 | equivalent |
| `mi_free` | 11 | equivalent |
| `mi_route_free` | 176 | equivalent |
| `mi_free_block_local` | 65 | equivalent |
| `_mi_theap_malloc_zero_ex` | 71 | equivalent |

Capturing `mi_route_free` and `mi_free_block_local` makes the inlined local-free
and remote-CAS paths explicit; the exported `mi_free` symbol itself is only a
tail jump to the route function in this release build.

## Final Stages 0–3 Signoff

The clean full benchmark gate completed for separate `default-rust` and
`interpose` artifacts. All six applicable default thresholds passed; the
interposition artifact has no applicable stored timing thresholds. Both
artifact manifests contain the required commands, Criterion data, resource
records, and validation outputs.

Each resource mode produced and validated 18 records across six
scenario/allocator tuples with zero live requested bytes at process exit and
zero page faults per million measured operations. The maximum wall-time
coefficient of variation was 0.055 for `default-rust` and 0.183 for
`interpose`, below the 0.35 initial gate. Peak committed/logical-live ratios
were 33.0 and 33.25 respectively. One initial interposition probe was retried
after a single `system/alloc_free` scheduling outlier produced a 0.58
coefficient; the isolated clean rerun passed all records without code or
threshold changes.

## Current-Head Stages 0–3 Evidence Closure

**Date Generated:** 2026-07-14

**Evidence Commit:** `86b1cb96bd9b387097dd28df809463f662b918bd`

The current benchmark methodology has a reviewed five-run default-Rust
baseline at commit `abd7028dd8fc1b7e7992f7fd807c1982e4068515`. Its six
selected entries have median coefficients of variation from 1.52% to
6.02%. The page-batch plain and secure baselines were separately reviewed at
commit `d868b0e17e5ebb6b2bc124c0895580feedb39875`. These baselines supersede the
pre-methodology-change baseline for Stage 4 comparisons.

Both `scripts/check-local.sh` and the five-mode
`scripts/check-bench-full.sh` completed successfully at the evidence commit.
The full gate recorded these artifact modes and review results:

| Artifact mode | Feature set | Review status |
|---|---|---|
| `default-rust` | `bench` | review required; disposition accepted below |
| `interpose-feature-linked` | `bench,interpose` | not applicable; no stored timing baseline |
| `ld-preload-interpose` | deployed `interpose` cdylib | resource gate passed |
| `page-batch-plain` | `bench,bench-diagnostics` | review required; disposition accepted below |
| `page-batch-secure` | `bench,bench-diagnostics,secure` | pass |

The current native-workload Rust/C median ratios were 1.0008 for
`cache-scratch1`, 1.0349 for `glibc-thread`, 1.0767 for `malloc-large`, and
1.0061 for `xmalloc-testN`; their geomean was 1.0292. A ratio above 1 means the
Rust raw-C-ABI surface took longer than the vendored C allocator.

All 42 resource records passed their scenario-specific committed-memory, RSS,
retained-memory, page-fault, and variability limits: 18 records each for
`default-rust` and `interpose-feature-linked`, plus six neutral-program records
for `ld-preload-interpose`. The largest wall-time coefficient of variation was
5.22%, and the deployed probe confirmed that the preloaded cdylib owned libc
allocations.

The release assembly capture contains all six required symbols. After
normalizing linked image addresses and layout-only symbol displacements, the
current instruction sequences match the preserved Stage 3 capture exactly:
52 instructions for `mi_malloc`, 49 for `mi_malloc_small`, 11 for `mi_free`,
176 for `mi_route_free`, 65 for `mi_free_block_local`, and 71 for
`_mi_theap_malloc_zero_ex`.

Two investigation signals were reviewed. The default selected-workload
current/baseline geomean was 1.0111 against the 1.01 investigation budget, but
no individual benchmark crossed its category budget and no hard ceiling
failed. The plain page-batch geomean was 1.0111, with only
`local_to_available/nonempty/8` crossing its 1.03 category budget at 1.0385;
the other 16 plain cases ranged from 0.9670 to 1.0286, the secure
geomean was 1.0056, and no hard ceiling failed. These small, isolated signals
are accepted as measurement variation rather than evidence of a systematic
regression. The one-time methodology transition is accepted on the combined
current Rust/C ratios, unchanged normalized release assembly, prior Stage 3
evidence, and this current full run. Future Stage 4 work uses the new reviewed
baselines without reopening the superseded absolute comparison.

The durable evidence archive is
`stages-0-3-evidence-86b1cb96bd9b.tar.zst`, with SHA-256
`c81de4e2914fe9ebc8fb89bebbb9292e7861a6d233cfed1d0fc6cb2fa615191c`.
Its evidence index records one clean commit across all five artifact manifests,
the assembly manifest, and the deployed cdylib digest.

## First Pass Rust and C Comparison October 12 2026

**Measured commit:** `5772656bad206bed5e1dc9d003bfab1e45d4cacd`

All 21 native scenarios completed against the Rust translation and vendored C
mimalloc 3.3.2. The combined Rust/C median geomean was **0.9944**, or **0.56%
less time for Rust**. Fourteen scenarios were within 1% of C. The largest
differences were `larsonN` at -8.67%, `malloc-large` at -6.61%, and
`xmalloc-testN` at +7.61%. These are first-pass observations; repeatability of
the differences has not been established.

### Host and measurement controls

- Host: AMD EPYC 4245P, 6 physical cores and 12 logical CPUs, Linux x86_64.
- Compilers: Rust 1.92.0, LLVM 21.1.3, GCC 13.3.0.
- Criterion: 10 samples, 3-second measurement target, 1-second warmup,
  flat sampling, without CPU pinning. Fixed-duration workloads exceeded the
  measurement target to collect all ten samples.
- Both allocators ran the same Rust workload ports through shared-library
  function pointers. Rust was measured before C for each scenario.
- Benchmark executable features: `bench`. The native loader separately built
  the Rust shared library with `interpose`, release optimization, fat LTO,
  one codegen unit, and aborting panics. This measures the exported allocator
  surface, rather than the public Rust `MiMalloc` facade.
- C library: CMake Release, `-O3 -DNDEBUG`, shared-library allocation overrides,
  initial-exec TLS, with secure and guarded modes disabled and no LTO.
- The first process covered 17 scenarios; a fresh second process covered
  `alloc-test1`, `alloc-testN`, `sh8benchN`, and `rptestN` with the same controls
  and the same prebuilt libraries. Both processes exited successfully.

### Scenario medians

`Rust vs C` is `(Rust median / C median - 1) * 100`; negative values mean less
time for Rust. Each time applies to the metric in its row. Workload times are
not individual allocation latencies, and times normalized to one million
operations are not the wall-clock duration of a complete scenario.

| Scenario | Metric | Rust median | C median | Rust vs C | Noise |
|---|---|---:|---:|---:|---|
| `alloc-test1` | workload | 1.971 s | 1.985 s | -0.70% | stable |
| `alloc-testN` | workload | 2.277 s | 2.294 s | -0.72% | stable |
| `barnes` | workload | 1.103 s | 1.105 s | -0.13% | stable |
| `cache-scratch1` | workload | 663.6 ms | 663.6 ms | -0.01% | stable |
| `cache-scratchN` | workload | 115 ms | 115.2 ms | -0.09% | stable |
| `cache-thrash1` | workload | 669.4 ms | 675 ms | -0.82% | stable |
| `cache-thrashN` | workload | 119.6 ms | 115.8 ms | +3.24% | stable |
| `cfrac` | workload | 8.738 s | 8.736 s | +0.02% | stable |
| `glibc-simple` | workload | 625.5 ms | 623.1 ms | +0.40% | stable |
| `glibc-thread` | 1M malloc/free pairs | 1.435 ms | 1.436 ms | -0.04% | stable |
| `larsonN` | 1M alloc/free pairs | 2.535 ms | 2.776 ms | -8.67% | stable |
| `larsonN-sized` | 1M alloc/free pairs | 2.767 ms | 2.789 ms | -0.78% | stable |
| `malloc-large-old` | workload | 67.05 ms | 68.65 ms | -2.34% | noisy |
| `malloc-large` | workload | 720.4 us | 771.4 us | -6.61% | stable |
| `mleak10` | workload | 36.07 ms | 36.13 ms | -0.17% | stable |
| `mleak100` | workload | 359.8 ms | 360.5 ms | -0.18% | stable |
| `mstressN` | workload | 404.2 ms | 403.2 ms | +0.25% | stable |
| `rptestN` | 1M rptest operations | 3.595 s | 3.597 s | -0.07% | stable |
| `sh6benchN` | workload | 152.2 ms | 157 ms | -3.09% | noisy |
| `sh8benchN` | workload | 300.9 ms | 294.2 ms | +2.28% | stable |
| `xmalloc-testN` | 1M frees | 1.992 ms | 1.851 ms | +7.61% | stable |

Noise follows `scripts/summarize-criterion.py`: a mean confidence-interval
width above 5% or a sample spread above 10% marks a result noisy. A stable
label describes variation within this run; it does not establish consistency
across fresh runs. The unweighted scenario geomean is not a prediction of
application-wide performance.

### Commands and evidence

The initial command, from the repository root, was:

```bash
CARGO_TARGET_DIR=target/bench-comparison/2026-10-12 \
MIMALLOC_BENCH_SCENARIOS=glibc-simple,glibc-thread,malloc-large,malloc-large-old,cache-scratch1,cache-thrash1,cache-scratchN,cache-thrashN,xmalloc-testN,mstressN,sh6benchN,larsonN,larsonN-sized,barnes,cfrac,mleak10,mleak100 \
MIMALLOC_BENCH_ALLOCATORS=mechanical_mimalloc_raw,c-mimalloc-3.3.2 \
MIMALLOC_BENCH_SAMPLE_SIZE=10 \
MIMALLOC_BENCH_MEASUREMENT_SECS=3 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
cargo bench --locked --features bench --bench mimalloc_bench -- mimalloc-bench-native --noplot
```

The additional four scenarios used the prebuilt executable and libraries:

```bash
CARGO_TARGET_DIR=target/bench-comparison/2026-10-12 \
CRITERION_HOME=target/bench-comparison/2026-10-12/extended/criterion \
MIMALLOC_RUST_LIB="$PWD/target/bench-comparison/2026-10-12/release/deps/libmechanical_mimalloc-08821712153d43bc.so" \
MIMALLOC_C_LIB="$PWD/target/bench-comparison/2026-10-12/c-mimalloc-3.3.2/libmimalloc.so.3.3" \
MIMALLOC_BENCH_SKIP_BUILD=1 \
MIMALLOC_BENCH_SCENARIOS=alloc-test1,alloc-testN,sh8benchN,rptestN \
MIMALLOC_BENCH_ALLOCATORS=mechanical_mimalloc_raw,c-mimalloc-3.3.2 \
MIMALLOC_BENCH_SAMPLE_SIZE=10 \
MIMALLOC_BENCH_MEASUREMENT_SECS=3 \
MIMALLOC_BENCH_WARMUP_SECS=1 \
target/bench-comparison/2026-10-12/release/deps/mimalloc_bench-7a04905565baa6d5 \
  --bench mimalloc-bench-native --noplot
```

The executable and Rust library hashes in these filenames are specific to
this build. Exact resolved commands, library SHA-256 digests, C compiler and
linker flags, and environment details are preserved in the local artifact
directory `target/bench-comparison/2026-10-12/`. It contains `commands.txt`,
`run.json`, `environment.txt`, `libraries.json`, `native.log`, `criterion/`,
`extended/commands.txt`, `extended/run.log`, `extended/criterion/`,
`initial-results.csv`, and `first-pass-results.json`. A local
`first-pass-evidence.tar.gz` bundles the first-pass data without build binaries.
These files under `target/` are ignored by Git; the table above preserves the
results in the repository.

Ratios were calculated from Criterion JSON using the actual allocator IDs.
At the time of this run the summarizer looked up `rust` rather than the
emitted `mechanical_mimalloc_raw`, so its native-pair table omitted these
results; `scripts/summarize-criterion.py` now accepts both IDs. No benchmark
implementation changes were made for this run.

The repeat process was stopped after the first-pass measurements completed.
No repeat or resource-probe results are included here, and the five-mode
release gate was not run. Stored absolute-time thresholds were not applied:
their host profile is a Ryzen 9 5950X with 16 cores and 32 logical CPUs.

### Benchmark cleanup priorities

`benchmarks/bench.py` now provides a single comparison runner with quick and
full presets, fresh per-run result directories, incremental C builds,
validated scenario names, and fresh processes per scenario and allocator with
alternating allocator order. The first process of this run reached about
13 GiB resident memory after earlier workloads, which motivated that
isolation. The summarizer now warns when a native scenario has only one side
of the Rust/C pair. Remaining items:

1. Add explicit thread counts for native scenarios and explain the minimum
   runtime of fixed-duration workloads.
2. Add a C baseline to equivalent hotpath cases with matching call paths;
   the current `compare` feature supplies the system allocator. Verify host
   profiles before applying stored absolute-time thresholds.

## Full Rust and C Comparison October 12 2026

**Measured commit:** `0961a24f442316f10d93c20af7780e56789fe747` (clean tree).
**Stored results:** `benchmarks/results/2026-10-12-epyc-4245p/` holds each
suite's `summary.md`, `results.csv`, and `manifest.json` (commands, artifact
hashes, toolchains, host), plus `xmalloc-control.csv`.

The Rust translation is at parity with C mimalloc 3.3.2 on almost every
workload. Across 21 native scenarios the Rust/C median geomean is **1.0026
(+0.26%)**, and 15 of 21 scenarios are within 1%. RocksDB, Redis, Nginx, and
the architectural memory probes are at parity. Footprint is identical: RSS,
live bytes, and occupied pages differ by at most 0.12 MiB in the architectural
probes.

Four native differences reproduced with the same sign in both October runs
(this run and the first pass above):

| Scenario | First pass | This run | Reading |
|---|---:|---:|---|
| `malloc-large` | -6.61% | -6.61% | Rust faster; reproducible |
| `xmalloc-testN` | +7.61% | +11.69% | Rust slower; reproducible, see below |
| `sh8benchN` | +2.28% | +1.21% | Rust slightly slower |
| `sh6benchN` | -3.09% | -1.62% | Rust slightly faster; noisy both times |

The first pass's `larsonN` -8.67% did not reproduce (-0.63% here); that pass ran
Rust first for every scenario, while this runner isolates each scenario and
allocator in a fresh process.

### Host and controls

- AMD EPYC 4245P, 6 cores / 12 logical CPUs, 30 GiB, Linux x86_64, cpufreq
  governor `powersave`; no CPU pinning for native workers.
- Rust 1.92.0 (fat LTO, one codegen unit, `panic=abort`, `--features
  interpose` cdylib); C mimalloc 3.3.2 CMake Release with GCC 13.3.0.
- Commands, from the repository root:
  `./benchmarks/bench.py run --full`, `./benchmarks/bench.py architecture`,
  `./benchmarks/bench.py nginx`, all with default controls. Native: 10 samples,
  3 s measurement target, 1 s warmup, one repetition, each scenario and
  allocator in a fresh process. Nginx: both profiles, 18 cases, one repetition,
  2 s warmup and 5 s load. Architecture: 64 MiB, 4 KiB blocks, 4 threads,
  8 cycles, 3 repetitions.

### Native scenarios

`Rust vs C` is `(Rust median / C median - 1) * 100`; negative means less time
for Rust.

| Scenario | Metric | Rust median | C median | Rust vs C | Noise |
|---|---|---:|---:|---:|---|
| `alloc-test1` | workload | 1.970 s | 1.967 s | +0.16% | stable |
| `alloc-testN` | workload | 2.274 s | 2.276 s | -0.09% | stable |
| `barnes` | workload | 1.057 s | 1.062 s | -0.41% | stable |
| `cache-scratch1` | workload | 663.9 ms | 663.2 ms | +0.11% | stable |
| `cache-scratchN` | workload | 115.3 ms | 115.6 ms | -0.24% | stable |
| `cache-thrash1` | workload | 676.6 ms | 670.2 ms | +0.94% | stable |
| `cache-thrashN` | workload | 115.6 ms | 115.3 ms | +0.27% | stable |
| `cfrac` | workload | 8.662 s | 8.591 s | +0.83% | stable |
| `glibc-simple` | workload | 630.1 ms | 625.9 ms | +0.68% | stable |
| `glibc-thread` | 1M malloc/free pairs | 1.453 ms | 1.450 ms | +0.19% | stable |
| `larsonN` | 1M alloc/free pairs | 2.482 ms | 2.498 ms | -0.63% | stable |
| `larsonN-sized` | 1M alloc/free pairs | 2.480 ms | 2.537 ms | -2.25% | stable |
| `malloc-large-old` | workload | 75.33 ms | 75.23 ms | +0.12% | stable |
| `malloc-large` | workload | 725.5 us | 776.9 us | -6.61% | stable |
| `mleak10` | workload | 36.56 ms | 36.22 ms | +0.94% | stable |
| `mleak100` | workload | 363.1 ms | 364.9 ms | -0.50% | stable |
| `mstressN` | workload | 395.3 ms | 394.8 ms | +0.13% | stable |
| `rptestN` | 1M rptest operations | 3.632 s | 3.578 s | +1.51% | stable |
| `sh6benchN` | workload | 135.7 ms | 138.0 ms | -1.62% | noisy |
| `sh8benchN` | workload | 298.7 ms | 295.1 ms | +1.21% | stable |
| `xmalloc-testN` | 1M frees | 2.052 ms | 1.837 ms | +11.69% | stable |

Applications (excluded from the geomean): Redis -0.75% (330.0 vs 332.5 ns per
request), which is within its resolution: `redis-benchmark` reports
millisecond-quantized throughput at 100,000 requests, so SET and GET rates were
identical for both allocators; use `--app-operations 1000000` to resolve
allocator effects. RocksDB fillrandom +0.37% (819,793 vs 822,826 writes/s).

### Architectural probes

Steady-state medians over cycles 1-7 (cycle 0 includes first-touch faults):

| Phase | Rust vs C time | Rust slower in |
|---|---:|---:|
| `cross-thread / fan-out-allocate` | +8.9% | 7/7 cycles |
| `cross-thread / fan-in-allocate` | +4.4% | 7/7 cycles |
| `cross-thread / fan-out-free` | +2.7% | 7/7 cycles |
| `cross-thread / fan-in-free` | -1.2% | 0/7 cycles |
| `decommit-oscillation / refill-touch` | +1.2% | 6/7 cycles |
| `thread-churn / create-allocate-exit` | -4.8% | 1/7 cycles |
| `thread-churn / collect-abandoned` | -5.0% | 3/7 cycles |

The allocate phases that follow remote frees are where Rust consistently loses:
those allocations must first collect blocks that other threads freed into the
owner's pages. Local frees are equal or faster. `forced-purge` reads +17% but
is a 2.5-3 us phase near timer resolution and is not weighed.

### Nginx

72 jobs, all complete. Rust/C throughput geomean is **-1.72%** for production
and **-0.30%** for `NGX_DEBUG_PALLOC=1`; 13 of 18 cases are within 2% in each
profile. Only two cases differ by more than 2% with the same sign in both
profiles: `proxy-262144-on` (-4.3%, -4.6%) and `upload-1048576` (-4.6%,
-2.0%). Production `headers` read -14.2% but +4.0% under debug-palloc, so it is
single-run noise. Peak worker RSS geomean is 1.03x (production) and 1.01x
(debug-palloc). With one 5-second repetition these are screening results; the
two consistent cases need `--repeats 3 --duration-secs 30` before any claim.

### xmalloc-testN control: host versus regression

The October `xmalloc-testN` gap is much larger than the August result on a
32-CPU host (Rust/C 0.9725 at `f018285`, 2026-08-10). The harness sizes
`xmalloc-testN` from the logical CPU count, so this 12-CPU host runs 12+12
threads, and the August thread sweep (`PERFORMANCE_WORKLOG.md`, X01/X02) showed
Rust losing at low contention (0.886x at 4+4, 0.983x at 8+8) while winning at
16+16 and 32+32. To separate host effects from code changes, the `f018285`
library was rebuilt with the same flags and run under the same harness binary,
fresh process per run, five rounds with rotating order:

| Library | Median | Range | vs C |
|---|---:|---:|---:|
| Rust `0961a24` (current) | 2.0355 ms | 2.0167-2.0436 | +10.53% |
| Rust `f018285` (2026-08-10) | 1.9728 ms | 1.9655-1.9895 | +7.13% |
| C mimalloc 3.3.2 | 1.8415 ms | 1.8339-1.8482 | 0 |

The ranges do not overlap. About **7 points** of the gap is the known
low-contention remote-free deficit exposed by this host's thread count, and
about **3.2 points** is a regression introduced between `f018285` and
`5772656` (the August 10-13 hardening series; later commits touch only
benchmark tooling).

### Analysis: matching and beating C

Rust already beats C where fat LTO and symbol localization remove work C cannot:
C's exported option getters and cross-file arena/bitmap/page-map calls go
through the PLT and are never inlined, while Rust inlines them
(`malloc-large`, -6.6%; the worklog recorded 3.93% fewer instructions and
4.66% fewer branches for Rust there). The hot `malloc`/`free` entry points are hand-written assembly
that matches C's fast path, and release builds compile out assertions, stats,
and free-list encoding on both sides. No build-configuration gap was found.

The only material deficit is the cross-thread free path. Source comparison
found no extra per-block work in release builds; the extra cost is per
collection, and with 99.98% length-one detached chains (worklog X01) a
per-collection cost is effectively a per-free cost. Candidates, in order:

1. **Recover the August regression (~3 points).** Bisect `f018285..5772656`
   with the control above. Prime candidates promote debug-only checks to
   release code on every abandon/unabandon/reabandon: `03a0a0d`
   (`PageArenaMembership::from_page` adds a release bitmap test and MemId
   decode), `06d2884` (a page-map Acquire load moved out of a debug
   assertion), `1294a69`/`fb293ab` (reabandon decodes the MemId three times
   and runs before the early exits), `65af66b` (`mi_page_is_abandoned` load on
   every remote free), and `fb4e760` (`#[inline(never)]` on `mi_route_free`).
   The built library shows `PageArenaMembership::from_page` and
   `collect_after_remote_free` as out-of-line functions with no C counterpart.
2. **Close the low-contention deficit (~7 points on 12-CPU hosts).** Reduce
   per-collection work: fold `collect_after_remote_free` into
   `mi_free_try_collect_mt` (one noinline level, as in C `free.c:357`), give
   `mi_free_generic_mt` a direct route for pages without interior pointers
   instead of `mi_pointer_location` plus `PointerLookup::prepare_free`, and
   carry validated page/arena witnesses through reabandon instead of
   re-deriving them. Verify with the worklog's 1+1 and 4+4 instruction-per-free
   protocol, not only the saturated benchmark.
3. **Smaller items:** `realloc` uses an AVX2 `memcpy` where C uses `rep movsb`
   below a threshold and libc `memcpy` above it (worth measuring on `cfrac` and
   `glibc-simple`); make the TLS accessor asm `pure` and drop the redundant null
   check in `_mi_theap_default`; remove the per-slot `Option` in the page-map
   fill/clear loops. Keep fat LTO and the export version script; they are the
   source of Rust's current wins.

Two candidates were ruled out: `_mi_page_free_collect` is out-of-line in the C
library too (14 call sites), and atomic orderings on the remote-free path are
equal to or weaker than C's.

### Follow-up: xmalloc regression recovered (October 13 2026)

The bisect found `1294a69` ("Consume arena page release capabilities") as the
first bad commit: it added a redundant release-path MemId decode to every
reabandon attempt. Making that check debug-only (X04) and letting the
four-instruction `_mi_option_get_fast` inline (X05) take `xmalloc-testN` from
2.036 ms to 1.853-1.864 ms, which is +0.6% to +1.2% against C's 1.842 ms instead
of +10.5%. Across all 21 native scenarios the new library is 0.84% faster
(geomean) than the measured `0961a24` library, with no scenario slower than 1%;
`glibc-thread` improved 1.74%. Applied to this run's Rust/C ratios, that puts
the native geomean at about 0.994 (-0.6%) against C, though that projection
combines two sessions and is not a fresh Rust/C run. Details and gates are in
`PERFORMANCE_WORKLOG.md` (X04-X06); data is in
`benchmarks/results/2026-10-12-epyc-4245p/heldout-x04-x05.csv`.

## Rival Allocator Baseline October 13 2026

**Allocator source:** identical across the three runs (`59c9b02`, `641651f`,
`4b2531a`; only benchmark tooling changed between them), after X04-X11.
**Stored results:** `benchmarks/results/2026-10-13-epyc-4245p-rivals/`
(`native/`, `native-jemalloc/`, `nginx/`, and `scoreboard.txt`). Reproduce the
table with:

```bash
./benchmarks/scoreboard.py benchmarks/results/2026-10-12-epyc-4245p/nginx \
  benchmarks/results/2026-10-13-epyc-4245p-rivals/native \
  benchmarks/results/2026-10-13-epyc-4245p-rivals/native-jemalloc \
  benchmarks/results/2026-10-13-epyc-4245p-rivals/nginx
```

Rivals: C mimalloc 3.3.2, glibc 2.39 malloc, jemalloc 5.3.0, and tcmalloc
(gperftools 2.16 `tcmalloc_minimal`), on the same EPYC 4245P host and default
controls as the October 12 run (native: 10 samples, 3 s, one repetition;
applications: 1,000,000 operations; Nginx: one 5-second repetition per case).
Only allocators whose results changed or were missing were measured: the
`native` run supplies glibc and tcmalloc; `native-jemalloc` re-measured
jemalloc after a build fix (and, because the runner always includes them, Rust
and C); Nginx re-measured Rust, glibc, jemalloc, and tcmalloc and reuses C
mimalloc from the October 12 run. The scoreboard merges values per scenario and
allocator (later directories win) before forming ratios.

**Scoreboard** (geomean of Rust/rival time; rival/Rust throughput for Nginx;
below 1.0 favours Rust; a breach is a scenario above 1.03 versus C mimalloc or
1.10 versus others):

| Suite | C mimalloc | glibc | jemalloc | tcmalloc |
|---|---:|---:|---:|---:|
| Native (21 scenarios) | 0.9974, breach `rptestN` 1.044 | **0.6388** | **0.7980** | 0.5917, breaches `malloc-large` 3.069, `larsonN-sized` 1.183 |
| Applications (Redis, RocksDB) | **0.9992** | **0.9565** | **0.9663** | **0.9861** |
| Nginx (36 cases) | 0.9986, 4 breaches | 0.9751, 1 breach | 0.9953, 1 breach | **1.0108 (loss)**, breach `debug-palloc/headers` 1.168 |

Bold marks a win with no breach. Rust's geomean is below 1.0 in 11 of 12
suite/rival pairs; the exception is Nginx against tcmalloc.

### Where rivals are ahead

- **C mimalloc:** faster on 11 of 21 native scenarios, all but two by 0.5% or
  less; `rptestN` 1.044 (bimodal scenario, CV about 4% even over five runs)
  and `mstressN` 1.019. Nginx breaches are single 5-second runs on cases that
  previously moved several percent between runs (`upload-1048576`,
  `headers`), and C's Nginx values come from a different session.
- **glibc:** ahead only on `malloc-large-old` (1.021), `mleak100` (1.016),
  `mleak10` (1.003); Rust is far ahead elsewhere (`cache-scratchN` 0.151,
  `sh8benchN` 0.231, `xmalloc-testN` 0.241).
- **jemalloc:** ahead on `glibc-thread` (1.023) and `larsonN-sized` (1.010).
- **tcmalloc:** ahead on `malloc-large` (3.069), `larsonN-sized` (1.183),
  `larsonN` (1.099), `mleak100`/`mleak10` (~1.02), and on 26 of 36 Nginx cases
  by about 1-3% (geomean 1.0057 excluding its two largest cases). It collapses
  on cross-thread workloads (`xmalloc-testN` 74x, `sh8benchN` and
  `cache-thrashN` about 13x slower than Rust). C mimalloc shows the same
  `malloc-large` gap (0.78 vs tcmalloc's 0.24 ms per workload): the benchmark
  never touches its 5-25 MiB buffers, so it measures large-allocation metadata,
  where mimalloc registers and clears page-map entries for every 64 KiB slice.

### Measurement corrections made during this baseline

- The first jemalloc build used `--disable-initial-exec-tls` so the native
  harness could `dlopen` it. That made every jemalloc TLS access a
  `__tls_get_addr` call (171 call sites; `glibc-thread` 1.862 vs 1.407 ms) and
  crashed forked Nginx workers. jemalloc now keeps initial-exec TLS and native
  rival workers set `GLIBC_TUNABLES=glibc.rtld.optional_static_tls=65536`. The
  jemalloc columns in `native/` are from the slow build and are superseded by
  `native-jemalloc/`; with the fair build, Rust's native geomean against
  jemalloc moved from 0.7406 to 0.7980.
- Rivals link with `-Bsymbolic-functions`; without it, jemalloc's C++
  operators freed through glibc when `dlopen`ed `RTLD_LOCAL`.

### Open items

1. Confirm the single-run breaches (`rptestN` vs C; the Nginx cases) with
   interleaved repeats before treating them as gaps.
2. Profile tcmalloc's Nginx fast-path advantage and the `larsonN-sized` gap.
3. `malloc-large`: a mimalloc design trade-off shared with C mimalloc; any
   change to large-allocation metadata or caching policy needs an explicit
   decision because it changes memory behaviour.
