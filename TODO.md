# Mechanical mimalloc performance parity plan

This is the ordered work plan for improving the mechanical Rust translation of
mimalloc 3.3.2. It is written so that one engineer can execute each experiment
independently, leave a reproducible record, and make an unambiguous keep/reject
decision.

The goal is not to win particular benchmarks. The goal is to make common
allocator operations do less real work while preserving mimalloc's semantics,
safety model, memory behavior, ABI, and portability. Benchmarks are evidence
for or against a general mechanism; they are not the product being optimized.

## Current evidence

The old complete ten-run comparison at Rust commit `8baba5e` is not a current
HEAD baseline. Since that run:

- the C++ allocation shim was compiled with optimization and the common
  `_Znwm`/`_Znam` success path was restored;
- normal page-queue movement was changed from generic remove/reinsert to an
  invariant-preserving in-place relink;
- targeted C comparisons reduced the remaining Larson gap to about 3.62% for
  unsized Larson and 1.78% for sized Larson;
- the remaining Larson artifact retired about 4.3% to 4.9% more instructions
  and 11.2% to 11.8% more branches per operation than C;
- profiling associated most of the remaining excess with page collection and
  abandoned-page lifecycle work, not cache misses or weaker atomic code;
- several intuitive changes have already failed measurement and must not be
  repeated without materially new evidence.

Read these before starting:

- `RUST_VS_C_MIMALLOC_BENCHMARK_2026-08-08.md`
- `LARSON_REGRESSION_INVESTIGATION_2026-08-08.md`
- `MIMALLOC_BENCH_HOTSPOT_REPORT.md`
- `BENCHMARK_RESULTS.md`
- `docs/performance.md`
- `docs/memory-ordering.md`
- `docs/rust-safety-model.md`

## Non-negotiable rules

Apply these rules to every item below.

1. Work on one hypothesis at a time.
   - Start from the latest accepted commit.
   - Make one mechanism change in one candidate.
   - Do not combine two individually unproven ideas.
   - If a candidate is rejected, abandon that candidate branch or worktree. Do
     not leave pieces of it in the next experiment.

2. Do not recognize benchmarks in production code.
   - No workload names, benchmark feature checks, magic thread counts, or
     benchmark-specific size sequences in allocator policy.
   - Do not change benchmark duration, operation count, allocation mix, or
     correctness checks to improve a result.
   - A new size or count threshold is allowed only when it expresses a general
     allocator invariant or matches an upstream mimalloc policy, and the reason
     is documented independently of benchmark results.

3. Preserve semantics and safety.
   - Do not weaken atomic ordering.
   - Do not remove pointer, list, ownership, corruption, or overflow validation
     unless a stronger invariant makes that validation redundant and the proof
     is represented in the type/API boundary.
   - Preserve C and C++ allocation failure behavior, `new_handler`,
     `bad_alloc`, alignment, sized-delete, option mutability, statistics, and
     public ABI behavior.
   - Keep secure, guarded, tracking, debug, and default modes correct.

4. Do not buy CPU speed with uncontrolled memory growth.
   - Record RSS, reserved, committed, page-committed, abandoned pages, purge
     calls, page searches, and page faults where applicable.
   - Reject a CPU improvement that creates a repeatable memory-policy
     regression unless the tradeoff is an intentional upstream-compatible
     policy decision reviewed separately.

5. Diagnostics must disappear from production artifacts.
   - Internal counters belong behind `bench-diagnostics` or external profiling.
   - Default release builds must gain no diagnostic atomics, branches, storage,
     symbols, or initialization.
   - Never use a diagnostics-enabled artifact for the final timing claim.

6. Prefer the C algorithm, but compare generated work rather than source style.
   - The Rust implementation may use types and ownership witnesses that C does
     not have.
   - A wrapper is not a problem if LLVM erases it.
   - A shorter-looking implementation is not a win if it adds calls, branches,
     spills, locked instructions, code size, or retained memory.

7. Record rejected results.
   - A failed experiment is useful evidence.
   - Record the exact baseline/candidate commits and artifact hashes, commands,
     raw results, codegen change, correctness result, and rejection reason.
   - Never describe a noisy or order-dependent result as an improvement.

## Definition of an accepted performance change

A production change may be kept only when all of the following are true:

- The change has a general allocator explanation, such as eliminating a
  redundant scan, atomic operation, metadata transition, option read, or
  allocation-boundary call.
