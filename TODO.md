# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`a6a560c`. The last pass migrated atomic test fixtures and random context
boundaries off file-level unsafe migration debt, split arena heap
visit/delete/move/destroy helpers into `src/allocator/arena/heap_pages.rs`,
split the heap-page visitor release-audit assertions, and narrowed arena
raw-pointer strict-type entries to symbol-level entries where the checker can
support that today.

The next pass should keep the same discipline: retire one measurable exception
at a time, keep allocator hot paths allocation-free and C ABI compatible, and
replace broad translated-style allowances with local Rust proof markers,
smaller audit modules, or symbol-level compatibility entries.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` now has 40 entries: 37 production,
  0 test, and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 40.
- `src/types/page.rs` now uses `SAFETY[AUDIT:typed-page-domain]` markers for
  thread-id construction, page-owner classification, and registered page-range
  witness construction, and is off the unsafe migration allowlist.
- `src/types/scalar/tests.rs` now uses
  `SAFETY[AUDIT:typed-scalar-test-fixtures]` markers for 31 unchecked scalar
  constructor fixtures and is off the unsafe migration allowlist.
- `src/types/ptr/tests.rs` now uses
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` markers for 87 grouped
  stack-local metadata and pointer arithmetic fixture blocks and is off the
  unsafe migration allowlist.
- No test files remain in `docs/unsafe-migration-allowlist.tsv`.
- The smallest remaining allowlisted production files are
  `src/types/arena.rs` at 260 lines,
  `src/types/page_queues.rs` at 304 lines,
  `src/types/heap.rs` at 312 lines,
  `src/allocator/arena_meta.rs` at 316 lines, and
  `src/raw/posix.rs` at 329 lines.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 3,915 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The largest unsafe audit marker clusters are still broad enough to revisit:
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 59 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 56 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 35 markers,
  `SAFETY[AUDIT:owned-allocation]` has 32 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 28 markers,
  `SAFETY[AUDIT:arena-id-memid-helpers]` has 25 markers,
  `SAFETY[AUDIT:page-count-accessors]` has 24 markers, and
  `SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 23 markers.
- The release audit still has large review units:
  `tests/release_fast_path_audit.rs` is 2,893 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,374 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines, and
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 662 lines. The new
  `tests/release_fast_path_audit/unsafe_performance/page_lifecycle.rs` module
  is 243 lines.
- `tests/release_fast_path_audit/unsafe_performance/page_lifecycle.rs` now owns
  the page lifecycle/page-queue assertion block around `_mi_page_free`,
  `_mi_page_abandon`, `_mi_page_free_collect`, `_mi_page_unfull`,
  `_mi_page_retire`, `_mi_page_queue_is_valid`, queue transition helpers, and
  the typed page count/validation child modules. It receives borrowed source
  strings from `unsafe_performance.rs`.
- `strict-type-allowlist.txt` has 0 remaining `remove:` entries and
  `REMOVE_ENTRY_CAP` is 0. No
  `raw-pointer-parameter|src/allocator/page.rs|*|...` entries remain:
  `src/allocator/page.rs` now has 47 symbol-level raw-pointer entries, and
  `src/allocator/page/counts.rs`, `src/allocator/page/flags.rs`, and
  `src/allocator/page/validation.rs` have 23 child-module raw-pointer entries.
  `scripts/check-strict-types.py` no longer aliases
  `src/allocator/page/bins.rs`, `src/allocator/page/counts.rs`,
  `src/allocator/page/flags.rs`, or `src/allocator/page/validation.rs` back to
  `src/allocator/page.rs`. `src/allocator/page/validation.rs` owns the
  `mi_page_block_at` bare-domain entry.

## Issue: Scalar test fixtures are still file-level unsafe debt

> **Comment:** `src/types/scalar/tests.rs` is the smaller of the two remaining
> allowlisted test files. The unsafe sites are unchecked scalar constructor
> fixtures with concrete values, so they should be converted to local
> operation-level proof markers instead of a file-level exception.

- [x] Add local `SAFETY[AUDIT:typed-scalar-test-fixtures]` markers to the
      unchecked scalar constructor fixtures.
- [x] Keep the tests table-driven or direct; do not introduce allocations,
      helper abstractions, or broader fixture state just to reduce comments.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove `src/types/scalar/tests.rs` from
      `docs/unsafe-migration-allowlist.tsv`.
- [x] Regenerate unsafe inventory and lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::scalar::tests --lib --all-features`,
`scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py --write && scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Pointer wrapper tests are the last allowlisted test fixture

> **Comment:** `src/types/ptr/tests.rs` is the other remaining test allowlist
> entry. It exercises stack-local allocator metadata and exposed-address pointer
> arithmetic for wrapper APIs. The fixture boundaries should be local and
> auditable instead of hidden by the file-level migration list.

- [x] Add local `SAFETY[AUDIT:typed-pointer-test-fixtures]` markers around the
      stack-local metadata, raw field writes, wrapper constructor calls, and
      exposed-address pointer arithmetic fixtures.
- [x] Keep pointer arithmetic expectations explicit and avoid new heap
      allocation, dynamic dispatch, or shared mutable fixture state.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove `src/types/ptr/tests.rs` from
      `docs/unsafe-migration-allowlist.tsv`.
- [x] Regenerate unsafe inventory and lower `ALLOWLIST_ENTRY_CAP` again.

Verify: `cargo test types::ptr::tests --lib --all-features`,
unsafe audit scripts, unsafe inventory regeneration/check, release audit, full
clippy, and full tests.

## Issue: Page wrapper production code still uses file-level unsafe allowance

> **Comment:** `src/types/page.rs` is the smallest remaining production
> allowlisted file. It already has a small typed surface around page flags,
> thread-id classification, and registered page ranges; the remaining unsafe
> operations should be documented at the constructor/classification boundary.

- [x] Add local `SAFETY[AUDIT:typed-page-domain]` markers for
      `ThreadId::new_unchecked`, `PageThreadId::owner`, and
      `RegisteredPageRange::new_unchecked`.
- [x] Preserve the current `#[repr(transparent)]`, `Copy`, and `#[inline]`
      shape of hot value wrappers.
