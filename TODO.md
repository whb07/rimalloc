# Remaining engineering work

This backlog contains concrete unresolved problems in `mechanical_mimalloc`.
It is not an unsafe inventory, release checklist, or mandate to rewrite the
mechanical allocator core. Preserve mimalloc 3.3.2 behavior, the supported
C/C++ ABI, allocator performance, `no_std` operation, and the mechanical
relationship with upstream.

## Rules for accepting work

A task belongs here only when it names at least one of:

- a reproduced correctness, safety, lifecycle, or compatibility defect;
- a test that is currently ineffective or permanently ignored;
- duplicated production validation or unsafe reasoning that can be removed;
- measured runtime, code-size, or resource overhead;
- documentation duplication that creates a real maintenance conflict.

Every implementation must state the problem in one sentence, add or identify a
regression test, make the smallest production change, and report the observable
result. A production refactor is complete only when it removes or narrows a real
caller obligation, unsafe operation, unchecked constructor, duplicate
validation path, internal ABI boundary, allocation, branch, lock, or other
runtime work. Moving an unsafe operation behind a new wrapper is not sufficient
unless the wrapper enforces an invariant that callers previously had to prove.

Do not:

- inventory, classify, or annotate every occurrence of a source pattern;
- add ledgers, allowlists, count caps, audit IDs, source-shape tests, or
  completion-evidence files;
- split modules solely for size, lint, or review-unit targets;
- add abstractions for one caller or unsupported future work;
- rewrite correct mechanical C-shaped code merely to make it look idiomatic;
- create a commit whose only result is checking boxes or recording `NO CHANGE`;
- claim improvement from comment volume, unsafe-block count, test count, commit
  count, or a single benchmark run.

Safety documentation is required for public unsafe APIs and for unsafe code
changed by a task. There is no remaining global sweep to rewrite every unsafe
function, block, or comment. Lint and forced-inline attributes change only when
a concrete production edit exposes an obsolete exception or measured
code-generation issue.

## Verification

