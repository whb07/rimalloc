# Implementation Specification: Stages 0–3

**Document status:** Proposed implementation specification
**Target:** Current `master` of `whb07/mechanical_mimalloc` as inspected through the GitHub connector
**Scope:** Benchmark and evidence repair, capability linearization, compatibility-policy extraction, and page-homogeneous batch primitives
**Out of scope:** Magazines themselves, runtime adaptation, native non-mimalloc page formats, public allocator-policy APIs, and changes to the C ABI

I have not executed the test or benchmark suite locally. Consequently, this specification defines the implementation and acceptance process, but all baseline numbers must be regenerated on the designated benchmark host during Stage 0.

---

# 1. Objectives

Stages 0–3 shall transform the repository from a well-audited translation with typed wrappers into a stable foundation for alternative allocator front ends.

The implementation must produce four outcomes:

1. **Stage 0 — Trustworthy evidence**

   * Benchmarks measure the intended allocator operations without hidden harness allocation or repeated thread construction.
   * Resource and latency evidence is reproducible and machine-readable.
   * Documentation accurately describes the current implementation.
   * Release comparisons start from clean, artifact-separated benchmark directories.

2. **Stage 1 — Linear capabilities**

   * Identity values remain cheap and copyable.
   * State, ownership, validation, detached-list, and transition capabilities become non-`Copy`.
   * Unchecked capability construction is centralized and audited.
   * Fallible consuming transitions return the original capability on failure.
   * Published and detached remote-free lists are represented as different types.

3. **Stage 2 — Explicit compatibility policy**

   * Existing heuristics move behind a concrete `CompatPolicy`.
   * Behavior, option-read frequency, raw layout, and generated hot-path code remain unchanged.
   * Mechanism code no longer directly owns the selected heuristic decisions.
   * No dynamic dispatch or trait-object policy call appears in allocator internals.

4. **Stage 3 — Page-homogeneous batches**

   * Detached page-local chains have typed head, tail, length, page identity, and accounting state.
   * Existing local-list and remote-list collection use the batch primitives.
   * Lease and return primitives exist for the future magazine implementation but are not used by the production allocation path yet.
   * Existing one-block allocation and local/remote free fast paths remain instruction-equivalent.

---

# 2. Normative terminology

In this specification:

* **MUST** is an acceptance requirement.
* **MUST NOT** is a prohibited design or implementation.
* **SHOULD** is expected unless there is a documented, measured reason not to follow it.
* **MAY** is optional.
* **Identity type** means a value that identifies allocator metadata but does not claim exclusive ownership or a specific mutable state.
* **Capability type** means a value whose existence proves an allocator state, ownership condition, validation result, or detached-resource ownership.
* **Raw bridge** means the narrow module permitted to convert validated raw identities into typed capabilities at C or translated compatibility boundaries.
* **Compatibility profile** means the behavior corresponding exactly to current mimalloc 3.3.2-compatible behavior.
* **Counted block** means a block still included in `page.used`.
* **Uncounted block** means a free block already excluded from `page.used`.

---

# 3. Cross-stage invariants

These requirements apply to every pull request in Stages 0–3.

## 3.1 ABI and public API

The following MUST remain unchanged:

* `mi_page_t`, `mi_theap_t`, `mi_heap_t`, `mi_block_t`, queue, arena, and public C-compatible layouts.
* Existing exported C symbol names and calling conventions.
* The Rust crate’s current public facade.
* The raw C-shaped Rust facade.
* Feature meanings for `secure`, `guarded`, `track-valgrind`, `track-asan`, `c-abi`, and `interpose`.
* The maintained-target statement until separate platform evidence promotes another target.

The project currently distinguishes the safe Rust facade, unstable raw Rust facade, and separately governed C ABI. Stages 0–3 must preserve that boundary.

No new stable public Rust API is required in these stages.

## 3.2 Hot-path invariants

The following production paths MUST NOT gain an unexplained:

* branch;
* call;
* load or store;
* atomic operation;
* lock prefix;
* stack spill;
* allocation;
* syscall;
* panic or formatting path;
* bounds-check slow path;
* trait-object dispatch.

The existing policy treats a hot-path regression over 2%, a repeated native-scenario regression over 3%, or a geomean regression over 1% as requiring investigation. It also explicitly rejects new hot-path traffic without measured justification.

For Stages 1 and 2, “investigate” is not sufficient: the intended acceptance standard is **instruction-equivalent hot paths**, because those stages are supposed to change representation and organization, not behavior.

## 3.3 Internal implementation constraints

Allocator-internal production code MUST NOT introduce:

* `Vec`, `Box`, `String`, `Arc`, `Mutex`, or another heap-allocating Rust container;
* dynamic dispatch using `dyn`;
* new process-global locks;
* a process-wide side table;
* allocator recursion;
* broad `Deref` or `DerefMut` implementations over metadata;
* broad `#![allow(dead_code)]` in `src/types`.

The current strict-type checker already rejects heap-allocating types, dynamic dispatch, direct block-link access, broad metadata references, raw bit manipulation, and pointer/atomic bypasses in allocator internals. The new work should extend this checker rather than create a parallel enforcement mechanism.

## 3.4 Mergeability

Every substage MUST be independently mergeable:

* compilation passes;
* existing tests pass;
* feature checks pass;
* the strict-type inventory is not weakened;
* the release audit is updated in the same pull request;
* benchmark and assembly evidence appropriate to the substage is attached.

No pull request should combine all of Stage 1, Stage 2, or Stage 3 into a single unreviewable patch.

---

# 4. Deliverable and branch structure

The recommended pull-request sequence is:

| PR   | Deliverable                                                             |
| ---- | ----------------------------------------------------------------------- |
| S0.1 | Benchmark support library and timed-region hygiene                      |
| S0.2 | Persistent remote-free benchmark topology                               |
| S0.3 | Clean-baseline runner, resource evidence, threshold schema              |
| S0.4 | Documentation reconciliation and evidence signoff                       |
| S1.1 | Capability inventory and non-`Copy` validation/block capabilities       |
| S1.2 | Non-`Copy` page capabilities and consuming transitions                  |
| S1.3 | Raw-bridge centralization and source enforcement                        |
| S1.4 | Published-versus-detached remote-list model                             |
| S1.5 | Linear abandoned-page disposition state machine                         |
| S2.1 | `CompatPolicy` scaffolding and pure reference model                     |
| S2.2 | Thread-heap, maintenance, and retirement policy migration               |
| S2.3 | Page-search, extension, remote collection, and reclaim policy migration |
| S2.4 | Exact-equivalence and assembly signoff                                  |
| S3.1 | Batch scalar types and batch-core model                                 |
| S3.2 | Local-free batch integration                                            |
| S3.3 | Detached-remote batch integration                                       |
| S3.4 | Lease/return primitives and differential test suite                     |
| S3.5 | Final Stages 0–3 benchmark and audit signoff                            |

Each pull request should state which later PRs depend on it and identify any deliberately temporary adapter.

---

# 5. Stage 0 — Repair the evidence layer

## 5.1 Purpose

Stage 0 establishes a trustworthy baseline before changing allocator capabilities or heuristics.

The current benchmark suite has a good overall structure, and the release runner already separates default Rust and interposition target directories.  However:

* the hot-path benchmark claims to avoid `Vec` growth or unrelated allocation inside timed regions;
* `lifo_stack` currently constructs a `Vec` inside the timed function while describing it as an on-stack ring;
* the remote-free benchmark creates and joins an owner thread for every measured repetition, even though only the free loop itself is timed;
* one workload comment still says the crate does not expose a native `GlobalAlloc`, although `MiMalloc` now implements it.
* smoke benchmarks currently reuse the ordinary `target/criterion` directory, allowing stale local baselines to influence output;
* the checked threshold file covers only a small subset and currently uses median-only, broad absolute ceilings.

Stage 0 MUST correct these before producing the baseline used by Stages 1–3.

---

## 5.2 Stage 0 file organization

Create a benchmark support module shared by the Criterion benches:

```text
benches/
  support/
    mod.rs
    allocators.rs
    pool.rs
    no_alloc.rs
    remote_fixture.rs
    metrics.rs
  hotpath.rs
  workloads.rs
  mimalloc_bench.rs

examples/
  benchmark_probe.rs

scripts/
  run-bench-clean.sh
  run-bench-resources.py
  check-bench-resources.py
  check-benchmark-doc-claims.py
  capture-benchmark-baseline.py

benchmarks/
  thresholds.json
  resource-thresholds.json
  host-profiles/
    README.md
```

`benches/support` must not become part of the allocator crate. It is benchmark-only tooling.

No new normal dependency is permitted. A dev-only dependency may be added only when the repository dependency policy approves it. Prefer Python for JSON aggregation and keep the Rust probe’s output format simple.

---

## 5.3 Allocator adapters

Move duplicated benchmark allocator adapters into `benches/support/allocators.rs`.

Define at least:

```rust
pub struct PublicMiMalloc;
pub struct RawMiMalloc;

#[cfg(feature = "compare")]
pub struct SystemAllocator;
```

### `PublicMiMalloc`

Delegates directly to:

```rust
mechanical_mimalloc::MiMalloc
```

This measures the intended public Rust global-allocator facade.

### `RawMiMalloc`

Delegates directly to:

```rust
mechanical_mimalloc::raw::mi_malloc
mechanical_mimalloc::raw::mi_free
mechanical_mimalloc::raw::mi_realloc
```

