# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after finishing the prior backlog on `master`. The last pass
improved the release-audit split, narrowed one strict-type allowance, moved
arena diagnostics behind clearer helpers, and extracted deferred-free callback
state out of the page translation unit. The remaining work is still the same
kind of cleanup: shrink broad exceptions, keep hot paths stable, and split
translated review units only at boundaries with clear invariants.

Review evidence:

- `docs/unsafe-migration-allowlist.tsv` still has 60 entries: 44 production,
  13 test, and 3 translation files.
- The smallest allowlisted test files are good next migration candidates:
  `src/types/freelist/tests.rs` is 84 lines,
  `src/types/padding/tests.rs` is 94 lines,
  `src/types/arena/tests.rs` is 109 lines, and
  `src/types/freelist_wrappers/tests.rs` is 119 lines.
- `strict-type-allowlist.txt` has 97 entries; 7 are still marked `remove:`.
  The removable entries are broad `bare-domain-usize` allowances for
  `alloc`, `aligned`, `arena`, `bitmap`, `free`, `internal`, and `page`.
- The largest source review units are still large:
  `src/allocator/arena.rs` is 4,613 lines,
  `src/allocator/page.rs` is 3,442 lines,
  `src/internal/bitmap.rs` is 2,604 lines,
  `src/allocator/alloc.rs` is 2,545 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- `tests/release_fast_path_audit.rs` is split, but the largest child modules are
  still review-heavy: root is 2,862 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,983 lines, and
  `tests/release_fast_path_audit/release_policy.rs` is 2,291 lines.
- `src/allocator/arena/debug.rs` is 600 lines and still has 35
  `SAFETY[AUDIT:arena-debug-render]` markers. It also needed a narrow
  `metadata-reference` strict-type allowance for `ArenaDebugView::prefix`.
- `src/allocator/page/deferred.rs` is a clean 139-line module, but
  `src/allocator/page.rs` still mixes queue counts, queue movement,
  free-list extension, fresh allocation, retirement, and generic allocation.
