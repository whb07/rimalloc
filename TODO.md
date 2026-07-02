# TODO: Next Audit Granularity, Strict-Type, and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`0bcbd7a`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical translation debt is concentrated in one remaining
oversized unsafe audit operation ID, several still-broad medium clusters, the
largest release-audit modules, and residual strict-type wildcard allowlist
entries.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe
  mentions, and 163 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 163 operation IDs with
  383 automated evidence rows.
- The only unsafe audit operation ID still above 100 markers is
  `allocation-abi-entry-boundary` with 104 markers, all in
  `src/allocator/alloc.rs`.
- The largest remaining marker clusters after that are
  `typed-pointer-test-fixtures` with 87 markers,
  `macos-zone-override` with 84 markers,
  `heap-lifecycle-boundary` with 80 markers,
  `options-callback-boundary` and `internal-translation-boundary` with 77
  markers each, `stats-merge-boundary` with 76 markers,
  `thread-heap-lifecycle`, `raw-posix-compat`, and
  `internal-bitmap-boundary` with 72 markers each, `free-path-boundary` with 70
  markers, and `raw-macro-libc-forwarding-boundary` and
  `aligned-heap-allocation-boundary` with 68 markers each.
- `strict-type-allowlist.txt` has 515 active entries: 323
  raw-pointer-parameter entries, 131 bare-domain `usize` entries, 35
  metadata-reference entries, 22 pointer/usize round-trip entries, 2
  atomic-cast entries, and 2 heap-allocating-type entries.
