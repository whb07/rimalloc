# THINGS_TO_TRY worklog

This is the experimental record for `THINGS_TO_TRY.md`.  It deliberately
records rejected work as well as retained work so that a later change does not
repeat a disproven optimization.

## Measurement protocol

- Compare one hypothesis at a time against its immediately preceding baseline.
- Use alternating A/B/B/A order, CPU-pinned where available, rather than
  batching all baseline runs before candidate runs.
- Capture Criterion median and p95 plus `perf stat` cycles, instructions,
  branches, branch misses, and L1D store misses when the host permits them.
- Repeat a measurement when its coefficient of variation, ordering effect, or
  counters disagree.  A noisy result is not accepted as a win.
- Retain a change only when it has a repeatable wall-time or resource win and
  no material regression in the directly affected benchmark set.  Validate
  retained Rust changes with format, Clippy, and the workspace test suite.
- Disassembly and text size are recorded whenever an experiment changes a hot
  loop, call boundary, inline policy, or code layout.

The worktree began at `deb8c8c7fe19d266331e3a4c80532d56a2db7016`.  Untracked
`BUFFER_SIZE_PERFORMANCE_REPORT.md` and `benchmark-random-8-8-16000-crt.txt`
predate this worklog and are outside these experiments.

## Experiment inventory

| ID | Proposal from `THINGS_TO_TRY.md` | Status | Result / evidence |
| --- | --- | --- | --- |
| 1.1 | Reject uninitialized threads before reclaim-option lookup | Rejected: noisy | No reproducible gain on this host. |
| 1.2 | Private inline option accessor on measured policy paths | Rejected: unmeasured | Candidate compiled and preserved diagnostics, but was reverted pending a valid release A/B run. |
| 1.3 | Carry collection state instead of publishing/reloading it | Rejected | Narrow local-list carry was 0.65% slower. |
| 1.4 | Empty-list remote batch direct-to-available path | Rejected: rare | Exact eligible state was 0.098% of detached tails in xmalloc-testN. |
| 1.5 | Cache reclaim inputs without a spilling context object | Complete: no viable candidate | 99.797% of reclaim attempts exit before cacheable inputs are read. |
| 1.6 | Revisit abandoned remote-free hot/cold placement | Rejected: regression | Removing only `#[cold]` was 1.58% slower in immediate xmalloc A/B. |
| 1.7 | Remote CAS ordering audit | Complete: no change | Existing audit passes; AcqRel/Acquire contract retained. |
| 2.1–2.2 | Hoist detached-chain invariants and prove bounds once | Complete: no change | Release assembly already hoists page geometry and capacity multiplication outside the node loop. |
| 2.3 | Cold-split corruption reporting | Rejected: regression | Correct release cold split, but 12.8% slower. |
| 2.4 | Remove surviving `PageCollectOwner` dispatch | Rejected: regression | Static dispatch removed owner-tag tests but regressed xmalloc-testN by 3.6–5.7%. |
| 2.5 | Use proven non-null block pointers in hot loops | Pending | Restrict to already-proven boundaries; inspect emitted null checks. |
| 3.1 / 2.5 | Proven-non-null default refill links | Rejected | Focused paired-allocation measurement was slightly slower. |
| 3.4–3.6 | Refill loop form, unroll factors, and density specialization | Pending | Compare pointer/count and 2/4/8-way candidates independently. |
| 3.7 | Sparse-store `prefetchw` | Unsupported on host | Required L1D store-miss attribution is unavailable. |
| 3.8 | Keep secure construction separate | Complete: no change | Distinct secure algorithm and ordinary core already preserved. |
| 4.1 | Preserve direct small lookup | Complete | Explicit non-change: existing assembly gate already protects it. |
| 4.2 | Cache usable size once | Complete: no change | Current allocation boundary already computes it once. |
| 4.3 | Allocation variants | Complete: no change | Hot public small paths already pass compile-time variants. |
| 4.4 | Dedicated `mi_free` core | Complete: no change | Export is already a two-instruction wrapper. |
| 4.5 / 3.3 | Typed page-extend private core | Rejected: noisy | Promising first pair did not reproduce. |
| 4.6 | Extended assembly gates | Complete: no change | Existing capture covers every specified public/route hot-path boundary; no retained new core warrants another gate. |
| 5.1 | Verify page-metadata alignment | Complete: no change | ABI layout/stride proves padding is not a permissible first move. |
| 5.2–5.3 / 5.5 | Store attribution, c2c, and queue-prefetch evidence | Pending | Measure first; no layout change or prefetch without quantified need. |
| 5.4 | Avoid wide page-metadata RMW | Complete: no change | Typed transactional stores are already narrower and safer. |
| 6 | Branch shaping / selective branchless candidates | Complete: no change | No concrete surviving branch justified a generic rewrite. |
| 7 | Inline/no-inline policy | Complete: no change | Existing split matches measured loop/ABI boundaries. |
| 8.1 | Generic maintenance counter equality | Rejected: near-neutral | Semantically valid, but 0.20% is below the acceptance bar. |
| 8.2–8.3 | Queue state and rejection order | Pending | Requires generic-workload diagnostics before source change. |
| 9.4 | Optional CPU-tuned build | Rejected on host | `target-cpu=native` showed no meaningful gain. |
| 9.1–9.3 | PGO, post-link ordering, alignment | Pending | Separate build-only experiments; generic artifact remains the acceptance target. |
| 10 | Explicitly deprioritized ideas | Complete | No implementation: they violate constraints, lack a measured target, or were previously harmful. |