- Assembly or mechanism counters confirm that the claimed work was actually
  removed. Moving samples to another symbol is not sufficient.
- Two independent balanced baseline/candidate comparisons show the same
  direction. At least one must reverse allocator order.
- The wall-time result agrees with the mechanism counters. A neutral wall-time
  result may be considered only when instructions/cycles and code complexity
  clearly decrease and every guard workload remains neutral.
- No held-out workload has a repeatable regression larger than its established
  noise. Treat a repeatable 1% regression as material even if an aggregate is
  positive.
- Full correctness, ABI, concurrency, C++ semantic, lint, format, and test gates
  pass.
- Resource evidence shows no unexplained RSS, committed-memory, abandonment,
  purge, or page-fault regression.

Do not use cross-workload geometric mean alone. Very short tests can dominate
relative aggregates. Report per-workload results, a duration-weighted summary,
variation, and resource changes.

## Standard experiment layout

Use a distinct ID such as `E03-local-to-available`. Store ignored artifacts
under `target/perf-experiments/<ID>/`:

```text
target/perf-experiments/<ID>/
  README.md
  environment.txt
  baseline-commit.txt
  candidate-commit.txt
  hashes.txt
  commands.txt
  baseline/
  candidate/
  assembly/
  perf/
  focused-results/
  guard-results/
  full-results/
```

`README.md` must contain:

- hypothesis;
- source and C reference locations;
- exact intended codegen change;
- correctness invariants;
- primary and held-out workloads;
- results with variation;
- resource results;
- final `KEEP` or `REJECT` decision.

Use separate Cargo target directories for baseline and candidate. Do not
overwrite an artifact and then assume its provenance from its filename. Record:

```bash
git rev-parse HEAD
rustc --version --verbose
cargo --version --verbose
cc --version
c++ --version
cmake --version
uname -a
lscpu
sha256sum <every allocator shared library>
ldd <every allocator shared library>
```

Capture host load, governor/frequency state, CPU affinity, and temperature when
the host exposes them. Stop and rerun if unrelated load, throttling, or thermal
drift changes between variants.

## Standard artifact construction

For the deployed Rust interposition artifact, use an isolated target directory:

```bash
CARGO_TARGET_DIR=target/perf-experiments/<ID>/<variant>-build \
  cargo rustc --release --features interpose --lib -- \
  --crate-type cdylib
```

Build C mimalloc 3.3.2 with the same release options used by the repository's
comparison harness:

```bash
cmake -S mimalloc-3.3.2 \
  -B target/perf-experiments/c-mimalloc-3.3.2 \
  -DCMAKE_BUILD_TYPE=Release \
  -DMI_BUILD_SHARED=ON \
  -DMI_BUILD_STATIC=OFF \
  -DMI_BUILD_OBJECT=OFF \
  -DMI_BUILD_TESTS=OFF \
  -DMI_OVERRIDE=ON
cmake --build target/perf-experiments/c-mimalloc-3.3.2 \
  --config Release --parallel
```

Before measuring an interposition artifact:

```bash
ldd -r <allocator.so>
env LD_PRELOAD=<allocator.so> /bin/true
nm -D <allocator.so> | sort
```

The smoke test must have no unresolved symbol or loader error. Verify that the
expected malloc/free and C++ allocation symbols resolve to the tested artifact,
not libc or libstdc++.

## Standard measurement protocol

### Focused A/B screen

Use focused measurements only to reject bad candidates quickly. They are not
enough to accept a change.

1. Run baseline then candidate.
2. Cool down.
3. Run candidate then baseline.
4. Use the same executable, operation count, environment, CPU set, and allocator
   options for both.
5. Collect at least elapsed time, cycles, instructions, branches, branch
   misses, cache misses, task clock, context switches, migrations, page faults,
   and max RSS.

Example counter set:

```bash
perf stat -x, -r 5 \
  -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses,context-switches,cpu-migrations,page-faults \
  -- env LD_PRELOAD=<allocator.so> <workload command>
```

Normalize counters by completed operations when the workload reports
throughput. Keep raw values too.

### Balanced `mimalloc-bench` comparison

Create two allocator-list files under the experiment directory. One lists
baseline before candidate; the other lists candidate before baseline:

```text
base /absolute/path/to/baseline.so
cand /absolute/path/to/candidate.so
```

```text
cand /absolute/path/to/candidate.so
base /absolute/path/to/baseline.so
```

