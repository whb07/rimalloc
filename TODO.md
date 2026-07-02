# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master`. The
last pass lowered the unsafe migration cap to 58, removed one stale strict-type
allowance, split a release-audit boundary module, and moved page count helpers
out of `page.rs`. The next pass should keep the same discipline: reduce broad
exceptions, preserve allocator hot-path shape, and split translated code only at
cohesive boundaries with clear invariants.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` now has 56 entries: 44 production,
  9 test, and 3 translation files.
- The smallest remaining allowlisted test fixtures are still reviewable:
  `src/types/freelist_wrappers/tests.rs` is 119 lines,
  `src/types/layout/tests.rs` is 139 lines, and
  `src/types/page_states/tests.rs` is 151 lines.
- `strict-type-allowlist.txt` has 95 entries; 6 are still marked `remove:`.
  The remaining broad `bare-domain-usize` allowances are for `alloc`,
  `aligned`, `arena`, `bitmap`, `internal`, and `page`.
- The largest production review units are still translated-style modules:
  `src/allocator/arena.rs` is 4,613 lines,
  `src/allocator/page.rs` is 3,220 lines,
  `src/internal/bitmap.rs` is 2,604 lines,
  `src/allocator/alloc.rs` is 2,545 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- `src/allocator/page/counts.rs` is 264 lines and now owns 25
  `SAFETY[AUDIT:page-count-accessors]` markers. The split improved module
  shape, but the repeated marker cluster is still noisy.
- `src/allocator/arena/debug.rs` is 620 lines and still owns 34
  `SAFETY[AUDIT:arena-debug-render]` markers around diagnostic metadata reads.
- The release audit is split, but its largest units remain close to the guard:
  `tests/release_fast_path_audit.rs` is 2,864 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,877 lines, and
  `tests/release_fast_path_audit/release_policy.rs` is 2,291 lines.

## Issue: Unsafe migration allowlist still has small test fixtures

> **Comment:** The remaining test-only unsafe fixtures are small enough to audit
> directly. Migrating them reduces legacy exception surface without changing
> production allocator behavior.

- [x] Re-rank the smallest allowlisted test files by proof complexity.
- [x] Migrate at least two local fixture files to operation-level
      `SAFETY[AUDIT:*]` markers.
- [x] Add matching entries to `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv`.
- [x] Remove the migrated files from `docs/unsafe-migration-allowlist.tsv` and
      lower `ALLOWLIST_ENTRY_CAP`.
- [x] Keep the audit category split visible so test progress does not hide
      production unsafe debt.
  - Evidence: `src/types/arena/tests.rs` now uses
    `SAFETY[AUDIT:typed-arena-test-fixtures]` for stack-local arena metadata,
    slice-range fixtures, and arena-role constructor proofs.
    `src/types/callbacks/tests.rs` now uses
    `SAFETY[AUDIT:typed-callback-test-fixtures]` for callback storage pointer
    round trips. Both files were removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now 56.

Verify: `scripts/check-unsafe-audit.py`,
`scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`,
`cargo clippy --workspace --all-targets --all-features`, and
`cargo test --workspace --all-targets --all-features`.

## Issue: Page count accessors repeat the same unsafe proof too often

> **Comment:** Moving page count accessors into `page/counts.rs` made the
> boundary clearer, but 25 repeated `page-count-accessors` markers in 264 lines
> still reads like a mechanical translation. The next pass should consolidate
> proof at small helpers or typed views without changing the caller-facing inline
> shape.

- [ ] Group the count accessors by shared raw field invariant.
- [ ] Introduce narrow safe helpers or typed local views where they reduce
      repeated unsafe blocks without adding allocation or dynamic dispatch.
- [ ] Keep current `#[inline]` behavior for hot count accessors.
- [ ] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`, and
      `docs/unsafe-inventory.md` if marker IDs or counts change.
- [ ] Preserve the existing `crate::page` re-export surface.

Verify: focused page tests, `scripts/check-unsafe-audit.py`,
`scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Strict-type removable entries are still broad

> **Comment:** The remaining `remove:` entries are file-level
> `bare-domain-usize` allowances. The next cleanup should eliminate or narrow
> one broad allowance by adding typed scalar domains around helper code that is
> not a C ABI boundary.

- [ ] Re-rank the 6 removable entries by checker hits and hot-path risk.
- [ ] Prefer `src/internal/mod.rs`, `src/internal/bitmap.rs`, or another
      helper-heavy area before touching allocation/free fast paths.
- [ ] Replace one broad file-level allowance with typed wrappers or narrower
      symbol-level compatibility entries.
- [ ] Lower `REMOVE_ENTRY_CAP` in `scripts/check-strict-types.py`.
- [ ] Preserve C ABI signatures, raw helper compatibility, and measured hot
      call shape.

Verify: `scripts/check-strict-types.py`,
`cargo test --test release_fast_path_audit --all-features`,
`cargo clippy --workspace --all-targets --all-features`, and
`cargo test --workspace --all-targets --all-features`.

## Issue: Release fast-path audit still has large child modules

> **Comment:** The release audit split is working, but `unsafe_performance.rs`
> remains close to the 3,000-line guard. Another cohesive assertion cluster can
> move into a child module so the audit stays reviewable as new checks are added.

- [ ] Inventory large assertion clusters in
      `tests/release_fast_path_audit/unsafe_performance.rs` and
      `tests/release_fast_path_audit/release_policy.rs`.
- [ ] Move one cohesive cluster into a child module under
      `tests/release_fast_path_audit/`.
- [ ] Keep source-reading helpers shared instead of duplicating file-slicing
      utilities.
- [ ] Preserve the current release-audit test coverage and tighten guards if
      the split creates enough margin.
- [ ] Update `tests/release_fast_path_audit/todo_backlog.rs` when the measured
      evidence changes.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Large allocator translation units still need boundary splits

> **Comment:** `arena.rs` and `page.rs` are still the biggest production review
> units. The next split should move one small, cohesive, non-hot helper cluster
> into a submodule while preserving ABI, symbol names, and fast-path code shape.

- [ ] Re-scan `src/allocator/arena.rs` and `src/allocator/page.rs` for helper
      clusters with narrow ownership invariants.
- [ ] Pick a low-risk cluster away from allocation/free inner loops unless the
      boundary already has release-audit coverage.
- [ ] Move the cluster into an existing or new allocator submodule with private
      implementation details and explicit re-exports only where needed.
- [ ] Update source-path assertions, strict-type path aliases, unsafe audit
      evidence, and generated unsafe inventory after the move.
- [ ] Preserve allocator ABI, public symbols, and benchmark-critical inline
      behavior.

Verify: `scripts/check-ffi-boundary.py`, `scripts/check-api-inventory.py`,
`scripts/check-public-abi.py`, focused release/page or arena tests, full
clippy, and full tests.

## Suggested implementation order

1. [ ] Shrink the unsafe migration allowlist using small test fixtures.
2. [ ] Consolidate repeated page count accessor safety proof.
3. [ ] Remove or narrow one broad strict-type `usize` allowance.
4. [ ] Split one more release-audit assertion cluster.
5. [ ] Split one production allocator helper boundary.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 58 entries and the cap is lowered.
- [ ] The page count accessor marker cluster is smaller or justified by a
      narrower helper proof.
- [ ] The strict-type removable-entry count is below 6 and capped lower.
- [ ] The largest release-audit child module has more guard margin.
- [ ] One large allocator translation unit is smaller without API, ABI, or
      hot-path performance drift.
