# Sampled `memcpy`/`memmove` buffer sizes

Initial sample: 2026-07-14 at `deb8c8c7fe19d266331e3a4c80532d56a2db7016`  
Complete-suite follow-up: 2026-08-08 at `865b26c22f37fdc9d85f4be50cf1a99783f500d1`

This is a workload sample for choosing tuning points, not a release benchmark.
The tracer records the requested length at exported `memcpy`/`memmove` calls.
The general samples use exact bins below 256 bytes and ranges above that. A
focused Redis pass used exact bins through 64 KiB, which is enough to classify
every Redis copy in this sample. Redis was sampled three times for stability,
followed by the refined exact-size pass; the other applications were sampled
once. Rust and C mimalloc produced the same distributions to the precision
shown.

## Redis server

| operation | calls | total bytes | mean | p50 | p90 | p99 | p999 |
|---|---:|---:|---:|---:|---:|---:|---:|
| `memmove` | 900,342 | 3,678,354,337 | 4,085.5 B | ~4,088 B | ~7,352 B | ~8,088 B | ~8,168 B |
| `memcpy` | 1,414,714 | 17,176,273 | 12.1 B | ~1 B | ~6 B | ~9 B | ~3.6 KiB |

`memmove` bucket share:

| requested length | share |
|---|---:|
| 0–2 KiB | 25.09% |
| 2–4 KiB | 25.05% |
| 4–8 KiB | 49.87% |
| above 8 KiB | ~0% |

`memcpy` exact small-size share:

| requested length | calls | share |
|---|---:|---:|
| 0 B | 4 | 0.00% |
| 1 B | 1,005,824 | 71.10% |
| 2–8 B | 312,311 | 22.08% |
| 9–64 B | 89,193 | 6.30% |
| 65 B and above | 7,382 | 0.52% |

Exact power-of-two classification from the focused Redis pass:

| operation | power-of-two sizes | non-power-of-two sizes |
|---|---:|---:|
| `memmove` | 1,151 (0.13%) | 899,191 (99.87%) |
| `memcpy` | 1,018,137 (71.97%) | 396,569 (28.03%) |

The Redis client is a different shape: its sampled `memcpy` mean was about
529 B, with p50 about 144 B and p90 about 1,536 B. Do not mix client and server
traffic when selecting fast paths.

## Espresso

Espresso was the other workload with substantial exported copy traffic:

| operation | calls | total bytes | mean | p50 | p90 | p99 | p999 |
|---|---:|---:|---:|---:|---:|---:|---:|
| `memcpy` | 83,199 | 28,578,823 | 343.5 B | ~168 B | ~520 B | ~2,056 B | ~16,392 B |

Most frequent sampled ranges were 128–143 B (25,720 calls), 160–175 B
(18,440), 256–271 B (16,000), and 512–527 B (7,220).

## Other applications

* Cfrac produced only three `memmove` calls (two at 16–31 B and one at
  32–47 B) and a few small `memcpy` calls.
* Barnes, larson-sized, mstress, rptest, alloc-test, glibc-simple/thread,
  sh6/sh8, xmalloc, and cache-scratch produced no meaningful exported
  `memmove` sample in this run.
* RocksDB `db_bench --benchmarks=fillrandom` produced no exported `memmove`
  calls and one exported 1,024-byte `memcpy` in the C run. Its copies are
  commonly inlined or hidden behind other code, so this workload cannot provide
  a reliable copy-size distribution through PLT interposition.

The same exact-bin pass classified allocator request sizes in the other
workloads. These are request sizes (`malloc`/`calloc`/C++ `new`), not copy
lengths:

| workload | operation | sampled calls | non-power-of-two calls | share |
|---|---|---:|---:|---:|
| larson-sized | C++ `new` | 129,197,280 | 128,285,223 | 99.29% |
| glibc-simple | `malloc` | 96,000,004 | 4 | ~0.00% |
| glibc-thread | `malloc` | 59,292,746 | 42,935,329 | 72.41% |
| mstress | `malloc` | 6,750,227 | 135,251 | 2.00% |
| mstress | `realloc` | 200 | 200 | 100.00% |
| rptest | `calloc` | 750,879 | 748,239 | 99.65% |
| alloc-test (1 thread) | C++ `new` | 50,130,729 | 41,771,888 | 83.33% |
| alloc-test (8 threads) | C++ `new` | 400,131,000 | 333,408,208 | 83.32% |
| sh6 | `malloc` | 193,000,018 | 118,000,017 | 61.14% |
| sh8 | `malloc` | 437,053,204 | 76,885,282 | 17.59% |
| xmalloc | `malloc` | 109,619,337 | 26,757 | 0.02% |