This measures the raw C-shaped facade.

### Naming

Criterion IDs MUST make the surface explicit:

```text
mechanical_mimalloc_public
mechanical_mimalloc_raw
system
c-mimalloc-3.3.2
```

A benchmark report must never label raw-facade results merely as “Rust” without also recording the measured surface.

### Equivalence expectation

For normal alignments in the default Rust artifact, public and raw results should be effectively equivalent. A sustained difference outside noise requires investigation, because the public facade should inline into the same allocator paths.

---

## 5.4 Timed-region allocation hygiene

### Fixed scratch storage

Replace the `vec!` in `lifo_stack` with storage created before `b.iter`.

One acceptable structure is:

```rust
struct PointerScratch {
    slots: Box<[MaybeUninit<*mut u8>]>,
}
```

The `Box` is created by the benchmark harness before measurement. The timed routine receives `&mut PointerScratch` and performs no growth, initialization allocation, or destruction allocation.

An inline fixed array is also acceptable:

```rust
struct PointerScratch {
    slots: [MaybeUninit<*mut u8>; POOL_SIZE],
}
```

The choice should be based on benchmark stack behavior, but the timed routine itself MUST allocate no harness memory.

### Counting system allocator

Add a benchmark-binary global allocator that wraps `std::alloc::System` and counts harness allocations:

```rust
struct CountingSystem;

static HARNESS_ALLOCS: AtomicU64;
static HARNESS_DEALLOCS: AtomicU64;
```

This allocator is not the allocator under test. Explicit calls through `PublicMiMalloc` or `RawMiMalloc` bypass it.

Provide:

```rust
struct HarnessAllocationSnapshot {
    allocs: u64,
    deallocs: u64,
}

fn assert_no_harness_allocations<F, R>(f: F) -> R
where
    F: FnOnce() -> R;
```

The assertion is enabled by:

```text
MIMALLOC_BENCH_ASSERT_NO_HARNESS_ALLOC=1
```

It MUST be used in a benchmark smoke or dedicated diagnostic pass, not necessarily in every release timing run.

### Covered timed helpers

At minimum, the no-harness-allocation assertion must cover:

* paired allocation/free;
* aligned paired allocation/free;
* free-only routine;
* remote-free timed routine;
* LIFO stack;
* single reuse.

The assertion may be disabled around setup, Criterion machinery, channel operations, string formatting, and report output.

### Verification

The counting allocator is the runtime proof. Benchmark smoke tests must execute
every covered timed helper with the assertion enabled.

---

## 5.5 Persistent remote-free fixture

Replace thread-per-sample construction with a persistent owner thread per benchmark case.

### Required protocol

The fixture owns one reusable pointer pool:

```rust
enum OwnerCommand {
    Prepare {
        layout: Layout,
    },
    Collect,
    Shutdown,
}

enum OwnerReply {
    Prepared(PointerPool),
    Collected,
    Stopped,
}
```

The actual protocol may combine channels, but it MUST preserve these semantics:

1. The owner thread is spawned once before `iter_custom`.
2. The owner thread initializes and retains its allocator thread-heap state.
3. A fixed-capacity pointer pool is allocated once by the harness.
4. The owner fills the pool using the allocator under test.
5. The prepared pool is transferred to the benchmark thread.
6. The timer starts.
7. The benchmark thread frees every pointer.
8. The timer stops.
9. The empty pool is returned to the owner outside the timer.
10. The owner performs any collection or next-round preparation outside the timer.
11. The fixture’s `Drop` sends `Shutdown`, joins the thread, and verifies that no live pointers remain.

### Criterion integration

The fixture must be created outside the custom timing loop:

```rust
let mut fixture = RemoteFreeFixture::new(allocator, layout);

b.iter_custom(|iterations| {
    fixture.measure_remote_free(iterations)
});
```

`measure_remote_free` may perform setup between iterations, but it adds only the durations of the free loops to the returned `Duration`.

### Required topologies

Stage 0 MUST provide:

1. **One owner, one remote freer**

   * The canonical remote-free hot-path benchmark.

2. **One owner, N remote free workers**

   * `N = 2, 4, 8`, bounded by available CPUs.
   * This belongs in a workload group rather than the tight hot-path group.

3. **N owners, one remote freer**

   * Exercises mixed-page remote frees.

4. **Owner exit after publication**

   * Correctness and lifecycle workload.
   * Not used as a nanosecond-level hot-path benchmark.

5. **Thread churn**

   * Repeated short-lived owner threads.
   * Separate from steady-state remote-free results.

The report MUST label steady-state and churn scenarios separately.

### Correctness checks

After every iteration in assertion mode:

* all pool entries must be null or marked empty;
* the owner thread must acknowledge pool return;
* no pointer may be freed twice;
* no pointer may remain live at fixture destruction;
* the owner thread must join without panic.

---

## 5.6 Clean benchmark directories

### Smoke mode

Change smoke benchmarking to use:

```text
target/bench-smoke/default-rust
```

The script MUST remove or recreate that directory before a smoke run.

Do not use the shared `target/criterion` directory for gated smoke evidence.

### Full mode

Continue using distinct directories:

```text
target/bench-full/default-rust
target/bench-full/interpose
```

The full runner already separates these modes.  Stage 0 must additionally ensure that each release-evidence invocation begins with fresh Criterion result directories.

The clean script MUST:

* resolve the repository root;
* reject paths outside the repository;
* reject `/`, `$HOME`, or an empty path;
* delete only the intended benchmark target directories;
* create a manifest recording the removed and recreated paths.

### No cross-mode saved baselines

Criterion baselines MUST NOT be shared across:

* default Rust build;
* interposition build;
* secure build;
* guarded build;
* instrumented diagnostics build.

Artifact mode and feature set must be fields in every output record.

---

## 5.7 Environment and provenance manifest

Every benchmark artifact directory must contain:

```text
environment.txt
manifest.json
commands.txt
criterion/
resource-results.jsonl
threshold-results.csv
```

`manifest.json` schema:

```json
{
  "schema": 1,
  "repository": "whb07/mechanical_mimalloc",
  "commit": "<full commit hash>",
  "dirty": false,
  "artifact_mode": "default-rust",
  "features": ["bench"],
  "target": "x86_64-unknown-linux-gnu",
  "rustc": "<version>",
  "llvm": "<version>",
  "c_compiler": "<version>",
  "kernel": "<version>",
  "cpu_model": "<model>",
  "logical_cpus": 32,
  "physical_cores": 16,
  "numa_nodes": 1,
  "governor": "<value-or-unknown>",
  "turbo": "<enabled-disabled-unknown>",
  "started_at_utc": "<timestamp>",
  "command_digest": "<sha256>"
}
```

The release runner MUST fail when:

* the working tree is dirty, unless `MIMALLOC_ALLOW_DIRTY_BENCH=1`;
* the commit cannot be resolved;
* the artifact mode or feature set is absent;
* the environment manifest and result directory disagree.

The project already requires environment capture and exact command preservation for release evidence.  Stage 0 makes that requirement machine-verifiable.

---

## 5.8 Resource metrics

### Existing APIs to reuse

The safe facade already exposes:

* current and peak reserved bytes;
* current and peak committed bytes;
* requested-byte totals;
* current and abandoned page counts;
* current and peak RSS;
* current and peak process commit;
* page faults;
* elapsed, user, and system CPU time.

Stage 0 should reuse these rather than introduce production telemetry.

### Probe process

Add `examples/benchmark_probe.rs`, built with `std` and `bench`.

The Python runner launches one fresh process for each tuple:

```text
artifact mode × allocator × scenario × thread count × size class
```

A fresh process is required because peak RSS and peak commit are process-lifetime metrics.

### Probe inputs

Use environment variables or command arguments for:

```text
scenario
allocator
size
alignment
operations
threads
live-set size
seed
artifact output path
```

### Probe record schema

Each JSON-lines record:

```json
{
  "schema": 1,
  "scenario": "remote_free_1x1",
  "allocator": "mechanical_mimalloc_public",
  "artifact_mode": "default-rust",
  "features": ["bench"],
  "size": 64,
  "alignment": 8,
  "threads": 2,
  "operations": 1000000,
  "seed": 16045690984503098046,

  "wall_ns": 123,
  "operations_per_second": 0.0,

  "logical_live_bytes_start": 0,
  "logical_live_bytes_peak": 262144,
  "logical_live_bytes_end": 0,

  "rss_current_start": null,
  "rss_current_end": null,
  "rss_peak_end": null,
  "commit_current_start": null,
  "commit_current_end": null,
  "commit_peak_end": null,
  "page_faults_delta": 0,

  "allocator_reserved_current_start": null,
  "allocator_reserved_current_end": null,
  "allocator_reserved_peak_end": null,
  "allocator_committed_current_start": null,
  "allocator_committed_current_end": null,
  "allocator_committed_peak_end": null,
  "allocator_pages_current_end": null,
  "allocator_pages_abandoned_end": null,

  "exit_status": 0
}
```

Unavailable values MUST be `null`, not zero. The safe process API notes that some zero-valued size fields may mean unavailable.

### Harness-owned live-byte accounting

Because release allocator stats may not always track every requested byte, each probe MUST independently track:

* logical bytes allocated;
* logical peak live bytes;
* logical bytes freed.

This is workload truth, not allocator-reported memory.