- `scripts/check-ffi-boundary.py`, `scripts/check-no-allocation-audit.py`,
  `scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
  `scripts/check-unsafe-inventory.py`, and `scripts/check-strict-types.py` all
  pass after the deferred-free split.

## Issue: Unsafe migration allowlist still has small reviewable entries

> **Comment:** The current allowlist still includes small test fixtures where
> unsafe operations are local and do not need assembly evidence. Migrating a few
> of those keeps reducing legacy exception surface without touching allocator hot
> paths.

- [ ] Inventory the four smallest allowlisted test files and confirm their
      unsafe operations are stack-local fixtures or wrapper proof helpers.
- [ ] Migrate at least two of those files to operation-level
      `SAFETY[AUDIT:*]` markers.
- [ ] Add `UNSAFE_AUDIT.md` operation entries and
      `docs/unsafe-audit-evidence.tsv` rows for the new IDs.
- [ ] Remove the migrated files from `docs/unsafe-migration-allowlist.tsv` and
      lower `ALLOWLIST_ENTRY_CAP`.
- [ ] Keep the audit output category split visible so test-only progress does
      not obscure production unsafe debt.
- Verify: `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`,
  `scripts/check-unsafe-inventory.py`, release audit, full clippy, and full
  tests.

## Issue: Strict-type removable entries remain broad file-level allowances

> **Comment:** The removable strict-type entries are still file-level
> `bare-domain-usize` allowances. The next pass should reduce one broad
> allowance to typed wrappers or symbol-level compatibility entries, starting
> away from the allocation/free fast path when possible.

- [ ] Re-rank the seven `remove:` entries by hot-path risk and number of current
      checker hits.
- [ ] Prefer `src/internal/mod.rs`, `src/internal/bitmap.rs`, or another
      helper-heavy file before touching `alloc`, `aligned`, `free`, or `page`.
- [ ] Replace one broad file-level `bare-domain-usize` entry with typed domain
      wrappers or narrower symbol-level entries.
- [ ] Lower `REMOVE_ENTRY_CAP` in `scripts/check-strict-types.py`.
- [ ] Preserve public C/platform ABI signatures and documented raw helper
      boundaries.
- Verify: `scripts/check-strict-types.py`, release audit, full clippy, full
  tests, and assembly capture for any changed hot path.

## Issue: Arena diagnostics still expose too many raw metadata reads

> **Comment:** `ArenaDebugView` improved the render path, but the module still
> has 35 repeated safety markers and a strict-type metadata-reference allowance
> for `prefix()`. A small refinement should make the diagnostic view own that
> proof without returning broad raw metadata references to callers.

- [ ] Inventory `ArenaDebugView::prefix` callers and the remaining
      `SAFETY[AUDIT:arena-debug-render]` marker clusters.
- [ ] Replace broad prefix borrowing with narrower accessors or copy-out views
      for the specific fields used by diagnostics.
- [ ] Remove the `metadata-reference|src/allocator/arena/debug.rs` strict-type
      allowance if the checker no longer needs it.
- [ ] Reduce the `arena-debug-render` marker count while keeping operation-level
      safety proof at the helper that performs the raw read.
- [ ] Keep `mi_debug_show_arenas` and `mi_arenas_print` ABI-compatible and
      allocation-free.
- Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
  `scripts/check-no-allocation-audit.py`, release audit, full clippy, and full
  tests.

## Issue: Release fast-path audit split is still uneven

> **Comment:** The release audit now has child modules, but
> `unsafe_performance.rs` is still nearly 3,000 lines. More of its structural
> assertions can move into domain modules without weakening the coverage.

- [ ] Inventory large assertion clusters in
      `tests/release_fast_path_audit/unsafe_performance.rs`, especially callback
      boundaries, strict-type migration guards, and allocation-layout checks.
- [ ] Move one cohesive cluster into a new child module under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [ ] Keep shared source-reading helpers in the parent and avoid duplicating
      slicing utilities.
- [ ] Tighten the release-audit size guard if the split gives enough margin.
- [ ] Preserve the 67-test release audit count and all existing assertions.
- Verify: `cargo test --test release_fast_path_audit --all-features`,
  `git diff --check`, full clippy, and full tests.

## Issue: Page module still mixes queue, count, and allocation policy

> **Comment:** Extracting deferred-free state was a good first production split,
> but `page.rs` remains a 3,442-line translated unit. The next split should keep
> avoiding hot-path reshaping and move a cohesive queue/count helper cluster.

- [ ] Inventory page queue count helpers, queue movement helpers, and queue
      validation helpers in `src/allocator/page.rs`.
- [ ] Pick one low-risk queue/count cluster with a small public surface and a
      clear ownership invariant.
- [ ] Move it into a documented `src/allocator/page/*.rs` module while
      preserving existing `#[inline]` and `#[inline(never)]` behavior.
- [ ] Update release-audit source-path assertions, strict-type allowances,
      unsafe inventory, and boundary scripts after the move.
- [ ] Keep allocator ABI, symbol exports, and benchmark-critical call shape
      unchanged.
- Verify: focused page/release tests, API inventory, public symbol checks, full
  clippy, and full tests.

## Suggested implementation order

1. [ ] Shrink the unsafe migration allowlist using small test fixtures.
2. [ ] Reduce one broad removable strict-type entry.
3. [ ] Tighten arena diagnostic metadata access.
4. [ ] Split one more release-audit unsafe/performance cluster.
5. [ ] Split one page queue/count helper cluster.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist is below 60 entries and the cap is lowered.
- [ ] The strict-type removable-entry count is below 7 and capped lower.
- [ ] Arena diagnostics no longer need a broad metadata-reference allowance for
      `ArenaDebugView::prefix`.
- [ ] Release fast-path audit modules are smaller and guarded against growing
      back into large review units.
- [ ] A second page submodule owns a clear page queue/count boundary without API,
      ABI, or hot-path performance drift.
