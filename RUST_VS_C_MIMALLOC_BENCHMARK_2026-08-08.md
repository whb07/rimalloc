# Rust vs C mimalloc Benchmark Report

**Report date:** 2026-08-08 (EDT)  
**Benchmark source revision:** `8baba5eb6018ea8c56a47de6f1c20a7fa07f39a4`  
**Source revision date:** 2026-08-08T12:17:38-04:00  
**C baseline:** vendored mimalloc 3.3.2  
**Host:** AMD Ryzen 9 5950X, 16 cores / 32 logical CPUs  
**Rust compiler:** `rustc 1.92.0 (ded5c06cf 2025-12-08)`  
**C compiler:** GCC 13.3.0

## Purpose

This report records the complete `mimalloc-bench` `allt` comparison between
the current mechanical Rust translation and C mimalloc 3.3.2. It includes the
initial artifact-loading failure, the corrected one-run result, the requested
ten-run averages, and the limits on what these results establish.

Lower elapsed time is better. `cmi` denotes C mimalloc and `rmi` denotes the
Rust translation.

## Allocator artifacts

Both allocators were freshly built as release-mode interposing shared
libraries and selected through `LD_PRELOAD`:

- C: `target/bench-allocators/libmimalloc-c.so`
- Rust: `target/bench-allocators/libmimalloc-rust.so`

Artifact SHA-256 values used for the valid runs:

```text
f150433fa5b0b2c3a6107dbbc684270184ace98e8a38101b12aaacc47d6d68eb  libmimalloc-c.so
433befad287a04576316d1c1b6efbb8363946c8976ba4f16207422af9f08b4d4  libmimalloc-rust.so
```

The C artifact was built from `mimalloc-3.3.2` with CMake in `Release` mode,
with shared-library and allocator-override support enabled. The corrected Rust
artifact was built with:

```bash
cargo rustc --release --features interpose --lib -- \
  --crate-type cdylib -C link-arg=-lstdc++
```

The benchmark suite used 32 logical CPUs. `allt` selected 19 measured cases
from 18 workload families. Lean and `lean-mathlib` were automatically excluded
because Lean was not installed.

## Initial invalid run and Redis failure

The first Rust shared library was built using the repository's documented
interpose command without an explicit `libstdc++` link. The `interpose` feature
includes C++ `new`/`delete` overrides, and the resulting shared library had
unresolved C++ runtime symbols but no recorded `libstdc++.so.6` dependency.

Rust runs of `cfrac`, `espresso`, and `barnes` failed immediately with:

```text
symbol lookup error: libmimalloc-rust.so: undefined symbol: _ZTVSt9exception
```

At Redis, `redis-server` failed for the same reason, the client could not
connect, and the suite aborted. This was an allocator artifact-linkage problem,
not a Redis problem. No numbers from that run are valid.

The Rust artifact was rebuilt with `-C link-arg=-lstdc++`. `ldd` then showed
`libstdc++.so.6`, and a neutral `LD_PRELOAD=... /bin/true` smoke test passed.
The complete suite was restarted from the beginning.

Because the Rust interpose artifact loads the C++ runtime while the C artifact
does not, process startup and RSS comparisons can include that runtime's cost.
This is required by the current Rust interpose feature's C++ override surface
and should be remembered when interpreting small differences.

## Corrected one-run result

The first corrected complete run produced the following elapsed times:

| Benchmark | C | Rust | Rust vs C |
|---|---:|---:|---:|
| `cfrac` | 2.710s | 2.750s | 1.48% slower |
| `espresso` | 2.890s | 2.900s | 0.35% slower |
| `barnes` | 2.100s | 2.100s | tie |
| `redis` | 2.759s | 2.860s | 3.66% slower |
| `larsonN-sized` | 2.008s | 2.246s | 11.85% slower |
| `mstressN` | 1.620s | 1.620s | tie |
| `rptestN` | 1.466s | 1.361s | 7.16% faster |
| `gs` | 0.200s | 0.200s | tie |
| `lua` | 0.810s | 0.820s | 1.23% slower |
| `alloc-test1` | 2.060s | 2.200s | 6.80% slower |
| `alloc-testN` | 2.210s | 2.330s | 5.43% slower |
| `sh6benchN` | 0.150s | 0.100s | 33.33% faster |
| `sh8benchN` | 0.240s | 0.240s | tie |
| `xmalloc-testN` | 0.218s | 0.210s | 3.67% faster |
| `cache-scratch1` | 0.840s | 0.830s | 1.19% faster |
| `cache-scratchN` | 0.060s | 0.060s | tie |
| `glibc-simple` | 1.360s | 1.320s | 2.94% faster |
| `glibc-thread` | 0.422s | 0.428s | 1.42% slower |
| `rocksdb` | 3.730s | 3.760s | 0.80% slower |

That one run had a Rust/C geometric-mean ratio of `0.9875` (Rust 1.25%
faster), while the sum of its reported benchmark times was 28.335s for Rust
and 27.853s for C (Rust 1.73% slower). The apparent disagreement is caused
mainly by equal-weight geometric averaging and the large relative gain in the
very short `sh6benchN` case.

## Ten-run command

The requested aggregate used one suite invocation with ten full repetitions:

```bash
cd mimalloc-bench/out/bench
../../bench.sh --procs=32 -r=10 \
  --external=../../rust-vs-c.allocators allt
```

