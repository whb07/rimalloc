# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`e8083ce`. The last pass retired the scalar, pointer, and page typed-wrapper
unsafe migration exceptions; split the page lifecycle/page-queue release-audit
cluster; and narrowed the page strict-type allowlist from path-wide raw-pointer
entries to symbol and child-module entries.

The next pass should keep the same discipline: retire one measurable exception
at a time, keep allocator hot paths allocation-free and C ABI compatible, and
replace translated-style file allowances with local Rust proof markers, focused
strict-type entries, or smaller release-audit modules.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` now has 36 entries: 33 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 36.
- `src/types/arena.rs` now uses 7
  `SAFETY[AUDIT:typed-arena-domain]` markers for slice-range count
  construction and arena role witnesses, and is off the unsafe migration
  allowlist.
- `src/types/page_queues.rs` now uses 10
  `SAFETY[AUDIT:typed-page-queue-domain]` markers for queue classification,
  queued-page membership, full/unfull transitions, queue removal, and
  arena-free page destruction, and is off the unsafe migration allowlist.
- `src/types/heap.rs` now uses 15
  `SAFETY[AUDIT:typed-heap-domain]` markers for heap classification,
  thread-heap state/locality checks, heap/tld projections, and local witness
  construction, and is off the unsafe migration allowlist.
- The smallest remaining allowlisted production files are:
  `src/allocator/arena_meta.rs` at 316 lines with 22 `unsafe` hits and 0 local
  markers,
  `src/raw/posix.rs` at 329 lines with 81 `unsafe` hits and 0 local markers,
  `src/allocator/bench_diagnostics.rs` at 350 lines with 8 `unsafe` hits and
  0 local markers, and
  `src/platform/wasi.rs` at 373 lines with 34 `unsafe` hits and 0 local
  markers, and
  `src/lib.rs` at 398 lines with 21 `unsafe` hits and 8 local markers.
- `src/types/freelist_wrappers.rs` now uses 23
  `SAFETY[AUDIT:typed-freelist-wrappers]` markers for encoded block links,
  block tags, available/local free-list conversions, and allocated/free block
  witnesses, and is off the unsafe migration allowlist.
- The broad freelist strict-type rows are gone:
  `raw-pointer-parameter|src/types/freelist_wrappers.rs|*|mi_block_t` was
  replaced by 7 symbol-level rows, and
  `pointer-usize-roundtrip|src/types/freelist_wrappers.rs|*|encoded-pointer`
  was removed as stale coverage because the module uses `ptr_exposed_addr`.
- The largest unsafe audit marker clusters remain broad enough to revisit:
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 89 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 58 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 56 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 35 markers,
  `SAFETY[AUDIT:typed-scalar-test-fixtures]` has 33 markers,
  `SAFETY[AUDIT:owned-allocation]` has 32 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 28 markers,
  `SAFETY[AUDIT:arena-id-memid-helpers]` has 25 markers,
  `SAFETY[AUDIT:page-count-accessors]` has 24 markers, and
  `SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 23 markers.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 3,915 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The release audit datatype acceptance cluster is now split out:
  `tests/release_fast_path_audit.rs` is 1,832 lines,
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,103 lines and
  borrows the pre-collected Rust source list,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,374 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines, and
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 662 lines.
- `strict-type-allowlist.txt` still has 29 path-wide raw-pointer entries with
  `symbol = *`, even after the page cleanup. `REMOVE_ENTRY_CAP` remains 0.

## Issue: Arena typed domain wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/arena.rs` is the smallest remaining production
> allowlist entry. Its unsafe sites are typed slice-range and arena-role
> witnesses, not allocator hot-path logic, so they should be documented at the
> constructor boundary instead of hidden by a file-level migration exception.

- [x] Add local `SAFETY[AUDIT:typed-arena-domain]` markers for
      `SliceRange` unchecked count construction and `SharedArena`,
      `ExclusiveArena`, and `ParentArena` role constructors.
- [x] Preserve the current small `Copy`/transparent wrapper shape and avoid new
      allocation, dynamic dispatch, or global arena-table validation.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove `src/types/arena.rs` from
      `docs/unsafe-migration-allowlist.tsv`, regenerate unsafe inventory, and
      lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::arena::tests --lib --all-features`,
unsafe audit scripts, unsafe inventory regeneration/check, release audit, full
clippy, and full tests.

## Issue: Page queue wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/page_queues.rs` is a compact state-witness module for
> normal, huge, full, queued, and unqueued page states. The unsafe operations
> already sit at queue classification and transition boundaries; they need
> local audit markers without changing the hot queue operations.

