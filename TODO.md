# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`fdf1a76`. The last pass migrated two test fixtures off the unsafe migration
allowlist, consolidated page count accessors, removed the arena strict-type
`usize` allowance, split one release-audit cluster, and moved page size-class
helpers into `src/allocator/page/bins.rs`.

The next pass should keep the same discipline: make small verified reductions,
preserve C ABI and allocator hot-path shape, and prefer typed invariants over
mechanical file-level exceptions.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 53 entries: 43 production, 7 test,
  and 3 translation files.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/page_states/tests.rs` is 151 lines, and
  `src/types/page_queues/tests.rs` is 160 lines,
  `src/types/heap/tests.rs` is 212 lines, and
  `src/types/memid/tests.rs` is 243 lines.
- The smallest remaining allowlisted production files are also narrow:
  `src/types/padding.rs` is 143 lines,
  `src/allocator/tracking.rs` is 185 lines,
  `src/internal/guarded.rs` is 244 lines, and
  `src/types/callbacks.rs` is 255 lines, and
  `src/types/page.rs` is 258 lines.
- `strict-type-allowlist.txt` has 94 entries; 5 are still marked `remove:`.
  The remaining broad `bare-domain-usize` allowances are for `alloc`,
  `aligned`, `bitmap`, `internal`, and `page`.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,628 lines,
  `src/allocator/page.rs` is 3,109 lines,
  `src/internal/bitmap.rs` is 2,604 lines,
  `src/allocator/alloc.rs` is 2,545 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- `src/allocator/page/bins.rs` is 128 lines and owns the
  `SAFETY[AUDIT:page-bin-size-classes]` boundary for bin table lookup,
  unchecked `Bin` construction, and `mi_good_size` symbol routing.
- The largest operation-level unsafe marker clusters are still
  `SAFETY[AUDIT:arena-debug-render]` with 34 markers in
  `src/allocator/arena/debug.rs`, `SAFETY[AUDIT:page-count-accessors]` with 22
  markers in `src/allocator/page/counts.rs`, and
  `SAFETY[AUDIT:owned-allocation]` with 17 markers in
  `src/safe/allocation.rs`.
- The release audit remains close to the module guard:
  `tests/release_fast_path_audit.rs` is 2,864 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,828 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,291 lines,
  and `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 700 lines.

## Issue: Small test fixtures still sit on the unsafe migration allowlist

> **Comment:** The remaining smallest test-only files are local fixture code,
> not allocator runtime code. They should have operation-level proof markers so
> test debt does not hide production unsafe migration progress.

- [x] Rank `src/types/freelist_wrappers/tests.rs`,
      `src/types/layout/tests.rs`, `src/types/page_states/tests.rs`, and
      `src/types/page_queues/tests.rs` by proof complexity.
- [x] Migrate at least two of those files to local
      `SAFETY[AUDIT:*]` markers.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove migrated files from `docs/unsafe-migration-allowlist.tsv`.
- [x] Lower `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
  - Evidence: `src/types/freelist_wrappers/tests.rs` now uses
    `SAFETY[AUDIT:typed-freelist-wrapper-test-fixtures]` for stack-local
    page/block encoded-link and free-list wrapper fixtures.
    `src/types/layout/tests.rs` now uses
    `SAFETY[AUDIT:typed-layout-test-fixtures]` for concrete unchecked scalar
    values and non-dereferenced pointer witnesses. Both files were removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now
    54.

Verify: `scripts/check-unsafe-audit.py`,
`scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Small production files are still covered by broad unsafe allowlist entries

> **Comment:** Several small production modules have narrow invariants and are
> good candidates for direct per-operation audits. Moving one off the allowlist
> reduces real production exception surface without refactoring hot allocator
> behavior.

- [x] Review the smallest production candidates:
      `src/types/padding.rs`, `src/internal/provenance.rs`,
      `src/allocator/tracking.rs`, `src/internal/guarded.rs`, and
      `src/types/callbacks.rs`.
