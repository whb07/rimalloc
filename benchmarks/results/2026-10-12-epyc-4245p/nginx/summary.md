# Nginx allocator comparison (complete)

Independent run medians; HTTP p99 is wrk's closed-loop latency histogram, not allocator p99. Peak RSS sums worker RSS (shared pages can be counted more than once). Allocator traffic comes from a separate instrumented preflight. CPU time is whole-worker CPU time, not allocator-only cycles. No rankings are combined between builds; this local load generator does not establish maximum sustainable throughput.

## production

| Case | Allocator | Requests/s | HTTP p99 ms | Peak worker RSS MiB | Allocations/request | Requested bytes/request | Idle final RSS MiB |
|---|---|---:|---:|---:|---:|---:|---:|
| static | rust | 574102 | 0.671 | 30.45 | 3.002 | 9220 | — |
| static | mimalloc | 576124 | 0.646 | 30.28 | 3.002 | 9220 | — |
| churn | rust | 141936 | 1.264 | 30.20 | 4.005 | 9735 | — |
| churn | mimalloc | 139026 | 1.343 | 30.22 | 4.005 | 9737 | — |
| headers | rust | 128872 | 2.348 | 82.71 | 4.180 | 25740 | — |
| headers | mimalloc | 150169 | 1.832 | 90.66 | 4.180 | 25740 | — |
| proxy-1024-on | rust | 315130 | 0.922 | 30.45 | 4.006 | 17424 | — |
| proxy-1024-on | mimalloc | 313923 | 0.849 | 30.37 | 4.006 | 17424 | — |
| proxy-1024-off | rust | 318537 | 0.847 | 32.39 | 4.006 | 17424 | — |
| proxy-1024-off | mimalloc | 316657 | 0.838 | 30.44 | 4.006 | 17424 | — |
| proxy-16384-on | rust | 265702 | 1.016 | 34.08 | 5.008 | 33828 | — |
| proxy-16384-on | mimalloc | 261521 | 1.122 | 31.67 | 5.008 | 33828 | — |
| proxy-16384-off | rust | 180181 | 1.725 | 30.58 | 4.011 | 17436 | — |
| proxy-16384-off | mimalloc | 181125 | 1.639 | 30.56 | 4.011 | 17436 | — |
| proxy-65536-on | rust | 149163 | 40.133 | 38.65 | 9.021 | 87198 | — |
| proxy-65536-on | mimalloc | 151598 | 40.132 | 36.03 | 9.021 | 87195 | — |
| proxy-65536-off | rust | 68329 | 38.241 | 34.34 | 4.046 | 17479 | — |
| proxy-65536-off | mimalloc | 69089 | 39.174 | 30.20 | 4.024 | 17472 | — |
| proxy-262144-on | rust | 60035 | 9.784 | 39.52 | 20.919 | 281655 | — |
| proxy-262144-on | mimalloc | 62749 | 9.069 | 38.06 | 20.971 | 282632 | — |
| proxy-262144-off | rust | 21006 | 36.416 | 33.45 | 4.080 | 17619 | — |
| proxy-262144-off | mimalloc | 21018 | 36.765 | 33.18 | 4.086 | 17651 | — |
| upload-4096 | rust | 286854 | 0.957 | 34.57 | 6.010 | 18451 | — |
| upload-4096 | mimalloc | 288763 | 1.033 | 30.55 | 6.009 | 18451 | — |
| download-4096 | rust | 549762 | 0.729 | 30.45 | 3.002 | 9220 | — |
| download-4096 | mimalloc | 546114 | 0.678 | 30.46 | 3.002 | 9220 | — |
| upload-65536 | rust | 143239 | 20.783 | 46.04 | 7.016 | 83199 | — |
| upload-65536 | mimalloc | 151932 | 21.388 | 40.17 | 7.016 | 83197 | — |
| download-65536 | rust | 299289 | 39.910 | 30.47 | 3.004 | 9224 | — |
| download-65536 | mimalloc | 302433 | 39.912 | 30.54 | 3.004 | 9224 | — |
| upload-1048576 | rust | 8919 | 71.739 | 42.54 | 6.245 | 82267 | — |
| upload-1048576 | mimalloc | 9351 | 59.115 | 42.49 | 6.216 | 82161 | — |
| download-1048576 | rust | 37743 | 14.789 | 30.32 | 3.024 | 9259 | — |
| download-1048576 | mimalloc | 38375 | 16.103 | 30.54 | 3.023 | 9256 | — |
| burst-idle | rust | 574878 | 0.648 | 30.20 | 3.002 | 9220 | 30.20 |
| burst-idle | mimalloc | 570618 | 0.684 | 30.53 | 3.002 | 9220 | 30.53 |