## Results

Results are appended in experiment order. Each entry includes the exact
implementation shape, commands/artifacts, repetitions, noise assessment, and
retain/reject decision.

### 1.1 — Early remote-reclaim rejection

**Hypothesis.** In an uninitialized releasing thread, checking small/medium
block eligibility and thread initialization before reading
`page_reclaim_on_free` removes an option-wrapper call and relaxed option load
from a path that cannot reclaim.

**Candidate shape.** After `try_free_abandoned`, test
`block_size <= MI_MEDIUM_MAX_OBJ_SIZE && _mi_thread_is_initialized()` before
loading `page_reclaim_on_free`; initialized threads then use the unchanged
decision and reclaim flow. The patch was deliberately one conditional nesting
change and was reverted after measurement.

**Evidence.** CPU 0, ten flat Criterion samples per run, one workload iteration
per sample, `xmalloc-testN-per-1M-frees/mechanical_mimalloc_raw`, 1 s warm-up.
The baseline cdylib SHA-256 was `8a987a…e8cf9`; candidate was
`88fc6b…77d5de`. Medians (ns per million frees) were baseline 28,734,494 and
31,310,463; candidate 25,940,309, 29,856,177, and 28,229,618. The candidate
appeared both 9.7% faster and 3.8% slower than the earlier baseline, while the
two baseline runs themselves differed by 9.0%. A pinned-library B→C pair was
also 31.31→28.23 ms, but it cannot overcome that host-phase variance. `perf`
was available for cycles/instructions/branches/branch-misses but not
`L1-dcache-store-misses`.

**Decision.** Rejected (reverted). The hypothesis is behavior-preserving but
this host cannot demonstrate a repeatable gain under the current workload
controls. It may be retried only with a stabilized host or an A/B alternation
that produces overlapping within-variant variance and a consistent delta.

### 1.2 — Private inline option accessor

**Candidate shape.** Added an `#[inline(always)]` crate-private accessor,
including the existing diagnostics counter, and changed only
`page_candidate_limit`, `reclaim_on_free_mode`, and `reclaim_limit` to use it;
the C-compatible `#[inline(never)]` wrapper remained unchanged.

**Evidence.** The diagnostics read-count reference test passed. A wider test
command with `bench-diagnostics` cannot link this crate's intentional custom
`panic_impl` into its std integration-test matrix, so it is not valid evidence
for this candidate. No release A/B measurement was completed.

**Decision.** Rejected (reverted), pending a valid release measurement.

### 1.3 — Carry collection state

**Inspection.** `RemoteFreeBatch::collect_into_page` computes the new used
count and publishes both it and `local_free`, but returns only
`LocalFreeList`. The partial collector then reloads `local_free`, `free`, and
later `used` to decide the quick move/full collection.

**Candidate shape.** Carried the returned `LocalFreeList` through the partial
collector, removing two `local_free` rereads while preserving the error
fallback. All plain page-batch test-mode cases and `cargo check --all-features`
passed.

**Evidence.** CPU-pinned forced plain collection (len 128, nonempty
destination): candidate median 182.90 ns; immediately rebuilt baseline 181.72
ns. Candidate was 0.65% slower.

**Decision.** Rejected (reverted). The larger used-count result-value shape
remains untried, but this minimal state-carry form is not beneficial.

### 1.4 — Empty-list remote batch direct-to-available path

