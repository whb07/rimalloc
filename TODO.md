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

- `docs/unsafe-migration-allowlist.tsv` has 45 entries: 39 production, 3 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 45.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/atomic/tests.rs` is 287 lines,
  `src/types/scalar/tests.rs` is 412 lines, and
  `src/types/ptr/tests.rs` is 618 lines.
- The smallest remaining allowlisted production files are narrow:
  `src/internal/random.rs` is 257 lines,
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines,
  `src/types/heap.rs` is 312 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 0 remaining `remove:` entries.
  `REMOVE_ENTRY_CAP` is 0, but `src/allocator/arena.rs` still has broad
  permanent raw-pointer and pointer/usize compatibility entries that should be
  revisited as cohesive child modules split out.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,337 lines after moving arena id/memid helpers
  into `src/allocator/arena/ids.rs`, which is 244 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The largest unsafe audit marker clusters are broad enough to revisit:
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 34 markers,
  `SAFETY[AUDIT:owned-allocation]` has 31 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 26 markers,
  `SAFETY[AUDIT:arena-id-memid-helpers]` has 24 markers,
  `SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 22 markers, and
  `SAFETY[AUDIT:page-count-accessors]` has 22 markers.
- The release audit still has large review units:
  `tests/release_fast_path_audit.rs` is 2,889 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,567 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 660 lines,
  `tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is
  206 lines,
  `tests/release_fast_path_audit/release_policy/security.rs` is 181 lines,
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  160 lines, and
  `tests/release_fast_path_audit/unsafe_performance/size_class_boundaries.rs`
  is 132 lines.
- The next arena split candidate is the heap visit/delete/move/destroy cluster
  near the top of `src/allocator/arena.rs`; it is already grouped around
  `mi_heap_visit_info_t`, `mi_heap_delete_visit_info_t`,
  `_mi_heap_visit_blocks`, `_mi_heap_move_pages`, and
  `_mi_heap_destroy_pages`.

## Issue: Atomic test fixtures still hide unsafe migration debt

> **Comment:** `src/types/atomic/tests.rs` is the smallest remaining
> allowlisted test file. It should be local proof code for stack-backed atomic
> wrappers, not a file-level unsafe exception.

- [ ] Migrate `src/types/atomic/tests.rs` from file-level unsafe allowlisting
      to local `SAFETY[AUDIT:typed-atomic-test-fixtures]` markers.
- [ ] Keep fixtures stack-local and preserve the current ordering, ownership,
      and layout proof intent.
- [ ] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [ ] Remove the file from `docs/unsafe-migration-allowlist.tsv`.
- [ ] Regenerate unsafe inventory and lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::atomic::tests --lib --all-features`,
`scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py --write && scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Random remains a small production unsafe allowlisted module

> **Comment:** `src/internal/random.rs` is the smallest remaining production
> allowlisted file. The unsafe operations are random-context raw pointer entry
> points and primitive entropy/clock bridges, while the ChaCha state update
> logic is ordinary safe Rust.

- [ ] Replace the file-level allowlist entry for `src/internal/random.rs` with
      operation-level `SAFETY[AUDIT:random-context-boundaries]` markers.
- [ ] Keep ChaCha block generation allocation-free and preserve the current
      integer operations and inlining shape.
- [ ] Keep raw C ABI shims narrow by converting pointers to references once and
      doing ordinary borrowed work after that point.
- [ ] Add release-audit evidence for the random context entry points and the
      primitive entropy/clock bridge.
- [ ] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`, generated
      unsafe inventory, and lower `ALLOWLIST_ENTRY_CAP` again.

Verify: random unit coverage if present, unsafe audit scripts,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Arena heap page visitors remain embedded in arena.rs

> **Comment:** The heap visit/delete/move/destroy cluster is a cohesive review
> unit at the top of `src/allocator/arena.rs`. Moving it to a child module
> should reduce the largest translated file without touching purge scheduling or
> page allocation hot paths.

- [ ] Move `mi_heap_visit_info_t`, `mi_heap_delete_visit_info_t`, heap
      visit/delete helpers, `_mi_heap_visit_blocks`, `mi_heap_visit_blocks`,
      `mi_heap_visit_abandoned_blocks`, `_mi_heap_move_pages`, and
      `_mi_heap_destroy_pages` into a new `src/allocator/arena/` child module.
- [ ] Keep all current public C ABI symbols and internal call sites stable.
- [ ] Keep callback wrappers and page ownership transitions explicit; avoid
      adding allocation, dynamic dispatch, or wider module visibility.
- [ ] Update strict-type path aliases, release-audit source assertions, unsafe
      audit evidence, and generated unsafe inventory as needed.
- [ ] Re-measure `src/allocator/arena.rs` and document the new line counts.

Verify: focused heap/arena tests where applicable,
`scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Release audit unsafe_performance.rs still carries broad source assertions

> **Comment:** `unsafe_performance.rs` is still 2,567 lines. The next split
> should move one cohesive arena, atomic, or OS assertion cluster into a child
> module while keeping the root audit test readable.

- [ ] Inventory assertion sections in `unsafe_performance.rs`, prioritizing
      arena heap-visitor assertions, atomic wrapper assertions, or OS/page-map
      assertion clusters.
- [ ] Move one cohesive cluster into a new or existing file under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [ ] Pass borrowed source strings rather than duplicating large file reads.
- [ ] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [ ] Preserve the release audit test count and coverage intent.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Arena strict-type entries are still broad after child splits

> **Comment:** The final `remove:` strict-type entries are gone, but several
> permanent `src/allocator/arena.rs|*` entries still cover raw page, heap,
> thread-heap, arena, subproc, and pointer/usize boundaries. As modules split
> out, these should become narrower path- or symbol-level entries where the
> checker can support that without losing C compatibility documentation.

- [ ] Re-run `scripts/check-strict-types.py` after the next arena child module
      split and collect the live arena hits.
- [ ] Replace any broad arena entries made unnecessary by the split with
      child-module or symbol-level entries.
- [ ] Keep permanent compatibility entries only where raw C-shaped signatures or
      translated arena range arithmetic genuinely remain.
- [ ] Update allowlist comments so each retained entry identifies its concrete
      boundary instead of only saying "translated C arena".
- [ ] Keep `REMOVE_ENTRY_CAP` at 0.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Suggested implementation order

1. [ ] Migrate `src/types/atomic/tests.rs` off the unsafe migration allowlist.
2. [ ] Migrate `src/internal/random.rs` off the unsafe migration allowlist.
3. [ ] Split the arena heap visit/delete/move/destroy cluster.
4. [ ] Split one more `unsafe_performance.rs` assertion cluster.
5. [ ] Narrow broad arena strict-type entries after the arena split.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist is below 45 entries and the cap is lowered
      with each migrated file.
- [ ] One additional production allowlisted file is covered by operation-level
      audit markers and evidence.
- [ ] `src/allocator/arena.rs` is smaller again without API, ABI, or hot-path
      performance drift.
- [ ] One release-audit module has more review margin.
- [ ] Broad arena strict-type documentation is narrower where the code now has
      child-module boundaries.
