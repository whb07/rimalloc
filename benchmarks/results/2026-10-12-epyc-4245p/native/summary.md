# Isolated Rust and C comparison

Status: complete.

Each scenario and allocator runs in a fresh process. Negative differences mean less time for Rust.
Medians aggregate repetition medians. Noise describes within-run variation; p95 is a batch statistic,
not individual allocation latency. This exploratory report does not apply release thresholds.

| Scenario | Metric | Rust median | C median | Rust vs C | Noise |
|---|---|---:|---:|---:|---|
| glibc-simple | per-workload | 630.1 ms | 625.9 ms | +0.68% | stable |
| malloc-large | per-workload | 725.5 us | 776.9 us | -6.61% | stable |
| malloc-large-old | per-workload | 75.33 ms | 75.23 ms | +0.12% | stable |
| cache-scratch1 | per-workload | 663.9 ms | 663.2 ms | +0.11% | stable |
| cache-thrash1 | per-workload | 676.6 ms | 670.2 ms | +0.94% | stable |
| cache-scratchN | per-workload | 115.3 ms | 115.6 ms | -0.24% | stable |
| cache-thrashN | per-workload | 115.6 ms | 115.3 ms | +0.27% | stable |
| mstressN | per-workload | 395.3 ms | 394.8 ms | +0.13% | stable |
| sh6benchN | per-workload | 135.7 ms | 138 ms | -1.62% | noisy |
| sh8benchN | per-workload | 298.7 ms | 295.1 ms | +1.21% | stable |
| alloc-test1 | per-workload | 1.97 s | 1.967 s | +0.16% | stable |
| alloc-testN | per-workload | 2.274 s | 2.276 s | -0.09% | stable |
| barnes | per-workload | 1.057 s | 1.062 s | -0.41% | stable |
| cfrac | per-workload | 8.662 s | 8.591 s | +0.83% | stable |
| mleak10 | per-workload | 36.56 ms | 36.22 ms | +0.94% | stable |
| mleak100 | per-workload | 363.1 ms | 364.9 ms | -0.50% | stable |
| glibc-thread | per-1M-malloc-free-pairs | 1.453 ms | 1.45 ms | +0.19% | stable |
| xmalloc-testN | per-1M-frees | 2.052 ms | 1.837 ms | +11.69% | stable |
| larsonN | per-1M-alloc-free-pairs | 2.482 ms | 2.498 ms | -0.63% | stable |
| larsonN-sized | per-1M-alloc-free-pairs | 2.48 ms | 2.537 ms | -2.25% | stable |
| rptestN | per-1M-rptest-ops | 3.632 s | 3.578 s | +1.51% | stable |
| redis | per-redis-request | 330 ns | 332.5 ns | -0.75% | noisy |
| rocksdb | per-rocksdb-write | 1.22 us | 1.215 us | +0.37% | stable |

Native scenario geomean Rust/C: 1.0026 (+0.26%).
An unweighted scenario geomean does not predict application-wide performance.

## Repetitions