After every Rust source change, run:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
```

Also run the focused parity, layout, ABI/FFI, Miri, concurrency, sanitizer, or
assembly checks justified by the changed behavior. A required failure must be
fixed or reported as an explicit blocker; unrelated passing checks do not waive
it.

For documentation-only changes, run:

```bash
scripts/check-fmt.sh
scripts/check-docs.sh
```

Release qualification is owned by [`docs/release.md`](docs/release.md) and must
not be duplicated here.

## Completed results retained from the improvement series

Detailed completion history belongs in Git. The retained results are:

- Deleted the 193-file `release_fast_path_audit` harness, unsafe evidence
  ledgers, migration/count allowlists, marker IDs, wording assertions, and
  benchmark-as-local-gate machinery (`4c51636`).
- Replaced capability-linearity text inspection with compiler assertions.
- Fixed the concurrency-model script so a zero-test selection fails and all six
  intended Loom models execute.
- Split initialization, option, statistics, and arena responsibilities by
  lifecycle or invariant rather than by a line-count cap.
- Removed synthetic reachability helpers, broad internal re-exports, and
  accidental crate-root coupling.
- Centralized repeated page, block, heap, arena, subprocess, TLS, statistics,
  free-route, and OS pointer invariants in typed operation boundaries.
- Removed or narrowed many unsafe caller obligations. Further safety narrowing
  must accompany a concrete task below and reduce production obligations or
  complexity rather than expand comments around unchanged operations.
- Isolated all eight process-global callback behaviors in dedicated child test
  processes and enabled them in normal test runs.
- Deleted the internal unsafe comment-marker auditor, lexical unsafe inventory,
  and their release/documentation wiring while retaining behavior and boundary
  checks.
- Deleted the unsafe-scope completion-evidence document and removed historical
  extraction-plan text from `ARCHITECTURE.md`.
- Made the zero-allocation assembly evidence filename agree with its captured
  `_mi_theap_malloc_zero` symbol and archive contract.

---

# Priority 0 — Correctness and ineffective tests

## 1. Resolve or close the thread-lifecycle TSan finding

Problem: `PERFORMANCE_WORKLOG.md` records a TSan race between
`_mi_theap_create` and `_mi_thread_done`, but the repository does not identify a
maintained minimized reproducer and exact invocation for that report.

- [ ] Record the exact current-HEAD TSan command, toolchain, probe source,
  allocator features, expected stacks, and relevant environment in
  `PERFORMANCE_WORKLOG.md`.
- [ ] Run that configuration at least three times. If the original artifact or
  probe cannot be recovered, construct the smallest maintained
  create/allocate/free/exit probe that exercises the reported boundary.
- [ ] If the report reproduces, compare each access and synchronization edge
  with C mimalloc 3.3.2 and classify it as a translation defect, inherited
  defect, or sanitizer false positive.
- [ ] For a defect, add a regression test before making the smallest change
  that establishes the missing happens-before relationship. Do not add a
  global lock or strengthen unrelated atomics.
- [ ] For a false positive, retain the minimized reproducer and use only an
  exact-boundary suppression. If the report does not reproduce, close the stale
  finding with the commands and results; do not retain an open-ended audit.

Verify: ten clean TSan runs of a reproducing fixed/suppressed probe, or ten
clean runs of the recovered original configuration when closing it as stale;
release thread stress; the six Loom models; behavioral parity; relevant ASan
and Valgrind checks; the common Rust gate; and allocation/free assembly if
synchronization changes.

Done when the repository contains a maintained reproducer plus a tested fix or
narrow suppression, or repeatable current evidence closes the stale report.

---

# Priority 1 — Measured regressions

## 2. Re-establish the post-refactor Rust/C performance baseline

Problem: structural changes invalidate older performance conclusions. No new
optimization is justified until the current Rust artifact is compared fairly
with mimalloc 3.3.2.

- [ ] Build clean Rust and C release artifacts in separate target directories
  and record commit, toolchains, commands, hashes, dependencies, and host state
  in `PERFORMANCE_WORKLOG.md`.
- [ ] Run the balanced comparison protocol in both allocator orders and capture
  normalized counters, resource use, and release assembly.
- [ ] Treat differences as actionable only when both orders and repeated runs
  distinguish them from host noise.

Verify: the full correctness and release gates pass on the exact measured
revision and every reported number resolves to an immutable artifact hash.

Done when there is a trustworthy current baseline. This task makes no
production change by itself and does not require a TODO-only completion commit.

## 3. Investigate low-contention remote-free overhead

Problem: existing evidence reports a Rust/C gap in 1+1 remote-free workloads,
especially for 64–2048-byte objects. Start only if task 2 reproduces it.

- [ ] Separate allocation work from remote publication, detachment, scan,
  collection, reclaim, and ownership release.
- [ ] Identify one repeated load, validation, branch, division, scan, or state
  reconstruction confirmed by both counters and assembly.
- [ ] Change one mechanism only. Preserve ownership, corruption detection, and
  debug/secure validation.

Verify: a focused invariant test; relevant Loom, Miri/fuzz, thread stress,
sanitizer, and Valgrind checks; disappearance of the targeted emitted work; two
balanced comparisons; held-out allocator workloads; and the common Rust gate.

Done when a retained change removes general allocator work and improves wall
time and normalized instructions/cycles without a held-out or resource
regression. Record rejected experiments only in `PERFORMANCE_WORKLOG.md` and
leave production unchanged. If task 2 does not reproduce the gap, remove this
conditional task in the same baseline-results change rather than committing a
standalone TODO update.

## 4. Investigate the `glibc-thread` difference

Problem: prior results reported an approximately 1% difference. Start only if
task 2 reproduces it at high resolution.

- [ ] Profile allocator symbols separately from the workload and C++ boundary.
- [ ] Require counters and annotated assembly to identify one common-path
  difference before editing code.
- [ ] Test one candidate mechanism at a time; do not guess from source layout,
  branch hints, or forced-inline attributes.

Verify: balanced order with enough samples to resolve a 1% effect, relevant
held-out allocation and resource probes, and the full correctness gate.

Done when a general improvement reduces the gap. If repeated measurements show
noise or compiler behavior, record that result only in `PERFORMANCE_WORKLOG.md`
and remove this conditional task in the same results change; do not modify
production or create a standalone TODO commit.

---

# Priority 2 — Remove maintenance machinery and duplicated policy

## 5. Replace the raw-bridge caller allowlist with enforceable boundaries

Problem: `scripts/check-strict-types.py` contains an exact
`RAW_BRIDGE_CALLERS` file allowlist and rejects inventory changes. This is a
source ledger, while the actual requirement is that unchecked state
construction remain confined to owning modules.

- [ ] Group each remaining `assume_*` bridge by the invariant owner that can
  validate or construct the state.
- [ ] Use Rust module privacy, private fields, checked constructors, or an
  operation-specific safe transition to make unauthorized construction
  unnameable or unusable.
- [ ] Remove a bridge entry only when its caller obligation has disappeared;
  do not move the same allowlist into another script or document.
- [ ] Delete `RAW_BRIDGE_CALLERS` and its inventory-equality check when the
  compiler-enforced boundaries cover every remaining bridge. Retain unrelated
  strict-type checks only where Rust visibility cannot express the boundary.

Verify: compile-fail or module tests for the new private boundaries, focused
state-transition tests, `scripts/check-strict-types.py`, relevant Miri/parity
checks, and the common Rust gate.

Done when exact caller-file bookkeeping is gone and callers have fewer
unchecked construction obligations. Deleting the allowlist without replacing
its real boundary protection is not completion.

## 6. Give duplicated policy one owner

Problem: feature and target-status policy is repeated in README and release
documentation, while this backlog previously duplicated the release checklist.

- [ ] Make `docs/features.md` the owner of feature semantics and supported
  combinations; replace README's repeated feature list with a short summary and
  link.
- [ ] Make `docs/platforms.md` the owner of maintained-target and artifact-status
  claims; replace repeated status matrices in README and `docs/release.md` with
  contextual summaries and links.
- [ ] Keep `docs/release.md` as the sole release qualification checklist. Other
  documents may name focused checks but must not copy the full sequence.
- [ ] Preserve experiment history only in `PERFORMANCE_WORKLOG.md` and current
  benchmark results in their existing result owner.

Verify: documentation formatting/checks, doctests, generated-artifact checks,
and focused `rg` comparisons showing the repeated lists were removed.

Done when each named policy has one update location and tracked duplicated text
decreases. Do not add a policy index or enforcement ledger.
