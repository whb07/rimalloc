# Isolated Rust and C comparison

Status: complete.

Each scenario and allocator runs in a fresh process. Negative differences mean less time for Rust.
Medians aggregate repetition medians. Noise describes within-run variation; p95 is a batch statistic,
not individual allocation latency. This exploratory report does not apply release thresholds.

| Scenario | Metric | Rust median | C median | Rust vs C | jemalloc median | Rust vs jemalloc | Noise |
|---|---|---:|---:|---:|---:|---:|---|
| glibc-simple | per-workload | 625.2 ms | 623.2 ms | +0.33% | 856.5 ms | -27.00% | stable |
| malloc-large | per-workload | 737.3 us | 770.5 us | -4.30% | 858.5 us | -14.12% | stable |
| malloc-large-old | per-workload | 75.43 ms | 74.95 ms | +0.64% | 75.54 ms | -0.15% | stable |
| cache-scratch1 | per-workload | 662.2 ms | 662.5 ms | -0.04% | 662.1 ms | +0.01% | stable |
| cache-thrash1 | per-workload | 661.8 ms | 661.7 ms | +0.01% | 662.2 ms | -0.06% | stable |
| cache-scratchN | per-workload | 112.7 ms | 112.7 ms | -0.04% | 112.8 ms | -0.08% | stable |
| cache-thrashN | per-workload | 112.7 ms | 112.6 ms | +0.07% | 112.7 ms | +0.03% | stable |
| mstressN | per-workload | 397.3 ms | 390.1 ms | +1.85% | 572.9 ms | -30.65% | stable |
| sh6benchN | per-workload | 127.7 ms | 126.6 ms | +0.88% | 193.7 ms | -34.05% | stable |
| sh8benchN | per-workload | 277.9 ms | 280.9 ms | -1.06% | 543.2 ms | -48.85% | stable |
| alloc-test1 | per-workload | 1.977 s | 1.975 s | +0.11% | 2.005 s | -1.38% | stable |
| alloc-testN | per-workload | 2.293 s | 2.287 s | +0.28% | 2.381 s | -3.68% | stable |
| barnes | per-workload | 1.06 s | 1.058 s | +0.21% | 1.11 s | -4.42% | stable |
| cfrac | per-workload | 9.132 s | 9.249 s | -1.27% | 9.24 s | -1.17% | stable |
| mleak10 | per-workload | 36.68 ms | 36.72 ms | -0.10% | 45.11 ms | -18.67% | stable |
| mleak100 | per-workload | 366.4 ms | 368.9 ms | -0.67% | 453.7 ms | -19.24% | stable |
| glibc-thread | per-1M-malloc-free-pairs | 1.454 ms | 1.449 ms | +0.36% | 1.422 ms | +2.25% | stable |
| xmalloc-testN | per-1M-frees | 1.827 ms | 1.852 ms | -1.32% | 16.38 ms | -88.84% | stable |
| larsonN | per-1M-alloc-free-pairs | 2.493 ms | 2.558 ms | -2.53% | 2.635 ms | -5.36% | stable |
| larsonN-sized | per-1M-alloc-free-pairs | 2.507 ms | 2.585 ms | -3.01% | 2.482 ms | +1.00% | stable |
| rptestN | per-1M-rptest-cpu-ops | 115.8 ms | 110.9 ms | +4.39% | 124.6 ms | -7.05% | noisy |
| redis | per-redis-request | 540.3 ns | 538.5 ns | +0.32% | 538.7 ns | +0.28% | stable |
| rocksdb | per-rocksdb-write | 1.239 us | 1.245 us | -0.49% | 1.331 us | -6.89% | stable |

Native scenario geomean Rust/C: 0.9974 (-0.26%).
Native scenario geomean Rust/jemalloc: 0.7980 (-20.20%).
An unweighted scenario geomean does not predict application-wide performance.

## Repetitions