## debug-palloc

| Case | Allocator | Requests/s | HTTP p99 ms | Peak worker RSS MiB | Allocations/request | Requested bytes/request | Idle final RSS MiB |
|---|---|---:|---:|---:|---:|---:|---:|
| static | rust | 538192 | 0.741 | 30.61 | 20.019 | 10222 | — |
| static | mimalloc | 545365 | 0.721 | 30.54 | 20.020 | 10226 | — |
| churn | rust | 140848 | 1.228 | 30.61 | 27.030 | 11002 | — |
| churn | mimalloc | 140588 | 1.281 | 30.55 | 27.027 | 11005 | — |
| headers | rust | 160019 | 2.190 | 106.75 | 30.102 | 26835 | — |
| headers | mimalloc | 153890 | 2.196 | 102.57 | 30.122 | 26853 | — |
| proxy-1024-on | rust | 307614 | 0.886 | 34.61 | 43.068 | 21111 | — |
| proxy-1024-on | mimalloc | 308014 | 0.915 | 33.54 | 43.069 | 21111 | — |
| proxy-1024-off | rust | 312752 | 0.904 | 34.67 | 41.067 | 20847 | — |
| proxy-1024-off | mimalloc | 314384 | 0.888 | 34.66 | 41.064 | 20846 | — |
| proxy-16384-on | rust | 258704 | 1.113 | 34.16 | 49.093 | 37739 | — |
| proxy-16384-on | mimalloc | 259503 | 1.086 | 34.23 | 49.088 | 37737 | — |
| proxy-16384-off | rust | 179823 | 1.520 | 34.05 | 41.094 | 20863 | — |
| proxy-16384-off | mimalloc | 178804 | 1.945 | 34.40 | 41.094 | 20863 | — |
| proxy-65536-on | rust | 147869 | 40.110 | 38.05 | 67.144 | 87670 | — |
| proxy-65536-on | mimalloc | 149493 | 40.090 | 38.41 | 67.142 | 87670 | — |
| proxy-65536-off | rust | 65458 | 37.784 | 34.41 | 41.321 | 20932 | — |
| proxy-65536-off | mimalloc | 67942 | 38.357 | 34.36 | 41.213 | 20915 | — |
| proxy-262144-on | rust | 58901 | 9.332 | 41.44 | 138.608 | 285049 | — |
| proxy-262144-on | mimalloc | 61716 | 9.072 | 38.45 | 138.753 | 285426 | — |
| proxy-262144-off | rust | 21056 | 38.372 | 35.17 | 41.657 | 21121 | — |
| proxy-262144-off | mimalloc | 20962 | 38.808 | 33.37 | 41.647 | 21117 | — |
| upload-4096 | rust | 279200 | 0.952 | 34.07 | 56.102 | 21526 | — |
| upload-4096 | mimalloc | 278571 | 0.962 | 34.48 | 56.117 | 21528 | — |
| download-4096 | rust | 524054 | 0.746 | 30.62 | 20.019 | 10231 | — |
| download-4096 | mimalloc | 523581 | 0.730 | 30.55 | 20.020 | 10235 | — |
| upload-65536 | rust | 144345 | 21.737 | 43.80 | 56.147 | 83135 | — |
| upload-65536 | mimalloc | 139699 | 20.496 | 44.94 | 56.148 | 83136 | — |
| download-65536 | rust | 295930 | 40.199 | 30.36 | 20.026 | 10237 | — |
| download-65536 | mimalloc | 296126 | 39.925 | 30.54 | 20.026 | 10241 | — |
| upload-1048576 | rust | 8190 | 78.551 | 46.77 | 53.822 | 86287 | — |
| upload-1048576 | mimalloc | 8360 | 71.639 | 44.15 | 53.816 | 86301 | — |
| download-1048576 | rust | 38349 | 17.128 | 30.61 | 20.124 | 10279 | — |
| download-1048576 | mimalloc | 38143 | 17.339 | 30.84 | 20.130 | 10286 | — |
| burst-idle | rust | 543305 | 0.732 | 30.61 | 20.019 | 10226 | 30.61 |
| burst-idle | mimalloc | 547799 | 0.616 | 30.85 | 20.019 | 10230 | 30.85 |

## Interpretation flags

- production/static/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/static/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/churn/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/churn/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/headers/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/headers/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-4096/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-4096/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-65536/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-65536/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-1048576/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-1048576/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/burst-idle/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/burst-idle/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/static/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/static/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/churn/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/churn/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/headers/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/headers/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-on/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-off/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-4096/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-4096/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-65536/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-65536/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-1048576/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-1048576/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/burst-idle/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/burst-idle/mimalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