**Eligibility instrumentation.** A temporary `bench-diagnostics` counter ran
immediately after tail detachment in the partial collector. It counted only the
proposed direct-route precondition: nonempty detached tail with both `free`
and `local_free` empty. The counter and its hot-path condition were removed
after the measurement; neither is part of the retained allocator.

**Evidence.** CPU-pinned `xmalloc-testN-per-1M-frees` against an isolated
`interpose,bench-diagnostics` cdylib, ten flat Criterion samples plus the
warm-up record. Across 11 diagnostic records, there were 2,182,308,142
detached tails and 2,135,331 eligible direct-to-available states: **0.09785%**
(about one in 1,022). The records are retained locally at
`target/things-to-try-direct-available.jsonl` and were generated with a
pinned diagnostic cdylib; timing from this run is not used as performance
evidence because diagnostics add relaxed counter traffic.

**Decision.** Rejected before a collector rewrite. The extra branch would run
for every detached tail to optimize a case that does not cover the relevant
remote-free volume. It is better represented by the current local-list path
than by a new product-path specialization.

### 1.5 — Cache reclaim inputs

**Assembly and profile inspection.** The cacheable values (`is_in_threadpool`,
`mostly_used`, page memid, and exclusive arena) occur after the early
`_mi_thread_is_initialized` check in `try_reclaim_abandoned`. They could only
be carried to `try_reabandon_mapped` by changing the consuming error result to
include a context value; that would add a larger return shape and pressure to
the overwhelmingly common rejection path.

**Evidence.** The 11-record pinned xmalloc diagnostic run counted
2,182,392,300 reclaim attempts. 2,177,960,821 (99.79694%) failed because the
thread was not initialized. Only 4,431,479 (0.20306%) reached the later input
logic; 4,431,477 of those then failed the queue limit. Thus the duplicate
`mostly_used`/memid work is outside 99.8% of this workload's reclaim attempts.

**Decision.** Complete with no source change. A cache context cannot improve
the dominant path and risks spills and a wider internal ABI for a sub-0.21%
path. The existing scalar loads remain the most performant viable form until a
workload demonstrates that successful or cross-thread reclaim is material.

### 1.6 — Abandoned remote-free hot/cold placement

**Candidate shape.** Removed only `#[cold]` from
`collect_after_remote_free`; its `#[inline(never)]` call boundary and every
collection/reclaim/reabandon operation stayed unchanged. This isolates
placement from inlining or behavior changes.

**Evidence.** Matching pinned `interpose` cdylibs ran CPU-pinned
`xmalloc-testN-per-1M-frees` with ten flat Criterion samples. The non-cold
candidate median was 17.808 ms (95% interval 17.580–22.303 ms; noisy). The
immediate original-cold baseline was 17.531 ms (17.483–18.582 ms), making the
candidate 1.58% slower.

**Decision.** Rejected (reverted). The existing cold placement is faster for
the measured remote-free workload. A future hot-prelude/cold-tail split would
need a distinct, demonstrated common subpath before it is worth its added
layout complexity.

### 1.7 — Remote CAS ordering audit

**Evidence.** `scripts/check-atomic-ordering-audit.py` passed, covering 29
source files and 354 direct ordering uses. The typed `xthread_free` push,
ownership release, and detach CAS loops use AcqRel success and Acquire failure,
matching the documented compatibility contract.

**Decision.** Complete with no source change. Relaxing this order has no
demonstrated x86 instruction benefit and lacks an upstream correctness proof.

### 2.3 — Cold corruption reporting

**Candidate shape.** `report_remote_list_corruption` and
`report_local_list_corruption` are cold, out-of-line leaves. Success paths now
branch to them rather than allowing formatting/error machinery to affect the
normal collection loop's inlining and register allocation.

**Evidence.** Plain and secure page-batch test-mode builds passed. Release
assembly placed both reporters in `.text.unlikely` and left normal collector
paths with error-edge calls only. With the exact benchmark ID on CPU 0, ten
samples and 1 s warm-up/measurement, the candidate median was 279.91 ns;
the immediately rebuilt baseline was 248.23 ns. This is a 12.8% regression.

**Decision.** Rejected (reverted): the cold call harms this small forced
collection path despite producing the intended code layout.

### 2.1–2.2 — Detached-chain invariant and overflow hoisting

**Assembly inspection.** In the release collector that inlines
`scan_detached_chain`, the page capacity, block size, page start, and
`block_size × capacity` product are loaded/calculated before the traversal
backedge. The node loop uses those registers for the subtraction, range test,
and alignment remainder. The checked multiplication's overflow edge is also
outside that loop.