### Derived metrics

The Python runner should calculate where inputs are available:

```text
peak_rss / logical_peak_live_bytes
peak_commit / logical_peak_live_bytes
allocator_peak_committed / logical_peak_live_bytes
end_rss - start_rss
end_commit - start_commit
page_faults per million operations
```

Division by zero or unavailable input yields `null`.

### Initial resource gates

Stage 0 does not need aggressive memory regression thresholds. Initial gates should verify:

* process exits successfully;
* logical live bytes end at zero;
* all mandatory records are present;
* RSS and commit values, when present, are nonnegative;
* no impossible peak/current relationship exists;
* page-fault deltas do not underflow;
* repeated runs produce a bounded coefficient of variation recorded in the report.

After the baseline is accepted, `benchmarks/resource-thresholds.json` may add reviewed scenario-specific limits.

---

## 5.9 Threshold schema upgrade

Upgrade `benchmarks/thresholds.json` to schema 2 while retaining schema-1 parsing for one migration cycle.

Recommended entry:

```json
{
  "id": "hotpath/paired_alloc_free/mechanical_mimalloc_public/16",
  "artifact_mode": "default-rust",
  "feature_set": ["bench"],
  "category": "hot-path allocation",
  "baseline": {
    "median_ns": 14500.0,
    "p95_ns": 14650.0
  },
  "limits": {
    "median_ratio": 1.02,
    "p95_ratio": 1.03
  },
  "host_profile": "release-linux-x86_64-primary",
  "baseline_commit": "<hash>"
}
```

The actual baseline values must come from the Stage 0 baseline protocol, not from this example.

### Threshold checker changes

`check-bench-thresholds.py` MUST:

* identify artifact mode and feature set;
* reject cross-mode comparisons;
* calculate median, mean, min, p95, and max from Criterion data;
* support median and p95 ratio limits;
* report missing and inapplicable entries separately;
* preserve schema-1 compatibility temporarily;
* include self-tests for:

  * pass;
  * regression;
  * missing result;
  * wrong artifact mode;
  * wrong feature set;
  * p95 failure with median pass;
  * malformed baseline.

The current checker reads Criterion median and mean only and compares median against one maximum ratio.

### Baseline creation protocol

A release baseline MUST be created from:

1. A clean commit.
2. A clean benchmark target directory.
3. Five complete runs on the designated host.
4. An environment manifest for every run.
5. Median of the five run medians for each benchmark.
6. Median of the five p95 values.
7. Manual review for thermal throttling, CPU migration, and outliers.
8. A committed baseline update in its own pull request.

The baseline-capture script MUST NOT edit `thresholds.json` automatically unless passed an explicit `--write-reviewed-baseline` flag.

---

## 5.10 Documentation reconciliation

Update:

```text
README.md
TYPES_SPEC.md
MIGRATION_PHASES.md
docs/strict-types.md
docs/performance.md
docs/limitations.md
UPSTREAM_DIFF.md
BENCHMARK_RESULTS.md
CHANGELOG.md
benches/workloads.rs
benches/hotpath.rs
```

### Required content changes

1. `TYPES_SPEC.md`

   * State whether it is normative, historical, or both.
   * Replace stale claims that all modules are public.
   * Name the implemented location as `src/types`, while noting the internal alias `core_types` where relevant.
   * Separate “original design target” from “current as-built architecture.”

2. `MIGRATION_PHASES.md`

   * Add Stages 20–23 or an equivalent new section:

     * evidence repair;
     * linear capabilities;
     * compatibility-policy extraction;
     * page-batch foundation.

3. `docs/strict-types.md`

   * Replace the broad statement that copied page witnesses are zero-cost with the identity-versus-capability rule introduced by Stage 1.

4. Benchmark comments

   * Correct the claim that the crate lacks a `GlobalAlloc`.
   * Describe the exact measured facade.
   * Remove “on-stack” wording unless the scratch really is on stack.

5. `docs/performance.md`

   * Document clean target directories.
   * Document artifact and feature isolation.
   * Document the new resource schema and baseline process.

### Documentation source check

Add `scripts/check-benchmark-doc-claims.py` and invoke it from `scripts/check-docs.sh`.

It should reject at least:

* “does not expose a native Rust `GlobalAlloc`”;
* “all modules are public” as a statement of current architecture;
* references to a nonexistent current `src/core_types/` directory without clarification;
* claims that the LIFO buffer is on-stack when it is not;
* benchmark commands that mix default and interpose result directories.

The current documentation check only runs Rust documentation, doc tests, and safe-doc validation.

---

## 5.11 Stage 0 acceptance

Stage 0 is complete only when:

* every hot-path timed helper passes the no-harness-allocation assertion;
* the LIFO benchmark performs no timed `Vec` construction;
* remote-free steady-state tests use persistent threads;
* churn scenarios remain separate;
* smoke and full benchmark directories are clean and artifact-specific;
* every full run emits manifest, environment, command, Criterion, resource, and threshold artifacts;
* public and raw Rust facade results are labeled separately;
* all stale documentation assertions are removed;
* a new reviewed baseline has been recorded;
* `scripts/check-local.sh` still passes; it currently includes formatting, tests, boundary checks, features, lints, layout, docs, packaging, and smoke benchmarks.

---

# 6. Stage 1 — Make capabilities linear

## 6.1 Purpose

The current type layer models numerous states, but many state witnesses derive `Clone` and `Copy`, including active, full, available, local, foreign, abandoned, owned-abandoned, and destroyed pages.  Block validation and free-list handles also derive `Copy`.

That weakens consuming transitions. For example, a copied `OwnedAbandonedPage` can survive a reclaim or reabandon operation. Existing tests currently reuse such a token after logically consuming operations.

Stage 1 makes transitions semantically meaningful without changing raw representation or hot-path code.

---

## 6.2 Identity-versus-capability classification

Classify identity and capability types in their owning modules. Compiler-checked
assertions must keep linear capability types non-`Copy` and non-`Clone`.

### Identity types that MAY remain `Copy`

These identify values or metadata but do not claim an exclusive state transition:

```text
PagePtr
PageConstPtr
BlockPtr
HeapPtr
ThreadHeapPtr
TldPtr
ArenaPtr
SubprocPtr
PageQueuePtr
UserPtr
InteriorUserPtr
PageStart
SliceStart
OsMemPtr

RequestedSize
EffectiveSize
PaddedSize
UsableSize
BlockSize
Bin
DirectBin
StatsBin
PageCapacity
UsedBlockCount
ReservedBlockCount
RetireExpire
ThreadId
PageThreadId
PageFlags
ThreadFreeWord
PageKeys
EncodedWord

NormalPageQueue
HugePageQueue
FullPageQueue
PageQueue<K>
ThreadHeapLocal
```

`ThreadHeapLocal` remains `Copy` by explicit design: it proves thread affinity, not exclusive ownership of all thread-heap operations. The documentation must say this directly.

### Capability types that MUST NOT implement `Copy` or `Clone`

```text
ActivePage
FullPage
RetiredPage
AvailablePage
LocalPage
ForeignPage
AbandonedPageType
AbandonedMappedPageType
OwnedAbandonedPage
DestroyedPage

PageForAbandon
PageAbandonOutcome
AllFreePage<S>
QueuedPage<K>
UnqueuedPage<S>

AllocatedBlock
FreeBlock
GuardedBlock
BlockForFree
ValidatedFree

AvailableFreeList
LocalFreeList
DetachedRemoteList
PublishedRemoteHead
EncodedFreeList
```

`PointerLocation`, `LocatedBlock`, and `ValidPointerLookup` should also become non-`Copy` if they can carry `GuardedBlock` or lead to a free capability.

### Value snapshots that remain `Copy`

`ThreadFreeWord` remains `Copy`: it is an observed atomic word, not ownership of the chain to which it points.

---

## 6.3 Required annotations

Every capability type MUST have:

```rust
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
```

Detached-list capabilities need a more specific message:

```rust
#[must_use = "detached free-list ownership must be collected, republished, or returned"]
```

Capability types MUST NOT implement `Drop`. A `Drop` implementation would add drop glue and cannot safely decide how to restore an arbitrary allocator state. Correct consumption is enforced through API shape, linting, tests, and review.

---

## 6.4 Accessor convention

Identity accessors may continue to take `self`.

Capability accessors MUST normally borrow:

```rust
impl ActivePage {
    pub(crate) fn page(&self) -> PagePtr;
}

impl ValidatedFree {
    pub(crate) fn page(&self) -> PagePtr;
}
```

Methods that perform a state transition or transfer ownership MUST take `self`.

This avoids consuming a capability merely to inspect its identity, while preserving consuming semantics for transitions.

---

## 6.5 Fallible consuming transitions

A fallible transition MUST return the original capability on failure.

### Active page

Replace:

```rust
fn available(self) -> Option<AvailablePage>;
fn mark_all_free(self) -> Option<AllFreePage<Active>>;
```

with:

```rust
fn try_available(self) -> Result<AvailablePage, ActivePage>;

fn try_mark_all_free(
    self,
) -> Result<AllFreePage<Active>, ActivePage>;
```

### Full page

```rust
fn try_mark_all_free(
    self,
) -> Result<AllFreePage<Full>, FullPage>;
```

### Owned abandoned page

```rust
fn try_mark_all_free(
    self,
) -> Result<AllFreePage<Abandoned>, OwnedAbandonedPage>;
```

