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

- `docs/unsafe-migration-allowlist.tsv` now has 40 entries: 37 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 40.
- The smallest remaining allowlisted production files are:
  `src/types/arena.rs` at 260 lines with 13 `unsafe` hits and 0 local
  `SAFETY[AUDIT:*]` markers,
  `src/types/page_queues.rs` at 304 lines with 32 `unsafe` hits and 0 local
  markers,
  `src/types/heap.rs` at 312 lines with 44 `unsafe` hits and 0 local markers,
  `src/allocator/arena_meta.rs` at 316 lines with 24 `unsafe` hits and 0 local
  markers, and
  `src/raw/posix.rs` at 329 lines with 81 `unsafe` hits and 0 local markers.
- `src/types/freelist_wrappers.rs` is 409 lines with 46 `unsafe` hits and
  0 local `SAFETY[AUDIT:*]` markers. It also still has broad strict-type rows:
  `raw-pointer-parameter|src/types/freelist_wrappers.rs|*|mi_block_t` and
  `pointer-usize-roundtrip|src/types/freelist_wrappers.rs|*|encoded-pointer`.
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
- The release audit still has large review units:
  `tests/release_fast_path_audit.rs` is 2,902 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,374 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines, and
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 662 lines. The root
  `datatype_acceptance_criteria_have_cross_cutting_guards` test spans roughly
  lines 1,327 through 2,420, making it the next clear release-audit split.
- `strict-type-allowlist.txt` still has 30 path-wide raw-pointer entries with
  `symbol = *`, even after the page cleanup. `REMOVE_ENTRY_CAP` remains 0.

## Issue: Arena typed domain wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/arena.rs` is the smallest remaining production
> allowlist entry. Its unsafe sites are typed slice-range and arena-role
> witnesses, not allocator hot-path logic, so they should be documented at the
> constructor boundary instead of hidden by a file-level migration exception.

- [ ] Add local `SAFETY[AUDIT:typed-arena-domain]` markers for
      `SliceRange` unchecked count construction and `SharedArena`,
      `ExclusiveArena`, and `ParentArena` role constructors.
- [ ] Preserve the current small `Copy`/transparent wrapper shape and avoid new
      allocation, dynamic dispatch, or global arena-table validation.
- [ ] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [ ] Remove `src/types/arena.rs` from
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

- [ ] Add local `SAFETY[AUDIT:typed-page-queue-domain]` markers for normal,
      huge, and full queue constructors, queued-page constructors, queue
      removal, full/unfull transitions, and arena-free destroyed-page
      construction.
- [ ] Keep `PageQueue<K>` and queue witnesses allocation-free, transparent, and
      `PhantomData` based.
- [ ] Keep queue-membership checks delegated to the existing C-shaped page
      helpers instead of adding duplicate traversal logic.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
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

- [ ] Add local `SAFETY[AUDIT:typed-heap-domain]` markers for main/owned/borrowed
      heap constructors, thread-heap borrowed/local classification, subproc
      witnesses, and thread-heap `heap`/`tld` projections.
- [ ] Keep raw `mi_heap_t` and `mi_theap_t` constructor boundaries explicit and
      avoid adding heap allocation or shared mutable state.
- [ ] Preserve the existing tests that classify main, initialized,
      uninitialized, deleted, borrowed, and local thread-heap states.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
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

- [ ] Add local `SAFETY[AUDIT:typed-freelist-wrappers]` markers for encoded
      pointer reads/writes, block tag reads/writes, allocated/free block
      witness construction, and local/remote free-list conversions.
- [ ] Keep free-list wrappers allocation-free and preserve raw block-link
      layout compatibility.
- [ ] Replace broad strict-type entries for
      `src/types/freelist_wrappers.rs` with symbol-level rows where the checker
      can express the active `mi_block_t` and encoded-pointer boundaries.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
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

- [ ] Move the datatype acceptance assertion cluster into a new module under
      `tests/release_fast_path_audit/`.
- [ ] Pass borrowed source strings into the new assertion helper rather than
      re-reading large files.
- [ ] Keep coverage for memid views, block-link wrappers, atomic field wrappers,
      strict-type checker rules, pointer provenance helpers, and package-level
      lint/allowance guards.
- [ ] Update `RELEASE_AUDIT_MODULES` and this TODO evidence with the new module
      and line counts.
- [ ] Preserve release-audit behavior and avoid weakening source assertions
      while moving them.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Suggested implementation order

1. [ ] Migrate `src/types/arena.rs` off the unsafe migration allowlist.
2. [ ] Migrate `src/types/page_queues.rs` off the unsafe migration allowlist.
3. [ ] Migrate `src/types/heap.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/types/freelist_wrappers.rs` off the unsafe migration
       allowlist and narrow its broad strict-type entries where possible.
5. [ ] Split the datatype acceptance release-audit assertion cluster.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file.
- [ ] `src/types/arena.rs`, `src/types/page_queues.rs`, `src/types/heap.rs`,
      and `src/types/freelist_wrappers.rs` are covered by operation-level audit
      markers and evidence instead of file-level migration exceptions.
- [ ] Broad strict-type rows for `src/types/freelist_wrappers.rs` are narrowed
      where the current checker can express the active boundaries.
- [ ] One more release-audit assertion cluster has a dedicated module.
