# Nginx allocator comparison (complete)

Independent run medians; HTTP p99 is wrk's closed-loop latency histogram, not allocator p99. Peak RSS sums worker RSS (shared pages can be counted more than once). Allocator traffic comes from a separate instrumented preflight. CPU time is whole-worker CPU time, not allocator-only cycles. No rankings are combined between builds; this local load generator does not establish maximum sustainable throughput.

## production

| Case | Allocator | Requests/s | HTTP p99 ms | Peak worker RSS MiB | Allocations/request | Requested bytes/request | Idle final RSS MiB |
|---|---|---:|---:|---:|---:|---:|---:|
| static | rust | 573166 | 0.679 | 30.45 | 3.002 | 9220 | — |
| static | glibc | 572377 | 0.723 | 19.33 | 3.002 | 9220 | — |
| static | jemalloc | 568662 | 0.696 | 21.02 | 3.002 | 9220 | — |
| static | tcmalloc | 570109 | 0.485 | 27.72 | 3.002 | 9220 | — |
| churn | rust | 141694 | 1.215 | 30.45 | 4.004 | 9733 | — |
| churn | glibc | 141512 | 1.315 | 19.33 | 4.005 | 9737 | — |
| churn | jemalloc | 141095 | 1.349 | 21.05 | 4.005 | 9737 | — |
| churn | tcmalloc | 141084 | 1.301 | 27.84 | 4.004 | 9734 | — |
| headers | rust | 154675 | 1.773 | 96.67 | 4.179 | 25739 | — |
| headers | glibc | 129614 | 1.996 | 73.78 | 4.179 | 25740 | — |
| headers | jemalloc | 134546 | 2.039 | 77.28 | 4.180 | 25740 | — |
| headers | tcmalloc | 140844 | 1.875 | 85.34 | 4.178 | 25732 | — |
| proxy-1024-on | rust | 317117 | 0.877 | 30.29 | 4.006 | 17424 | — |
| proxy-1024-on | glibc | 313571 | 0.888 | 20.89 | 4.006 | 17424 | — |
| proxy-1024-on | jemalloc | 315810 | 0.889 | 22.66 | 4.007 | 17425 | — |
| proxy-1024-on | tcmalloc | 319617 | 0.839 | 29.68 | 4.006 | 17424 | — |
| proxy-1024-off | rust | 320806 | 0.865 | 32.38 | 4.006 | 17424 | — |
| proxy-1024-off | glibc | 318692 | 0.860 | 21.36 | 4.006 | 17424 | — |
| proxy-1024-off | jemalloc | 319232 | 0.824 | 22.78 | 4.007 | 17424 | — |
| proxy-1024-off | tcmalloc | 323534 | 0.828 | 29.68 | 4.006 | 17424 | — |
| proxy-16384-on | rust | 266094 | 1.035 | 34.14 | 5.008 | 33828 | — |
| proxy-16384-on | glibc | 263966 | 0.999 | 21.66 | 5.008 | 33828 | — |
| proxy-16384-on | jemalloc | 266304 | 1.007 | 22.91 | 5.008 | 33828 | — |
| proxy-16384-on | tcmalloc | 269349 | 0.993 | 29.80 | 5.008 | 33828 | — |
| proxy-16384-off | rust | 182718 | 1.462 | 30.09 | 4.010 | 17435 | — |
| proxy-16384-off | glibc | 182136 | 1.460 | 20.80 | 4.010 | 17435 | — |
| proxy-16384-off | jemalloc | 182466 | 1.449 | 22.90 | 4.011 | 17435 | — |
| proxy-16384-off | tcmalloc | 182742 | 1.451 | 29.68 | 4.011 | 17435 | — |
| proxy-65536-on | rust | 154053 | 40.144 | 37.88 | 9.021 | 87191 | — |
| proxy-65536-on | glibc | 150273 | 40.139 | 25.26 | 9.021 | 87197 | — |
| proxy-65536-on | jemalloc | 151190 | 40.160 | 27.84 | 9.021 | 87199 | — |
| proxy-65536-on | tcmalloc | 155352 | 40.138 | 34.30 | 9.021 | 87195 | — |
| proxy-65536-off | rust | 68612 | 38.586 | 34.29 | 4.051 | 17479 | — |
| proxy-65536-off | glibc | 68164 | 38.596 | 22.02 | 4.140 | 17493 | — |
| proxy-65536-off | jemalloc | 69083 | 39.209 | 24.13 | 4.024 | 17475 | — |
| proxy-65536-off | tcmalloc | 69013 | 38.474 | 31.30 | 4.044 | 17477 | — |
| proxy-262144-on | rust | 62877 | 8.995 | 38.02 | 20.948 | 282210 | — |
| proxy-262144-on | glibc | 61734 | 8.379 | 23.67 | 20.980 | 282818 | — |
| proxy-262144-on | jemalloc | 63374 | 8.692 | 41.43 | 20.955 | 282336 | — |
| proxy-262144-on | tcmalloc | 64561 | 8.720 | 40.68 | 20.910 | 281655 | — |
| proxy-262144-off | rust | 21299 | 37.560 | 33.09 | 4.085 | 17646 | — |
| proxy-262144-off | glibc | 21008 | 37.299 | 22.44 | 4.092 | 17627 | — |
| proxy-262144-off | jemalloc | 20784 | 37.836 | 25.38 | 4.082 | 17635 | — |
| proxy-262144-off | tcmalloc | 21121 | 36.987 | 32.18 | 4.084 | 17633 | — |
| upload-4096 | rust | 291066 | 0.966 | 34.46 | 6.009 | 18451 | — |
| upload-4096 | glibc | 289301 | 0.942 | 21.78 | 6.009 | 18451 | — |
| upload-4096 | jemalloc | 290910 | 0.956 | 23.50 | 6.009 | 18451 | — |
| upload-4096 | tcmalloc | 295985 | 0.952 | 30.80 | 6.009 | 18451 | — |
| download-4096 | rust | 550385 | 0.684 | 30.45 | 3.002 | 9220 | — |
| download-4096 | glibc | 544765 | 0.686 | 19.07 | 3.002 | 9220 | — |
| download-4096 | jemalloc | 545752 | 0.742 | 21.18 | 3.002 | 9220 | — |
| download-4096 | tcmalloc | 549933 | 0.715 | 27.84 | 3.002 | 9220 | — |
| upload-65536 | rust | 149807 | 22.130 | 45.07 | 7.016 | 83201 | — |
| upload-65536 | glibc | 146689 | 20.766 | 35.80 | 7.016 | 83203 | — |
| upload-65536 | jemalloc | 143561 | 22.928 | 38.60 | 7.016 | 83204 | — |
| upload-65536 | tcmalloc | 152036 | 22.528 | 45.05 | 7.015 | 83196 | — |
| download-65536 | rust | 301420 | 40.141 | 30.46 | 3.004 | 9224 | — |
| download-65536 | glibc | 301939 | 39.926 | 19.34 | 3.004 | 9224 | — |
| download-65536 | jemalloc | 302130 | 39.910 | 21.05 | 3.003 | 9222 | — |
| download-65536 | tcmalloc | 303728 | 40.148 | 27.72 | 3.004 | 9224 | — |
| upload-1048576 | rust | 8955 | 61.202 | 41.84 | 6.207 | 81994 | — |
| upload-1048576 | glibc | 9782 | 58.118 | 36.89 | 6.206 | 81966 | — |
| upload-1048576 | jemalloc | 10094 | 59.843 | 36.16 | 6.225 | 82150 | — |
| upload-1048576 | tcmalloc | 9229 | 65.663 | 45.30 | 6.224 | 82118 | — |
| download-1048576 | rust | 36691 | 17.410 | 30.45 | 3.025 | 9260 | — |
| download-1048576 | glibc | 38117 | 17.321 | 19.32 | 3.025 | 9260 | — |
| download-1048576 | jemalloc | 38188 | 18.593 | 21.05 | 3.025 | 9260 | — |
| download-1048576 | tcmalloc | 37797 | 17.028 | 27.72 | 3.025 | 9261 | — |
| burst-idle | rust | 568617 | 0.710 | 30.41 | 3.002 | 9220 | 30.41 |
| burst-idle | glibc | 565801 | 0.708 | 19.34 | 3.002 | 9220 | 19.34 |
| burst-idle | jemalloc | 567905 | 0.696 | 21.16 | 3.002 | 9221 | 21.16 |
| burst-idle | tcmalloc | 567664 | 0.679 | 27.84 | 3.002 | 9221 | 27.84 |