### Abandoned ownership claim

```rust
fn try_claim_ownership(
    self,
) -> Result<OwnedAbandonedPage, AbandonedPageType>;
```

### Queue membership

Replace `Option<QueuedPage<...>>` with a result that returns the page capability:

```rust
fn try_queue(
    page: ActivePage,
    queue: NormalPageQueue,
) -> Result<QueuedPage<NormalQueue>, ActivePage>;

fn try_queue_full(
    page: FullPage,
    queue: FullPageQueue,
) -> Result<QueuedPage<FullQueue>, FullPage>;
```

Queue identities are `Copy`, so returning them in the error is not necessary.

### Why `Result`

This prevents call sites from reconstructing a capability using `PagePtr` merely because an `Option` consumed the original proof.

---

## 6.6 Infallible consuming transitions

Use `into_` naming for transformations that transfer the state proof:

```rust
impl AvailablePage {
    fn pop_one(self) -> (AllocatedBlock, ActivePage);
}

impl ActivePage {
    fn into_full(
        self,
        from: NormalPageQueue,
        to: FullPageQueue,
    ) -> FullPage;

    fn into_abandon_candidate(self) -> PageForAbandon;
}

impl FullPage {
    fn into_abandon_candidate(self) -> PageForAbandon;
}

impl AllFreePage<Active> {
    fn into_retired(self) -> RetiredPage;
}

impl OwnedAbandonedPage {
    fn into_reclaimed(self, target: ThreadHeapLocal) -> ActivePage;

    fn into_mapped_abandoned(self) -> AbandonedMappedPageType;
}
```

A transition function MUST be the only normal internal way to construct its output capability.

---

## 6.7 Block and validation capability API

### `AllocatedBlock`

```rust
impl AllocatedBlock {
    pub(crate) fn as_ptr(&self) -> BlockPtr;
    pub(crate) fn into_ptr(self) -> BlockPtr;
}
```

Returning the pointer to the user-facing allocation path consumes the allocated-block capability.

### `FreeBlock`

```rust
impl FreeBlock {
    pub(crate) fn as_ptr(&self) -> BlockPtr;
}
```

Free-list insertion consumes `FreeBlock`.

### `BlockForFree`

```rust
impl BlockForFree {
    pub(crate) fn block(&self) -> BlockPtr;
    pub(crate) fn was_guarded(&self) -> WasGuarded;
    pub(crate) fn into_free_block(self) -> FreeBlock;
}
```

### `ValidatedFree`

Replace copy-style field extraction with:

```rust
struct ValidatedFreeParts {
    user: UserPtr,
    page: PagePtr,
    block: BlockForFree,
}

impl ValidatedFree {
    pub(crate) fn page(&self) -> PagePtr;
    pub(crate) fn user(&self) -> UserPtr;
    pub(crate) fn block(&self) -> &BlockForFree;

    pub(crate) fn into_parts(self) -> ValidatedFreeParts;
}
```

Free routing should inspect by borrow and finally consume `ValidatedFree` into parts exactly once.

### Guarded block

`GuardedBlock` should be consumed by unguarding:

```rust
fn unguard(
    self,
    page: PagePtr,
    user: UserPtr,
) -> BlockForFree;
```

This prevents duplicate unguard operations from duplicated state tokens.

---

## 6.8 Separate published and detached remote lists

The current `RemoteFreeList` is used both for a list detached by an atomic collect and for a newly published head that remains stored in `xthread_free`. Those are different ownership states. The current free path passes a newly published block head into abandoned-page partial collection while the atomic word still points to that head.

Replace `RemoteFreeList` with two types.

### Published head

```rust
#[must_use]
pub(crate) struct PublishedRemoteHead {
    head: BlockPtr,
}
```

Invariant:

* `head` is the head currently published in `page.xthread_free`;
* the caller does not own the complete chain;
* the caller may detach the tail only under the owned abandoned-page protocol;
* the head itself must remain published until the atomic owner protocol changes it.

### Detached list

```rust
#[must_use]
pub(crate) struct DetachedRemoteList {
    head: Option<BlockPtr>,
}
```

Invariant:

* the atomic word no longer owns the list head;
* no concurrent remote pusher can mutate links within the detached chain;
* the capability must be collected, republished, or otherwise consumed.

### Atomic API changes

```rust
enum RemotePushResult {
    Published {
        previous: ThreadFreeWord,
        head: PublishedRemoteHead,
    },
    Retry {
        observed: ThreadFreeWord,
    },
}

impl AtomicThreadFree {
    fn try_push(
        self,
        expected: ThreadFreeWord,
        block: BlockPtr,
    ) -> RemotePushResult;

    unsafe fn detach_head(
        self,
    ) -> DetachedRemoteList;
}
```

The CAS loop must compile to the same instructions as the current loop.

### Partial collection

```rust
impl PublishedRemoteHead {
    unsafe fn detach_tail(
        self,
        page: &OwnedAbandonedPage,
    ) -> (
        PublishedRemoteHead,
        Option<DetachedRemoteList>,
    );
}
```

This method:

1. Reads `head.next`.
2. If null, returns no detached tail.
3. Writes null to `head.next`.
4. Returns the remaining tail as a unique detached-list capability.
5. Leaves the head published in `xthread_free`.

This exactly represents the current small-page partial-collection protocol without falsely calling the entire chain detached.

---

## 6.9 Remove false abandoned-page coercion

The current regular local collection path constructs an `OwnedAbandonedPage` merely to call a shared chain collector.  That conversion MUST be removed.

Create a private page-pointer mechanism:

```rust
unsafe fn collect_detached_chain_into_local_raw(
    page: PagePtr,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, RemoteListCorruption>;
```

Expose two typed wrappers:

```rust
unsafe fn collect_remote_for_local_page(
    page: &LocalPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, RemoteListCorruption>;

unsafe fn collect_remote_for_owned_abandoned_page(
    page: &OwnedAbandonedPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, RemoteListCorruption>;
```

The raw helper may receive `PagePtr`, but only these wrappers may call it.

No `LocalPage -> OwnedAbandonedPage` conversion may remain.

---

## 6.10 Raw-bridge centralization

Add:

```text
src/types/raw_bridge.rs
```

All capability constructors become private to `src/types`:

```rust
pub(in crate::types) unsafe fn new_unchecked(...);
```

The allocator obtains capabilities through audited bridge functions:

```rust
pub(crate) unsafe fn assume_active_page(
    page: PagePtr,
) -> ActivePage;

pub(crate) unsafe fn assume_full_page(
    page: PagePtr,
) -> FullPage;

pub(crate) unsafe fn assume_local_page(
    page: PagePtr,
) -> LocalPage;

pub(crate) unsafe fn assume_foreign_page(
    page: PagePtr,
) -> ForeignPage;

pub(crate) unsafe fn assume_owned_abandoned_page(
    page: PagePtr,
) -> OwnedAbandonedPage;
```

Each function MUST carry:

* a distinct safety contract;
* an audit marker;
* a list of legal caller families;
* no metadata mutation;
* `#[inline(always)]`.

### Legal raw-bridge callers

Raw-bridge construction is allowed only at:

* exported or C-shaped entry points;
* initialization boundaries;
* queue lookup/classification boundaries;
* atomic ownership success boundaries;
* tests using local synthetic fixtures.

A normal typed state transition MUST NOT go through the raw bridge.

---

## 6.11 Source enforcement

Extend `scripts/check-strict-types.py` with:

### Non-copy capability rule

Reject:

```text
derive(... Copy ...)
derive(... Clone ...)
impl Copy for <capability>
impl Clone for <capability>
```

for every type in the capability inventory.

### Constructor locality rule

Reject capability `new_unchecked` calls outside:

```text
src/types/page_states.rs
src/types/page_queues.rs
src/types/raw_bridge.rs
src/types/*/tests.rs
```

No allowlist wildcard is permitted.

### False-state conversion rule

Reject direct construction of:

```text
OwnedAbandonedPage
LocalPage
ForeignPage
FullPage
ActivePage
```

outside the transition and raw-bridge modules.

### Detached-list rule

Reject `DetachedRemoteList` construction except in:

* `AtomicThreadFree::detach_head`;
* `PublishedRemoteHead::detach_tail`;
* test fixtures;
* explicitly documented raw compatibility ingress.

### Compiler checks

Crate unit tests verify:

* every required capability is non-`Copy`;
* raw constructors are not crate-wide;
* published and detached remote types both exist;
* `LocalPage` collection does not mention `OwnedAbandonedPage`;
* fallible transitions return `Result<Next, Original>`.

---

## 6.12 Linear abandoned-page state machine

Refactor the abandoned free lifecycle into explicit consuming outcomes.

### Result type

```rust
enum AbandonedDisposition {
    StillAbandoned(AbandonedPageType),
    Mapped(AbandonedMappedPageType),
    Reclaimed(ActivePage),
    Destroyed(DestroyedPage),
}
```

### Try-free

```rust
unsafe fn try_free_abandoned(
    page: OwnedAbandonedPage,
) -> Result<DestroyedPage, OwnedAbandonedPage>;
```

On `used != 0`, return the original owned capability.

### Try-reclaim

```rust
unsafe fn try_reclaim_abandoned(
    page: OwnedAbandonedPage,
    policy: ReclaimDecision,
) -> Result<ActivePage, OwnedAbandonedPage>;
```

