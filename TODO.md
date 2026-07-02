# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`eee7448`. The last pass migrated memid test fixtures and callback wrappers
off file-level unsafe migration debt, narrowed the aligned allocation
strict-type allowance, split the size-class release-audit assertions, and moved
arena id/memid helpers into `src/allocator/arena/ids.rs`.

The next pass should continue the same pattern: reduce one measurable exception
at a time, preserve C ABI and allocator hot-path behavior, and replace broad
translated-style surfaces with typed Rust wrappers or operation-level audit
evidence.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 43 entries: 38 production, 2 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 43.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/scalar/tests.rs` is 412 lines and
  `src/types/ptr/tests.rs` is 618 lines.
- `src/types/atomic/tests.rs` now uses
  `SAFETY[AUDIT:typed-atomic-test-fixtures]` markers for stack-local atomic
  words, pointer cells, page/free-list metadata, once state, and lock storage.
  It is 356 lines with 16 local audit markers.
- `src/internal/random.rs` now uses
  `SAFETY[AUDIT:random-context-boundaries]` markers for random context raw
  pointer shims and primitive entropy/clock bridges. It is 282 lines with 12
  local audit markers.
- `src/allocator/arena/heap_pages.rs` now owns heap visit/delete/move/destroy
  helpers with `SAFETY[AUDIT:arena-heap-page-visitors]` markers. It is 543
  lines with 56 local audit markers, and `src/allocator/arena.rs` is down to
  3,915 lines after the split.
- The smallest remaining allowlisted production files are narrow:
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines,
  `src/types/heap.rs` is 312 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 0 remaining `remove:` entries.
  `REMOVE_ENTRY_CAP` is 0. Arena raw-pointer entries are now symbol-level for
  `src/allocator/arena.rs`, `src/allocator/arena/debug.rs`,
  `src/allocator/arena/ids.rs`, and `src/allocator/arena/heap_pages.rs`. One
  path-level `src/allocator/arena.rs` pointer/usize entry remains for arena
  range, alignment, and huge-page interleave arithmetic because the checker
  does not yet classify pointer/usize hits by function symbol.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 3,915 lines after moving arena id/memid helpers
  into `src/allocator/arena/ids.rs`, which is 244 lines, and heap page visitors
  into `src/allocator/arena/heap_pages.rs`, which is 543 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The largest unsafe audit marker clusters are broad enough to revisit:
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 34 markers,
  `SAFETY[AUDIT:owned-allocation]` has 31 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 26 markers,
  `SAFETY[AUDIT:arena-id-memid-helpers]` has 24 markers,
  `SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 22 markers, and
  `SAFETY[AUDIT:page-count-accessors]` has 22 markers.
- The release audit still has large review units:
  `tests/release_fast_path_audit.rs` is 2,893 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,561 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 662 lines,
  `tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is
  207 lines,
  `tests/release_fast_path_audit/release_policy/security.rs` is 181 lines,
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  162 lines,
  `tests/release_fast_path_audit/unsafe_performance/size_class_boundaries.rs`
  is 132 lines, and
  `tests/release_fast_path_audit/unsafe_performance/heap_page_visitors.rs` is
  56 lines.
- The heap visit/delete/move/destroy cluster now lives in
  `src/allocator/arena/heap_pages.rs` around `mi_heap_visit_info_t`,
  `mi_heap_delete_visit_info_t`, `_mi_heap_visit_blocks`,
  `_mi_heap_move_pages`, and `_mi_heap_destroy_pages`.

## Issue: Atomic test fixtures still hide unsafe migration debt

> **Comment:** `src/types/atomic/tests.rs` was the smallest remaining
> allowlisted test file. It now uses local proof markers for stack-backed atomic
> wrappers instead of a file-level unsafe exception.

- [x] Migrate `src/types/atomic/tests.rs` from file-level unsafe allowlisting
      to local `SAFETY[AUDIT:typed-atomic-test-fixtures]` markers.
- [x] Keep fixtures stack-local and preserve the current ordering, ownership,
      and layout proof intent.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove the file from `docs/unsafe-migration-allowlist.tsv`.
- [x] Regenerate unsafe inventory and lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::atomic::tests --lib --all-features`,
`scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py --write && scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Random remains a small production unsafe allowlisted module

> **Comment:** `src/internal/random.rs` was the smallest remaining production
> allowlisted file. The unsafe operations are now local random-context raw
> pointer entry points and primitive entropy/clock bridges, while the ChaCha
> state update logic remains ordinary safe Rust.

- [x] Replace the file-level allowlist entry for `src/internal/random.rs` with
      operation-level `SAFETY[AUDIT:random-context-boundaries]` markers.
- [x] Keep ChaCha block generation allocation-free and preserve the current
      integer operations and inlining shape.
- [x] Keep raw C ABI shims narrow by converting pointers to references once and
      doing ordinary borrowed work after that point.
- [x] Add release-audit evidence for the random context entry points and the
      primitive entropy/clock bridge.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`, generated
      unsafe inventory, and lower `ALLOWLIST_ENTRY_CAP` again.