From `mimalloc-bench/out/bench`, run both orders. Five repetitions in each
order give ten samples per allocator:

```bash
../../bench.sh --procs=32 -r=5 \
  --external=/absolute/path/to/base-first.allocators <test selection>
../../bench.sh --procs=32 -r=5 \
  --external=/absolute/path/to/candidate-first.allocators <test selection>
```

Do the same with C/current-Rust allocator lists before claiming C parity.

### Workload families

Select the focused workload from the mechanism, not from whichever test gives
the largest percentage. Every accepted candidate must also pass held-out
families:

| Mechanism | Primary probes | Required held-out guards |
|---|---|---|
| local small allocation/free | hotpath paired/single reuse, `glibc-simple` | Redis, RocksDB, alloc-test, large allocation, fragmentation |
| cross-thread free/collection | `xmalloc-testN`, Larson sized/unsized | mstress, rptest, glibc-thread, Redis, fragmentation |
| page queue/reclaim | Larson sized/unsized, `glibc-thread` | xmalloc, mstress, cache-scratch, RocksDB, RSS/resource probes |
| C++ allocation ABI | alloc-test1/N, Larson sized/unsized | C++ semantic tests, Redis, glibc-simple/thread, RocksDB |
| page refill/extension | page-batch plain/secure, malloc-large | glibc-simple/thread, xmalloc, fragmentation, RSS/resource probes |
| memory policy | fragmentation, malloc-large, resource probes | complete `allt`, Redis, RocksDB, mstress, cache-scratch |

### Assembly comparison

Run `scripts/capture-release-assembly.sh` before and after. For internal symbols
not captured by that script, record `nm -C -S` and bounded `objdump -Cd` output.

For each changed hot function count:

- bytes and instructions;
- conditional and unconditional control transfers;
- calls and tail calls;
- stack spills/reloads;
- loads and stores in the target loop;
- atomic/locked instructions;
- duplicated cold/error machinery in hot text.

Compare against both the previous Rust artifact and the corresponding C
mimalloc 3.3.2 function.

## Standard correctness gate

