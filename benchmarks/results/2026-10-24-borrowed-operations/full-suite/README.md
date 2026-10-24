# Full normal-mode benchmark comparison

Baseline allocator: `4219da2bcdbba7f9273c46b5dbc9e194f343464a`.
Candidate allocator: `70a3ce3ab5980660ddd4c9f0ace1d5052b80d993`.
This extends the earlier limited comparison with fresh before/after measurements
of every native/application case in `run --full`, both complete Nginx profiles,
all axum cases, and the architectural and retention families. These are Rust
baseline versus Rust candidate measurements; older application-versus-C results
are not reused. Allocator Rust source is unchanged from the measured candidate.

The primary aggregates are +0.51% for 21 native cases, +0.34% for Redis/RocksDB,
+0.16% for axum, -0.15% for production Nginx, and -1.06% for debug-palloc Nginx.
All five aggregates are within the predeclared 1% slowdown review limit.
Of 62 primary normalized-time cases, sized Larson alone exceeds the 3% case
limit: +8.96%. Its separate nine-pair, longer-interval confirmation is +0.20%.
Redis is -0.05% and RocksDB is +0.73% individually.

**This is not a blanket performance clearance.** The longer debug-palloc
header-heavy confirmation measures +22.66% normalized time, +22.67% CPU per
request, +12.91% p99, and -13.56% peak worker PSS. Its primary screen had moved
in the opposite direction. A subsequent identical-baseline-library control
also varies substantially: paired normalized-time changes range from -19.80%
to +6.41%, with a -12.05% median and +10.97% median paired peak-PSS change despite
no allocator change. That control demonstrates poor discrimination in this
case; it does not establish that the candidate has no cost. The +22.66% finding
remains recorded and this case remains inconclusive for a no-regression claim.
The earlier default-linked Rust facade and collection findings in the
[parent report](../README.md) also remain open.

All adverse primary observations and confirmations are retained. In particular,
the original thread-churn abandoned-collection phase is +10.25%; its separate
nine-pair/32-cycle confirmation is +0.49%, with identical operation/live-byte
counts in every matched phase. The original other-thread-collection retention
sample has approximately 1.87 MiB higher median settled PSS for the candidate.
Both labels exhibit both memory levels, and the second process in each of its
three pairs has the higher level. A six-pair balanced repeat measures +7 KiB
median paired PSS, or +1 KiB after subtracting each process's baseline. Its
same-library control measures -4.5 KiB raw and +0.5 KiB baseline-adjusted.
The repeat does not reproduce a persistent 1.87 MiB increase.

## Interpretation and protocol

Positive percentages mean slower normalized time or a larger separately named
metric. Each case uses the median paired candidate/baseline ratio; primary
aggregates are equal-weight geometric means of case ratios. HTTP normalized
time is inverse throughput. Separately displayed label medians need not yield
the median paired percentage when divided. No latency or memory acceptance
budget is invented after seeing results, and memory/idle time is not mixed into
a performance score. All raw per-cycle architectural values remain available.

Both labels use the same cached executable, allocator alias, working paths,
arguments, environment and workload configuration within each pair. The alias
target changes only after the preceding job exits; actual library hashes and
comparison labels are recorded outside worker inputs. Ownership checks verify
that applications load the selected allocator. Historical cached-build manifests
also contain an unused Rust-library entry; runtime commands select the frozen
`old.so`/`new.so` inputs instead. Their hashes match the original clean allocator
build manifests. All 17 frozen executable/library fingerprints are checked again
after timing. Builds, tests and separate benchmark families do not compete with
timed runs.

The host is the same Linux x86_64 Azure VM with AMD EPYC 7763 CPUs used in the
parent experiment, with Rust 1.92.0/LLVM 21.1.3. This extension uses normal
`interpose` mode. Full secure-mode application coverage is not claimed.

- Native: all 21 cases, three paired rounds, ten Criterion samples, one-second
  target warmup and three-second target measurement. Fixed-duration vendor
  workloads can take substantially longer than those targets.
- Redis/RocksDB: five paired rounds, ten samples per label, one million
  operations, one thread and 256-byte values. Redis covers SET/GET; RocksDB
  covers the repository's `fillrandom` workload. Both use CPUs 0 and 2.
