# Borrowed allocator operations

The [expanded normal-mode comparison](full-suite/README.md) now includes all 21
native vendor cases, Redis/RocksDB, all 36 Nginx profile cases, all three axum
cases, architecture and retention. All primary aggregates meet the 1% review
limit. A longer debug-palloc header confirmation is +22.66%; a same-baseline
control also varies substantially, so that case remains inconclusive for a
no-regression claim. The original screen and microbenchmark findings below remain
visible. Full secure-mode application coverage is not claimed.

Baseline: `4219da2bcdbba7f9273c46b5dbc9e194f343464a`.
Measured candidate: `70a3ce3ab5980660ddd4c9f0ace1d5052b80d993`.
Both use Rust 1.92.0 / LLVM 21.1.3 on the same Linux x86_64 Azure VM
(AMD EPYC 7763). Production allocator builds were captured before timing from clean
checkouts; the baseline page fixture includes the documented parity patch below.
Build commands, features, tool versions, binary hashes, controllers,
probe sources, raw timings, and verification logs are embedded in the manifests
and results. Absolute paths identify the original experiment; temporary build
paths need not remain available.

The implementation retains borrowed bitmap storage through arena claim,
dirty/commit, and rollback operations; actual page-field loans and bounded
`MaybeUninit<u8>` storage through allocation initialization; and actual list-field
loans through detached-list collection and publication. Raw compatibility and
physical storage/link operations retain unsafe boundaries. Persistent allocator
layouts, public interfaces, and synchronization protocols are preserved.

Production lexical counts, masking comments, strings, and test items and counting
all configuration branches, decrease from 4,672 to 4,549: **88 unsafe blocks and
35 named unsafe function definitions removed**, with unsafe impls unchanged.
The 54 unsafe function-pointer types are reported separately and unchanged.
Intermediate count snapshots and the counting script are included.

Final screen results (positive means slower):

| Suite | Normal | Secure |
| --- | ---: | ---: |
| Native workloads (11 normal / 6 secure) | -1.81% | -0.95% |
| Resource probe (2 cases) | -0.49% | -0.16% |
| Arena churn (3 cases) | -0.19% | +0.02% |
| Default-linked Rust facade (2 cases) | +5.44% | +0.45% |
| Single-fixture Criterion collection (18 cases) | +0.31% | +1.10% |

All native, resource, and arena cases pass both review limits. The default
normal Rust facade and secure single-fixture collection aggregates exceed the
1% limit. The five individual screen findings above 3% are:

| Case | Slowdown |
| --- | ---: |
| Rust facade, normal, 16 bytes | +5.43% |
| Rust facade, normal, 64 bytes | +5.44% |
| plain_links/local_to_available/empty/8 | +3.31% |
| secure_encoded/detached_remote/empty/8 | +8.60% |
| secure_encoded/detached_remote/nonempty/8 | +7.52% |

The independently planned diagnostic controls give:

| Control | Normal | Secure |
| --- | ---: | ---: |
| Rust facade, all four matched caller placements | +0.23% | Not measured |
| Collection, all four matched placements, batched fixtures | +0.07% | -0.42% |
| Collection, default-linked batched fixtures | +0.53% | -0.18% |
| Resource confirmation, 200M operations, 11 paired rounds | -0.19% | Not measured |

Every case averaged across the four matched placements is inside the review
limits. The normal collection control's originally flagged local eight-block
case is -0.26%; the secure remote eight-block cases are +0.30% and +0.15%.
Normal collection remains placement sensitive: its default batched probe has
one +7.74% case, and individual matched placements reach +5.91%, even though
the combined case ratios pass. All placements are retained, with none selected
for its performance. These results support layout/timer sensitivity rather
than a consistent increase in allocator work. They do **not** clear every
default-linked microbenchmark or establish a blanket zero-regression claim.
The original screen findings remain above their stated review limits.

The final default-linked facade loops have identical normalized instruction
sequences in both feature modes; instruction order, scalar immediates, and
within-function branch targets are preserved by the comparison. Normal-mode
caller placement changes from byte 16 to byte 0 within a cache line. This is
code-generation evidence for the placement diagnosis, not a claim that the
observed default-linked slowdown is absent.

The longer resource confirmation measures +0.05% for allocation/free and -0.43%
for remote free. In the short resource probes, median process RSS/commit fields
match between labels within every feature/scenario pair, and median page-fault
deltas are zero. For normal allocation/free, both labels' median current RSS
starts at 3,538,944 bytes and ends at 3,801,088 bytes. These are process
observations, not allocator-internal counters.

The suite summaries and every final holdout ratio are independently recomputed
from raw times/samples. Post-run input fingerprint checks cover all three
experiments and the cached native harness. Final source and library identities
are checked against the committed implementation.

The initial candidate, `14a22aef5973aa98b8cd092f1d303b083edc08b6`, exceeded several
screen budgets. Its complete experiment is retained under `initial/`, including
adverse runs and the subsequent diagnostic measurements. In particular, the
default-linked 64-byte Rust facade probe was 7.14% slower. Software sampling
attributed most of the extra time to its unchanged caller loop. The initial
four-placement control measured -0.30% across the 64-byte cases and +0.78% across
both sizes. Those controls test layout sensitivity; they do not erase the
observed default-linked result.

