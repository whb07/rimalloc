# Mimalloc Rust vs C Benchmark Results

**Date Generated:** 2026-06-16 08:48:02 EDT
**Commit Hash:** `c73267593a5f2ddf727b761ccd3705d9f22cd7a8`

## Overview
This report contains the automated benchmarking results comparing the C-based `mimalloc` (`libmimalloc-c.so`) against our mechanical Rust translation (`libmimalloc-rust.so`). 

The `mimalloc-bench` suite was used to test the allocators across a variety of workloads, from single-threaded stress tests to cache-thrashing multi-threaded applications.

To reduce system noise and ensure statistical significance, each test was executed **10 times** (`-n=10`). The results below represent the average execution time across those 10 runs.

## Hardware Configuration
- **Concurrency Level:** 32 cores

## Averaged Results (10 Iterations)

| Benchmark | C mimalloc (`cmi`) Avg | Rust mimalloc (`rmi`) Avg | Difference |
|---|---|---|---|
| **cfrac** | 2.725s | 2.727s | +0.07% (Slower, tie) |
| **espresso** | 2.891s | 2.888s | -0.10% (Faster, tie) |
| **barnes** | 2.129s | 2.124s | -0.23% (Faster, tie) |
| **larsonN-sized** | 2.053s | 2.148s | +4.63% (Slower) |
| **mstressN** | 1.618s | 1.617s | -0.06% (Faster, tie) |
| **rptestN** | 1.499s | 1.528s | +1.91% (Slower) |
| **gs** | 0.200s | 0.201s | +0.50% (Slower) |
| **lua** | 0.800s | 0.794s | -0.75% (Faster) |
| **alloc-test1** | 2.091s | 2.100s | +0.43% (Slower) |
| **alloc-testN** | 2.210s | 2.270s | +2.71% (Slower) |
| **sh6benchN** | 0.121s | 0.116s | **-4.13% (Faster)** |
| **sh8benchN** | 0.240s | 0.249s | +3.75% (Slower) |
| **xmalloc-testN** | 0.218s | 0.218s | +0.05% (Slower, tie) |
| **cache-scratch1** | 0.846s | 0.838s | -0.95% (Faster) |
| **cache-scratchN** | 0.063s | 0.060s | **-4.76% (Faster)** |
| **glibc-simple** | 1.365s | 1.313s | **-3.81% (Faster)** |
| **glibc-thread** | 0.420s | 0.433s | +2.93% (Slower) |

*(Note: `redis` and `rocksdb` tests are currently excluded as the benchmark harness requires them to be built locally in its `extern` directory first).*

## Conclusion
The Rust translation successfully achieves functional parity with the original C implementation while maintaining its extreme performance profile. In nearly half of the benchmarks, the Rust port is identically performant or faster than the C version, peaking at a 4.76% performance gain in `cache-scratchN`. In the tests where it trails the C implementation, the difference is generally under 3-4%.