- axum: JSON, pipelined plaintext and SQLite fortunes, three paired rounds,
  30-second load/5-second warmup, 256 connections and disjoint four-core server
  and generator sets. Pipelined plaintext p99 remains unavailable.
- Nginx: all 18 cases in each of production and debug-palloc, three paired
  rounds, five-second load/two-second warmup, separate instrumented preflight,
  two workers, 256 connections and disjoint server/generator/upstream cores.
  Burst/idle keeps two bursts and both ten-second idle windows.
- Architecture: all four scenarios, five paired rounds, 64 MiB, 4 KiB blocks,
  four threads and eight cycles. Retention: all five actions, three paired
  rounds, 64 MiB/64-byte blocks, ten-second windows and 100 ms sampling, with
  a separate two-pair idle/empty same-baseline calibration.
- Confirmations: sized Larson uses nine pairs with three 15-second workloads
  per worker, discarding the first as warmup. Thread churn uses nine pairs and
  32 cycles. HTTP repeats six selected case/profile pairs with six paired
  rounds, 15-second load and five-second warmup. Selection is the two largest
  p99 changes and largest peak-PSS change per profile, taking their union.
  Before any HTTP confirmation timing, its initially planned five rounds were
  increased to six to balance first-label order after the independent retention
  study exposed an order-associated memory pattern. Both plans are retained.
  The later memory confirmation uses two same-baseline pairs and six balanced
  old/new pairs of the other-thread-collection action. The header calibration
  uses four balanced pairs with the same baseline library on both sides.

The sampled 30-second header profiles place 77.27%/79.69% of user CPU in the
unchanged Nginx `ngx_http_parse_header_line` function. Allocation/free entrypoints
account for about 2.2% in each label. Most additional sampled CPU per request
appears in that parser. This is evidence for caller/workload sensitivity, not
proof of its precise mechanism or an exemption for the slowdown. Hardware
cycle/instruction counters are unavailable on this VM. Both software CPU-clock
captures have zero lost samples. Perf returned SIGTERM after target exit; complete
Nginx protocols and readable captures were validated, and the original baseline
capture was reused without retiming it. Raw perf recordings are identified by
hash/size rather than embedded.

HTTP p99 is wrk's local closed-loop observation, not allocator p99 or a fixed-rate
latency SLO. All completed HTTP measurements report zero errors. Nginx retains
its closed-loop quality flag; axum uses local closed-loop load as well.
PSS/RSS covers processes, including their harness and
application buffers; it is not an allocator-internal counter. Nonempty passive
retention decay remains right-censored in the ten-second windows. Assisted
collection/exit decay is observed for both labels. Empty-control work is below
one microsecond, so its work-time percentage changes are uninformative.

## Harness correction and evidence

The first Nginx attempt stopped after 30 completed workers when wrk exited
between its liveness check and a `/proc/.../smaps_rollup` read. This happened in
an upstream preflight before loading the compared allocator. The optional load
sampler already ignored `FileNotFoundError`; the one-line correction also handles
`ProcessLookupError`. Server disappearance and nonzero wrk exits remain errors.
A regression test reproduces the error before the correction. The entire Nginx
family was restarted under the corrected sampler; every original successful and
failed result remains under `failed-attempts/nginx-001`, with a relocation map.
No favorable timing was selected from that failed attempt. The initial plan's
all-source-unchanged flag describes the initial snapshot; the recorded sampler
correction supplies the subsequent Python source hash and exact patch. C/Rust
workload sources and allocator binaries remain unchanged.

`manifest.json` identifies builds, source/feature identities, commands, plans,
input hashes and final verification logs. `results.json` contains primary and
confirmation summaries, the same-baseline controls and diagnostic profiles.
`raw-data.tar.gz` retains controllers, benchmark Python sources, every raw sample,
protocol observation and log, including adverse results. Analysis scripts support
relocated extraction and independently recompute the recorded ratios and memory
summaries; decay recomputation uses this host's 4 KiB page size.
After extraction to a new directory, all eight summary recomputations match the
original JSON exactly and regenerated table Markdown is byte-identical.
The archive contains 9,550 files; its checksum and this verification are recorded
in the manifest.