- [x] Keep registered-range construction as a witness boundary; do not make it
      validate global page-map state.
- [x] Add matching unsafe audit and evidence entries.
- [x] Remove `src/types/page.rs` from
      `docs/unsafe-migration-allowlist.tsv`, regenerate unsafe inventory, and
      lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::page::tests --lib --all-features`,
unsafe audit scripts, unsafe inventory regeneration/check, release audit, full
clippy, and full tests.

## Issue: Release audit unsafe_performance.rs still has a page lifecycle assertion wall

> **Comment:** The last release-audit split proved the pattern works. The next
> high-signal split is the page lifecycle/page-queue assertion cluster, which is
> cohesive and already uses borrowed source strings in `unsafe_performance.rs`.

- [x] Move the page lifecycle and page-queue assertion cluster into a new module
      under `tests/release_fast_path_audit/unsafe_performance/`.
- [x] Pass borrowed source strings into the new assertion helper rather than
      re-reading large files.
- [x] Keep coverage for `_mi_page_free`, `_mi_page_abandon`,
      `_mi_page_free_collect`, `_mi_page_unfull`, `_mi_page_retire`,
      `_mi_page_queue_is_valid`, page queue transition helpers, and the typed
      page count/validation child modules.
- [x] Update `RELEASE_AUDIT_MODULES` and this TODO evidence with the new module
      and line counts.
- [x] Preserve release-audit behavior and avoid weakening source assertions
      while moving them.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Page strict-type entries remain broad after page child splits

> **Comment:** Page child modules already own bins, counts, flags, and
> validation logic, but the strict-type checker still aliases several of those
> files back to `src/allocator/page.rs`. The page raw-pointer compatibility
> entries should now be narrowed to child-module or symbol-level entries where
> possible, matching the arena strict-type cleanup.

- [x] Re-run `scripts/check-strict-types.py` and collect live hits for
      `src/allocator/page.rs` and the existing page child modules.
- [x] Remove page child path aliases that are no longer needed for current hits.
- [x] Replace broad page raw-pointer entries with child-module or symbol-level
      entries for `mi_page_t`, `mi_block_t`, and `mi_theap_t` boundaries.
- [x] Keep permanent compatibility entries only where C-shaped hot page
      lifecycle, queue, and free-list signatures genuinely remain.
- [x] Keep `REMOVE_ENTRY_CAP` at 0 and update TODO/release-audit evidence.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Suggested implementation order

1. [x] Migrate `src/types/scalar/tests.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/types/ptr/tests.rs` off the unsafe migration allowlist.
3. [x] Migrate `src/types/page.rs` off the unsafe migration allowlist.
4. [x] Split the page lifecycle/page-queue release-audit assertion cluster.
5. [x] Narrow broad page strict-type entries after the existing child splits.

## Definition of done for this backlog

- [x] The unsafe migration allowlist cap is lowered after each migrated file.
- [x] No test files remain in `docs/unsafe-migration-allowlist.tsv`.
- [x] `src/types/page.rs` is covered by operation-level audit markers and
      evidence instead of a file-level migration exception.
- [x] One more release-audit assertion cluster has a dedicated module.
- [x] Page strict-type documentation is narrower where existing child modules
      provide concrete boundaries.