- `strict-type-allowlist.txt` still has 26 wildcard entries: 13
  bare-domain `usize` entries, 4 raw-pointer-parameter entries, 4
  pointer/usize round-trip entries, 2 metadata-reference entries, 2 atomic-cast
  entries, and 1 heap-allocating-type entry.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  module cap is now 850 lines; the largest modules are
  `unsafe_performance/raw_facade/raw_override.rs` at 754 lines,
  `unsafe_performance/page_arena_behavior.rs` at 724 lines,
  `release_policy/docs_and_linking.rs` at 633 lines,
  `release_policy/api_surface.rs` at 517 lines,
  `datatype_acceptance/strict_type.rs` at 512 lines,
  `release_policy/release_process.rs` at 504 lines,
  `release_policy/security.rs` at 451 lines,
  `unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 404 lines,
  `unsafe_performance/os_memory.rs` at 403 lines, and
  `unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs`
  at 400 lines.
- The previous lifecycle, platform, raw facade, page/arena, atomic, and root
  release-audit aggregate splits are complete and guarded by focused
  release-audit tests. The full workspace validation for the last item passed:
  `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`, focused release-audit
  tests, unsafe audit scripts, strict-type checks, no-allocation audit, unsafe
  inventory refresh/check, and `git diff --check`.

## Issue 1: Split allocation ABI entry audit cluster

> **Comment:** `allocation-abi-entry-boundary` has 104 markers in
> `src/allocator/alloc.rs`. It currently mixes raw public allocation exports,
> thread-heap entry shims, heap/default dispatch, product allocation, realloc
> family behavior, string/realpath duplication, C++ new retry paths, guarded
> allocation, and page allocation handoff markers.

- [ ] Split `allocation-abi-entry-boundary` into narrow operation IDs for raw
      public allocation exports, thread-heap entry shims, heap/default dispatch,
      product/calloc/mallocn allocation, realloc/reallocn/reallocf/recalloc,
      string and realpath duplication, C++ new retry paths, guarded allocation,
      and page-allocation handoff behavior.
- [ ] Update `UNSAFE_AUDIT.md`, evidence rows, and release-audit guards for each
      new allocation operation ID.
- [ ] Keep the hot allocation path C-shaped: no added heap allocation, dynamic
      dispatch, broad trait abstraction, or extra raw-pointer conversions beyond
      the existing compatibility boundary.

Verify: allocation boundary tests, release fast-path allocation modules, unsafe
audit scripts, unsafe evidence scripts, full formatting, full clippy, and full
tests.

## Issue 2: Split broad runtime and internal marker clusters

> **Comment:** The next source-level marker clusters are below 100 but still
> combine unrelated responsibilities: `options-callback-boundary` and
> `internal-translation-boundary` have 77 markers each,
> `stats-merge-boundary` has 76, `thread-heap-lifecycle` has 72, and
> `heap-lifecycle-boundary` has 80.

- [ ] Split `options-callback-boundary` into option storage/locking,
      C-string/message output, warning/error callbacks, assertion failure, and
      allocation failure callback operation IDs.
- [ ] Split `internal-translation-boundary` into page metadata helpers,
      encoded block-link helpers, CPU memcpy/memset helpers, and internal
      allocation/thread-heap shims.
- [ ] Split `stats-merge-boundary`, `heap-lifecycle-boundary`, and
      `thread-heap-lifecycle` where release-audit checks already distinguish
      public merge/export shims, process/main heap lifecycle, non-main heap
      lifecycle, thread-heap refcounting, and thread-heap create/delete paths.

Verify: primitive options tests, lifecycle stats exports, release fast-path
runtime modules, unsafe audit scripts, unsafe evidence scripts, full clippy, and
full tests.

## Issue 3: Split raw/POSIX, bitmap, and release-audit no-export modules

> **Comment:** `raw-posix-compat` has 72 markers,
> `internal-bitmap-boundary` has 72, and `raw-macro-libc-forwarding-boundary`
> has 68. The release-audit no-export table in
> `unsafe_performance/raw_facade/raw_override.rs` is still the largest audit
> module at 754 lines, blocking a lower module cap.

- [ ] Split `raw-posix-compat` by errno/platform shims, malloc/free/usable-size
      compatibility, aligned/POSIX allocation, reallocarray/reallocarr/expand,
      string duplication, environment duplication, and Windows-compatible aligned
      recalloc shims.
- [ ] Split `internal-bitmap-boundary` into bitmap traversal, claim/find,
      range/chunk iteration, bitfield, and arena callback operation IDs.
- [ ] Split `raw_facade/raw_override.rs` into allocator/page, OS/random,
      init/threadlocal/heap/thread-heap, options/stats, and libc/POSIX no-export
      audit modules. After the largest modules are below 700 lines, lower the
      release-audit module cap from 850 to 700.

Verify: raw/POSIX compatibility tests, bitmap tests, release fast-path audit,
unsafe audit scripts, unsafe evidence scripts, full clippy, and full tests.

## Issue 4: Reduce strict-type wildcard debt

> **Comment:** The strict-type checker still has 26 wildcard entries. These are
> intentional compatibility seams, but wildcard allowlist entries hide whether a
> path has accumulated new untyped code since the last review.

- [ ] Replace wildcard entries with symbol-specific entries wherever the source
      already has named compatibility functions or typed wrappers.
- [ ] For retained wildcard entries, add release-audit evidence that proves the
      file is a raw ABI/platform/proof-fixture boundary and not a growing typed
      domain.
- [ ] Add a strict-type guard that tracks the wildcard count by category and
      fails if the count grows without updating `TODO.md`.

Verify: strict-type checks, datatype acceptance release-audit modules, full
formatting, full clippy, and full tests.

## Suggested implementation order

1. [ ] Split allocation ABI entry audit cluster.
2. [ ] Split broad runtime and internal marker clusters.
3. [ ] Split raw/POSIX, bitmap, and release-audit no-export modules.
4. [ ] Reduce strict-type wildcard debt.

## Definition of done for this backlog

- [ ] No unsafe audit operation ID has more than 100 source markers.
- [ ] The broad 70+ marker clusters listed above are split into narrower review
      units with separate docs, evidence rows, and release-audit guards.
- [ ] Release-audit modules are split enough to lower the module cap to 700
      lines without weakening coverage.
- [ ] Strict-type wildcard entries are reduced or guarded by category-specific
      evidence, and the wildcard count cannot grow silently.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
