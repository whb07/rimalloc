# TODO: Next Audit Cluster and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`e44eabb`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical translation debt is concentrated in source audit
operation IDs that are still broad enough to hide unrelated unsafe invariants,
plus two release-audit modules that block a lower module-size cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe
  mentions, and 270 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 270 operation IDs with
  589 automated evidence rows.
- The largest source audit operation IDs are `os-memory-allocation-boundary`
  with 59 markers, `arena-heap-page-visitors` and
  `init-thread-heap-tls-boundary` with 56 markers each,
  `typed-atomic-domain` with 51 markers, `threadlocal-key-manager` and
  `ffi-libc-helpers` with 50 markers each, and
  `page-queue-transition-boundary` with 49 markers.
- The completed typed fixture split left the largest new fixture IDs at
  `typed-heap-thread-tld-pointer-test-fixtures` with 35 markers,
  `typed-arena-subproc-pointer-test-fixtures` with 17 markers, and
  `typed-thread-heap-state-test-fixtures` with 15 markers.
- The completed platform split left the largest new platform IDs at
  `platform-thread-heap-association-boundary` with 32 markers,
  `macos-zone-allocation-boundary` with 22 markers,
  `emscripten-memory-boundary` with 20 markers, and
  `macos-zone-lookup-boundary` with 16 markers.
- The completed free/aligned split left the largest new allocation IDs at
  `free-validation-boundary` with 28 markers,
  `aligned-realloc-heap-export-boundary` with 26 markers,
  `aligned-global-allocation-export-boundary` with 17 markers, and
  `aligned-realloc-request-boundary` and
  `aligned-heap-allocation-export-boundary` with 15 markers each.
- The completed page-map/atomic split left the largest split IDs at
  `internal-atomic-fetch-boundary` with 22 markers,
  `page-map-lookup-boundary` with 19 markers,
  `internal-atomic-cas-boundary` with 17 markers,
  `page-map-page-unregister-boundary` with 15 markers, and
  `page-map-page-register-boundary` with 14 markers.
- The largest remaining medium clusters are `init-thread-lifecycle-boundary`
  with 48 markers, `raw-macro-cxx-boundary` with 46 markers,
  `page-map-init-boundary` with 45 markers,
  `options-message-output-boundary` with 39 markers, and
  `arena-purge-boundary` with 38 markers.
- `strict-type-allowlist.txt` has 634 active entries: 335
  raw-pointer-parameter entries, 220 bare-domain `usize` entries, 40
  metadata-reference entries, 23 pointer/usize round-trip entries, 14
  atomic-cast entries, and 2 heap-allocating-type entries.
- `strict-type-allowlist.txt` has 1 retained wildcard entry, the
  `heap-allocating-type` Loom proof-model boundary in
  `src/types/atomic/concurrency_models.rs`. Strict-type wildcard guard: total=1; heap-allocating-type=1.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  module cap is now 600 lines; the largest modules are
  `datatype_acceptance/strict_type.rs` at 592 lines,
  `release_policy/security.rs` at 584 lines,
  `release_policy/api_surface.rs` at 523 lines,
  `release_policy/release_process.rs` at 504 lines,
  `unsafe_performance/lifecycle_boundaries/process_lifecycle.rs` at 426 lines,
  `unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten.rs`
  at 421 lines,
  `interposition_exports.rs` at 413 lines,
  `unsafe_performance/os_memory.rs` at 403 lines,
  `allowance_inventory.rs` at 398 lines, and
  `unsafe_performance/allocation_free.rs` at 397 lines.