Verify: random unit coverage if present, unsafe audit scripts,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Arena heap page visitors remain embedded in arena.rs

> **Comment:** The heap visit/delete/move/destroy cluster now lives in
> `src/allocator/arena/heap_pages.rs`, reducing the largest translated arena
> file without touching purge scheduling or page allocation hot paths.

- [x] Move `mi_heap_visit_info_t`, `mi_heap_delete_visit_info_t`, heap
      visit/delete helpers, `_mi_heap_visit_blocks`, `mi_heap_visit_blocks`,
      `mi_heap_visit_abandoned_blocks`, `_mi_heap_move_pages`, and
      `_mi_heap_destroy_pages` into a new `src/allocator/arena/` child module.
- [x] Keep all current public C ABI symbols and internal call sites stable.
- [x] Keep callback wrappers and page ownership transitions explicit; avoid
      adding allocation, dynamic dispatch, or wider module visibility.
- [x] Update strict-type path aliases, release-audit source assertions, unsafe
      audit evidence, and generated unsafe inventory as needed.
- [x] Re-measure `src/allocator/arena.rs` and document the new line counts.

Verify: focused heap/arena tests where applicable,
`scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Release audit unsafe_performance.rs still carries broad source assertions

> **Comment:** The heap-page visitor assertion cluster now lives in
> `tests/release_fast_path_audit/unsafe_performance/heap_page_visitors.rs`.
> The root audit keeps borrowed source strings and is down to 2,561 lines.

- [x] Inventory assertion sections in `unsafe_performance.rs`, prioritizing
      arena heap-visitor assertions, atomic wrapper assertions, or OS/page-map
      assertion clusters.
- [x] Move one cohesive cluster into a new or existing file under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [x] Pass borrowed source strings rather than duplicating large file reads.
- [x] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [x] Preserve the release audit test count and coverage intent.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Arena strict-type entries are still broad after child splits

> **Comment:** Arena raw-pointer strict-type entries are now symbol-level across
> `arena.rs`, `arena/debug.rs`, `arena/ids.rs`, and `arena/heap_pages.rs`. The
> retained path-level pointer/usize entry documents the checker limitation and
> the concrete arena range/alignment arithmetic boundary.

- [x] Re-run `scripts/check-strict-types.py` after the next arena child module
      split and collect the live arena hits.
- [x] Replace any broad arena entries made unnecessary by the split with
      child-module or symbol-level entries.
- [x] Keep permanent compatibility entries only where raw C-shaped signatures or
      translated arena range arithmetic genuinely remain.
- [x] Update allowlist comments so each retained entry identifies its concrete
      boundary instead of only saying "translated C arena".
- [x] Keep `REMOVE_ENTRY_CAP` at 0.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Suggested implementation order

1. [x] Migrate `src/types/atomic/tests.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/internal/random.rs` off the unsafe migration allowlist.
3. [x] Split the arena heap visit/delete/move/destroy cluster.
4. [x] Split one more `unsafe_performance.rs` assertion cluster.
5. [x] Narrow broad arena strict-type entries after the arena split.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 45 entries and the cap is lowered
      with each migrated file.
- [x] One additional production allowlisted file is covered by operation-level
      audit markers and evidence.
- [x] `src/allocator/arena.rs` is smaller again without API, ABI, or hot-path
      performance drift.
- [x] One release-audit module has more review margin.
- [x] Broad arena strict-type documentation is narrower where the code now has
      child-module boundaries.
