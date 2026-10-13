# Isolated Rust and C comparison

Status: complete.

Each scenario and allocator runs in a fresh process. Negative differences mean less time for Rust.
Medians aggregate repetition medians. Noise describes within-run variation; p95 is a batch statistic,
not individual allocation latency. This exploratory report does not apply release thresholds.

| Scenario | Metric | Rust median | C median | Rust vs C | glibc median | Rust vs glibc | jemalloc median | Rust vs jemalloc | tcmalloc median | Rust vs tcmalloc | Noise |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| glibc-simple | per-workload | 625.1 ms | 624.9 ms | +0.03% | 1.557 s | -59.85% | 1.015 s | -38.43% | 679 ms | -7.94% | stable |
| malloc-large | per-workload | 734.3 us | 781.6 us | -6.05% | 1000 us | -26.57% | 910.6 us | -19.36% | 240.2 us | +205.69% | stable |
| malloc-large-old | per-workload | 75.87 ms | 75.65 ms | +0.29% | 73.85 ms | +2.74% | 75.35 ms | +0.69% | 74.89 ms | +1.31% | stable |
| cache-scratch1 | per-workload | 662.4 ms | 662.7 ms | -0.05% | 663.4 ms | -0.15% | 662.6 ms | -0.03% | 662.6 ms | -0.03% | stable |
| cache-thrash1 | per-workload | 661.8 ms | 661.8 ms | +0.00% | 662.4 ms | -0.09% | 662.3 ms | -0.08% | 662.2 ms | -0.06% | stable |
| cache-scratchN | per-workload | 112.7 ms | 112.8 ms | -0.10% | 748 ms | -84.93% | 112.7 ms | -0.01% | 1.581 s | -92.87% | noisy |
| cache-thrashN | per-workload | 112.5 ms | 112.5 ms | -0.05% | 112.6 ms | -0.16% | 112.6 ms | -0.15% | 1.44 s | -92.19% | noisy |
| mstressN | per-workload | 392 ms | 395.8 ms | -0.95% | 498.8 ms | -21.42% | 580.1 ms | -32.42% | 414.4 ms | -5.40% | stable |
| sh6benchN | per-workload | 130.5 ms | 123.9 ms | +5.27% | 412.9 ms | -68.40% | 247 ms | -47.19% | 146.4 ms | -10.92% | noisy |
| sh8benchN | per-workload | 281.1 ms | 281.5 ms | -0.13% | 1.216 s | -76.87% | 650.8 ms | -56.80% | 3.545 s | -92.07% | stable |
| alloc-test1 | per-workload | 1.976 s | 1.974 s | +0.12% | 2.364 s | -16.41% | 2.07 s | -4.56% | 1.976 s | -0.01% | stable |
| alloc-testN | per-workload | 2.29 s | 2.283 s | +0.32% | 2.908 s | -21.26% | 2.572 s | -10.94% | 2.393 s | -4.29% | stable |
| barnes | per-workload | 1.058 s | 1.06 s | -0.17% | 1.108 s | -4.48% | 1.106 s | -4.33% | 1.105 s | -4.18% | stable |
| cfrac | per-workload | 9.001 s | 9.027 s | -0.29% | 9.365 s | -3.89% | 9.576 s | -6.01% | 9.078 s | -0.85% | stable |
| mleak10 | per-workload | 35.77 ms | 36.87 ms | -2.97% | 36.57 ms | -2.17% | 44.92 ms | -20.36% | 35.9 ms | -0.35% | stable |
| mleak100 | per-workload | 365.6 ms | 367.8 ms | -0.61% | 360.5 ms | +1.39% | 430 ms | -14.98% | 356.8 ms | +2.46% | stable |
| glibc-thread | per-1M-malloc-free-pairs | 1.44 ms | 1.456 ms | -1.14% | 2.364 ms | -39.08% | 1.885 ms | -23.63% | 1.468 ms | -1.92% | stable |
| xmalloc-testN | per-1M-frees | 1.825 ms | 1.858 ms | -1.78% | 7.571 ms | -75.90% | 16.86 ms | -89.18% | 134.7 ms | -98.65% | stable |
| larsonN | per-1M-alloc-free-pairs | 2.505 ms | 2.565 ms | -2.34% | 3.755 ms | -33.28% | 3.212 ms | -22.01% | 2.268 ms | +10.46% | noisy |
| larsonN-sized | per-1M-alloc-free-pairs | 2.523 ms | 2.579 ms | -2.17% | 3.737 ms | -32.49% | 3.063 ms | -17.63% | 2.119 ms | +19.07% | stable |
| rptestN | per-1M-rptest-cpu-ops | 109 ms | 113.2 ms | -3.67% | 150.3 ms | -27.46% | 126.3 ms | -13.69% | 124.9 ms | -12.71% | stable |
| redis | per-redis-request | 538.5 ns | 535.5 ns | +0.56% | 544 ns | -1.01% | 551.7 ns | -2.40% | 521 ns | +3.36% | stable |
| rocksdb | per-rocksdb-write | 1.252 us | 1.251 us | +0.10% | 1.345 us | -6.90% | 1.333 us | -6.07% | 1.321 us | -5.22% | stable |