Run focused tests while iterating. Before a candidate can be kept, run all of
the following from a clean candidate tree:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
scripts/check-behavioral-parity.py
scripts/check-safety-stress.sh
scripts/check-concurrency-models.sh
scripts/check-layout.sh
scripts/check-public-abi.py
scripts/check-atomic-ordering-audit.py
scripts/check-cxx-operator-new-parity.py
scripts/check-release-cxx-artifacts.py
scripts/check-release-ld-preload.py
scripts/check-release-integrity.py
scripts/check-bench-full.sh
```

If a command is unavailable or fails for an unrelated environmental reason,
record the exact command and failure. Do not silently skip it and do not call
the candidate accepted until the gate is completed on a suitable host.

Changes to unsafe list traversal, pointer provenance, atomics, page ownership,
or batch state also require the relevant Miri/fuzz/stress checks documented in
`docs/testing.md`. Add a focused invariant or concurrency-model regression test
before changing the implementation.

---

# Ordered TODO

Complete these items in order. There may be only one production candidate in
progress. An item may end with `NO CHANGE` when its prerequisite evidence is
absent.

## E00 — Establish a fresh current-HEAD C parity baseline

Status: [x] completed 2026-08-09; see `PERFORMANCE_WORKLOG.md` and
`target/perf-experiments/E00-current-head-baseline/`

Purpose: replace the pre-fix `8baba5e` full-suite numbers with authoritative
current-HEAD evidence.

### Work

- [x] Build a fresh C mimalloc 3.3.2 release shared library.
- [x] Build a fresh current Rust `interpose` release shared library.
- [x] Record artifact hashes, dynamic dependencies, exported symbols, toolchain,
      host state, and exact commands.
- [x] Run `allt` with C first/Rust second for five repetitions.
- [x] Run `allt` with Rust first/C second for five repetitions.
- [x] Run both Larson variants in a separate ten-sample balanced comparison.
- [x] Run alloc-test1/N in a separate ten-sample balanced comparison to confirm
      whether the repaired C++ path removed the old 6% to 7% loss.
- [x] Collect `perf stat` for Larson sized/unsized, xmalloc, alloc-test1/N,
      glibc-simple/thread, and malloc-large.
- [x] Run the repository resource probes for both artifacts.
- [x] Publish a per-workload table with mean, median, standard deviation,
      coefficient of variation, order split, Rust/C ratio, RSS, and page faults.

### Completion condition

This item is complete only when every result can be traced to the recorded
artifact hashes. Do not begin optimization from the old full-suite report.

## E01 — Improve non-production observability of collection work

Status: [x] complete; diagnostics retained; E02 unblocked

Purpose: distinguish excessive call frequency from excessive work per call
before changing collection logic.

### Files in scope

- `src/allocator/bench_diagnostics.rs`
- `src/allocator/page/lifecycle.rs`
- `src/allocator/free/abandoned.rs`
- `src/types/page_batch.rs`
- `benches/mimalloc_bench.rs`

### Counters to add or verify

- [x] Full collection calls split by local/abandoned owner and force/non-force.
- [x] Partial collection calls and final-head collection calls.
- [x] Remote lists detached, total blocks scanned, and length histogram buckets.
- [x] Local-to-available moves split between empty-available O(1) moves and
      force append scans.
- [x] Local-list blocks scanned solely to find a tail.
- [x] Remote-list validation failures by reason.
- [x] Ownership-release CAS attempts and retries.
- [x] Reclaim attempts/success/failure by reason, including thread-uninitialized.
- [x] Reabandon attempts/success/failure by reason.
- [x] Queue searches, candidates scanned, selected position, failed extension,
      fresh-page allocation, and retry.

Prefer per-thread diagnostics accumulation where possible. Use atomics only
where aggregation truly crosses threads. Sampling is acceptable if the sample
rate and scaling are exact and tested.

### Validation

- [x] Add tests for reset, snapshot, JSON output, and counter relationships.
- [x] Build default release without `bench-diagnostics` and prove with `nm`,
      `.text` size, and hot-path assembly that diagnostics add no production
      symbols or instructions.
- [x] Run diagnostic builds of Larson, xmalloc, glibc-thread, and page-batch.
- [x] Normalize counts per allocator operation and record which branches or
      scans dominate.

### Completion condition

Keep diagnostics only if they answer a concrete decision below and compile
entirely out of default release. Diagnostics timing is never a speed claim.

## E02 — Eliminate the redundant empty-available local-list scan

Status: [x] original attempt rejected 2026-08-09; corrected experiment E02R
**ACCEPTED** 2026-08-10 after root-causing the configuration-dependent forced
splice defect; production O(1) transfer retained; see `PERFORMANCE_WORKLOG.md`

Priority: highest current production candidate.

General hypothesis: when `page->free` is empty, moving an already-owned
`local_free` list to `free` requires only ownership transfer of the head. It
does not require finding the local tail. C mimalloc performs this move in O(1).
The Rust generic full collector currently routes the local list through batch
detachment, which can scan the list before performing the same move.

### Compare first

- Rust: `collect_page_free_lists` and local-list helpers in
  `src/allocator/page/lifecycle.rs` and `src/types/page_batch.rs`.
- C: `_mi_page_free_collect` in `mimalloc-3.3.2/src/page.c`.
- Existing Rust O(1) precedent: `mi_page_free_quick_collect_raw`.

### Required invariant

The concrete local or claimed-abandoned owner must have exclusive mutation
rights to both `local_free` and `free`. The local list must already be a
page-homogeneous list produced by validated allocator transitions. Document why
converting its head from `LocalFreeList` to `AvailableFreeList` preserves used
count and link encoding without reading the tail.

### Candidate modification

- [x] Split local collection into the same cases as C:
  1. `local_free` empty: do nothing;
  2. `free` empty: transfer `local_free` to `free` in O(1), clear
     `local_free`, and clear `free_is_zero`;
  3. `free` nonempty and `force == true`: perform the validated tail scan and
     prepend/append exactly as required;
  4. `free` nonempty and not forced: leave `local_free` in place.
- [x] Do not alter remote-list collection, used-count accounting, force
      semantics, corruption handling, or list encoding in this experiment.
- [x] Do not add a new abstraction if existing typed list conversions can
      express the transfer.

### Tests to add before timing

- [x] Empty local list leaves every field unchanged.
- [x] Nonempty local plus empty available transfers the exact head, clears
      local, preserves every link, preserves used/capacity, and clears
      `free_is_zero`.
- [x] Nonempty local plus nonempty available and `force == false` does not
      splice lists.
- [x] Forced nonempty/nonempty collection preserves every block exactly once.
- [x] Both `LocalPage` and `OwnedAbandonedPage` owner paths preserve their
      ownership state.
- [x] Secure encoded-link mode passes the same cases.
- [x] Corrupted forced lists still report failure without an out-of-bounds
      read.

### Mechanism validation

- [x] Assembly shows no local-list traversal in the empty-available branch.
- [x] Diagnostics show fewer local-list nodes scanned, with unchanged remote
      blocks collected and used counts.
- [x] No new call, owner-discriminant test, or locked instruction appears in
      the common branch.

### Performance validation

Primary: Larson sized/unsized, xmalloc, glibc-thread, page-batch collection.

Held-out: mstress, rptest, Redis, glibc-simple, malloc-large, fragmentation,
RocksDB, and the complete balanced `allt` screen.

Reject if the scan disappears in assembly but total instructions/cycles do not
improve in any workload that exercises it, or if corruption/safety behavior is
weakened rather than made redundant by ownership.

## E03 — Match C's abandoned-page decision spine

Status: [x] completed 2026-08-09; `NO CHANGE`; see `PERFORMANCE_WORKLOG.md`

General hypothesis: Rust performs the same collect → free → reclaim → reabandon
→ unown policy as C, but linear ownership return shapes may leave additional
branches, moves, calls, or spills in `collect_after_remote_free`.

### Compare first

- Rust: `collect_after_remote_free`, `try_free_abandoned`,
  `try_reclaim_abandoned`, `try_reabandon_mapped`, and
  `release_abandoned_ownership` in `src/allocator/free/abandoned.rs`.
- C: `mi_free_try_collect_mt` and its helpers in
  `mimalloc-3.3.2/src/free.c`.

### Investigation

- [x] Produce annotated Rust/C control-flow graphs or side-by-side assembly.
- [x] Count function bytes, calls, control transfers, stack frame size, spills,
      page-field reloads, and reconstruction of thread-free words.
- [x] Attribute each extra Rust instruction to a required invariant, an
      optimizer/code-layout effect, or redundant source work.
- [x] Use E01 counters to confirm the hot outcomes and their frequencies.

### Candidate modification

Change only one demonstrated source of surplus work. Suitable examples are:

- carrying one already-extracted page pointer or block size through the
  decision sequence;
- borrowing an ownership witness through a failure check and consuming it only
  on the successful transition;
- removing a repeated witness-to-raw-to-witness conversion within one proven
  ownership epoch;
- making the release path pass the already-constructed expected thread-free
  word instead of reconstructing it.

Do not rewrite all helpers at once. Do not replace typed ownership with
unchecked raw state merely to resemble C source. The candidate must retain a
single obvious owner for every success and failure edge.

### Tests and validation

- [x] Verify transition coverage for destroyed, reclaimed, mapped, and
      still-abandoned outcomes. (Existing typed disposition/state tests retained;
      no candidate required new behavior tests.)
- [x] Verify concurrent remote frees during ownership release in loom or
      concurrency-model coverage. (Existing release-vs-push model retained.)
- [x] Confirm unchanged atomic orderings and CAS success/failure behavior.
- [x] Confirm assembly removes the exact targeted work. (`N/A`: no candidate.)
- [x] Run the cross-thread primary and held-out workload families. (`N/A`: no candidate.)

Reject if samples merely move between helper symbols or code size grows enough
to erase the intended instruction reduction.

## E04 — Re-evaluate early uninitialized-thread reclaim rejection

Status: [x] completed 2026-08-09; **REJECTED** at balanced focused screen;
see `PERFORMANCE_WORKLOG.md`

General hypothesis: a thread that is not initialized cannot reclaim an
abandoned page. Existing diagnostics previously showed this outcome for about
99.8% of reclaim attempts in xmalloc. Rejecting it before avoidable reclaim
policy work may save general thread-teardown/cross-thread-free work.

This idea was previously inconclusive because host variance exceeded the
apparent effect. Do not keep it without stronger evidence now.

### Candidate modification

- [x] Preserve the order: collect remote blocks, then try to free a now-empty
      page first.
- [x] Only after `try_free_abandoned` fails, test whether the current thread is
      initialized before reading or constructing reclaim-only policy state.
- [x] If uninitialized, skip only reclaim. Continue with the unchanged
      reabandon and ownership-release paths.
- [x] Do not initialize TLS as a side effect and do not cache option values
      across public allocator calls.

### Validation

- [x] Verify existing tests cover initialized and uninitialized freeing threads,
      including TLS destructor timing; no new behavior surface was introduced.
- [x] Diagnostics show identical disposition counts and fewer reclaim-policy
      reads on the uninitialized path.
- [x] Inspect assembly for the avoided option/policy work and return-value
      spills. (Option work moved as intended, but total function size grew.)
- [x] Measure both a workload with uninitialized releasing threads and one with
      initialized cross-thread reclaim. The second must remain neutral.
- [x] Run the proportionate lifecycle/concurrency gate before timing. (Focused
      concurrency passed; the complete release gate was intentionally not run
      after the candidate failed its balanced performance rejection screen.)

Reject again if two balanced comparisons do not reproduce the same direction.

## E05 — Remove any second traversal of a detached remote batch

Status: [x] completed 2026-08-09; `NO CHANGE`; see `PERFORMANCE_WORKLOG.md`

General hypothesis: each detached remote chain should be traversed at most once
to validate it, find its tail, and count its blocks. That one traversal should
produce a typed batch whose head, tail, page, and length are reused by every
subsequent splice and used-count update.

### Investigation

- [x] Trace full local collection, full abandoned collection, and partial
      abandoned collection separately.
- [x] With E01 counters and annotated assembly, determine whether any path
      traverses the same links twice.
- [x] Distinguish a second traversal from the already-rejected idea of merely
      deleting per-node membership validation.

### Candidate modification

Proceed only if a real second traversal is measured.

- [x] Candidate steps were evaluated and found already implemented: the first
      successful scan returns `RemoteFreeBatch`, and splice/accounting consume
      that metadata without rediscovery.
- [x] Existing scan bounds, corruption classifications, and return ABI were
      preserved; no candidate source change was warranted.

### Validation

- [x] Counter relationships prove one scan per detached batch.
- [x] Property/fuzz tests cover lengths 1, capacity, over-capacity, cycles,
      foreign-page links, invalid encoded links, and `len > used`.
- [x] Secure and ordinary link modes pass.
- [x] Assembly confirmed there is no second traversal to remove; therefore the
      candidate-only instruction/branch/code-size criterion is not applicable.

If no second traversal exists after E02, record `NO CHANGE` and continue.

## E06 — Tighten the ownership-release CAS loop

Status: [x] completed 2026-08-09; `NO CHANGE`; see `PERFORMANCE_WORKLOG.md`

General hypothesis: Rust and C implement the same ownership-release protocol,
but typed state construction or reloads may add work on CAS failure/retry edges.

### Work

- [x] Compare `release_abandoned_ownership` with C
      `mi_abandoned_page_unown_from_free` instruction by instruction.
- [x] Use diagnostics to record CAS attempts, retries, and nested collections.
- [x] Confirm whether the common case is first-attempt success.
- [x] Identify repeated construction of `(head, owned)` words, redundant loads,
      or conversions that survive optimization.

### Candidate modification

Change only a proven redundant operation. Preserve AcqRel success, Acquire
failure, weak-CAS retry behavior, ownership bits, and concurrent-free handling.
Do not change ordering for an x86-only instruction-count claim.

### Validation

- [x] Loom/concurrency models cover remote push racing with owner release,
      multiple retries, a newly nonempty list, page destruction, and reabandon.
- [x] Assembly proved the suspected repeated construction/load does not survive
      optimization, so there was no targeted instruction to remove.
- [x] CAS retry counts and page dispositions were recorded and conserved.
- [x] Cross-thread and local controls were measured; no candidate was created
      after the mechanism gate failed.

## E07 — Reassess reclaim and reabandon input derivation

Status: [x] completed 2026-08-09; `NO CHANGE`; see `PERFORMANCE_WORKLOG.md`

General hypothesis: successful or late-stage reclaim/reabandon paths may
rederive arena identity, slice range, page ownership, thread-pool state, or
exclusive-arena suitability after those facts were already validated.

### Evidence gate

Do not implement this item unless E01 shows that at least 1% of relevant
operations reach the repeated work in a stable workload. Earlier xmalloc data
showed that approximately 99.8% of reclaim attempts exited before these inputs,
so widening the common failure path is likely harmful.

### Candidate modification

- [x] Select one repeated derivation visible in assembly and counters.
      (`NO CHANGE`: none survives the evidence gate.)
- [x] No repeated derivation survived the evidence gate, so no scalar/newtype or
      context object was introduced.
- [x] Arena suitability, page-map, bitmap, and abandonment validation remained
      unchanged.

### Validation

- [x] Measured Larson successful/late reclaim and xmalloc early-failure control.
- [x] Assembly confirmed no candidate spill or larger early-exit return because
      no source change was made.
- [x] Recorded reclaim/reabandon dispositions and RSS; arena rollback and purge
      policy were unchanged and the final production artifact matches B00.

If the evidence gate is not met, record `NO CHANGE`.

## E08 — Inspect the residual page-queue search difference

Status: [x] completed 2026-08-09; **REJECTED** after two balanced screens;
see `PERFORMANCE_WORKLOG.md`

Priority: secondary. The retained in-place relink already removed the largest
known queue mismatch and all locked instructions from normal move-to-front.

### Work

- [x] Compare `mi_page_queue_find_free_ex_typed` with C
      `mi_page_queue_find_free_ex` in source and annotated assembly.
- [x] Separate queue scanning, collection calls, candidate selection, page
      extension, fresh-page allocation, and move-to-front costs.
- [x] Use E01 counters to compare candidates scanned and outcome frequency with
      C where equivalent instrumentation is possible.
- [x] Identify one extra load, transition, option read, or proof reconstruction
      on a common outcome.

### Candidate modification

Only implement a candidate tied to a measured common outcome. Keep C's search
order and memory policy: collect before classifying full, retain the same
candidate limit, prefer the same fuller pages, retire/fresh/retry in the same
order, and update direct-page caches correctly.

### Required tests

- [x] Empty queue and fresh-page path.
- [x] Immediate first-page success.
- [x] Expandable candidate.
- [x] Multiple candidates and candidate limit.
- [x] All-free candidate disposal.
- [x] Full-page movement and retain policy.
- [x] Failed extension and first-try retry.
- [x] Middle/tail move-to-front with direct cache updates.
- [x] Queue/heap counts, links, owner words, and flags after every transition.

Reject source-only simplifications that do not reduce emitted work. Do not
retry whole-function forced inlining, generic classification simplification,
or power-of-two membership masks; they have already regressed measured cases.

## E09 — Verify every C++ allocation/deallocation fast boundary

Status: [x] completed 2026-08-09; `NO CHANGE`; see `PERFORMANCE_WORKLOG.md`

Purpose: ensure the earlier `_Znwm`/`_Znam` repair generalizes across the full
C++ ABI instead of only the Larson-imported symbols.

### Work

- [x] Inventory scalar/array, throwing/nothrow, sized/unsized, and aligned
      `new`/`delete` symbols for supported Itanium ABI builds.
- [x] Confirm symbol ownership with `nm`, `readelf`, and `dladdr` tests.
- [x] Compare Rust and C entry assembly for every overload.
- [x] Confirm common successful ordinary allocations use the assembly allocator
      fast path and reach the optimized C++ helper only when semantics require
      failure handling.
- [x] Confirm delete overloads tail-call the correct free route without
      duplicate size/usable-size work in release.

### Candidate modification

Modify only an overload with a demonstrated extra common-path call, spill, or
semantic detour. Do not duplicate delete symbols already provided by
`src/raw/macros.rs`. Do not bypass `new_handler`, `bad_alloc`, alignment, or
nothrow behavior.

### Validation

- [x] C++11, C++14, C++17, and C++20 semantic parity.
- [x] Dynamic, static, and standalone preload artifacts.
- [x] Throwing, nothrow, zero-size, OOM, over-aligned, sized-delete, and array
      cases.
- [x] alloc-test1/N and Larson sized/unsized as primary probes.
- [x] Redis, glibc-simple/thread, RocksDB, and complete `allt` as guards.

## E10 — Revisit page refill/extension only if the new baseline identifies it

Status: [x] completed 2026-08-09; `NO CHANGE`; see `PERFORMANCE_WORKLOG.md`

The ordinary refill loop already closely matches C and previous unrolling,
nonnull specialization, and alternate loop forms were neutral or slower.

### Evidence gate

Proceed only if fresh current-HEAD profiling shows
`mi_page_free_list_extend` accounts for a material Rust/C instruction or cycle
gap after normalizing by blocks extended.

### Work if the gate is met

- [x] Compare blocks extended per call and calls per operation before comparing
      function samples.
- [x] Compare Rust/C loop-carried dependencies, link store order, address
      arithmetic, spills, and secure/default compile-time separation.
- [x] Check whether the difference is policy frequency or cost per block.
- [x] Select one surplus instruction or reload as the candidate. (`NO CHANGE`:
      no normalized per-block surplus instruction exists.)

### Validation

- [x] Page-batch ordinary and secure modes.
- [x] Small, medium, large, aligned, zeroed, and randomized allocation
      sequences.
- [x] malloc-large, glibc-simple/thread, xmalloc, fragmentation, and RSS.
- [x] Fuzz/property tests for capacity, commit boundaries, and link integrity.

If no normalized Rust/C gap exists, record `NO CHANGE`.

## E11 — Separate compiler effects from translation effects

Status: [x] completed 2026-08-09; see `PERFORMANCE_WORKLOG.md`

Purpose: avoid contorting correct Rust source to compensate for a difference
caused primarily by GCC-versus-LLVM code generation.

### Work

- [x] Build C mimalloc with the current GCC release configuration.
- [x] Build the same C source and options with Clang/LLVM.
- [x] Compare C-GCC, C-Clang, and Rust on the same balanced workload set.
- [x] Compare assembly for the remaining hot functions.
- [x] Classify each remaining gap as algorithm/source-shape, compiler/code
      layout, or measurement noise.

Do not claim Rust parity by selecting the slower C compiler. The official C
baseline remains the established release build. This experiment only decides
where engineering effort belongs.

### Optional follow-up: PGO

Attempt PGO only after source-shape experiments are exhausted and only if a
runnable deployed-equivalent artifact can be produced. The previous
interposition PGO attempt failed because the profiling runtime left `atexit`
unresolved.

If retried:

- train on a mixed allocation corpus spanning local, cross-thread, C++, large,
  aligned, zeroed, fragmentation, and real-application behavior;
- validate on held-out workloads not used for training;
- preserve a portable non-PGO build;
- reject a profile that improves trained tests but regresses held-out behavior
  or changes loader/interposition semantics.

## E12 — Final parity and release qualification

Status: [x] completed 2026-08-09; performance investigation complete. The
baseline behavioral-parity failure was repaired afterward; establish a fresh
accepted performance baseline before beginning another optimization experiment.

### Work

- [x] Rebuild fresh C and final Rust artifacts. The worktree intentionally
      contained retained diagnostics/docs, so exact hashes and dirty provenance
      were recorded instead of claiming a clean tree.
- [x] Record hashes and the complete environment.
- [x] Run balanced ten-sample C/Rust `allt` in both allocator orders.
- [x] Run focused higher-resolution comparisons for every workload outside the
      parity band.
- [x] Run `scripts/check-bench-full.sh` and preserve all five artifact modes.
      The official bundler rejected truthful dirty manifests; a checksummed,
      explicitly dirty fallback archive preserves the complete evidence.
- [x] Run the complete standard correctness gate. The original E12 run recorded
      the B00 behavioral-parity subprocess failure verbatim; the follow-up
      parity repair passes the full gate and is documented in the worklog.
- [x] Compare final assembly against the E00 baseline and C for every modified
      hot function.
- [x] Report CPU, instruction, branch, cache, RSS, committed-memory, abandoned
      page, purge, and page-fault effects.
- [x] Update `BENCHMARK_RESULTS.md` and the relevant investigation report with
      exact artifact provenance and rejected-experiment references.

### Success condition

The effort succeeds when the final allocator is broadly at parity or better,
the remaining differences are within measured noise or have a documented
general tradeoff, and every retained change makes an allocator mechanism
strictly clearer or cheaper without weakening safety, semantics, memory
behavior, or portability.

---

## Previously rejected directions

Do not retry these without a new profiler signal, a different mechanism, and a
written reason the old result no longer applies:

- `target-cpu=native` on the current host;
- blanket `inline(always)` or `inline(never)` changes;
- generic branchless rewrites;
- changing atomic orderings;
- two-link refill-loop unrolling;
- ordinary-path prefetching or queue-node prefetching;
- page metadata reordering/padding or moving `xthread_free` to another cache
  line;
- generic static dispatch for the complete `PageCollectOwner` collector;
- deleting detached-chain membership validation without a replacement
  invariant;
- power-of-two membership masks;
- simplifying page-queue classification wholesale;
- forcing the large direct-cache updater inline;
- caching mutable allocator options across public calls;
- combining several weak ideas into one candidate.

When in doubt, make the allocator do measurably less work in a general state
transition, prove that state transition correct, and then let the complete
workload and resource suite decide whether the change belongs.