**Decision.** Complete with no source change. The proposed explicit cache
would duplicate values LLVM already keeps live, while the remaining per-node
address/range/alignment validation is necessary before reading each encoded
link. A power-of-two-only second loop would duplicate a safety-critical scan
and enlarge hot code without an unhoisted operation to remove.

### 3.1 / 2.5 — Default refill pointer proof

**Candidate shape.** The plain refill loop used `BlockPtr::new_unchecked` for
its already-proven non-null successor and emitted `None` for the final link
when the ordinary build's empty-list invariant holds. Secure behavior remained
on the existing path.

**Evidence.** Page-batch test mode and page tests completed. On CPU 0, paired
public alloc/free at 16 bytes with ten samples and 1 s warm-up/measurement,
candidate median was 45.734 µs (two mild outliers); immediately rebuilt
baseline was 45.587 µs. The 0.32% candidate regression is not a win.

**Decision.** Rejected (reverted).

### 3.7 — Sparse-store `prefetchw`

**Evidence.** `perf stat -e L1-dcache-store-misses` reports `<not supported>`
on this host. No equivalent store-miss event is exposed by `perf list`.

**Decision.** No source change. Prefetching sparse refill stores without the
required attribution would be architecture-specific speculation, not a
measured optimization.

### 3.8 — Secure construction separation

**Inspection.** `mi_page_free_list_extend_secure_typed` owns randomized slice
selection and encoded-link construction. The ordinary loop is a separate core;
the caller chooses them through compile-time `MI_SECURE` conditions.

**Decision.** Complete with no source change. Sharing the implementations would
put secure-only branches and state into the ordinary refill path.

### 4.2 — Cache usable size once

**Inspection.** `_mi_malloc_generic_typed` calls
`mi_page_usable_block_size(page)` once, solely to write the nullable usable-size
output before invoking `_mi_page_malloc_zero_typed`. The latter does not repeat
that query in the normal allocation path.

**Decision.** Complete with no source change. Introducing a local or variant
would not remove work and would be churn without a measurable target.

### 4.4 — Dedicated `mi_free` core

**Inspection.** Release `mi_free` is exactly `xor %esi,%esi; jmp
mi_route_free`: it passes a null usable-size output without a surviving branch.
The shared router must remain generic for `mi_ufree` and other callers.

**Decision.** Complete with no source change. Duplicating the full router to
specialize a wrapper that has no redundant branch would add code and layout
risk without an evidenced target.

### 4.3 — Allocation variants

**Inspection.** Public small malloc and calloc wrappers call the typed small
allocator with compile-time `Zeroed::No`/`Zeroed::Yes` and null usable-output
constants. The generic page allocator is a slow-path dispatcher shared by
sized/aligned requests.

**Decision.** Complete with no source change. Duplicating the generic helper
would create large copies without demonstrating a surviving hot-path branch.

### 5.1 — Page-metadata alignment

**Inspection.** Layout assertions fix hot fields at offsets 0x08 through 0x30;
`mi_page_t` is 0x78 in the ordinary layout (0x88 with encoded links) and the
metadata size helper rounds only to 16 bytes. Consecutive records therefore
rotate through cache-line offsets. Changing their stride, representation, or
field order is an ABI/layout change explicitly excluded by the experiment.

**Decision.** Complete with no source change. A future allocator-base alignment
experiment needs runtime address sampling and a separate compatibility review;
there is no justified field-layout edit.

### 5.4 — Wide page-metadata RMW

**Inspection.** Page batches publish `free` and `used` through separate typed
stores under consuming batch/lease capabilities; lease return commits both
inside one API. The fields have distinct concurrency and aliasing semantics.

**Decision.** Complete with no source change. A packed or 128-bit RMW would
write more cache-line state, require preserving unrelated fields, and erode the
existing proof boundary without removing a measured store.

### 8.1 — Generic maintenance equality

**Candidate shape.** Replaced `generic_count >= 1000` with equality. The
counter increments once and resets at 1000, so it cannot legitimately overshoot.

**Evidence.** CPU-pinned fixed-64 single-thread workload (200 samples):
candidate median 64.220 µs; immediately rebuilt baseline 64.346 µs. The 0.20%
candidate improvement is too small to establish a strong gain.

**Decision.** Rejected (reverted).

### 9.4 — Optional CPU-tuned build