| Scenario | Round | Allocator | Median | Batch p95 | Noise |
|---|---:|---|---:|---:|---|
| glibc-simple | 1 | rimalloc_raw | 625.2 ms | 626.4 ms | stable |
| glibc-simple | 1 | c-mimalloc-3.3.2 | 623.2 ms | 624.8 ms | stable |
| glibc-simple | 1 | jemalloc | 856.5 ms | 860.6 ms | stable |
| malloc-large | 1 | rimalloc_raw | 737.3 us | 742.4 us | stable |
| malloc-large | 1 | c-mimalloc-3.3.2 | 770.5 us | 778.7 us | stable |
| malloc-large | 1 | jemalloc | 858.5 us | 859.8 us | stable |
| malloc-large-old | 1 | rimalloc_raw | 75.43 ms | 75.94 ms | stable |
| malloc-large-old | 1 | c-mimalloc-3.3.2 | 74.95 ms | 75.49 ms | stable |
| malloc-large-old | 1 | jemalloc | 75.54 ms | 75.69 ms | stable |
| cache-scratch1 | 1 | rimalloc_raw | 662.2 ms | 662.9 ms | stable |
| cache-scratch1 | 1 | c-mimalloc-3.3.2 | 662.5 ms | 663.4 ms | stable |
| cache-scratch1 | 1 | jemalloc | 662.1 ms | 663.9 ms | stable |
| cache-thrash1 | 1 | rimalloc_raw | 661.8 ms | 662.3 ms | stable |
| cache-thrash1 | 1 | c-mimalloc-3.3.2 | 661.7 ms | 662.1 ms | stable |
| cache-thrash1 | 1 | jemalloc | 662.2 ms | 662.4 ms | stable |
| cache-scratchN | 1 | rimalloc_raw | 112.7 ms | 118 ms | stable |
| cache-scratchN | 1 | c-mimalloc-3.3.2 | 112.7 ms | 114.6 ms | stable |
| cache-scratchN | 1 | jemalloc | 112.8 ms | 113 ms | stable |
| cache-thrashN | 1 | rimalloc_raw | 112.7 ms | 116.8 ms | stable |
| cache-thrashN | 1 | c-mimalloc-3.3.2 | 112.6 ms | 113.4 ms | stable |
| cache-thrashN | 1 | jemalloc | 112.7 ms | 117.2 ms | stable |
| mstressN | 1 | rimalloc_raw | 397.3 ms | 408.8 ms | stable |
| mstressN | 1 | c-mimalloc-3.3.2 | 390.1 ms | 392.7 ms | stable |
| mstressN | 1 | jemalloc | 572.9 ms | 575.9 ms | stable |
| sh6benchN | 1 | rimalloc_raw | 127.7 ms | 130.8 ms | stable |
| sh6benchN | 1 | c-mimalloc-3.3.2 | 126.6 ms | 131.8 ms | stable |
| sh6benchN | 1 | jemalloc | 193.7 ms | 197.4 ms | stable |
| sh8benchN | 1 | rimalloc_raw | 277.9 ms | 285.6 ms | stable |
| sh8benchN | 1 | c-mimalloc-3.3.2 | 280.9 ms | 285.4 ms | stable |
| sh8benchN | 1 | jemalloc | 543.2 ms | 549.1 ms | stable |
| alloc-test1 | 1 | rimalloc_raw | 1.977 s | 2.001 s | stable |
| alloc-test1 | 1 | c-mimalloc-3.3.2 | 1.975 s | 1.996 s | stable |
| alloc-test1 | 1 | jemalloc | 2.005 s | 2.024 s | stable |
| alloc-testN | 1 | rimalloc_raw | 2.293 s | 2.331 s | stable |
| alloc-testN | 1 | c-mimalloc-3.3.2 | 2.287 s | 2.328 s | stable |
| alloc-testN | 1 | jemalloc | 2.381 s | 2.412 s | stable |
| barnes | 1 | rimalloc_raw | 1.06 s | 1.064 s | stable |
| barnes | 1 | c-mimalloc-3.3.2 | 1.058 s | 1.063 s | stable |
| barnes | 1 | jemalloc | 1.11 s | 1.111 s | stable |
| cfrac | 1 | rimalloc_raw | 9.132 s | 9.164 s | stable |
| cfrac | 1 | c-mimalloc-3.3.2 | 9.249 s | 9.273 s | stable |
| cfrac | 1 | jemalloc | 9.24 s | 9.26 s | stable |
| mleak10 | 1 | rimalloc_raw | 36.68 ms | 36.9 ms | stable |
| mleak10 | 1 | c-mimalloc-3.3.2 | 36.72 ms | 36.95 ms | stable |
| mleak10 | 1 | jemalloc | 45.11 ms | 45.36 ms | stable |
| mleak100 | 1 | rimalloc_raw | 366.4 ms | 369.6 ms | stable |
| mleak100 | 1 | c-mimalloc-3.3.2 | 368.9 ms | 371.7 ms | stable |
| mleak100 | 1 | jemalloc | 453.7 ms | 463.1 ms | stable |
| glibc-thread | 1 | rimalloc_raw | 1.454 ms | 1.477 ms | stable |
| glibc-thread | 1 | c-mimalloc-3.3.2 | 1.449 ms | 1.467 ms | stable |
| glibc-thread | 1 | jemalloc | 1.422 ms | 1.431 ms | stable |
| xmalloc-testN | 1 | rimalloc_raw | 1.827 ms | 1.83 ms | stable |
| xmalloc-testN | 1 | c-mimalloc-3.3.2 | 1.852 ms | 1.853 ms | stable |
| xmalloc-testN | 1 | jemalloc | 16.38 ms | 16.97 ms | stable |
| larsonN | 1 | rimalloc_raw | 2.493 ms | 2.511 ms | stable |
| larsonN | 1 | c-mimalloc-3.3.2 | 2.558 ms | 2.582 ms | stable |
| larsonN | 1 | jemalloc | 2.635 ms | 2.659 ms | stable |
| larsonN-sized | 1 | rimalloc_raw | 2.507 ms | 2.525 ms | stable |
| larsonN-sized | 1 | c-mimalloc-3.3.2 | 2.585 ms | 2.593 ms | stable |
| larsonN-sized | 1 | jemalloc | 2.482 ms | 2.499 ms | stable |
| rptestN | 1 | rimalloc_raw | 115.8 ms | 133.9 ms | noisy |
| rptestN | 1 | c-mimalloc-3.3.2 | 110.9 ms | 120.8 ms | noisy |
| rptestN | 1 | jemalloc | 124.6 ms | 125.7 ms | stable |
| redis | 1 | rimalloc_raw | 540.3 ns | 554 ns | stable |
| redis | 1 | c-mimalloc-3.3.2 | 538.5 ns | 544 ns | stable |
| redis | 1 | jemalloc | 538.7 ns | 565 ns | stable |
| rocksdb | 1 | rimalloc_raw | 1.239 us | 1.251 us | stable |
| rocksdb | 1 | c-mimalloc-3.3.2 | 1.245 us | 1.255 us | stable |
| rocksdb | 1 | jemalloc | 1.331 us | 1.351 us | stable |