Any rejected condition returns the same owned capability.

### Try-reabandon

```rust
unsafe fn try_reabandon_mapped(
    page: OwnedAbandonedPage,
) -> Result<AbandonedMappedPageType, OwnedAbandonedPage>;
```

### Release ownership

```rust
unsafe fn release_abandoned_ownership(
    page: OwnedAbandonedPage,
    expected: ThreadFreeWord,
) -> AbandonedDisposition;
```

If another remote chain appears during release, the function continues consuming and transforming the same owned capability. It must never recreate `OwnedAbandonedPage` from `PagePtr` inside the loop.

### Top-level collection

```rust
unsafe fn collect_after_remote_free(
    page: OwnedAbandonedPage,
    published: PublishedRemoteHead,
) -> AbandonedDisposition;
```

The free caller may ignore the returned terminal disposition, but the internal state machine must produce exactly one.

---

## 6.13 Stage 1 tests

### Compile-time non-`Copy` tests

Use compile-fail doctests or a dedicated `trybuild`-free source audit. No new dev dependency is necessary.

Reject patterns such as:

```rust
fn requires_copy<T: Copy>() {}
requires_copy::<OwnedAbandonedPage>();
```

### Move-semantics unit tests

Rewrite existing tests so a consumed capability cannot be reused. For example:

```rust
let owned = ...;
let owned = match owned.try_mark_all_free() {
    Ok(all_free) => {
        // Test terminal path.
        return;
    }
    Err(owned) => owned,
};

let active = owned.into_reclaimed(heap_owner());
```

### Transition tests

Cover:

* active → available → active;
* active → all-free;
* full → all-free;
* active → full → active;
* abandoned → owned;
* owned → reclaimed;
* owned → mapped;
* owned → destroyed;
* failed ownership claim returns abandoned capability;
* failed all-free check returns original capability;
* failed queue-membership check returns original page capability.

### Remote-list ownership tests

Cover:

* successful push yields `PublishedRemoteHead`;
* atomic detach yields `DetachedRemoteList`;
* published-head tail detachment leaves head linked to null;
* detached tail is collected exactly once;
* no API converts a published head directly into a full detached list;
* CAS retry returns only an observed word and retains the block for retry.

### Assembly gates

Capture and compare at least:

```text
mi_malloc
mi_malloc_small
mi_free
_mi_theap_malloc_zero_ex
```

The current project already records instruction-equivalent free-list pop, local free, and remote CAS sequences.  Stage 1 MUST preserve those sequences.

---

## 6.14 Stage 1 acceptance

Stage 1 is complete when:

* the capability inventory is committed;
* all state and detached-resource capabilities are non-`Copy` and non-`Clone`;
* all fallible consuming transitions return the original capability;
* all unchecked capability creation is centralized;
* published and detached remote chains are different types;
* local collection no longer fabricates abandoned ownership;
* abandoned-page collection has one consuming disposition state machine;
* all feature tests pass;
* strict-type and release audits pass;
* required hot-path assembly is instruction-equivalent;
* Stage 0 performance and resource gates pass.

---

# 7. Stage 2 — Extract `CompatPolicy`

## 7.1 Purpose

Allocator mechanism and heuristic decisions are currently interleaved. Examples include:

* generic maintenance every 1,000 generic allocations;
* page extension based on a fixed 4 KiB target;
* page-search limits using `page_full_retain` and `page_max_candidates`;
* retirement using 16 cycles for small pages and one quarter of that for larger pages;
* initialization of reclaim, abandon, and full-retain behavior directly from options;
* abandoned-page reclaim limits selected directly in the free path.

Stage 2 moves these decisions into one concrete policy without changing any result or option-read timing.

---

## 7.2 Architecture

Create:

```text
src/allocator/policy/
  mod.rs
  compat.rs
  decisions.rs
  reference.rs      # cfg(test)
  tests.rs
```

In `src/allocator/mod.rs`:

```rust
pub(crate) mod policy;
```

In `policy/mod.rs`:

```rust
pub(crate) use compat::CompatPolicy;

pub(crate) type ActivePolicy = CompatPolicy;
```

There is deliberately:

* no public policy API;
* no trait object;
* no `dyn`;
* no function-pointer table;
* no per-allocation policy pointer;
* no genericization of every allocator module.

`CompatPolicy` is a zero-sized concrete type.

---

## 7.3 Policy result types

Add or reuse typed results.

```rust
pub(crate) struct ThreadHeapPolicy {
    allow_page_reclaim: bool,
    allow_page_abandon: bool,
    page_full_retain: PageFullRetain,
}

pub(crate) struct PageFullRetain(c_long);

pub(crate) struct PageExtendCount(NonZeroU16);

pub(crate) struct GenericMaintenanceInterval(c_long);

pub(crate) enum RemoteCollectMode {
    PartialKeepPublishedHead,
    FullDetach,
}

pub(crate) enum ReclaimContext {
    SameThreadHeap,
    CrossThreadHeap,
    ThreadPool,
}

pub(crate) struct ReclaimDecision {
    reclaim_on_free: PageReclaimOnFree,
    max_queue_count: Option<PageQueueCount>,
}
```

Existing `PageReclaimOnFree`, `PageReclaimLimit`, `RetireExpire`, `BlockSize`, and queue count types should be reused where semantically correct.

All policy decision types should be `Copy`; they are values, not resource capabilities.

---

## 7.4 Policy methods

### Generic maintenance

```rust
impl CompatPolicy {
    #[inline(always)]
    pub(crate) const fn generic_maintenance_interval(
    ) -> GenericMaintenanceInterval;

    #[inline(always)]
    pub(crate) fn generic_collect_limit(
    ) -> GenericMaintenanceInterval;
}
```

Compatibility values:

```text
maintenance interval = 1000
generic collect option clamped to 1 … 1,000,000
```

The option must be read at the same point and frequency as before.

### Thread-heap initialization

```rust
pub(crate) fn thread_heap_policy() -> ThreadHeapPolicy;

pub(crate) fn adjust_for_thread_pool(
    policy: ThreadHeapPolicy,
) -> ThreadHeapPolicy;
```

Compatibility behavior:

* `allow_page_reclaim = page_reclaim_on_free >= 0`;
* `allow_page_abandon = page_full_retain >= 0`;
* `page_full_retain = clamp(page_full_retain, -1, 32)`;
* thread-pool initialization divides positive retain count by four.

The current thread-heap initialization performs this division after option setup.

### Page extension

```rust
pub(crate) fn page_extend_count(
    block_size: BlockSize,
    capacity: PageCapacity,
    reserved: ReservedBlockCount,
) -> Option<PageExtendCount>;
```

Compatibility behavior:

```text
remaining = reserved - capacity

max_extend =
    if block_size >= 4096:
        MI_MIN_EXTEND
    else:
        4096 / block_size

max_extend = max(max_extend, MI_MIN_EXTEND)
extend = min(remaining, max_extend)
```

Security-dependent `MI_MIN_EXTEND` behavior remains unchanged.

### Candidate scanning

```rust
pub(crate) fn initial_full_retain(
    block_size: BlockSize,
    configured: PageFullRetain,
) -> PageReclaimLimit;

pub(crate) fn page_candidate_limit(
) -> PageReclaimLimit;
```

Compatibility behavior:

* full retain becomes zero above `MI_SMALL_MAX_OBJ_SIZE`;
* candidate limit reads `page_max_candidates`;
* candidate and retain countdown semantics remain unchanged.

### Retirement

```rust
pub(crate) const fn retire_expire(
    block_size: BlockSize,
) -> RetireExpire;
```

Compatibility values:

```text
small page/object = 16
larger page/object = 4
```

### Reclaim on free

```rust
pub(crate) fn reclaim_on_free_mode(
) -> PageReclaimOnFree;

pub(crate) fn reclaim_limit(
    context: ReclaimContext,
) -> PageReclaimLimit;
```

Compatibility behavior:

* same-thread non-thread-pool uses `page_max_reclaim`;
* thread-pool uses `page_cross_thread_max_reclaim`;
* eligible cross-thread reclaim uses `page_cross_thread_max_reclaim`;
* disabled remains represented by a negative limit.

### Remote collection mode

```rust
pub(crate) const fn remote_collect_mode(
    block_size: BlockSize,
) -> RemoteCollectMode;
```

Compatibility behavior:

```text
block_size <= MI_SMALL_SIZE_MAX
    => partial collection preserving one published head

otherwise
    => full collection
```

### Policy purity

For each option-dependent method, split loading from pure interpretation:

```rust
fn page_candidate_limit_from_raw(raw: c_long) -> PageReclaimLimit;

fn page_candidate_limit() -> PageReclaimLimit {
    let raw = _mi_option_get_fast(...);
    Self::page_candidate_limit_from_raw(raw)
}
```

This allows exhaustive testing without mutating process-global options.

---

## 7.5 Decisions included in Stage 2

Stage 2 MUST move the following decisions behind `CompatPolicy`:

1. Generic maintenance interval.
2. Generic full-collection threshold.
3. Thread-heap reclaim/abandon flags.
4. Full-page retain count.
5. Thread-pool retain adjustment.
6. Page extension count.
7. Page candidate scan limit.
8. Small-versus-large retirement expiry.
9. Reclaim-on-free mode.
10. Same-thread and cross-thread reclaim limits.
11. Partial-versus-full remote collection.
12. Any immediate helper that exactly duplicates one of those decisions.