Collection disassembly also exposed a concrete code-generation cost: an outlined
`PageCollection::collect_head` forced temporary field-view storage onto the stack.
Forcing this method inline removed that materialization. The fixture entry's
stack reservation (excluding saved registers) changed from 80 bytes in normal
mode and 96 in secure mode to 40 bytes in each, matching the baseline reservation.
Physical chain scanning retains its unsafe storage/link boundary; secure mode
emits a separate scan kernel. No production alignment padding was added. The final experiment repeats the complete comparison after
this change.

The intermediate candidate `076ef273adc3e3216690d5b207919fa94b819f70` is
retained under `before-bitmap-abi/`. Its longer resource holdout still measured
+1.84% across the two normal scenarios. The final source passes borrowed headers
and chunk slices directly to private outlined bitmap kernels. Arena allocation
stack reservation decreases from 136 to 88 bytes, and static stack stores from
26 to 19; the baseline reservation is 40 bytes. Static instruction count rises
from 246 to 259 (baseline 207), so stack reduction alone is not a runtime claim.
A separately recorded
three-version preflight exercises all six label orders twice. Against baseline,
it measured -0.02% for allocation/free, -0.94% for remote free, and -0.48%
aggregate. The exact preflight patch and library hash match the committed final
source and its clean release build. This preflight is separate from the final
full comparisons.

The 1% aggregate and 3% per-case slowdown review limits were recorded before
timing and retained throughout. Ratios are candidate time / baseline time;
positive percentages mean slower. Cases use the median paired ratio across
rounds, and suite aggregates are equal-weight geometric means of case ratios.
The native apparent gains include high-variance Larson workloads and are not
claimed as a reproducible speedup. Baseline/candidate order alternates by round.
Over-budget screen results require
investigation and remain visible even when a separate holdout resolves them.

The native screen covers 11 normal scenarios with five paired rounds and six
bounded secure scenarios with nine rounds. Short scenarios retain three raw
runs and discard the first as warmup using the unchanged baseline harness;
timed scenarios use three normal or five secure seconds. The Rust facade uses
nine paired rounds, 4,096 live allocations, and sizes 16 and 64. The layout
control uses all four 16-byte caller placements within one cache line, nine
paired rounds, and 8,192 batches, applying each placement equally to both labels.
No placement is selected for its performance.

Arena churn creates and destroys heaps, allocates and touches memory, and forces
collection in nine paired rounds for each of three size/live-set combinations.
Page collection measures all 18 cases per mode in three paired rounds, using
20 Criterion samples per case. The separately documented baseline fixture patch
adds the zero-flag store performed by the candidate fixture; the normal baseline
allocator libraries are pristine. Resource probes use seven paired rounds and
20 million operations. The final single-thread resource probe is pinned to CPU
0, correcting the initial experiment's allowance to migrate between CPUs 0 and
2. Remote probes retain CPUs 0 and 2. Other single-thread probes use CPU 0. The final
resource comparison uses the same library alias path and `paired` mode string
for both labels. The controller changes the alias target only between serialized
runs and records the actual library and comparison label outside the probe.
This removes label-dependent input-length/path differences; earlier experiments
and the preflight used their original distinct paths and remain separate. The
full timing results, rather than the preflight alone, determine acceptance.
Builds and correctness checks finish before timed suites, which run sequentially.

Separate collection holdouts retain every one of the 18 cases, nine paired
rounds, and the default-linked binary plus all four matched placements. The
matched binaries place fixture entry, outlined scan kernel, caller, and diagnostic
counter storage at the same addresses for both labels; actual addresses and
instructions are checked before timing. These holdouts prepare 32 independent
fixtures outside each interval and time 32 transitions together, repeating
8,192 batches after warmup. This changes timer amortization and live fixture
count equally for both labels. It is a separate workload and cannot replace the
original single-fixture Criterion findings. The additional normal-mode holdout
was planned after its final screen flagged the eight-element local case and
before any holdout build or timing. Normal-mode fat LTO initially inlined the
entry and scanner, so its corrected probe retains the entry through the same
black-boxed indirect call in both labels and matches the actual outlined remote
kernel, `RemoteFreeBatch::from_detached`. The correction preceded all normal
control timing and is recorded with the untimed setup failures. Secure mode
retains the direct call and standalone scanner. All placements and cases remain
in the data.

Formatting, workspace Clippy, all-target/all-feature workspace tests, constructor
compile tests, C behavior parity, API/ABI/FFI checks, no-std checking, concurrency
models, ASan, TSan, and full Miri all passed for the final Rust source. Miri uses
eight configurations: stacked/tree borrows, with/without strict provenance, in
normal and secure/guarded feature sets, with symbolic alignment enabled. Every
normal configuration passes 249 tests and every secure configuration passes 256,
with 60 OS-dependent ignores each. Earlier source versions' verification is
recorded separately in their archives. Nightly checks require
`-Aunused-unsafe` for a pre-existing CPU-intrinsic lint change; the mandatory
stable checks pass without that override.

This is evidence for the listed workloads on one host. The complete secure
vendor suite is not covered. Hardware cycle/instruction counters are unavailable
on this VM; profiles use software CPU-clock sampling. Internal allocator resource
counters are unavailable in the neutral C probe and remain null; only OS/process
resource observations can be compared. Microbenchmarks and code generation do
not establish zero cost for every application.

`manifest.json` and `results.json` contain the final experiment. `initial/`
retains the original experiment, profiles, layout controls, and unexecuted plans
clearly marked as superseded. `before-bitmap-abi/` retains the complete intermediate
experiment and its later profiling and controlled holdouts. Large executables,
shared libraries, and `perf.data` files
are identified by their build inputs or hashes rather than embedded.