The focused class-counter pass evaluates the exact request length before
histogram bucketing, including requests above 64 KiB. Across the ten Rust
workload samples it classified 742,197,126 non-power-of-two requests out of
1,481,926,671 (50.08%). The corresponding C sample was 729,555,351 out of
1,468,102,050 (49.69%). The small difference is normal for these stochastic
workloads; the per-workload percentages are the useful tuning signal.

## Complete mimalloc-bench follow-up

The follow-up ran every workload defined by `mimalloc-bench/bench.sh` three
times with the current Rust allocator. The preload order was the copy tracer,
the mechanical-mimalloc interposition library, and then libc. This order sees
application calls and exported calls made while servicing allocator operations.
The tracer also classifies fortified `__memcpy_chk`/`__memmove_chk` calls under
the corresponding base operation.

Twenty-four workload groups completed all three runs. Five were unavailable:
`lean` and `lean-mathlib` lacked the Lean toolchain, `linux` lacked its source
tree, and `spec`/`spec-bench` lacked a SPEC CPU 2017 installation. No completed
run failed or timed out. The `security` group is reported separately because
its deliberately adversarial copies are not representative tuning traffic.

`allt` is not the complete suite: it selects only 18 of the 29 defined groups.
The follow-up explicitly selected every group, including `larson`,
`malloc-large`, `mleak`, `rbstress`, `cthrash`, `z3`, and `security`.

### Traffic by workload

These are run-one counts; the other two runs were identical except for the
stochastic RocksDB and Ruby stress workloads. Counts at or below 135 calls were
mostly the fixed process/allocator startup pattern and are retained rather than
silently reported as zero.

| workload | `memcpy` calls | `memmove` calls | classification |
|---|---:|---:|---|
| rbstress | 146,288,707 | 10,361 | substantial; Ruby interpreter plus stress script |
| RocksDB fillrandom | 11,573,227 | 45,428 | substantial |
| Lua build | 8,080,333 | 302,227 | substantial; compiler/linker child processes included |
| Redis server | 1,414,213 | 900,342 | substantial |
| Ghostscript | 347,614 | 153,140 | substantial |
| Z3 | 156,778 | 82,847 | substantial |
| Espresso | 83,199 | 0 | substantial |
| security | 9,006 | 421 | synthetic; excluded from tuning weights |
| alloc-test | 135 | 0 | startup-dominated |
| cscratch, cthrash, mleak | 118 each | 0 | two-variant startup pattern |
| Barnes, glibc-simple/thread, larson, larson-sized, malloc-large, mstress, rptest, sh6/sh8, xmalloc | 59 each | 0 | fixed startup pattern only |
| Cfrac | 64 | 3 | negligible |

The three rbstress `memcpy` totals were 146,288,707, 146,290,309, and
146,290,837. RocksDB totals were 11,573,227, 11,568,781, and 11,573,557;
its `memmove` totals were 45,428, 45,365, and 45,389. All substantial
non-stochastic workloads had identical operation counts across the three runs.

### Most frequent exact lengths

| workload | operation | calls | six most frequent requested lengths |
|---|---|---:|---|
| Espresso | `memcpy` | 83,199 | 128 B (30.9%), 160 B (22.2%), 256 B (19.2%), 512 B (8.7%), 224 B (4.2%), 1,024 B (3.9%) |
| Ghostscript | `memcpy` | 347,614 | 4 B (36.3%), 1 B (12.3%), 16 B (8.5%), 3 B (7.6%), 7 B (5.6%), 5 B (5.5%) |
| Ghostscript | `memmove` | 153,140 | 12 B (76.9%), 1 B (6.6%), 2 B (4.8%), 0 B (3.3%), 3 B (1.6%), 8 B (1.1%) |
| Lua build | `memcpy` | 8,080,333 | 8 B (67.3%), 16 B (4.4%), 128 B (4.2%), 88 B (3.8%), 176 B (1.3%), 2 B (1.3%) |
| Lua build | `memmove` | 302,227 | 0 B (72.6%), 8 B (17.7%), 16 B (4.4%), 24 B (2.2%), 4 B (1.1%), 32 B (0.5%) |
| rbstress | `memcpy` | 146,288,707 | 2 B (34.5%), 1 B (27.6%), 4 B (17.2%), 8 B (13.8%), 16 B (3.4%), 14 B (3.4%) |
| rbstress | `memmove` | 10,361 | 0 B (54.1%), 8 B (10.6%), 16 B (1.4%), 17 B (0.9%), 20 B (0.8%), 19 B (0.8%) |
| Redis | `memcpy` | 1,414,213 | 1 B (71.1%), 6 B (14.2%), 5 B (7.1%), 9 B (6.3%), 8 B (0.7%), 7 B (0.1%) |
| RocksDB | `memcpy` | 11,573,227 | 24 B (26.1%), 100 B (23.8%), 16 B (17.9%), 7 B (8.7%), 131 B (8.6%), 3 B (6.4%) |
| RocksDB | `memmove` | 45,428 | 32 B (99.6%), 28 B (0.2%), 56 B (0.1%), 4 B, 16 B, and 8 B (each below 0.1%) |
| Z3 | `memcpy` | 156,778 | 8 B (37.6%), 16 B (29.9%), 12 B (8.4%), 160 B (3.4%), 24 B (1.9%), 224 B (1.2%) |
| Z3 | `memmove` | 82,847 | 32 B (24.4%), 48 B (23.2%), 8 B (10.1%), 64 B (8.7%), 112 B (5.9%), 56 B (4.9%) |

