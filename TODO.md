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

# Priority 0 — Correctness and ineffective tests

## 1. Resolve or close the thread-lifecycle TSan finding

**Done — X40, 2026-10-22.** The sustained release probe reproduced the
`_mi_theap_create` / `_mi_thread_done` race in three Rust and three C 3.3.2 runs.
Explicit teardown retained a freed heap pointer in the pthread destructor key;
this is an inherited lifetime defect. Teardown now clears that association
before metadata release, without adding a lock or strengthening atomics.

`tests/thread_lifecycle.rs` and the direct Unix pthread-key unit test retain
regression coverage. Both failed before the repair. The sustained and minimized
Rust release TSan probes each passed ten runs afterward. Release thread stress,
all six Loom models, behavioral parity, ASan, Valgrind, and the common Rust gate
pass; native allocation/free disassembly is unchanged. X40 in
`PERFORMANCE_WORKLOG.md` records exact commands, toolchains, stacks, the C
comparison, failed setup attempts, and raw evidence locations. No suppression
was needed.

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