The final formatting, workspace Clippy and all-target/all-feature workspace test
commands all pass and are recorded in `final-checks.json` inside the archive.
The benchmark Python tests pass with 72 tests, eight optional integrations
skipped. The parent
report retains the full Miri, sanitizer, ABI/API/FFI, parity and concurrency checks
for the unchanged allocator source.

## Primary normalized-time aggregates

| Family | Profile | Cases | Change |
| --- | --- | --- | --- |
| applications | applications | 2 | +0.34% |
| webserver | webserver | 3 | +0.16% |
| native | native | 21 | +0.51% |
| nginx | production | 18 | -0.15% |
| nginx | debug-palloc | 18 | -1.06% |

## Primary cases: applications

| Case | Median paired change | Every paired change |
| --- | --- | --- |
| redis | -0.05% | -0.05%, -2.52%, +0.45%, +0.29%, -0.19% |
| rocksdb | +0.73% | +2.37%, +0.73%, -1.96%, +0.16%, +1.14% |

## Primary cases: webserver

| Case | Median paired change | Every paired change |
| --- | --- | --- |
| fortunes | -0.48% | -0.48%, -1.70%, +0.53% |
| json | +0.29% | -3.25%, +0.29%, +2.32% |
| plaintext | +0.67% | -0.82%, +0.98%, +0.67% |

## Primary cases: native

| Case | Median paired change | Every paired change |
| --- | --- | --- |
| alloc-test1 | +0.22% | +0.22%, +0.06%, +0.40% |
| alloc-testN | -0.00% | -0.03%, -0.00%, +0.03% |
| barnes | -0.01% | -0.01%, +0.39%, -0.26% |
| cache-scratch1 | +0.04% | +0.08%, +0.04%, -0.05% |
| cache-scratchN | -0.02% | -0.02%, -0.51%, +0.11% |
| cache-thrash1 | +0.02% | -0.02%, +0.04%, +0.02% |
| cache-thrashN | -0.17% | -0.17%, -0.60%, +0.89% |
| cfrac | -0.83% | -0.64%, -0.83%, -1.10% |
| glibc-simple | +0.35% | +0.35%, +0.23%, +0.69% |
| glibc-thread | -0.21% | -0.21%, -0.64%, +0.71% |
| larsonN | +1.22% | +1.22%, +10.53%, +0.57% |
| larsonN-sized | +8.96% | +8.96%, +11.22%, +1.18% |
| malloc-large | -0.50% | -0.50%, +0.03%, -1.15% |
| malloc-large-old | -0.07% | +0.01%, -0.58%, -0.07% |
| mleak10 | +1.77% | -2.30%, +2.16%, +1.77% |
| mleak100 | +2.28% | -4.07%, +2.28%, +5.43% |
| mstressN | +0.62% | +0.62%, +1.24%, -1.58% |
| rptestN | +2.17% | -4.73%, +12.34%, +2.17% |
| sh6benchN | -0.81% | -0.10%, -0.81%, -1.53% |
| sh8benchN | -1.46% | -0.46%, -1.46%, -1.97% |
| xmalloc-testN | -2.32% | -1.91%, -5.15%, -2.32% |

## Primary cases: nginx