Redis `memmove` remains a dense distribution rather than an exact-size case:
25.0% was 2,049–4,096 B and 49.8% was 4,097–8,192 B. Ghostscript `memcpy`
was 77.0% in 1–8 B, while Z3 `memmove` was concentrated in 17–128 B.

### Alignment and overlap

Alignment varies enough that aligned-only results cannot choose the general
implementation. The share where both addresses were 16-byte aligned was 99.9%
for Espresso `memcpy`, 62.3% for Lua `memcpy`, 23.6% for Z3 `memcpy`, 16.8%
for rbstress `memcpy`, 1.3% for RocksDB `memcpy`, and 1.3% for Redis `memcpy`.
Almost none of Redis's `memmove` traffic had both addresses even 8-byte
aligned; its dominant destination/source offsets modulo 64 were 12/10.

| workload | `memmove` overlap shape |
|---|---|
| Redis | 99.9% overlapping with destination above source |
| Ghostscript | 99.6% non-overlapping |
| Lua build | 92.8% non-overlapping; 7.0% destination above source |
| RocksDB | 96.9% non-overlapping; 3.1% destination above source |
| Z3 | 76.9% non-overlapping; 21.7% destination above source |
| rbstress | 65.0% non-overlapping; 13.4% destination below and 21.5% above source |

This makes overlap direction a required `memmove` benchmark dimension. A
non-overlapping-only microbenchmark would represent RocksDB and Ghostscript but
would miss the dominant Redis case.

## Tuning takeaways

Use per-workload weighting; do not aggregate all calls into one histogram or
rbstress alone will dominate it. The complete pass supports these initial
microbenchmark cases:

* tiny `memcpy`: exact 1, 2, 3, 4, 5, 6, 7, 8, 12, 14, and 16 B;
* small/medium `memcpy`: exact 24, 88, 100, 128, 131, 160, 176, 224, 256,
  512, 1,024, and 2,048 B;
* tiny `memmove`: exact 0, 1, 2, 8, 12, 16, 24, 32, 48, 56, 64, and 112 B;
* large `memmove`: a dense 2–8 KiB sweep, including 4 KiB and 8 KiB;
* source/destination alignment combinations drawn from the table above;
* non-overlap and both overlap directions, especially destination-above-source.

Zero-length `memmove` is common in Lua and rbstress, so its return path should
be measured separately, but it should not be mixed into copied-byte throughput.
There is still no evidence for one magic threshold: the suite contains several
distinct regimes, from one- and two-byte copies through medium fixed sizes to
Redis's dense 2–8 KiB overlapping moves.

The follow-up changes the earlier RocksDB conclusion. With the tracer ahead of
the current Rust allocator, RocksDB exposes substantial exported copy traffic.
That result is repeatable, but callsite sampling is still needed to separate
application-originated calls from allocator-service calls before giving
RocksDB a large cross-workload weight.

## Measurement caveat

The tracer observes exported `memcpy`, `memmove`, `__memcpy_chk`, and
`__memmove_chk` calls only. It does not see compiler-inlined copies, vectorized
loops, `rep movsb` generated inside a caller, or hidden libc-internal copying.
The Lua result describes a build workload, and the rbstress result includes the
Ruby interpreter; they are valid end-to-end workloads but not source-isolated
application distributions.

The reproducible tooling is provided as `scripts/copy-size-tracer.c`,
`scripts/run-copy-size-suite.py`, and `scripts/summarize-copy-sizes.py`. Exact
bins cover 0–64 KiB, with power-of-two ranges above that. Raw per-process TSVs,
run logs, status records, aggregate CSV, and generated Markdown are in the
ignored `target/size-analysis/` directory when measurements are present.