---

## 7.6 Decisions excluded from Stage 2

Do not move these into `CompatPolicy` yet:

* size-class geometry;
* bin mapping;
* page-map geometry;
* arena slice size;
* raw structure layouts;
* allocation alignment semantics;
* zero-size allocation semantics;
* encoded-pointer algorithms;
* secure encoding keys;
* guarded-page layout;
* atomic memory ordering;
* corruption-detection rules;
* ABI error codes;
* platform primitive selection.

These are either allocator mechanism, compatibility format, safety protocol, or platform policy—not the initial tuning surface.

---

## 7.7 Mechanism call-site migration

The following files should stop directly reading the scoped options or hardcoding the scoped values:

```text
src/allocator/page.rs
src/allocator/page/lifecycle.rs
src/allocator/free/abandoned.rs
src/allocator/thread_heap.rs
src/allocator/init.rs
```

Example conversion:

Before:

```rust
let mut max_extend = if bsize >= MI_MAX_EXTEND_SIZE {
    MI_MIN_EXTEND
} else {
    MI_MAX_EXTEND_SIZE / bsize
};
```

After:

```rust
let Some(extend) = ActivePolicy::page_extend_count(
    block_size,
    capacity,
    reserved,
) else {
    return true;
};
```

Before:

```rust
if (*theap).generic_count >= 1000 {
    ...
}
```

After:

```rust
if ActivePolicy::generic_maintenance_due(
    (*theap).generic_count,
) {
    ...
}
```

The generated release code must be verified rather than assumed equivalent.

---

## 7.8 Preserve option-read frequency

`CompatPolicy` MUST NOT cache global options in Stage 2.

This matters because:

* applications may set options during initialization;
* the current mechanism’s load timing is part of compatibility behavior;
* benchmark diagnostics already count reads of reclaim and candidate options.

Add a diagnostic test that:

1. Resets benchmark diagnostics.
2. Executes each targeted slow path once.
3. Captures option-read counts.
4. Compares them to the pre-migration reference count.

No scoped option should be read more or less often after extraction.

Caching belongs to a later profile or adaptive-policy stage.

---

## 7.9 Reference policy model

`policy/reference.rs`, under `#[cfg(test)]`, contains independent formulas corresponding to the pre-extraction implementation.

It MUST NOT call `CompatPolicy`.

Examples:

```rust
fn reference_extend_count(
    block_size: usize,
    capacity: u16,
    reserved: u16,
    secure_level: usize,
) -> u16;
```

```rust
fn reference_retire_expire(
    block_size: usize,
) -> u8;
```

```rust
fn reference_thread_heap_options(
    reclaim_on_free: c_long,
    full_retain: c_long,
    in_thread_pool: bool,
) -> ThreadHeapPolicy;
```

The reference functions are test oracles and should remain simple enough to review against the old code.

---

## 7.10 Stage 2 tests

### Exhaustive scalar tests

For every valid `u16` capacity/reserved pair where `capacity <= reserved`, and representative block sizes around:

```text
1
MI_MIN_EXTEND
8
16
64
1024
4095
4096
4097
MI_SMALL_MAX_OBJ_SIZE
MI_SMALL_MAX_OBJ_SIZE + 1
MI_MEDIUM_MAX_OBJ_SIZE
```

compare `CompatPolicy::page_extend_count` to the reference model.

### Option-range tests

Test raw values around:

```text
c_long::MIN
-2
-1
0
1
2
4
32
33
1000
10000
1_000_000
c_long::MAX
```

for:

* reclaim mode;
* full retain;
* candidate limit;
* generic collection;
* same-thread and cross-thread reclaim.

### Thread-pool tests

Verify:

```text
-1 remains -1
0 remains 0
1 / 4 becomes 0
2 / 4 becomes 0
3 / 4 becomes 0
4 / 4 becomes 1
32 / 4 becomes 8
```

using the same signed integer semantics as current code.

### Boundary tests

Test retirement and remote collection exactly at:

```text
MI_SMALL_MAX_OBJ_SIZE
MI_SMALL_MAX_OBJ_SIZE + 1
MI_SMALL_SIZE_MAX
MI_SMALL_SIZE_MAX + 1
```

### Source-boundary test

Add:

```text
scripts/check-policy-boundaries.py
```

It rejects direct reads of scoped options outside `src/allocator/policy`.

It also rejects the old hardcoded policy constant names outside the policy module where feasible.

### Assembly and benchmark tests

The allocation and free hot-path assembly must remain instruction-equivalent.

Stage 0 smoke and full tests must pass without changing baseline numbers solely to accommodate Stage 2.

---

## 7.11 Stage 2 acceptance

Stage 2 is complete when:

* `CompatPolicy` owns every decision listed in Section 7.5;
* mechanism code no longer directly reads those options;
* no trait object or dynamic policy dispatch exists;
* no option-read count changes;
* reference-model tests cover the boundary space;
* raw layouts and symbols are unchanged;
* hot-path assembly is instruction-equivalent;
* Stage 0 performance/resource evidence passes;
* compatibility behavior is byte-for-byte or decision-for-decision equivalent on all tested inputs.

---

# 8. Stage 3 — Introduce page-homogeneous batches

## 8.1 Purpose

The current available and local free-list wrappers carry only a head pointer. Local-to-available collection walks the entire local list to find its tail before splicing it.  Detached remote collection similarly walks the chain to count it, identify the tail, validate it against capacity and `used`, splice it, and reduce `used`.

Stage 3 captures those facts in a first-class batch. This is not primarily an immediate speed optimization; it creates the ownership and accounting unit required by magazines, per-CPU depots, and batch remote-free processing.

---

## 8.2 Batch scalar types

Add to `src/types/scalar.rs`:

```rust
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct BatchLen(NonZeroU16);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct BatchLimit(NonZeroU16);
```

### `BatchLen`

```rust
impl BatchLen {
    pub(crate) fn new(value: u16) -> Option<Self>;
    pub(crate) unsafe fn new_unchecked(value: u16) -> Self;
    pub(crate) const fn get(self) -> u16;
    pub(crate) const fn as_usize(self) -> usize;
}
```

### `BatchLimit`

Same basic API, plus:

```rust
pub(crate) fn clamp_to_remaining(
    self,
    remaining: usize,
) -> Option<BatchLimit>;
```

An empty batch is represented as `Option<Batch>`, never as `BatchLen(0)`.

### Used-count operations

Extend `UsedBlockCount`:

```rust
fn checked_add_batch(
    self,
    len: BatchLen,
    capacity: PageCapacity,
) -> Option<UsedBlockCount>;

fn checked_sub_batch(
    self,
    len: BatchLen,
) -> Option<UsedBlockCount>;
```

The production path may use an unchecked/inlined form only after the batch invariant has proved the bounds.

---

## 8.3 Batch module

Create:

```text
src/types/page_batch.rs
src/types/page_batch/tests.rs
```

Re-export the necessary crate-private types through `src/types/mod.rs`.

### Internal core

```rust
struct BatchCore {
    page: PagePtr,
    head: BlockPtr,
    tail: BlockPtr,
    len: BatchLen,
}
```

`BatchCore` is private and is neither `Copy` nor `Clone`.

### Public crate-private batch capabilities

```rust
#[must_use = "remote free batches must be collected or republished"]
pub(crate) struct RemoteFreeBatch(BatchCore);

#[must_use = "local free batches must be returned to page availability"]
pub(crate) struct LocalFreeBatch(BatchCore);

#[must_use = "leased batches must be allocated from or returned to their page"]
pub(crate) struct LeasedBatch(BatchCore);
```

These are non-`Copy`, non-`Clone`, and have no `Drop`.

---

## 8.4 Batch invariants

Every `BatchCore` MUST satisfy:

1. `page`, `head`, and `tail` are non-null.
2. `len >= 1`.
3. Starting at `head`, exactly `len - 1` decoded next links reach `tail`.
4. `tail.next` is null while the batch is detached.
5. Every link is decoded using the same page and page keys.
6. Every block belongs to the batch’s page.
7. No block appears twice.
8. The traversal does not exceed page capacity.
9. The batch origin determines `page.used` accounting.

### Origin accounting

#### `RemoteFreeBatch`

* Every block is free.
* Every block is still included in `page.used`.
* Collection into local or available storage must subtract `len` once.

#### `LocalFreeBatch`

* Every block is free.
* Every block has already been excluded from `page.used`.
* Moving it to `page.free` must not change `used`.

#### `LeasedBatch`

* Every block was detached from `page.free`.
* On lease, `page.used` was increased by `len`.
* Blocks remain counted while cached by a future local front end.
* Allocating a block from the lease does not change `used`.
* Returning unused blocks to `page.free` subtracts the returned length.

---

## 8.5 Batch traversal and errors

Define:

```rust
pub(crate) enum BatchValidationError {
    TooLong {
        observed_at_least: usize,
        capacity: PageCapacity,
    },
    ExceedsUsed {
        len: BatchLen,
        used: UsedBlockCount,
    },
    DifferentPage,
    InvalidLink,
}
```

A cycle naturally becomes `TooLong` after capacity-bounded traversal.

### Scan helper

```rust
unsafe fn scan_detached_chain(
    page: PagePtr,
    head: BlockPtr,
    capacity: PageCapacity,
) -> Result<BatchCore, BatchValidationError>;
```

The helper:

* walks with the existing encoded-link abstraction;
* never uses direct `mi_block_s.next` access;
* performs at most `capacity + 1` iterations;
* records tail and length;
* verifies the terminating null;
* performs page-membership checks in hardened/debug modes;
* allocates no memory;
* takes no lock.

---

## 8.6 Detached remote list to batch

```rust
impl DetachedRemoteList {
    pub(crate) unsafe fn into_batch_for_local(
        self,
        page: &LocalPage,
    ) -> Result<Option<RemoteFreeBatch>, BatchValidationError>;

    pub(crate) unsafe fn into_batch_for_abandoned(
        self,
        page: &OwnedAbandonedPage,
    ) -> Result<Option<RemoteFreeBatch>, BatchValidationError>;
}
```

The implementation delegates to one private raw scan after extracting the owner’s `PagePtr`.

After scanning, it checks:

```text
batch.len <= page.used
```

Failure maps to the existing messages:

```text
"corrupted thread-free list\n"
"corrupted meta-data in thread-free list\n"
```

Do not silently change these diagnostics in Stage 3.

---

## 8.7 Remote batch collection

```rust
impl RemoteFreeBatch {
    pub(crate) fn page(&self) -> PagePtr;
    pub(crate) fn head(&self) -> BlockPtr;
    pub(crate) fn tail(&self) -> BlockPtr;
    pub(crate) fn len(&self) -> BatchLen;

    pub(crate) unsafe fn collect_into_local_page(
        self,
        owner: &LocalPage,
        current: LocalFreeList,
    ) -> LocalFreeList;

    pub(crate) unsafe fn collect_into_owned_abandoned_page(
        self,
        owner: &OwnedAbandonedPage,
        current: LocalFreeList,
    ) -> LocalFreeList;
}
```

Collection performs:

1. Assert owner page identity matches batch page.
2. Link `batch.tail` to `current.head`.
3. Set `page.local_free = batch.head`.
4. Subtract `batch.len` from `page.used`.
5. Consume the batch.

No second traversal is permitted.

---

## 8.8 Local list to batch

Add a typed detach operation:

```rust
impl LocalPage {
    pub(crate) unsafe fn detach_local_free_batch(
        &self,
    ) -> Result<Option<LocalFreeBatch>, BatchValidationError>;
}
```

The method:

1. Reads `page.local_free`.
2. Returns `Ok(None)` if empty.
3. Scans the chain using a capacity bound.
4. On success, clears `page.local_free`.
5. Returns `LocalFreeBatch`.
6. On failure, leaves or restores `page.local_free` unchanged.

The method does not modify `page.used`, because local free already decremented it.

### Prepending to available

```rust
impl LocalFreeBatch {
    pub(crate) unsafe fn prepend_to_available(
        self,
        owner: &LocalPage,
        available: AvailableFreeList,
    ) -> AvailableFreeList;
}
```

It:

1. Links `tail` to the existing available head.
2. Returns a new available list headed by `head`.
3. Performs no `used` update.
4. Consumes the batch.

---

## 8.9 Integrate local collection

Refactor `_mi_page_free_collect`:

Before:

```text
read local list
walk to local tail
link tail to available
set free
clear local
```

After:

```text
read whether movement is required
detach LocalFreeBatch
prepend batch to available
set free
```

Behavior remains:

* if available is empty, move local to available;
* if `force`, prepend local even when available is nonempty;
* otherwise retain local when available is already nonempty;
* `free_is_zero` becomes false when movement occurs;
* forced collection ends with an empty local list.

The old `LocalFreeList::prepend_to_available` tail-walking implementation should be deleted or retained only as a test reference during migration.

---

## 8.10 Integrate detached remote collection

Refactor the common remote collector:

```rust
unsafe fn collect_detached_remote_for_local(
    page: &LocalPage,
    detached: DetachedRemoteList,
) -> Result<LocalFreeList, BatchValidationError>;
```

and:

```rust
unsafe fn collect_detached_remote_for_abandoned(
    page: &OwnedAbandonedPage,
    detached: DetachedRemoteList,
) -> Result<LocalFreeList, BatchValidationError>;
```

Each function:

1. Converts detached list to `RemoteFreeBatch`.
2. Returns current local list for an empty detached list.
3. Emits the existing corruption diagnostic on validation failure.
4. Collects a valid batch into local in one splice and one used-count update.

### Published-head partial collection

The small-page abandoned path operates on `PublishedRemoteHead`:

```rust
let (published, detached_tail) =
    published.detach_tail(&owned_page);

if let Some(detached_tail) = detached_tail {
    collect_detached_remote_for_abandoned(
        &owned_page,
        detached_tail,
    )?;
}
```

The still-published head remains counted and linked to null.

This preserves the current protocol while making ownership explicit.

---

## 8.11 Lease primitive

Stage 3 must implement but not production-wire a lease operation.

```rust
impl AvailablePage {
    pub(crate) unsafe fn lease(
        self,
        limit: BatchLimit,
    ) -> (LeasedBatch, ActivePage);
}
```

### Lease algorithm

1. Read the available-list head.
2. Traverse at most `limit` blocks or until null.
3. Record head, tail, and length.
4. Save the pointer following the leased tail.
5. Set leased tail’s next link to null.
6. Set `page.free` to the saved remaining head.
7. Increase `page.used` by leased length.
8. Return `LeasedBatch` and `ActivePage`.

The page’s `used` count now means, for these blocks:

```text
live user allocations + blocks leased to a local front end
```

This is exactly the accounting needed by a future magazine.

### Allocate one from lease

```rust
impl LeasedBatch {
    pub(crate) unsafe fn take_one(
        self,
    ) -> (
        AllocatedBlock,
        Option<LeasedBatch>,
    );
}
```

Behavior:

* removes the head block;
* does not change `page.used`;
* returns the remaining detached lease, if any;
* preserves page identity;
* does not perform zeroing, padding, tracking, or user-visible initialization. Those remain allocation-mechanism responsibilities.

### Return unused lease

```rust
impl LeasedBatch {
    pub(crate) unsafe fn return_to_available(
        self,
        owner: &LocalPage,
        available: AvailableFreeList,
    ) -> (
        AvailableFreeList,
        UsedBlockCount,
    );
}
```

Behavior:

1. Link batch tail to available head.
2. Make batch head the new available head.
3. Subtract batch length from `used`.
4. Consume the batch.

### Production use

No call from `mi_malloc`, `mi_malloc_small`, `_mi_page_malloc_zero_typed`, or the current local free path is permitted in Stage 3.

The lease API is exercised only by:

* unit tests;
* differential tests;
* exact function-item reachability where required by dead-code discipline.

Stage 4 will wire it into magazines.

---

## 8.12 Free-list wrapper restructuring

After Stage 3:

### `AvailableFreeList`

Remains a lightweight page-field value:

```rust
pub(crate) struct AvailableFreeList(Option<BlockPtr>);
```

It is non-`Copy`.

It supports:

```rust
empty
head
as_raw
from_raw
pop_one
```

### `LocalFreeList`

Remains a page-field value, non-`Copy`.

It supports:

```rust
empty
head
as_raw
from_raw
```

Its old arbitrary tail-splice method is removed. Batch conversion owns tail discovery.

### `DetachedRemoteList`

Owns a detached atomic chain.

### `PublishedRemoteHead`

Owns no detached chain; it represents a still-published head.

### Batch types

Own detached, page-homogeneous multi-block chains.

This separation makes it impossible to pass a local-list snapshot where detached-chain ownership is required.

---

## 8.13 Stage 3 correctness tests

### Basic chain lengths

For lengths:

```text
1, 2, 3, 4, 7, 8, 15, 16, capacity - 1, capacity
```

test:

* detected head;
* detected tail;
* exact length;
* null tail;
* page identity;
* forward iteration sequence.

### Destination combinations

For each batch type, test:

* empty destination list;
* one-element destination;
* multi-element destination.

Verify exact resulting link order.

### Accounting tests

#### Remote batch

Starting:

```text
used = N + K
remote batch len = N
```

After collection:

```text
used = K
```

#### Local batch

`used` unchanged after local-to-available movement.

#### Lease batch

Starting:

```text
used = K
available has N or more blocks
```

After leasing `N`:

```text
used = K + N
```

After allocating `M` from lease:

```text
used remains K + N
remaining lease len = N - M
```

After returning remaining `N - M`:

```text
used = K + M
```

The `M` allocated blocks remain counted.

### Corruption tests

Test:

* chain longer than capacity;
* chain length greater than `used` for remote batch;
* cycle;
* self-loop;
* pointer from another page;
* invalid encoded link in secure mode;
* null head passed through an optional-list conversion;
* duplicate block.

No corruption test may hang.

### Secure mode

Run batch link tests with:

```text
--features secure
```

All traversal and splicing must use page-local encoding helpers.

### Differential reference tests

Keep test-only reference implementations of the old algorithms:

```rust
reference_prepend_local_to_available
reference_collect_remote_into_local
reference_available_pop
```

For randomized valid list topologies:

1. Clone the synthetic metadata and block arrays into two independent fixtures.
2. Run the old reference algorithm on one.
3. Run the batch algorithm on the other.
4. Compare:

   * page free head;
   * local free head;
   * every next link;
   * `used`;
   * capacity;
   * resulting list order;
   * diagnostics outcome.

Use a fixed seed and print it on failure.

### Atomic-model tests