| Case | Median paired change | Every paired change |
| --- | --- | --- |
| debug-palloc/proxy-1024-off | +0.92% | +0.84%, +2.74%, +0.92% |
| production/proxy-1024-off | -1.15% | -1.37%, -1.15%, -0.11% |
| debug-palloc/proxy-16384-off | -1.08% | -1.60%, -1.08%, +1.35% |
| production/proxy-16384-off | +0.24% | -0.31%, +0.24%, +8.40% |
| debug-palloc/proxy-262144-off | +0.54% | +1.08%, -1.19%, +0.54% |
| production/proxy-262144-off | +0.03% | +0.18%, -0.77%, +0.03% |
| debug-palloc/proxy-65536-off | -0.14% | -0.14%, -0.24%, +0.61% |
| production/proxy-65536-off | +1.01% | +0.65%, +2.16%, +1.01% |
| debug-palloc/proxy-1024-on | +0.95% | -0.70%, +6.43%, +0.95% |
| production/proxy-1024-on | -0.49% | +1.91%, -0.49%, -1.73% |
| debug-palloc/proxy-16384-on | -0.22% | -8.19%, -0.22%, +1.09% |
| production/proxy-16384-on | +0.78% | +2.66%, +0.78%, -0.73% |
| debug-palloc/proxy-262144-on | +0.12% | +3.03%, +0.12%, -0.77% |
| production/proxy-262144-on | +0.20% | +0.20%, +0.27%, -0.44% |
| debug-palloc/proxy-65536-on | +0.00% | +1.11%, +0.00%, -0.53% |
| production/proxy-65536-on | -0.54% | -0.54%, +0.03%, -1.09% |
| debug-palloc/burst-idle | +0.32% | -2.48%, +0.32%, +1.34% |
| production/burst-idle | -0.41% | -0.41%, -0.10%, -2.25% |
| debug-palloc/churn | +0.23% | +0.89%, +0.23%, -0.54% |
| production/churn | -0.48% | +0.48%, -0.48%, -0.79% |
| debug-palloc/download-1048576 | +0.67% | -2.16%, +3.02%, +0.67% |
| production/download-1048576 | +0.13% | +0.13%, +0.71%, -0.27% |
| debug-palloc/download-4096 | -0.90% | -2.92%, -0.90%, +0.12% |
| production/download-4096 | -0.66% | -0.66%, -1.95%, +3.39% |
| debug-palloc/download-65536 | +0.93% | +0.80%, +2.47%, +0.93% |
| production/download-65536 | -0.17% | -1.05%, +0.16%, -0.17% |
| debug-palloc/headers | -15.68% | -11.56%, -20.77%, -15.68% |
| production/headers | -0.25% | -0.86%, -0.25%, +0.79% |
| debug-palloc/static | -1.46% | -8.23%, +0.93%, -1.46% |
| production/static | +1.08% | -0.43%, +1.08%, +2.08% |
| debug-palloc/upload-1048576 | -0.45% | -0.45%, -1.92%, -0.02% |
| production/upload-1048576 | +0.09% | +0.43%, -1.34%, +0.09% |
| debug-palloc/upload-4096 | -1.45% | -1.58%, +0.21%, -1.45% |
| production/upload-4096 | -0.28% | -0.28%, +0.54%, -4.94% |
| debug-palloc/upload-65536 | -1.08% | +1.68%, -1.08%, -1.33% |
| production/upload-65536 | -1.80% | -6.40%, -1.80%, +0.43% |

## Redis components

| Operation | Median paired change |
| --- | --- |
| SET | -0.45% |
| GET | -0.51% |

## Longer native confirmation

| Case | Median paired change | Every paired change |
| --- | --- | --- |
| larsonN-sized | +0.20% | -1.81%, -19.74%, +0.20%, +11.63%, -4.81%, +3.26%, +0.47%, +5.52%, -1.38% |

## Primary HTTP observations