**Candidate shape.** Isolated release benchmark build with
`RUSTFLAGS='-C target-cpu=native'`; no source or distributed artifact change.

**Evidence.** CPU-pinned fixed-64 workload median was 65.138 µs with 7.5%
outliers. Nearby generic baseline was 65.479 µs; the sub-1% difference is not
a strong portable result.

**Decision.** Rejected for this host. Generic build remains the only supported
artifact; any deployment-specific tuning needs its own target-host evidence.

### 6 — Branchless code

**Inspection.** `mi_likely`/`mi_unlikely` are intentional identity helpers;
small allocator arithmetic already uses branchless invariant-backed operations
where appropriate. No audited hot branch had a demonstrated misprediction or
instruction-count target, and prior broad branchless capacity work regressed.

**Decision.** Complete with no source change. Branch shaping remains part of
individual future candidates, not a global rewrite.

### 7 — Inlining policy

**Inspection.** Page extension, reclaim, and C-shaped wrappers are out of
line; small typed accessors are `inline(always)`; error paths are cold where
appropriate. The only mixed hot/cold candidate (`collect_after_remote_free`)
remains separately tracked as 1.6.

**Decision.** Complete with no source change. Blanket annotation changes would
alter code layout without a bounded, measured target.

### 2.4 — `PageCollectOwner` dispatch

**Baseline assembly.** Release assembly contained one shared
`collect_page_free_lists` body. It preserved the enum tag and executed two
owner-discriminant tests around remote and local batch operations.

**Candidate shape.** Replaced the private enum with a private static-dispatch
trait implemented only by `LocalPage` and `OwnedAbandonedPage`. The common
collector was generic over that trait; no dynamic dispatch, raw proof
reconstruction, ownership transition, or public API changed. Release assembly
then removed the owner-tag tests and reduced the collector prologue, confirming
that the generic code specialized as intended.

**Evidence.** A detached unmodified `HEAD` worktree and the candidate were
built as matching pinned `interpose` cdylibs (baseline SHA-256
`8a987a…e8cf9`, candidate `ddf84f…c642c`). CPU-pinned
`xmalloc-testN-per-1M-frees` used ten flat Criterion samples and one-second
warm-up/measurement configuration; the workload itself runs for five seconds
per sample. Alternating medians were baseline 17.245 and 17.474 ms per million
frees, candidate 18.234 and 18.095 ms. The first candidate interval was broad
(17.707–24.089 ms), so the second baseline/candidate pair was run after a
30-second cool-down. The confirmation remained slower: 18.095 versus 17.474
ms, a 3.55% regression; the first pair was 5.74% slower.

**Decision.** Rejected (reverted). The eliminated tag tests are outweighed by
the specialized code's layout and/or instruction-cache cost on the target
remote-free workload. The existing enum remains the smaller, faster form.

### 4.5 / 3.3 — Internal ABI-wrapper audit

**Evidence.** Internal `page.rs` callers at the active-page, initialization,
generic-allocation, and retirement paths call the C-shaped
`mi_page_extend_free` wrapper, which reconstructs typed pointers before
dispatching to `mi_page_extend_free_typed`. This is a genuine private-core
candidate. By contrast, `mi_free` is already a compact wrapper and is not a
useful split target.

**Candidate shape.** Routed three callers that already held `ThreadHeapLocal`
and `PagePtr` proofs directly to `mi_page_extend_free_typed`; raw-pointer
callers retained the noinline ABI wrapper. `cargo check --all-features` passed.

**Evidence.** CPU-pinned fixed-64 workload: first candidate 62.686 µs versus
immediate baseline 65.479 µs (4.27% faster). The confirming candidate run was
noisy: median 64.309 µs with 11.738 µs standard deviation. It cannot validate
the apparent win.

**Decision.** Rejected (reverted) as non-reproducible.

### 4.6 — Extended assembly gates

**Inspection.** `scripts/capture-release-assembly.sh` captures the requested
public and routing boundaries: `mi_malloc`, `mi_malloc_small`, `mi_free`,
`mi_route_free`, `mi_free_block_local`, and `_mi_theap_malloc_zero_ex`. The
release audit verifies the façade symbols required by the strict-wrapper
policy, while the two route symbols cover the local-free path.

**Decision.** Complete with no source change. None of the experiments so far
retained a new hot loop or wrapper, so adding a new static assembly requirement
would not guard a proven optimization. Any retained candidate that introduces
or changes a hot core must add its exact symbol to this capture before merge.