Extend existing concurrency models to verify:

* atomic detach returns a chain to only one collector;
* a detached chain is not subsequently observed as atomic head;
* a published head remains visible while its tail is detached;
* remote push concurrent with detach ends in either the old or new atomic chain, never a lost node;
* the batch conversion occurs only after unique detachment.

---

## 8.14 Stage 3 performance tests

### Assembly

The following must remain instruction-equivalent:

```text
mi_malloc
mi_malloc_small
mi_free local path
mi_free remote CAS loop
_mi_theap_malloc_zero_ex
```

The production allocation path still uses one-block pop and must not mention `LeasedBatch`.

### Slow-path targeted benchmarks

Add or retain scenarios for:

* forced page-free collection;
* local-list-to-available splice at list lengths 1, 8, 32, 128;
* detached remote collection at list lengths 1, 8, 32, 128;
* mixed empty/nonempty available destination;
* secure encoded-list collection.

These benchmarks may improve or slightly change because the code is being reorganized. Any repeated regression over 3% requires investigation and explanation.

### Memory

Batch objects are stack values and must allocate no memory. The resource probe should show no meaningful RSS or commit increase attributable to Stage 3.

---

## 8.15 Stage 3 acceptance

Stage 3 is complete when:

* batch scalar types exist;
* `RemoteFreeBatch`, `LocalFreeBatch`, and `LeasedBatch` are non-`Copy`;
* every batch records page, head, tail, and nonzero length;
* detached remote collection uses `RemoteFreeBatch`;
* local-to-available movement uses `LocalFreeBatch`;
* published-head partial collection remains distinct;
* lease, take-one, and return primitives exist and pass accounting tests;
* the production allocation path does not use lease primitives yet;
* randomized differential tests pass;
* secure encoded-list tests pass;
* atomic ownership models pass;
* raw layouts and symbols are unchanged;
* hot-path assembly is instruction-equivalent;
* Stage 0 benchmark and resource gates pass.

---

# 9. Required file-level change map

## Stage 0

| File                                | Required change                                                                     |
| ----------------------------------- | ----------------------------------------------------------------------------------- |
| `benches/hotpath.rs`                | Remove timed harness allocations; use shared adapters and persistent remote fixture |
| `benches/workloads.rs`              | Correct stale `GlobalAlloc` claim; use shared allocator adapters                    |
| `benches/mimalloc_bench.rs`         | Normalize allocator naming and artifact metadata                                    |
| `benches/support/*`                 | Add fixed pools, adapters, counting system allocator, remote fixture, metrics       |
| `examples/benchmark_probe.rs`       | Fresh-process resource scenarios                                                    |
| `scripts/check-bench-smoke.sh`      | Clean dedicated smoke target                                                        |
| `scripts/check-bench-full.sh`       | Clean full targets and emit complete artifacts                                      |
| `scripts/check-bench-thresholds.py` | Schema 2, artifact-aware median/p95 gates                                           |
| `benchmarks/thresholds.json`        | Reviewed clean baseline                                                             |
| `docs/*`, `TYPES_SPEC.md`           | Reconcile claims and document protocol                                              |

## Stage 1

| File                              | Required change                                   |
| --------------------------------- | ------------------------------------------------- |
| `src/types/page_states.rs`        | Non-`Copy` capabilities and consuming transitions |
| `src/types/page_queues.rs`        | Result-returning membership transitions           |
| `src/types/validation.rs`         | Linear validated-free path                        |
| `src/types/validation_extra.rs`   | Linear block capabilities                         |
| `src/types/freelist_wrappers.rs`  | Non-copy list-field handles                       |
| `src/types/atomic/thread_free.rs` | Published versus detached remote list             |
| `src/types/raw_bridge.rs`         | Central unchecked capability construction         |
| `src/allocator/free.rs`           | Consume validation and page-owner capabilities    |
| `src/allocator/free/abandoned.rs` | Linear disposition state machine                  |
| `src/allocator/page/lifecycle.rs` | Separate local and abandoned collection           |
| `scripts/check-strict-types.py`   | Architectural boundary enforcement                |
| `src/types/linearity_tests.rs`    | Compiler-checked capability linearity              |

## Stage 2

| File                                 | Required change                               |
| ------------------------------------ | --------------------------------------------- |
| `src/allocator/policy/*`             | Concrete compatibility policy                 |
| `src/allocator/page.rs`              | Use policy for maintenance, extension, search |
| `src/allocator/page/lifecycle.rs`    | Use policy for retirement                     |
| `src/allocator/free/abandoned.rs`    | Use policy for reclaim and collection mode    |
| `src/allocator/init.rs`              | Use policy for thread-heap options            |
| `src/allocator/thread_heap.rs`       | Use policy for thread-pool adjustment         |
| `scripts/check-policy-boundaries.py` | Enforce scoped decisions                      |
| `tests/*`                            | Behavioral policy coverage                    |

## Stage 3

| File                              | Required change                                            |
| --------------------------------- | ---------------------------------------------------------- |
| `src/types/scalar.rs`             | Batch length/limit and used-count operations               |
| `src/types/page_batch.rs`         | Batch core and three batch capabilities                    |
| `src/types/freelist_wrappers.rs`  | Remove arbitrary tail-splice ownership                     |
| `src/types/atomic/thread_free.rs` | Batch conversion from detached chains                      |
| `src/allocator/page/lifecycle.rs` | Local and remote batch collection                          |
| `src/allocator/free/abandoned.rs` | Published-head partial batch flow                          |
| `tests/*`                         | Differential, corruption, accounting, secure, atomic tests |
| `benches/*`                       | Slow-path batch benchmarks                                 |

---

# 10. Verification matrix

Every final stage signoff should run this matrix.

| Check                       |  Stage 0 |     Stage 1 |             Stage 2 |            Stage 3 |
| --------------------------- | -------: | ----------: | ------------------: | -----------------: |
| Formatting                  | Required |    Required |            Required |           Required |
| Unit tests                  | Required |    Required |            Required |           Required |
| Release audit               | Required |    Required |            Required |           Required |
| Strict-type checker         | Required |    Required |            Required |           Required |
| Default feature             | Required |    Required |            Required |           Required |
| `std`                       | Required |    Required |            Required |           Required |
| `secure`                    | Required |    Required |            Required |           Required |
| `guarded`                   | Required |    Required |            Required |           Required |
| `track-valgrind`            | Required |    Required |            Required |           Required |
| `track-asan` compile/export | Required |    Required |            Required |           Required |
| C ABI symbol check          | Required |    Required |            Required |           Required |
| Layout check                | Required |    Required |            Required |           Required |
| Interposition check         | Required |    Required |            Required |           Required |
| Benchmark smoke             | Required |    Required |            Required |           Required |
| Full clean benchmark        | Baseline |     Signoff |             Signoff |            Signoff |
| Resource probe              | Baseline |     Signoff |             Signoff |            Signoff |
| Assembly comparison         | Baseline |    Required |            Required |           Required |
| Random differential tests   |        — | Recommended | Required for policy | Required for lists |
| Loom/model tests            | Existing |    Expanded |            Existing |           Expanded |

---

# 11. Rollback boundaries

Each stage must have an obvious rollback point.

## Stage 0 rollback

Benchmark tooling can be reverted without allocator changes. Do not start Stage 1 until the new baseline is accepted.

## Stage 1 rollback

Keep raw representation unchanged. If a non-`Copy` conversion changes hot-path code, revert only the affected capability conversion and document why it remains an identity-like witness. Do not weaken the entire capability policy.

## Stage 2 rollback

Each policy method can be inlined back into its original mechanism call site because no data layout changes. Preserve the reference policy tests even during rollback.

## Stage 3 rollback

The old algorithms remain as test-only reference implementations. Production can be switched back to head-only collection while preserving the batch types and lease tests. No C layout rollback is necessary.

---

# 12. Final definition of done for Stages 0–3

The four-stage program is complete when all of the following are true:

1. Benchmark hot paths contain no unintended harness allocation.
2. Remote-free steady-state benchmarks use persistent threads.
3. Clean, artifact-specific benchmark directories are mandatory.
4. Performance, environment, command, and resource outputs are machine-readable.
5. Documentation describes the current architecture accurately.
6. Every allocator-state and detached-resource capability is non-`Copy`.
7. Failed consuming transitions return their original capability.
8. Unchecked capability construction exists only in one audited raw bridge and true typed transitions.
9. Published remote heads and detached remote lists are different types.
10. Local remote collection never fabricates abandoned-page ownership.
11. Abandoned-page collection is a consuming state machine with one terminal disposition.
12. Existing heuristic decisions are owned by a concrete `CompatPolicy`.
13. `CompatPolicy` changes neither behavior nor option-read frequency.
14. No dynamic policy dispatch exists.
15. Page batches carry page identity, head, tail, and nonzero length.
16. Remote and local batch accounting match existing `used` semantics.
17. Lease and return primitives correctly reserve and release page blocks.
18. Production allocation remains on the existing one-block path.
19. Raw layouts, symbols, and public API remain unchanged.
20. Allocation, local-free, and remote-CAS hot paths remain instruction-equivalent.
21. All release, feature, fuzz/model, differential, benchmark, and resource gates pass against the Stage 0 baseline.

At that point, Stage 4 can introduce a single-page thread magazine without first having to invent ownership, batch accounting, policy, or evidence infrastructure inside the magazine patch itself.