| Case | Time change | p99 ms old → new | p99 change | Peak PSS MiB old → new | PSS change | CPU/request change |
| --- | --- | --- | --- | --- | --- | --- |
| axum/fortunes | -0.48% | 3.15 → 3.23 | +2.63% | 9.80 → 9.97 | +1.59% | -0.46% |
| axum/json | +0.29% | 1.10 → 1.16 | -2.43% | 7.92 → 7.92 | +0.31% | +0.30% |
| axum/plaintext | +0.67% | — | — | 7.30 → 7.23 | -0.28% | +0.65% |
| debug-palloc/burst-idle | +0.32% | 2.47 → 2.33 | -5.68% | 14.72 → 14.71 | +0.02% | +0.46% |
| debug-palloc/churn | +0.23% | 7.07 → 7.16 | -1.12% | 14.71 → 14.72 | -0.03% | -1.27% |
| debug-palloc/download-1048576 | +0.67% | 55.31 → 55.60 | +0.52% | 14.69 → 14.71 | -0.05% | +0.28% |
| debug-palloc/download-4096 | -0.90% | 2.37 → 2.35 | +2.30% | 14.72 → 14.67 | -0.32% | -0.88% |
| debug-palloc/download-65536 | +0.93% | 3.98 → 4.20 | +8.48% | 14.71 → 14.71 | +0.00% | -0.16% |
| debug-palloc/headers | -15.68% | 6.68 → 5.74 | -12.07% | 35.10 → 40.51 | +11.24% | -15.52% |
| debug-palloc/proxy-1024-off | +0.92% | 5.30 → 6.20 | +9.71% | 17.80 → 17.81 | +0.02% | +1.22% |
| debug-palloc/proxy-1024-on | +0.95% | 5.69 → 5.66 | -8.46% | 17.87 → 17.75 | -0.38% | +1.14% |
| debug-palloc/proxy-16384-off | -1.08% | 9.26 → 9.21 | -1.44% | 17.76 → 17.79 | -0.28% | -0.97% |
| debug-palloc/proxy-16384-on | -0.22% | 6.64 → 6.96 | +4.76% | 18.31 → 18.33 | +0.41% | -0.61% |
| debug-palloc/proxy-262144-off | +0.54% | 61.57 → 60.58 | +0.50% | 19.81 → 19.78 | +0.85% | +0.51% |
| debug-palloc/proxy-262144-on | +0.12% | 23.46 → 27.50 | +20.14% | 22.54 → 24.97 | +4.56% | +0.20% |
| debug-palloc/proxy-65536-off | -0.14% | 21.42 → 20.95 | +0.14% | 17.60 → 17.58 | -0.30% | +0.09% |
| debug-palloc/proxy-65536-on | +0.00% | 16.73 → 17.29 | -5.77% | 18.88 → 18.65 | -1.50% | +0.01% |
| debug-palloc/static | -1.46% | 2.61 → 2.43 | -6.86% | 14.67 → 14.66 | -0.07% | -1.17% |
| debug-palloc/upload-1048576 | -0.45% | 148.60 → 149.47 | -0.58% | 33.43 → 34.02 | -1.52% | -0.31% |
| debug-palloc/upload-4096 | -1.45% | 6.79 → 6.70 | -0.16% | 18.84 → 18.80 | +0.39% | -1.37% |
| debug-palloc/upload-65536 | -1.08% | 12.01 → 12.22 | +8.17% | 34.31 → 32.56 | -4.06% | -0.83% |
| production/burst-idle | -0.41% | 2.26 → 2.25 | -0.40% | 14.49 → 14.49 | +0.01% | -0.39% |
| production/churn | -0.48% | 6.99 → 7.28 | +4.88% | 14.51 → 14.55 | +0.31% | +0.14% |
| production/download-1048576 | +0.13% | 54.81 → 55.06 | +0.28% | 14.48 → 14.47 | +0.15% | -0.27% |
| production/download-4096 | -0.66% | 2.46 → 2.32 | -8.38% | 14.47 → 14.43 | -0.02% | -0.69% |
| production/download-65536 | -0.17% | 3.96 → 4.09 | -0.53% | 14.43 → 14.48 | +0.07% | -0.04% |
| production/headers | -0.25% | 7.85 → 8.18 | +1.00% | 29.61 → 29.64 | +0.14% | -0.24% |
| production/proxy-1024-off | -1.15% | 5.23 → 5.60 | +3.65% | 16.61 → 16.61 | -0.38% | -1.06% |
| production/proxy-1024-on | -0.49% | 5.06 → 5.33 | +5.36% | 16.68 → 16.62 | -0.72% | -0.59% |
| production/proxy-16384-off | +0.24% | 9.54 → 9.15 | -5.76% | 16.55 → 16.62 | +0.33% | +0.18% |
| production/proxy-16384-on | +0.78% | 6.46 → 6.49 | +0.51% | 17.32 → 17.32 | +0.00% | +0.67% |
| production/proxy-262144-off | +0.03% | 60.74 → 59.93 | +0.37% | 18.41 → 18.16 | -1.35% | +0.23% |
| production/proxy-262144-on | +0.20% | 26.97 → 26.34 | -1.71% | 19.35 → 20.05 | +6.59% | -0.03% |
| production/proxy-65536-off | +1.01% | 20.14 → 22.41 | +11.24% | 16.34 → 16.43 | +0.66% | +1.03% |
| production/proxy-65536-on | -0.54% | 18.82 → 18.19 | -3.34% | 17.56 → 17.74 | +1.40% | -0.54% |
| production/static | +1.08% | 2.24 → 2.28 | +4.69% | 14.43 → 14.43 | +0.03% | +1.38% |
| production/upload-1048576 | +0.09% | 163.63 → 143.10 | -10.37% | 32.14 → 31.69 | -1.40% | -0.13% |
| production/upload-4096 | -0.28% | 6.92 → 6.46 | -10.71% | 17.69 → 17.55 | -0.57% | -0.27% |
| production/upload-65536 | -1.80% | 11.32 → 11.74 | +11.28% | 31.68 → 31.69 | +0.04% | -1.79% |