## Application measurements

Redis's normalized time averages the reciprocal SET and GET throughputs with equal request counts. RocksDB uses reciprocal aggregate fillrandom throughput. Applications are excluded from the native geomean. Application p95 in the repetition table describes whole-sample throughput variation. Redis client latency percentiles are shown separately below. Fixed work counts apply; Criterion measurement/warmup seconds do not apply to applications.

| Scenario / command | Round | Allocator | Median ops/s | Redis client p95 ms |
|---|---:|---|---:|---:|
| redis / SET | 1 | rimalloc_raw | 1624696.44 | 0.863 |
| redis / GET | 1 | rimalloc_raw | 2148230.12 | 0.399 |
| redis / SET | 1 | c-mimalloc-3.3.2 | 1635324.12 | 0.855 |
| redis / GET | 1 | c-mimalloc-3.3.2 | 2150537.50 | 0.399 |
| redis / SET | 1 | jemalloc | 1661129.62 | 0.759 |
| redis / GET | 1 | jemalloc | 2103051.75 | 0.407 |
| rocksdb / fillrandom | 1 | rimalloc_raw | 807113.00 | — |
| rocksdb / fillrandom | 1 | c-mimalloc-3.3.2 | 803155.50 | — |
| rocksdb / fillrandom | 1 | jemalloc | 751532.50 | — |