## debug-palloc

| Case | Allocator | Requests/s | HTTP p99 ms | Peak worker RSS MiB | Allocations/request | Requested bytes/request | Idle final RSS MiB |
|---|---|---:|---:|---:|---:|---:|---:|
| static | rust | 548413 | 0.713 | 30.64 | 20.019 | 10224 | — |
| static | glibc | 544542 | 0.749 | 19.37 | 20.017 | 10224 | — |
| static | jemalloc | 549053 | 0.474 | 21.23 | 20.019 | 10228 | — |
| static | tcmalloc | 550786 | 0.708 | 28.11 | 20.019 | 10228 | — |
| churn | rust | 142277 | 1.139 | 30.49 | 27.026 | 11002 | — |
| churn | glibc | 142379 | 1.233 | 19.35 | 27.026 | 11003 | — |
| churn | jemalloc | 144010 | 1.259 | 21.08 | 27.026 | 11006 | — |
| churn | tcmalloc | 141975 | 1.392 | 28.23 | 27.029 | 11007 | — |
| headers | rust | 149038 | 1.744 | 102.49 | 30.106 | 26839 | — |
| headers | glibc | 128435 | 2.171 | 88.99 | 30.111 | 26844 | — |
| headers | jemalloc | 159626 | 1.832 | 100.58 | 30.106 | 26843 | — |
| headers | tcmalloc | 174097 | 1.710 | 109.48 | 30.106 | 26843 | — |
| proxy-1024-on | rust | 305487 | 0.886 | 33.79 | 43.071 | 21112 | — |
| proxy-1024-on | glibc | 283583 | 0.951 | 21.57 | 43.073 | 21113 | — |
| proxy-1024-on | jemalloc | 301779 | 0.869 | 23.59 | 43.070 | 21112 | — |
| proxy-1024-on | tcmalloc | 312505 | 0.877 | 30.16 | 43.071 | 21112 | — |
| proxy-1024-off | rust | 310623 | 0.886 | 34.35 | 41.065 | 20846 | — |
| proxy-1024-off | glibc | 287718 | 1.009 | 21.98 | 41.075 | 20849 | — |
| proxy-1024-off | jemalloc | 307737 | 0.873 | 23.46 | 41.069 | 20847 | — |
| proxy-1024-off | tcmalloc | 314464 | 0.929 | 30.91 | 41.068 | 20847 | — |
| proxy-16384-on | rust | 258294 | 1.040 | 33.86 | 49.092 | 37738 | — |
| proxy-16384-on | glibc | 240807 | 1.126 | 21.45 | 49.096 | 37742 | — |
| proxy-16384-on | jemalloc | 252858 | 1.071 | 24.31 | 49.090 | 37739 | — |
| proxy-16384-on | tcmalloc | 262152 | 1.021 | 29.91 | 49.086 | 37733 | — |
| proxy-16384-off | rust | 179046 | 1.540 | 34.05 | 41.093 | 20863 | — |
| proxy-16384-off | glibc | 172610 | 1.677 | 21.96 | 41.101 | 20866 | — |
| proxy-16384-off | jemalloc | 178733 | 1.466 | 23.84 | 41.093 | 20863 | — |
| proxy-16384-off | tcmalloc | 180112 | 1.549 | 30.79 | 41.092 | 20863 | — |
| proxy-65536-on | rust | 150644 | 40.169 | 38.17 | 67.143 | 87670 | — |
| proxy-65536-on | glibc | 137033 | 40.118 | 25.68 | 67.158 | 87686 | — |
| proxy-65536-on | jemalloc | 145688 | 40.124 | 29.69 | 67.151 | 87677 | — |
| proxy-65536-on | tcmalloc | 150844 | 40.119 | 35.04 | 67.143 | 87668 | — |
| proxy-65536-off | rust | 68226 | 38.313 | 34.36 | 41.197 | 20912 | — |
| proxy-65536-off | glibc | 65885 | 38.168 | 22.95 | 41.201 | 20914 | — |
| proxy-65536-off | jemalloc | 68316 | 39.664 | 25.18 | 41.209 | 20914 | — |
| proxy-65536-off | tcmalloc | 68117 | 38.644 | 31.54 | 41.193 | 20911 | — |
| proxy-262144-on | rust | 59909 | 7.117 | 36.79 | 138.544 | 284865 | — |
| proxy-262144-on | glibc | 54984 | 10.484 | 32.45 | 138.259 | 284091 | — |
| proxy-262144-on | jemalloc | 61884 | 6.476 | 34.64 | 138.334 | 284416 | — |
| proxy-262144-on | tcmalloc | 62364 | 9.078 | 41.04 | 138.463 | 284665 | — |
| proxy-262144-off | rust | 21014 | 36.915 | 36.73 | 41.640 | 21113 | — |
| proxy-262144-off | glibc | 20728 | 37.209 | 24.71 | 41.651 | 21117 | — |
| proxy-262144-off | jemalloc | 20880 | 37.082 | 26.44 | 41.649 | 21117 | — |
| proxy-262144-off | tcmalloc | 21127 | 38.439 | 33.16 | 41.625 | 21099 | — |
| upload-4096 | rust | 277641 | 0.941 | 34.61 | 56.106 | 21527 | — |
| upload-4096 | glibc | 243134 | 1.084 | 22.66 | 56.114 | 21530 | — |
| upload-4096 | jemalloc | 272372 | 0.982 | 24.45 | 56.103 | 21527 | — |
| upload-4096 | tcmalloc | 283931 | 0.934 | 30.66 | 56.125 | 21530 | — |
| download-4096 | rust | 529661 | 0.728 | 30.64 | 20.020 | 10233 | — |
| download-4096 | glibc | 517629 | 0.728 | 19.37 | 20.020 | 10234 | — |
| download-4096 | jemalloc | 510198 | 2.694 | 21.19 | 20.020 | 10237 | — |
| download-4096 | tcmalloc | 532248 | 0.753 | 28.11 | 20.020 | 10237 | — |
| upload-65536 | rust | 141925 | 23.165 | 49.17 | 56.141 | 83128 | — |
| upload-65536 | glibc | 139263 | 21.151 | 36.45 | 56.144 | 83141 | — |
| upload-65536 | jemalloc | 142131 | 21.689 | 36.08 | 56.148 | 83138 | — |
| upload-65536 | tcmalloc | 145566 | 21.833 | 45.16 | 56.147 | 83137 | — |
| download-65536 | rust | 298613 | 40.379 | 30.62 | 20.026 | 10239 | — |
| download-65536 | glibc | 294272 | 40.040 | 19.48 | 20.030 | 10242 | — |
| download-65536 | jemalloc | 295846 | 40.085 | 21.08 | 20.026 | 10243 | — |
| download-65536 | tcmalloc | 297823 | 40.303 | 27.86 | 20.029 | 10245 | — |
| upload-1048576 | rust | 8604 | 60.863 | 48.39 | 53.561 | 85939 | — |
| upload-1048576 | glibc | 9879 | 65.131 | 37.21 | 53.679 | 86066 | — |
| upload-1048576 | jemalloc | 8342 | 80.895 | 40.14 | 53.776 | 86250 | — |
| upload-1048576 | tcmalloc | 8845 | 62.884 | 47.16 | 53.510 | 85833 | — |
| download-1048576 | rust | 38837 | 15.457 | 30.61 | 20.129 | 10284 | — |
| download-1048576 | glibc | 38714 | 15.345 | 19.48 | 20.136 | 10287 | — |
| download-1048576 | jemalloc | 38146 | 16.816 | 21.23 | 20.119 | 10283 | — |
| download-1048576 | tcmalloc | 38594 | 15.535 | 27.98 | 20.126 | 10286 | — |
| burst-idle | rust | 549274 | 0.703 | 30.61 | 20.019 | 10228 | 30.61 |
| burst-idle | glibc | 535759 | 0.726 | 19.61 | 20.019 | 10229 | 19.61 |
| burst-idle | jemalloc | 546405 | 0.707 | 21.33 | 20.020 | 10232 | 21.33 |
| burst-idle | tcmalloc | 551148 | 0.610 | 27.86 | 20.019 | 10232 | 27.86 |