The run completed without crashes, nonzero exits, or missing samples. Each of
the 19 cases has exactly 10 C samples and 10 Rust samples. Values below are the
arithmetic mean elapsed times over those samples. The percentage is calculated
as `(Rust mean / C mean - 1) * 100`.

## Ten-run averaged results

| Benchmark | C mean | Rust mean | Rust vs C |
|---|---:|---:|---:|
| `cfrac` | 2.7360s | 2.7490s | 0.48% slower |
| `espresso` | 2.9000s | 2.9000s | tie |
| `barnes` | 2.1190s | 2.1210s | 0.09% slower |
| `redis` | 2.7257s | 2.7257s | tie |
| `larsonN-sized` | 2.0440s | 2.3046s | **12.75% slower** |
| `mstressN` | 1.6170s | 1.6250s | 0.49% slower |
| `rptestN` | 1.3336s | 1.3278s | 0.43% faster |
| `gs` | 0.2030s | 0.2040s | 0.49% slower |
| `lua` | 0.8110s | 0.8150s | 0.49% slower |
| `alloc-test1` | 2.0870s | 2.2340s | **7.04% slower** |
| `alloc-testN` | 2.2210s | 2.3620s | **6.35% slower** |
| `sh6benchN` | 0.1490s | 0.1090s | **26.85% faster** |
| `sh8benchN` | 0.2560s | 0.2580s | 0.78% slower |
| `xmalloc-testN` | 0.2165s | 0.2109s | **2.59% faster** |
| `cache-scratch1` | 0.8400s | 0.8320s | 0.95% faster |
| `cache-scratchN` | 0.0670s | 0.0680s | 1.49% slower |
| `glibc-simple` | 1.3650s | 1.3240s | **3.00% faster** |
| `glibc-thread` | 0.4253s | 0.4297s | 1.03% slower |
| `rocksdb` | 3.7870s | 3.7930s | 0.16% slower |

### Aggregate interpretation

- Geometric mean of the 19 per-benchmark Rust/C mean ratios: `0.9959`, or
  **Rust 0.41% faster**.
- Sum of the 19 C mean times: **27.9031s**.
- Sum of the 19 Rust mean times: **28.3927s**, or **Rust 1.75% slower** by this
  workload-duration-weighted summary.
- Exact classification: Rust was faster in 5 cases, slower in 12, and tied in
  2.
- With a practical +/-0.5% parity band: 4 Rust wins, 9 ties, and 6 Rust losses.

The cross-workload geometric mean is pulled in Rust's favor by `sh6benchN`,
where the relative improvement is large. The duration-weighted sum is pulled
in C's favor by `larsonN-sized`, `alloc-test1`, and `alloc-testN`.

### Variability

Most cases were stable. The noisiest elapsed-time samples by coefficient of
variation were:

| Benchmark | C CV | Rust CV |
|---|---:|---:|
| `rptestN` | 11.23% | 8.80% |
| `sh6benchN` | 8.04% | 10.98% |
| `cache-scratchN` | 7.21% | 6.20% |
| `sh8benchN` | 6.43% | 6.00% |

The suite reports short cases at relatively coarse precision, so the exact
magnitude of the `sh6benchN` result should not be overinterpreted. The direction
was generally favorable to Rust, but a higher-resolution targeted benchmark is
needed for a precise percentage. `rptestN` also varied enough that its 0.43%
average difference should be treated as parity, not a demonstrated Rust win.

The clearest results from this run are:

- Rust was substantially faster on `sh6benchN`, subject to short-test timing
  uncertainty.
- Rust was modestly faster on `xmalloc-testN` and `glibc-simple`.
- Rust was substantially slower than C on `larsonN-sized`, `alloc-test1`, and
  `alloc-testN`.
- Redis averaged to an exact tie at the suite's recorded precision.
- Most remaining differences were approximately 1% or less.

## Important clarification: no regression was established

The word **regression** was initially used incorrectly when describing the
current Rust-vs-C differences. These runs compare two implementations at one
point in time:

- Rust at source revision `8baba5eb6018ea8c56a47de6f1c20a7fa07f39a4`;
- C mimalloc 3.3.2.

They do **not** compare current Rust with an earlier Rust revision. Therefore,
this evidence establishes current relative slowdowns and speedups against C;
it does not establish that Rust became slower, and it cannot identify a commit
where a regression occurred.

`BENCHMARK_RESULTS.md` contains a historical June 16 ten-run result at Rust
commit `c73267593a5f2ddf727b761ccd3705d9f22cd7a8`. That report showed
`larsonN-sized` only 1.10% slower than C and showed both `alloc-test` cases
slightly faster than C. The difference from the current numbers is a signal
worth investigating, but it is not proof of a regression because the old and
new results were not collected as a controlled, matched old-Rust/new-Rust run
with identical artifacts and environment.

Determining whether and when a real regression occurred requires rebuilding an
older Rust revision and the current revision with the same toolchain and
artifact linkage, running them interleaved under the same conditions, and then
benchmark-bisecting the intervening commits if the slowdown reproduces.

## Evidence locations

At report creation time, the generated local evidence was:

- ten-run raw CSV: `mimalloc-bench/out/bench/benchres.csv`;
- corrected one-run log: `bench-allt-rust-vs-c-20260808-215615.log`;
- ten-run log: `bench-allt-rust-vs-c-r10-20260808-220001.log`.

The raw benchmark output and allocator build directory are ignored build
artifacts. This report preserves the measured values and methodology in the
tracked repository.
