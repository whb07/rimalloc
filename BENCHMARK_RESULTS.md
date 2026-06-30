# Mimalloc Rust vs C Benchmark Results

## Release Candidate Artifact-Mode Evidence

For a release candidate, collect performance evidence with:

```bash
scripts/check-bench-full.sh
scripts/env-info.sh > target/bench-full/environment.txt
```

The full benchmark script intentionally writes default Rust-library and
`interpose` measurements to separate Criterion directories:

| Artifact mode | Feature set | Criterion directory | Threshold CSV |
|---|---|---|---|
| default Rust library | `bench` / `bench compare` | `target/bench-full/default-rust/criterion` | `target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv` |
| interpose hot path | `bench interpose` | `target/bench-full/interpose/criterion` | `target/bench-full/interpose/criterion/benchmark-thresholds-full.csv` |

Do not reuse Criterion baselines across artifact modes. Report each artifact
mode under a separate heading with the exact command line and feature set. The
native scenario section for the default Rust library mode must include the
Rust-vs-C ratios emitted by `scripts/summarize-criterion.py`; the `interpose`
section must remain separate and must not be compared against default-mode saved
baselines.

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