- The completed release-audit split moved
  `unsafe_performance/page_arena_behavior.rs` behind focused child modules; its
  largest child is
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs` at 367
  lines. `release_policy/docs_and_linking.rs` is now a 10-line module with
  docs child modules up to
  `release_policy/docs_and_linking/c_migration_guidance.rs` at 172 lines.
- Full validation for the last completed item passed:
  `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`,
  `cargo test --test release_fast_path_audit --all-features`, unsafe audit
  scripts, strict-type checks, no-allocation audit, unsafe inventory
  refresh/check, and `git diff --check`.

## Issue 1: Split typed pointer and heap proof-fixture audit clusters

> **Comment:** `typed-pointer-test-fixtures` had 87 markers in
> `src/types/ptr/tests.rs`, and `typed-heap-test-fixtures` had 55 markers. These
> were proof fixtures, but one operation ID per whole fixture file made pointer
> family regressions harder to review.

- [x] Split `typed-pointer-test-fixtures` by pointer family and proof purpose:
      page fields, block links, heap/thread-heap/TLD fields, arena/subproc
      fields, page-queue fields, and exposed-address/provenance arithmetic.
- [x] Split `typed-heap-test-fixtures` by ownership witness type: main heap,
      owned heap, borrowed heap, thread-heap state, and thread-local promotion.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so each new proof-fixture operation ID is named and
      counted independently.

Verify: pointer and heap type tests, datatype acceptance release-audit modules,
unsafe audit scripts, unsafe evidence scripts, full formatting, full clippy, and
full tests.

## Issue 2: Split macOS zone and platform primitive marker clusters

> **Comment:** `macos-zone-override` had 83 markers, while adjacent platform
> primitive clusters remained broad: `platform-tls-primitives` had 54 markers,
> `emscripten-primitive-boundary` had 48, and `wasi-primitive-boundary` had 42.

- [x] Split `macos-zone-override` into operation IDs for zone declarations,
      zone lookup/name helpers, allocation/free/realloc/memalign trampolines,
      batch allocation/free, fork hooks, interpose section publishing, and
      process initialization registration.
- [x] Split platform primitive markers where existing release-audit modules
      already distinguish TLS/thread-heap association, Emscripten primitives,
      WASI memory growth, and Unix/Windows primitive allocation behavior.
- [x] Preserve the raw platform ABI shape: no new heap allocation, no dynamic
      dispatch, no panicking path, and no broad wrapper that changes the
      interposition or platform callback contract.

Verify: platform primitive release-audit modules, raw/interposition export
tests, unsafe audit scripts, unsafe evidence scripts, full formatting, full
clippy, and full tests.

## Issue 3: Split free-path and aligned allocation marker clusters

> **Comment:** `free-path-boundary` had 69 markers,
> `aligned-heap-allocation-boundary` had 68, and `aligned-realloc-boundary` had
> 67. Each combines raw ABI shims, validation, padding, stats, local/remote
> routing, and allocation/reallocation variants.

- [x] Split `free-path-boundary` by validation/double-free detection, padding
      decode/shrink/check, usable-size queries, free stats, local free, remote
      free, abandoned free, and public raw free shims.
- [x] Split `aligned-heap-allocation-boundary` by aligned layout parsing,
      over-allocation/padding setup, heap/default dispatch, malloc/zalloc/calloc
      families, and public heap/global raw export shims.
- [x] Split `aligned-realloc-boundary` by old-allocation validation, copy bounds,
      realloc/rezalloc/recalloc product handling, heap/default dispatch, and
      public raw export shims.

Verify: allocation boundary tests, aligned export tests, no-allocation critical
path tests, release fast-path allocation/free modules, unsafe audit scripts,
unsafe evidence scripts, full formatting, full clippy, and full tests.

## Issue 4: Split page-map registration and internal atomic marker clusters

> **Comment:** `page-map-register-boundary` had 66 markers and
> `internal-atomic-field-boundary` had 65. Both now have typed wrappers, but the
> audit IDs still group several distinct operations.

- [x] Split `page-map-register-boundary` into operation IDs for unsafe destroy,
      page registration/unregistration, range registration/unregistration,
      lookup, commit, submap allocation/publication, and range set/clear
      primitives.
- [x] Split `internal-atomic-field-boundary` into operation IDs for raw
      pointer-to-atomic views, usize/isize/i64 load/store, fetch arithmetic,
      compare-exchange families, exchange, max loops, and volatile delta reads.
- [x] Update release-audit evidence so page-map typed range witnesses and
      atomic memory-order mapping guards cover each split operation ID.

Verify: page-map tests, atomic ordering audit, primitive options release-audit
module, local boundary release-audit module, unsafe audit scripts, unsafe
evidence scripts, full formatting, full clippy, and full tests.

## Issue 5: Split largest release-audit modules and lower the module cap

> **Comment:** The source audit clusters are now small enough that the largest
> release-audit modules are the next reviewability bottleneck.
  > `unsafe_performance/page_arena_behavior.rs` was 656 lines and
  > `release_policy/docs_and_linking.rs` was 633 lines.

- [x] Split `unsafe_performance/page_arena_behavior.rs` into focused modules for
      memid/arena dispatch, page-map range witnesses, arena allocation/page
      allocation, purge/clock/atomic behavior, and page-map atomic helpers.
- [x] Split `release_policy/docs_and_linking.rs` into focused modules for public
      docs/build-mode policy, feature/no-std docs, FFI linking artifacts, symbol
      versioning, and C migration guidance.
- [x] Once every release-audit module is below 600 lines, lower the release
      audit module cap from 700 to 600 without weakening coverage.

Verify: release fast-path audit, docs/linking release policy modules, unsafe
audit scripts, unsafe evidence scripts, full formatting, full clippy, and full
tests.

## Suggested implementation order

1. [x] Split typed pointer and heap proof-fixture audit clusters.
2. [x] Split macOS zone and platform primitive marker clusters.
3. [x] Split free-path and aligned allocation marker clusters.
4. [x] Split page-map registration and internal atomic marker clusters.
5. [x] Split largest release-audit modules and lower the module cap.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 60 source markers.
- [x] The largest remaining page-map and atomic clusters listed above are split
      into narrower review units with separate docs, evidence rows, and
      release-audit guards.
- [x] Release-audit modules are split enough to lower the module cap to 600
      lines without weakening coverage.
- [x] The strict-type wildcard guard remains at exactly one retained
      proof-model wildcard unless `TODO.md` is updated with new measured
      evidence.
- [x] `TODO.md` and its release-audit guard reflect the current measured counts.
- [x] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