## Longer HTTP confirmations

| Case | Time change | p99 ms old → new | p99 change | Peak PSS MiB old → new | PSS change | CPU/request change |
| --- | --- | --- | --- | --- | --- | --- |
| debug-palloc/headers | +22.66% | 5.93 → 6.76 | +12.91% | 102.94 → 88.95 | -13.56% | +22.67% |
| debug-palloc/proxy-1024-off | -1.43% | 5.57 → 5.56 | +0.02% | 17.89 → 17.88 | +0.06% | -1.47% |
| debug-palloc/proxy-262144-on | -0.09% | 22.88 → 23.16 | +1.59% | 29.19 → 28.73 | -7.95% | -0.12% |
| production/proxy-262144-on | +0.40% | 23.47 → 22.63 | -5.73% | 27.53 → 26.28 | -5.27% | +0.43% |
| production/proxy-65536-off | -0.18% | 20.18 → 19.84 | -1.77% | 16.61 → 16.58 | -0.11% | -0.16% |
| production/upload-65536 | +0.76% | 11.14 → 11.37 | +3.16% | 32.94 → 32.73 | -0.31% | +0.78% |

## Same-baseline header control (old/new mean repeat A/B)

| Case | Time change | p99 ms old → new | p99 change | Peak PSS MiB old → new | PSS change | CPU/request change |
| --- | --- | --- | --- | --- | --- | --- |
| debug-palloc/headers | -12.04% | 6.76 → 6.31 | -3.09% | 91.95 → 98.96 | +10.97% | -12.12% |

## Primary architectural observations

| Scenario / phase | Elapsed ms old → new | Median paired time change | Process RSS MiB old → new |
| --- | --- | --- | --- |
| cross-thread/baseline | 0.00 → 0.00 | — | 3.97 → 3.97 |
| cross-thread/fan-in-allocate | 0.47 → 0.46 | -2.77% | 98.64 → 100.25 |
| cross-thread/fan-in-free | 0.44 → 0.43 | -2.47% | 98.64 → 100.25 |
| cross-thread/fan-out-allocate | 0.71 → 0.71 | +0.23% | 98.64 → 100.25 |
| cross-thread/fan-out-free | 0.30 → 0.30 | -1.74% | 98.64 → 100.25 |
| cross-thread/final-collect | 0.53 → 0.52 | -2.06% | 5.09 → 6.64 |
| decommit-oscillation/baseline | 0.00 → 0.00 | — | 3.92 → 3.97 |
| decommit-oscillation/forced-purge | 0.01 → 0.01 | +1.24% | 8.19 → 8.24 |
| decommit-oscillation/free | 8.78 → 8.87 | -0.27% | 8.25 → 8.30 |
| decommit-oscillation/refill-touch | 28.87 → 28.89 | +0.16% | 72.19 → 72.24 |
| segment-occupancy/baseline | 0.00 → 0.00 | — | 3.94 → 3.97 |
| segment-occupancy/empty | 0.14 → 0.14 | -4.03% | 74.38 → 74.41 |
| segment-occupancy/empty-collect | 0.43 → 0.34 | -21.31% | 10.38 → 10.41 |
| segment-occupancy/filled | 25.36 → 24.77 | +0.01% | 74.25 → 74.28 |
| segment-occupancy/half-per-page | 0.06 → 0.06 | +2.29% | 74.38 → 74.41 |
| segment-occupancy/half-per-page-collect | 0.01 → 0.01 | -22.92% | 74.38 → 74.41 |
| segment-occupancy/one-per-page | 0.02 → 0.02 | -0.05% | 74.38 → 74.41 |
| segment-occupancy/one-per-page-collect | 0.01 → 0.00 | -24.18% | 74.38 → 74.41 |
| segment-occupancy/packed-same-live | 0.92 → 0.93 | +1.02% | 16.08 → 16.11 |
| segment-occupancy/tenth-per-page | 0.04 → 0.04 | -0.71% | 74.38 → 74.41 |
| segment-occupancy/tenth-per-page-collect | 0.01 → 0.00 | -23.06% | 74.38 → 74.41 |
| thread-churn/baseline | 0.00 → 0.00 | — | 3.97 → 3.93 |
| thread-churn/collect-abandoned | 0.49 → 0.52 | +10.25% | 6.80 → 5.09 |
| thread-churn/create-allocate-exit | 3.16 → 2.92 | -4.11% | 70.80 → 69.09 |
| thread-churn/free-after-owner-exit | 0.45 → 0.45 | +0.60% | 70.80 → 69.09 |

