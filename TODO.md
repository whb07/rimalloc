# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after finishing the prior backlog on `master`. The last pass
improved the release-audit split, narrowed one strict-type allowance, moved
arena diagnostics behind clearer helpers, and extracted deferred-free callback
state out of the page translation unit. The remaining work is still the same
kind of cleanup: shrink broad exceptions, keep hot paths stable, and split
translated review units only at boundaries with clear invariants.

Review evidence:

- `docs/unsafe-migration-allowlist.tsv` still has 58 entries: 44 production,
  11 test, and 3 translation files.
- The smallest allowlisted test files are good next migration candidates:
  `src/types/arena/tests.rs` is 109 lines,
  `src/types/freelist_wrappers/tests.rs` is 119 lines,
  `src/types/callbacks/tests.rs` is 135 lines, and
  `src/types/layout/tests.rs` is 139 lines.
- `strict-type-allowlist.txt` has 95 entries; 6 are still marked `remove:`.
  The removable entries are broad `bare-domain-usize` allowances for
  `alloc`, `aligned`, `arena`, `bitmap`, `internal`, and `page`.
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
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,870 lines, and
  `tests/release_fast_path_audit/release_policy.rs` is 2,291 lines.
- `src/allocator/arena/debug.rs` is 625 lines and still has 34
  `SAFETY[AUDIT:arena-debug-render]` markers. It no longer needs a
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

- [x] Inventory the four smallest allowlisted test files and confirm their
      unsafe operations are stack-local fixtures or wrapper proof helpers.
- [x] Migrate at least two of those files to operation-level
      `SAFETY[AUDIT:*]` markers.
- [x] Add `UNSAFE_AUDIT.md` operation entries and
      `docs/unsafe-audit-evidence.tsv` rows for the new IDs.
- [x] Remove the migrated files from `docs/unsafe-migration-allowlist.tsv` and
      lower `ALLOWLIST_ENTRY_CAP`.
- [x] Keep the audit output category split visible so test-only progress does
      not obscure production unsafe debt.
  - Evidence: `src/types/freelist/tests.rs` now carries
    `SAFETY[AUDIT:typed-freelist-test-fixtures]` for stack-local block metadata
    and raw link helper fixtures, and `src/types/padding/tests.rs` now carries
    `SAFETY[AUDIT:typed-padding-test-fixtures]` for stack-local padding records,
    byte buffers, and non-dereferenced canary witnesses. Both files were removed
    from `docs/unsafe-migration-allowlist.tsv`, `ALLOWLIST_ENTRY_CAP` is now 58,
    and `scripts/check-unsafe-audit.py` reports 58 legacy files across
    production, test, and translation categories.
- Verify: `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`,
  `scripts/check-unsafe-inventory.py`, release audit, full clippy, and full
  tests.

## Issue: Strict-type removable entries remain broad file-level allowances

> **Comment:** The removable strict-type entries are still file-level
> `bare-domain-usize` allowances. The next pass should reduce one broad
> allowance to typed wrappers or symbol-level compatibility entries, starting
> away from the allocation/free fast path when possible.

- [x] Re-rank the seven `remove:` entries by hot-path risk and number of current
      checker hits.
- [x] Prefer `src/internal/mod.rs`, `src/internal/bitmap.rs`, or another
      helper-heavy file before touching `alloc`, `aligned`, `free`, or `page`.
- [x] Replace one broad file-level `bare-domain-usize` entry with typed domain
      wrappers or narrower symbol-level entries.
- [x] Lower `REMOVE_ENTRY_CAP` in `scripts/check-strict-types.py`.
- [x] Preserve public C/platform ABI signatures and documented raw helper
      boundaries.
  - Evidence: removing the broad `bare-domain-usize` entry for
    `src/allocator/free.rs` produced zero current strict-type checker hits, so
    the stale removable allowance was deleted. `scripts/check-strict-types.py`
    now caps removable entries at 6, with the remaining broad entries limited
    to `alloc`, `aligned`, `arena`, `bitmap`, `internal`, and `page`.
- Verify: `scripts/check-strict-types.py`, release audit, full clippy, full
  tests, and assembly capture for any changed hot path.

## Issue: Arena diagnostics still expose too many raw metadata reads

> **Comment:** `ArenaDebugView` improved the render path, but the module still
> has repeated safety markers. The diagnostic view now owns the prefix proof
> without returning broad raw metadata references to callers.

- [x] Inventory `ArenaDebugView::prefix` callers and the remaining
      `SAFETY[AUDIT:arena-debug-render]` marker clusters.
- [x] Replace broad prefix borrowing with narrower accessors or copy-out views
      for the specific fields used by diagnostics.
- [x] Remove the `metadata-reference|src/allocator/arena/debug.rs` strict-type
      allowance if the checker no longer needs it.
- [x] Reduce the `arena-debug-render` marker count while keeping operation-level
      safety proof at the helper that performs the raw read.
- [x] Keep `mi_debug_show_arenas` and `mi_arenas_print` ABI-compatible and
      allocation-free.
  - Evidence: `ArenaDebugView` now builds an `ArenaDebugPrefixSnapshot` in its
    audited constructor instead of returning `&MiArenaPrefix` from `prefix()`.
    The field-specific accessors read that copy-out view, the
    `metadata-reference` allowlist entry for `src/allocator/arena/debug.rs` was
    removed, `scripts/check-strict-types.py` passes without a replacement
    allowance, and the repeated `arena-debug-render` marker count is down to
    34 while the exported debug C ABI functions remain unchanged.
- Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
  `scripts/check-no-allocation-audit.py`, release audit, full clippy, and full
  tests.

## Issue: Release fast-path audit split is still uneven

> **Comment:** The release audit now has child modules, but
> `unsafe_performance.rs` is still nearly 3,000 lines. More of its structural
> assertions can move into domain modules without weakening the coverage.

- [x] Inventory large assertion clusters in
      `tests/release_fast_path_audit/unsafe_performance.rs`, especially callback
      boundaries, strict-type migration guards, and allocation-layout checks.
- [x] Move one cohesive cluster into a new child module under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [x] Keep shared source-reading helpers in the parent and avoid duplicating
      slicing utilities.
- [x] Tighten the release-audit size guard if the split gives enough margin.
- [x] Preserve the 67-test release audit count and all existing assertions.
  - Evidence: the page-map, safe pointer lookup, heap area, main heap predicate,
    meta-page classification, and deferred-free callback boundary assertions now
    live in
    `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs`.
    `unsafe_performance.rs` dropped to 2,870 lines, the new child module is 160
    lines, the shared source reads stay in the parent, the module size guard is
    tightened to 3,000 lines, and the release audit still runs 67 tests.
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

1. [x] Shrink the unsafe migration allowlist using small test fixtures.
2. [x] Reduce one broad removable strict-type entry.
3. [x] Tighten arena diagnostic metadata access.
4. [x] Split one more release-audit unsafe/performance cluster.
5. [ ] Split one page queue/count helper cluster.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 60 entries and the cap is lowered.
- [x] The strict-type removable-entry count is below 7 and capped lower.
- [x] Arena diagnostics no longer need a broad metadata-reference allowance for
      `ArenaDebugView::prefix`.
- [x] Release fast-path audit modules are smaller and guarded against growing
      back into large review units.
- [ ] A second page submodule owns a clear page queue/count boundary without API,
      ABI, or hot-path performance drift.