## Interpretation flags

- production/static/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/static/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/static/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/static/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/churn/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/churn/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/churn/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/churn/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/headers/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/headers/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/headers/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/headers/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-1024-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-16384-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-65536-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/proxy-262144-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-4096/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-4096/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-4096/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-4096/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-4096/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-4096/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-65536/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-65536/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-65536/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-65536/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-65536/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-65536/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-1048576/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-1048576/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/upload-1048576/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-1048576/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-1048576/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/download-1048576/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/burst-idle/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/burst-idle/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/burst-idle/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- production/burst-idle/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/static/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/static/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/static/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/static/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/churn/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/churn/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/churn/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/churn/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/headers/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/headers/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/headers/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/headers/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-1024-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-16384-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-65536-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-on/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-on/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-on/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-on/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-off/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-off/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-off/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/proxy-262144-off/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-4096/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-4096/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-4096/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-4096/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-4096/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-4096/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-4096/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-65536/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-65536/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-65536/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-65536/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-65536/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-65536/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-65536/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-1048576/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-1048576/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/upload-1048576/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-1048576/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-1048576/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-1048576/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/download-1048576/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/burst-idle/rust: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/burst-idle/glibc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/burst-idle/jemalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
- debug-palloc/burst-idle/tcmalloc: local closed-loop wrk; not fixed-rate latency or proven sustainable throughput