| Scenario | Round | Allocator | Median | Batch p95 | Noise |
|---|---:|---|---:|---:|---|
| glibc-simple | 1 | rimalloc_raw | 630.1 ms | 631.9 ms | stable |
| glibc-simple | 1 | c-mimalloc-3.3.2 | 625.9 ms | 628 ms | stable |
| malloc-large | 1 | rimalloc_raw | 725.5 us | 734.6 us | stable |
| malloc-large | 1 | c-mimalloc-3.3.2 | 776.9 us | 784.4 us | stable |
| malloc-large-old | 1 | rimalloc_raw | 75.33 ms | 75.99 ms | stable |
| malloc-large-old | 1 | c-mimalloc-3.3.2 | 75.23 ms | 75.99 ms | stable |
| cache-scratch1 | 1 | rimalloc_raw | 663.9 ms | 665.1 ms | stable |
| cache-scratch1 | 1 | c-mimalloc-3.3.2 | 663.2 ms | 664 ms | stable |
| cache-thrash1 | 1 | rimalloc_raw | 676.6 ms | 689.5 ms | stable |
| cache-thrash1 | 1 | c-mimalloc-3.3.2 | 670.2 ms | 679.4 ms | stable |
| cache-scratchN | 1 | rimalloc_raw | 115.3 ms | 119.9 ms | stable |
| cache-scratchN | 1 | c-mimalloc-3.3.2 | 115.6 ms | 118.9 ms | stable |
| cache-thrashN | 1 | rimalloc_raw | 115.6 ms | 123 ms | stable |
| cache-thrashN | 1 | c-mimalloc-3.3.2 | 115.3 ms | 118.3 ms | stable |
| mstressN | 1 | rimalloc_raw | 395.3 ms | 401.1 ms | stable |
| mstressN | 1 | c-mimalloc-3.3.2 | 394.8 ms | 399.6 ms | stable |
| sh6benchN | 1 | rimalloc_raw | 135.7 ms | 194.6 ms | noisy |
| sh6benchN | 1 | c-mimalloc-3.3.2 | 138 ms | 188.7 ms | noisy |
| sh8benchN | 1 | rimalloc_raw | 298.7 ms | 302.5 ms | stable |
| sh8benchN | 1 | c-mimalloc-3.3.2 | 295.1 ms | 300.1 ms | stable |
| alloc-test1 | 1 | rimalloc_raw | 1.97 s | 1.995 s | stable |
| alloc-test1 | 1 | c-mimalloc-3.3.2 | 1.967 s | 1.986 s | stable |
| alloc-testN | 1 | rimalloc_raw | 2.274 s | 2.305 s | stable |
| alloc-testN | 1 | c-mimalloc-3.3.2 | 2.276 s | 2.316 s | stable |
| barnes | 1 | rimalloc_raw | 1.057 s | 1.068 s | stable |
| barnes | 1 | c-mimalloc-3.3.2 | 1.062 s | 1.064 s | stable |
| cfrac | 1 | rimalloc_raw | 8.662 s | 8.675 s | stable |
| cfrac | 1 | c-mimalloc-3.3.2 | 8.591 s | 8.635 s | stable |
| mleak10 | 1 | rimalloc_raw | 36.56 ms | 36.76 ms | stable |
| mleak10 | 1 | c-mimalloc-3.3.2 | 36.22 ms | 36.55 ms | stable |
| mleak100 | 1 | rimalloc_raw | 363.1 ms | 367.1 ms | stable |
| mleak100 | 1 | c-mimalloc-3.3.2 | 364.9 ms | 366.9 ms | stable |
| glibc-thread | 1 | rimalloc_raw | 1.453 ms | 1.489 ms | stable |
| glibc-thread | 1 | c-mimalloc-3.3.2 | 1.45 ms | 1.469 ms | stable |
| xmalloc-testN | 1 | rimalloc_raw | 2.052 ms | 2.057 ms | stable |
| xmalloc-testN | 1 | c-mimalloc-3.3.2 | 1.837 ms | 1.839 ms | stable |
| larsonN | 1 | rimalloc_raw | 2.482 ms | 2.508 ms | stable |
| larsonN | 1 | c-mimalloc-3.3.2 | 2.498 ms | 2.534 ms | stable |
| larsonN-sized | 1 | rimalloc_raw | 2.48 ms | 2.516 ms | stable |
| larsonN-sized | 1 | c-mimalloc-3.3.2 | 2.537 ms | 2.549 ms | stable |
| rptestN | 1 | rimalloc_raw | 3.632 s | 3.726 s | stable |
| rptestN | 1 | c-mimalloc-3.3.2 | 3.578 s | 3.718 s | stable |
| redis | 1 | rimalloc_raw | 330 ns | 345 ns | noisy |
| redis | 1 | c-mimalloc-3.3.2 | 332.5 ns | 335 ns | stable |
| rocksdb | 1 | rimalloc_raw | 1.22 us | 1.238 us | stable |
| rocksdb | 1 | c-mimalloc-3.3.2 | 1.215 us | 1.233 us | stable |

## Application measurements

Redis's normalized time averages the reciprocal SET and GET throughputs with equal request counts. RocksDB uses reciprocal aggregate fillrandom throughput. Applications are excluded from the native geomean. Application p95 in the repetition table describes whole-sample throughput variation. Redis client latency percentiles are shown separately below. Fixed work counts apply; Criterion measurement/warmup seconds do not apply to applications.

| Scenario / command | Round | Allocator | Median ops/s | Redis client p95 ms |
|---|---:|---|---:|---:|
| redis / SET | 1 | rimalloc_raw | 2631579.00 | 0.375 |
| redis / GET | 1 | rimalloc_raw | 3571428.50 | 0.255 |
| redis / SET | 1 | c-mimalloc-3.3.2 | 2631579.00 | 0.367 |
| redis / GET | 1 | c-mimalloc-3.3.2 | 3571428.50 | 0.255 |
| rocksdb / fillrandom | 1 | rimalloc_raw | 819793.00 | — |
| rocksdb / fillrandom | 1 | c-mimalloc-3.3.2 | 822825.50 | — |