Native scenario geomean Rust/C: 0.9920 (-0.80%).
Native scenario geomean Rust/glibc: 0.6362 (-36.38%).
Native scenario geomean Rust/jemalloc: 0.7406 (-25.94%).
Native scenario geomean Rust/tcmalloc: 0.5892 (-41.08%).
An unweighted scenario geomean does not predict application-wide performance.

## Repetitions

| Scenario | Round | Allocator | Median | Batch p95 | Noise |
|---|---:|---|---:|---:|---|
| glibc-simple | 1 | mechanical_mimalloc_raw | 625.1 ms | 625.9 ms | stable |
| glibc-simple | 1 | c-mimalloc-3.3.2 | 624.9 ms | 628.1 ms | stable |
| glibc-simple | 1 | glibc | 1.557 s | 1.558 s | stable |
| glibc-simple | 1 | jemalloc | 1.015 s | 1.019 s | stable |
| glibc-simple | 1 | tcmalloc | 679 ms | 681 ms | stable |
| malloc-large | 1 | mechanical_mimalloc_raw | 734.3 us | 741.2 us | stable |
| malloc-large | 1 | c-mimalloc-3.3.2 | 781.6 us | 786.7 us | stable |
| malloc-large | 1 | glibc | 1000 us | 1.028 ms | stable |
| malloc-large | 1 | jemalloc | 910.6 us | 911.1 us | stable |
| malloc-large | 1 | tcmalloc | 240.2 us | 240.5 us | stable |
| malloc-large-old | 1 | mechanical_mimalloc_raw | 75.87 ms | 76.18 ms | stable |
| malloc-large-old | 1 | c-mimalloc-3.3.2 | 75.65 ms | 75.94 ms | stable |
| malloc-large-old | 1 | glibc | 73.85 ms | 74.02 ms | stable |
| malloc-large-old | 1 | jemalloc | 75.35 ms | 75.72 ms | stable |
| malloc-large-old | 1 | tcmalloc | 74.89 ms | 75.27 ms | stable |
| cache-scratch1 | 1 | mechanical_mimalloc_raw | 662.4 ms | 662.7 ms | stable |
| cache-scratch1 | 1 | c-mimalloc-3.3.2 | 662.7 ms | 664.3 ms | stable |
| cache-scratch1 | 1 | glibc | 663.4 ms | 665.8 ms | stable |
| cache-scratch1 | 1 | jemalloc | 662.6 ms | 663.9 ms | stable |
| cache-scratch1 | 1 | tcmalloc | 662.6 ms | 663.1 ms | stable |
| cache-thrash1 | 1 | mechanical_mimalloc_raw | 661.8 ms | 662.1 ms | stable |
| cache-thrash1 | 1 | c-mimalloc-3.3.2 | 661.8 ms | 662.2 ms | stable |
| cache-thrash1 | 1 | glibc | 662.4 ms | 663.4 ms | stable |
| cache-thrash1 | 1 | jemalloc | 662.3 ms | 663 ms | stable |
| cache-thrash1 | 1 | tcmalloc | 662.2 ms | 662.7 ms | stable |
| cache-scratchN | 1 | mechanical_mimalloc_raw | 112.7 ms | 116.6 ms | stable |
| cache-scratchN | 1 | c-mimalloc-3.3.2 | 112.8 ms | 120.1 ms | stable |
| cache-scratchN | 1 | glibc | 748 ms | 1.009 s | noisy |
| cache-scratchN | 1 | jemalloc | 112.7 ms | 117.2 ms | stable |
| cache-scratchN | 1 | tcmalloc | 1.581 s | 1.764 s | noisy |
| cache-thrashN | 1 | mechanical_mimalloc_raw | 112.5 ms | 114.8 ms | stable |
| cache-thrashN | 1 | c-mimalloc-3.3.2 | 112.5 ms | 112.7 ms | stable |
| cache-thrashN | 1 | glibc | 112.6 ms | 119.5 ms | stable |
| cache-thrashN | 1 | jemalloc | 112.6 ms | 113.4 ms | stable |
| cache-thrashN | 1 | tcmalloc | 1.44 s | 1.511 s | noisy |
| mstressN | 1 | mechanical_mimalloc_raw | 392 ms | 407 ms | stable |
| mstressN | 1 | c-mimalloc-3.3.2 | 395.8 ms | 408.1 ms | stable |
| mstressN | 1 | glibc | 498.8 ms | 512.3 ms | stable |
| mstressN | 1 | jemalloc | 580.1 ms | 586.4 ms | stable |
| mstressN | 1 | tcmalloc | 414.4 ms | 417 ms | stable |
| sh6benchN | 1 | mechanical_mimalloc_raw | 130.5 ms | 159.4 ms | noisy |
| sh6benchN | 1 | c-mimalloc-3.3.2 | 123.9 ms | 132.8 ms | noisy |
| sh6benchN | 1 | glibc | 412.9 ms | 433.4 ms | stable |
| sh6benchN | 1 | jemalloc | 247 ms | 253.8 ms | stable |
| sh6benchN | 1 | tcmalloc | 146.4 ms | 149.7 ms | stable |
| sh8benchN | 1 | mechanical_mimalloc_raw | 281.1 ms | 284.8 ms | stable |
| sh8benchN | 1 | c-mimalloc-3.3.2 | 281.5 ms | 284.4 ms | stable |
| sh8benchN | 1 | glibc | 1.216 s | 1.243 s | stable |
| sh8benchN | 1 | jemalloc | 650.8 ms | 656.1 ms | stable |
| sh8benchN | 1 | tcmalloc | 3.545 s | 3.583 s | stable |
| alloc-test1 | 1 | mechanical_mimalloc_raw | 1.976 s | 1.99 s | stable |
| alloc-test1 | 1 | c-mimalloc-3.3.2 | 1.974 s | 1.98 s | stable |
| alloc-test1 | 1 | glibc | 2.364 s | 2.371 s | stable |
| alloc-test1 | 1 | jemalloc | 2.07 s | 2.078 s | stable |
| alloc-test1 | 1 | tcmalloc | 1.976 s | 1.993 s | stable |
| alloc-testN | 1 | mechanical_mimalloc_raw | 2.29 s | 2.324 s | stable |
| alloc-testN | 1 | c-mimalloc-3.3.2 | 2.283 s | 2.3 s | stable |
| alloc-testN | 1 | glibc | 2.908 s | 2.927 s | stable |
| alloc-testN | 1 | jemalloc | 2.572 s | 2.59 s | stable |
| alloc-testN | 1 | tcmalloc | 2.393 s | 2.423 s | stable |
| barnes | 1 | mechanical_mimalloc_raw | 1.058 s | 1.066 s | stable |
| barnes | 1 | c-mimalloc-3.3.2 | 1.06 s | 1.063 s | stable |
| barnes | 1 | glibc | 1.108 s | 1.112 s | stable |
| barnes | 1 | jemalloc | 1.106 s | 1.11 s | stable |
| barnes | 1 | tcmalloc | 1.105 s | 1.107 s | stable |
| cfrac | 1 | mechanical_mimalloc_raw | 9.001 s | 9.021 s | stable |
| cfrac | 1 | c-mimalloc-3.3.2 | 9.027 s | 9.04 s | stable |
| cfrac | 1 | glibc | 9.365 s | 9.371 s | stable |
| cfrac | 1 | jemalloc | 9.576 s | 9.632 s | stable |
| cfrac | 1 | tcmalloc | 9.078 s | 9.279 s | stable |
| mleak10 | 1 | mechanical_mimalloc_raw | 35.77 ms | 35.97 ms | stable |
| mleak10 | 1 | c-mimalloc-3.3.2 | 36.87 ms | 37.13 ms | stable |
| mleak10 | 1 | glibc | 36.57 ms | 36.94 ms | stable |
| mleak10 | 1 | jemalloc | 44.92 ms | 45.3 ms | stable |
| mleak10 | 1 | tcmalloc | 35.9 ms | 36.29 ms | stable |
| mleak100 | 1 | mechanical_mimalloc_raw | 365.6 ms | 369.9 ms | stable |
| mleak100 | 1 | c-mimalloc-3.3.2 | 367.8 ms | 370.8 ms | stable |
| mleak100 | 1 | glibc | 360.5 ms | 363.7 ms | stable |
| mleak100 | 1 | jemalloc | 430 ms | 432.8 ms | stable |
| mleak100 | 1 | tcmalloc | 356.8 ms | 360.4 ms | stable |
| glibc-thread | 1 | mechanical_mimalloc_raw | 1.44 ms | 1.463 ms | stable |
| glibc-thread | 1 | c-mimalloc-3.3.2 | 1.456 ms | 1.473 ms | stable |
| glibc-thread | 1 | glibc | 2.364 ms | 2.384 ms | stable |
| glibc-thread | 1 | jemalloc | 1.885 ms | 1.894 ms | stable |
| glibc-thread | 1 | tcmalloc | 1.468 ms | 1.471 ms | stable |
| xmalloc-testN | 1 | mechanical_mimalloc_raw | 1.825 ms | 1.828 ms | stable |
| xmalloc-testN | 1 | c-mimalloc-3.3.2 | 1.858 ms | 1.86 ms | stable |
| xmalloc-testN | 1 | glibc | 7.571 ms | 7.577 ms | stable |
| xmalloc-testN | 1 | jemalloc | 16.86 ms | 17.44 ms | stable |
| xmalloc-testN | 1 | tcmalloc | 134.7 ms | 134.9 ms | stable |
| larsonN | 1 | mechanical_mimalloc_raw | 2.505 ms | 2.521 ms | stable |
| larsonN | 1 | c-mimalloc-3.3.2 | 2.565 ms | 2.581 ms | stable |
| larsonN | 1 | glibc | 3.755 ms | 4.029 ms | noisy |
| larsonN | 1 | jemalloc | 3.212 ms | 3.242 ms | stable |
| larsonN | 1 | tcmalloc | 2.268 ms | 2.28 ms | stable |
| larsonN-sized | 1 | mechanical_mimalloc_raw | 2.523 ms | 2.532 ms | stable |
| larsonN-sized | 1 | c-mimalloc-3.3.2 | 2.579 ms | 2.59 ms | stable |
| larsonN-sized | 1 | glibc | 3.737 ms | 3.768 ms | stable |
| larsonN-sized | 1 | jemalloc | 3.063 ms | 3.135 ms | stable |
| larsonN-sized | 1 | tcmalloc | 2.119 ms | 2.125 ms | stable |
| rptestN | 1 | mechanical_mimalloc_raw | 109 ms | 113.2 ms | stable |
| rptestN | 1 | c-mimalloc-3.3.2 | 113.2 ms | 118.1 ms | stable |
| rptestN | 1 | glibc | 150.3 ms | 151.4 ms | stable |
| rptestN | 1 | jemalloc | 126.3 ms | 127.2 ms | stable |
| rptestN | 1 | tcmalloc | 124.9 ms | 128.3 ms | stable |
| redis | 1 | mechanical_mimalloc_raw | 538.5 ns | 551.5 ns | stable |
| redis | 1 | c-mimalloc-3.3.2 | 535.5 ns | 540 ns | stable |
| redis | 1 | glibc | 544 ns | 552.5 ns | stable |
| redis | 1 | jemalloc | 551.7 ns | 559.5 ns | stable |
| redis | 1 | tcmalloc | 521 ns | 530 ns | stable |
| rocksdb | 1 | mechanical_mimalloc_raw | 1.252 us | 1.264 us | stable |
| rocksdb | 1 | c-mimalloc-3.3.2 | 1.251 us | 1.269 us | stable |
| rocksdb | 1 | glibc | 1.345 us | 1.362 us | stable |
| rocksdb | 1 | jemalloc | 1.333 us | 1.349 us | stable |
| rocksdb | 1 | tcmalloc | 1.321 us | 1.331 us | stable |