- [x] Pick at least one low-risk file and replace file-level allowlisting with
      operation-level `SAFETY[AUDIT:*]` markers.
- [x] Keep typed wrapper invariants private and do not widen public APIs.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`,
      `docs/unsafe-migration-allowlist.tsv`, and generated inventory.
- [x] Lower `ALLOWLIST_ENTRY_CAP` again.
  - Evidence: `src/internal/provenance.rs` now uses
    `SAFETY[AUDIT:provenance-miri-registry]` for the Miri-only raw pointer
    registry wrapper `Send`/`Sync` impls. The file was removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now
    53.

Verify: unsafe audit scripts, focused unit tests for the migrated module,
release audit, full clippy, and full tests.

## Issue: The page strict-type allowance is still file-wide after the bin split

> **Comment:** `src/allocator/page.rs` is smaller now, and the moved bin helpers
> are covered by a path alias. The remaining page `usize` allowance should be
> narrowed to true raw ABI and C-parity helper boundaries instead of masking the
> entire module.

- [ ] Run `scripts/check-strict-types.py` after temporarily removing
      `bare-domain-usize|src/allocator/page.rs|*|usize` to identify live hits.
- [ ] Classify each hit as ABI, hot C-parity helper, test-only, or migratable
      typed domain.
- [ ] Replace the file-wide allowance with symbol-level entries or typed
      scalar wrappers where practical.
- [ ] Keep `src/allocator/page/bins.rs` and `src/allocator/page/counts.rs`
      strict-type aliases accurate.
- [ ] Lower or tighten the strict-type removable-entry guard if the broad page
      allowance is removed.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`, release audit,
full clippy, and full tests.

## Issue: Release fast-path audit has one large assertion module left

> **Comment:** `unsafe_performance.rs` is still 2,828 lines and close to the
> 3,000-line guard. Another cohesive assertion group should move into a child
> module so new guards do not immediately fight the limit.

- [ ] Inventory assertion clusters in
      `tests/release_fast_path_audit/unsafe_performance.rs`.
- [ ] Move one cohesive cluster into
      `tests/release_fast_path_audit/unsafe_performance/`.
- [ ] Keep source reads shared or pass borrowed source strings; do not duplicate
      large file-loading blocks.
- [ ] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [ ] Preserve the release audit test count and coverage intent.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Large allocator translation units still need cohesive boundaries

> **Comment:** The page bin split helped, but `arena.rs`, `page.rs`, and
> `bitmap.rs` remain large translated review units. The next production split
> should move a cohesive helper cluster with clear ownership invariants and
> existing audit coverage.

- [ ] Re-scan `src/allocator/arena.rs`, `src/allocator/page.rs`, and
      `src/internal/bitmap.rs` for low-risk helper clusters.
- [ ] Prefer a non-hot or already-audited helper group with narrow imports.
- [ ] Move the group into an existing or new submodule with explicit
      re-exports only for current callers.
- [ ] Update source-path assertions, strict-type aliases, unsafe audit evidence,
      and generated unsafe inventory as needed.
- [ ] Preserve C ABI symbols, public Rust facade shape, and benchmark-critical
      inline behavior.

Verify: focused module tests, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Suggested implementation order

1. [x] Migrate two small test fixtures off the unsafe migration allowlist.
2. [x] Migrate one small production file off the unsafe migration allowlist.
3. [ ] Narrow or remove the page strict-type `usize` allowance.
4. [ ] Split one more release-audit assertion cluster.
5. [ ] Split one more production helper boundary.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 56 entries and the cap is lowered.
- [x] At least one production allowlisted file is covered by operation-level
      audit markers.
- [ ] One strict-type broad `remove:` allowance is narrowed or eliminated.
- [ ] The largest release-audit child module has more guard margin.
- [ ] One large production translation unit is smaller without API, ABI, or
      hot-path performance drift.
