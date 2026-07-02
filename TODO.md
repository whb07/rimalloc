# TODO: Next Audit Granularity, Strict-Type, and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`0bcbd7a`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical translation debt is concentrated in several
still-broad medium clusters, the largest release-audit modules, and residual
strict-type wildcard allowlist entries.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe
  mentions, and 216 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 216 operation IDs with
  487 automated evidence rows.
- No unsafe audit operation ID is above 100 source markers after splitting
  `allocation-abi-entry-boundary` into `allocation-realloc-entry-boundary` with
  37 markers, `allocation-cxx-new-boundary` with 30 markers,
  `allocation-product-entry-boundary` with 21 markers,
  `allocation-string-path-boundary` with 20 markers, and
  `allocation-platform-shim-boundary` with 4 markers.
- The Issue 2 runtime/internal split removed the broad
  `heap-lifecycle-boundary`, `options-callback-boundary`,
  `internal-translation-boundary`, `stats-merge-boundary`, and
  `thread-heap-lifecycle` source markers. The largest replacement clusters are
  `options-message-output-boundary` with 39 markers,
  `internal-page-metadata-boundary` with 37 markers,
  `thread-heap-visit-boundary` with 28 markers, and
  `heap-ownership-query-boundary` with 23 markers.
- The Issue 3 raw/POSIX and bitmap split removed the broad
  `internal-bitmap-boundary`, `raw-posix-compat`, and
  `raw-macro-libc-forwarding-boundary` source markers. The largest replacement
  clusters are `raw-macro-unix-aligned-posix-boundary` and
  `raw-macro-linux-libc-boundary` with 20 markers each,
  `raw-posix-realloc-boundary` and `internal-bitmap-range-chunk-boundary` with
  19 markers each, and `raw-posix-environment-boundary` and
  `raw-posix-aligned-allocation-boundary` with 13 markers each.
- The largest remaining marker clusters are
  `typed-pointer-test-fixtures` with 87 markers,
  `macos-zone-override` with 83 markers,
  `free-path-boundary` with 69 markers,
  `aligned-heap-allocation-boundary` with 68 markers,
  `aligned-realloc-boundary` with 67 markers,
  `page-map-register-boundary` with 66 markers,
  `internal-atomic-field-boundary` with 65 markers,
  `os-memory-allocation-boundary` with 59 markers, and
  `init-thread-heap-tls-boundary` and `arena-heap-page-visitors` with 56
  markers each.
- `strict-type-allowlist.txt` has 515 active entries: 323
  raw-pointer-parameter entries, 131 bare-domain `usize` entries, 35
  metadata-reference entries, 22 pointer/usize round-trip entries, 2
  atomic-cast entries, and 2 heap-allocating-type entries.
- `strict-type-allowlist.txt` still has 26 wildcard entries: 13
  bare-domain `usize` entries, 4 raw-pointer-parameter entries, 4
  pointer/usize round-trip entries, 2 metadata-reference entries, 2 atomic-cast
  entries, and 1 heap-allocating-type entry.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  module cap is now 700 lines; the largest modules are
  `unsafe_performance/page_arena_behavior.rs` at 656 lines,
  `release_policy/docs_and_linking.rs` at 633 lines,
  `release_policy/security.rs` at 526 lines,
  `release_policy/api_surface.rs` at 523 lines,
  `datatype_acceptance/strict_type.rs` at 512 lines,
  `release_policy/release_process.rs` at 504 lines,
  `unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 426 lines,
  `interposition_exports.rs` at 413 lines,
  `unsafe_performance/os_memory.rs` at 403 lines, and
  `unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs`
  at 400 lines.
- The previous lifecycle, platform, raw facade, allocation ABI, page/arena,
  atomic, and root release-audit aggregate splits are complete and guarded by
  focused release-audit tests. The full workspace validation for the last item passed:
  `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`, focused release-audit
  tests, unsafe audit scripts, strict-type checks, no-allocation audit, unsafe
  inventory refresh/check, and `git diff --check`.

## Issue 1: Split allocation ABI entry audit cluster

> **Comment:** `allocation-abi-entry-boundary` had 104 markers in
> `src/allocator/alloc.rs`. It mixed raw public allocation exports, thread-heap
> entry shims, heap/default dispatch, product allocation, realloc family
> behavior, string/realpath duplication, C++ new retry paths, guarded
> allocation, and page allocation handoff markers.

- [x] Split `allocation-abi-entry-boundary` into narrow operation IDs for raw
      public allocation exports, thread-heap entry shims, heap/default dispatch,
      product/calloc/mallocn allocation, realloc/reallocn/reallocf/recalloc,
      string and realpath duplication, C++ new retry paths, guarded allocation,
      and page-allocation handoff behavior.
- [x] Update `UNSAFE_AUDIT.md`, evidence rows, and release-audit guards for each
      new allocation operation ID.
- [x] Keep the hot allocation path C-shaped: no added heap allocation, dynamic
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

- [x] Split `options-callback-boundary` into option storage/locking,
      C-string/message output, warning/error callbacks, assertion failure, and
      allocation failure callback operation IDs.
- [x] Split `internal-translation-boundary` into page metadata helpers,
      encoded block-link helpers, CPU memcpy/memset helpers, and internal
      allocation/thread-heap shims.
- [x] Split `stats-merge-boundary`, `heap-lifecycle-boundary`, and
      `thread-heap-lifecycle` where release-audit checks already distinguish
      public merge/export shims, process/main heap lifecycle, non-main heap
      lifecycle, thread-heap refcounting, and thread-heap create/delete paths.

Verify: primitive options tests, lifecycle stats exports, release fast-path
runtime modules, unsafe audit scripts, unsafe evidence scripts, full clippy, and
full tests.

## Issue 3: Split raw/POSIX, bitmap, and release-audit no-export modules

> **Comment:** `internal-bitmap-boundary` has 71 markers,
> `raw-posix-compat` has 70, and `raw-macro-libc-forwarding-boundary` has 65.
> The release-audit no-export table in
> `unsafe_performance/raw_facade/raw_override.rs` is still the largest audit
> module at 754 lines, blocking a lower module cap.

- [x] Split `raw-posix-compat` by errno/platform shims, malloc/free/usable-size
      compatibility, aligned/POSIX allocation, reallocarray/reallocarr/expand,
      string duplication, environment duplication, and Windows-compatible aligned
      recalloc shims.
- [x] Split `internal-bitmap-boundary` into bitmap traversal, claim/find,
      range/chunk iteration, bitfield, and arena callback operation IDs.
- [x] Split `raw_facade/raw_override.rs` into allocator/page, OS/random,
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

1. [x] Split allocation ABI entry audit cluster.
2. [x] Split broad runtime and internal marker clusters.
3. [x] Split raw/POSIX, bitmap, and release-audit no-export modules.
4. [ ] Reduce strict-type wildcard debt.

## Definition of done for this backlog

- [x] No unsafe audit operation ID has more than 100 source markers.
- [ ] The broad 70+ marker clusters listed above are split into narrower review
      units with separate docs, evidence rows, and release-audit guards.
- [x] Release-audit modules are split enough to lower the module cap to 700
      lines without weakening coverage.
- [ ] Strict-type wildcard entries are reduced or guarded by category-specific
      evidence, and the wildcard count cannot grow silently.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