## Application measurements

Redis's normalized time averages the reciprocal SET and GET throughputs with equal request counts. RocksDB uses reciprocal aggregate fillrandom throughput. Applications are excluded from the native geomean. Application p95 in the repetition table describes whole-sample throughput variation. Redis client latency percentiles are shown separately below. Fixed work counts apply; Criterion measurement/warmup seconds do not apply to applications.

| Scenario / command | Round | Allocator | Median ops/s | Redis client p95 ms |
|---|---:|---|---:|---:|
| redis / SET | 1 | mechanical_mimalloc_raw | 1636665.62 | 0.843 |
| redis / GET | 1 | mechanical_mimalloc_raw | 2155172.50 | 0.399 |
| redis / SET | 1 | c-mimalloc-3.3.2 | 1642036.00 | 0.847 |
| redis / GET | 1 | c-mimalloc-3.3.2 | 2162164.62 | 0.399 |
| redis / SET | 1 | glibc | 1582282.44 | 0.835 |
| redis / GET | 1 | glibc | 2192982.50 | 0.391 |
| redis / SET | 1 | jemalloc | 1616815.94 | 0.775 |
| redis / GET | 1 | jemalloc | 2072541.13 | 0.415 |
| redis / SET | 1 | tcmalloc | 1763690.38 | 0.719 |
| redis / GET | 1 | tcmalloc | 2098638.19 | 0.407 |
| rocksdb / fillrandom | 1 | mechanical_mimalloc_raw | 798589.00 | — |
| rocksdb / fillrandom | 1 | c-mimalloc-3.3.2 | 799375.00 | — |
| rocksdb / fillrandom | 1 | glibc | 743505.00 | — |
| rocksdb / fillrandom | 1 | jemalloc | 750087.00 | — |
| rocksdb / fillrandom | 1 | tcmalloc | 756932.00 | — |