## Longer thread-churn confirmation

| Scenario / phase | Elapsed ms old → new | Median paired time change | Process RSS MiB old → new |
| --- | --- | --- | --- |
| thread-churn/baseline | 0.00 → 0.00 | — | 3.96 → 3.97 |
| thread-churn/collect-abandoned | 0.49 → 0.49 | +0.49% | 6.75 → 6.80 |
| thread-churn/create-allocate-exit | 2.76 → 2.77 | +0.50% | 70.75 → 70.80 |
| thread-churn/free-after-owner-exit | 0.44 → 0.44 | -0.39% | 70.75 → 70.80 |

## retention-aa observations

| Action | PSS after passive 10s MiB old → new | PSS after assisted 10s MiB old → new | Fill change | Refill change | Refill minor faults old → new |
| --- | --- | --- | --- | --- | --- |
| empty | 9.46 → 9.45 | 9.46 → 9.45 | control | control | 0.00 → 0.00 |
| idle | 77.28 → 77.28 | 77.28 → 77.28 | +3.87% | -1.76% | 98.00 → 97.50 |

## retention observations

| Action | PSS after passive 10s MiB old → new | PSS after assisted 10s MiB old → new | Fill change | Refill change | Refill minor faults old → new |
| --- | --- | --- | --- | --- | --- |
| empty | 9.45 → 9.45 | 9.45 → 9.45 | control | control | 0.00 → 0.00 |
| idle | 77.28 → 77.27 | 77.28 → 77.27 | +2.46% | +0.84% | 98.00 → 98.00 |
| other-collect | 77.28 → 77.28 | 13.22 → 15.08 | -2.44% | +0.80% | 97.00 → 80.00 |
| owner-collect | 77.28 → 77.28 | 13.16 → 13.15 | -1.28% | -0.24% | 114.00 → 114.00 |
| owner-exit | 77.28 → 77.28 | 15.04 → 15.04 | -1.63% | -0.49% | 96.00 → 96.00 |

## retention-aa-followup: every memory pair

| Round | First label | Settled PSS MiB old → new | New − old MiB |
| --- | --- | --- | --- |
| 1 | old | 15.10 → 15.09 | -0.005 |
| 2 | new | 13.22 → 13.22 | -0.004 |

Median paired settled PSS difference: -0.004 MiB; baseline-adjusted: +0.000 MiB.

## retention-followup: every memory pair

| Round | First label | Settled PSS MiB old → new | New − old MiB |
| --- | --- | --- | --- |
| 1 | old | 15.09 → 15.10 | +0.006 |
| 2 | new | 15.10 → 15.09 | -0.003 |
| 3 | old | 15.09 → 15.10 | +0.008 |
| 4 | new | 13.21 → 15.09 | +1.878 |
| 5 | old | 15.09 → 15.09 | +0.004 |
| 6 | new | 15.08 → 15.09 | +0.008 |

Median paired settled PSS difference: +0.007 MiB; baseline-adjusted: +0.001 MiB.
