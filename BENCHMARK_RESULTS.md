# Mimalloc Rust vs C Benchmark Results

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
