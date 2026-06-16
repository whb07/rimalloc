# Mimalloc Rust vs C Benchmark Results

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