- [x] Add local `SAFETY[AUDIT:typed-page-queue-domain]` markers for normal,
      huge, and full queue constructors, queued-page constructors, queue
      removal, full/unfull transitions, and arena-free destroyed-page
      construction.
- [x] Keep `PageQueue<K>` and queue witnesses allocation-free, transparent, and
      `PhantomData` based.
- [x] Keep queue-membership checks delegated to the existing C-shaped page
      helpers instead of adding duplicate traversal logic.
- [x] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::page_queues::tests --lib --all-features`,
unsafe audit scripts, unsafe inventory regeneration/check, release audit, full
clippy, and full tests.

## Issue: Heap and thread-heap wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/heap.rs` owns heap, thread-heap, and subproc witness
> construction. The code is already type-oriented, but the raw heap/theap
> classification and thread-local ownership reads still need operation-level
> proof markers.

- [x] Add local `SAFETY[AUDIT:typed-heap-domain]` markers for main/owned/borrowed
      heap constructors, thread-heap borrowed/local classification, subproc
      witnesses, and thread-heap `heap`/`tld` projections.
- [x] Keep raw `mi_heap_t` and `mi_theap_t` constructor boundaries explicit and
      avoid adding heap allocation or shared mutable state.
- [x] Preserve the existing tests that classify main, initialized,
      uninitialized, deleted, borrowed, and local thread-heap states.
- [x] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::heap::tests --lib --all-features`, unsafe audit
scripts, unsafe inventory regeneration/check, release audit, full clippy, and
full tests.

## Issue: Free-list wrappers still mix unsafe migration debt with broad strict-type entries

> **Comment:** `src/types/freelist_wrappers.rs` owns encoded block links,
> block tags, allocated/free/available block witnesses, and local/remote
> free-list conversion. It is the right module for these operations, but both
> the unsafe migration allowlist and strict-type allowlist are still broad.

- [x] Add local `SAFETY[AUDIT:typed-freelist-wrappers]` markers for encoded
      pointer reads/writes, block tag reads/writes, allocated/free block
      witness construction, and local/remote free-list conversions.
- [x] Keep free-list wrappers allocation-free and preserve raw block-link
      layout compatibility.
- [x] Replace broad strict-type entries for
      `src/types/freelist_wrappers.rs` with symbol-level rows where the checker
      can express the active `mi_block_t` and encoded-pointer boundaries.
- [x] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::freelist_wrappers::tests --lib --all-features`,
`scripts/check-strict-types.py`, unsafe audit scripts, unsafe inventory
regeneration/check, release audit, full clippy, and full tests.

## Issue: Release audit root still has a datatype acceptance assertion wall

> **Comment:** `tests/release_fast_path_audit.rs` still carries the large
> `datatype_acceptance_criteria_have_cross_cutting_guards` assertion cluster.
> It covers important cross-cutting guards, but the root harness is harder to
> review than the already-split `unsafe_performance` modules.

- [x] Move the datatype acceptance assertion cluster into a new module under
      `tests/release_fast_path_audit/`.
- [x] Pass borrowed source strings into the new assertion helper rather than
      re-reading large files.
- [x] Keep coverage for memid views, block-link wrappers, atomic field wrappers,
      strict-type checker rules, pointer provenance helpers, and package-level
      lint/allowance guards.
- [x] Update `RELEASE_AUDIT_MODULES` and this TODO evidence with the new module
      and line counts.
- [x] Preserve release-audit behavior and avoid weakening source assertions
      while moving them.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Suggested implementation order

1. [x] Migrate `src/types/arena.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/types/page_queues.rs` off the unsafe migration allowlist.
3. [x] Migrate `src/types/heap.rs` off the unsafe migration allowlist.
4. [x] Migrate `src/types/freelist_wrappers.rs` off the unsafe migration
       allowlist and narrow its broad strict-type entries where possible.
5. [x] Split the datatype acceptance release-audit assertion cluster.

## Definition of done for this backlog

- [x] The unsafe migration allowlist cap is lowered after each migrated file.
- [x] `src/types/arena.rs`, `src/types/page_queues.rs`, `src/types/heap.rs`,
      and `src/types/freelist_wrappers.rs` are covered by operation-level audit
      markers and evidence instead of file-level migration exceptions.
- [x] Broad strict-type rows for `src/types/freelist_wrappers.rs` are narrowed
      where the current checker can express the active boundaries.
- [x] One more release-audit assertion cluster has a dedicated module.
