# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`96bd4bf`. The last pass migrated two small test fixtures and one production
file off the unsafe migration allowlist, narrowed the page strict-type
allowance, split a release-audit assertion cluster, and moved bitmap bit-field
helpers into `src/internal/bitmap/bfield.rs`.

The next pass should keep the same discipline: make small verified reductions,
preserve C ABI and allocator hot-path shape, and prefer typed invariants over
mechanical file-level exceptions.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 51 entries: 42 production, 6 test,
  and 3 translation files.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/page_queues/tests.rs` is 160 lines,
  `src/types/heap/tests.rs` is 212 lines,
  `src/types/memid/tests.rs` is 243 lines,
  `src/types/atomic/tests.rs` is 287 lines, and
  `src/types/scalar/tests.rs` is 412 lines.
- The smallest remaining allowlisted production files are narrow:
  `src/types/padding.rs` is 143 lines,
  `src/allocator/tracking.rs` is 185 lines,
  `src/types/callbacks.rs` is 255 lines,
  `src/internal/random.rs` is 257 lines,
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 104 entries; 4 are still marked `remove:`.
  The remaining broad `bare-domain-usize` allowances are for `alloc`,
  `aligned`, `bitmap`, and `internal`.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,628 lines,
  `src/allocator/page.rs` is 3,109 lines,
  `src/allocator/alloc.rs` is 2,545 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- `src/internal/bitmap/bfield.rs` is 271 lines and owns the bit-field atomic
  helper layer that was previously embedded in `src/internal/bitmap.rs`.
- The largest operation-level unsafe marker clusters are still
  `SAFETY[AUDIT:arena-debug-render]` with 34 markers in
  `src/allocator/arena/debug.rs`,
  `SAFETY[AUDIT:page-count-accessors]` with 22 markers in
  `src/allocator/page/counts.rs`,
  `SAFETY[AUDIT:unsafe-callback]` with 19 markers in `src/safe/tests.rs`, and
  `SAFETY[AUDIT:owned-allocation]` with 17 markers in
  `src/safe/allocation.rs`.
- The release audit remains close to the module guard:
  `tests/release_fast_path_audit.rs` is 2,864 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,756 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,291 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 700 lines, and
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  160 lines.

## Issue: Remaining small test fixtures still hide unsafe migration debt

> **Comment:** `page_states` and `page_queues` tests are fixture-heavy, but they
> are still local proof code rather than allocator runtime code. Moving at least
> one more fixture file to operation-level markers keeps test debt from masking
> production migration progress.

- [x] Rank `src/types/page_states/tests.rs`,
      `src/types/page_queues/tests.rs`, `src/types/heap/tests.rs`, and
      `src/types/memid/tests.rs` by proof complexity.
- [x] Migrate at least one test fixture file to local `SAFETY[AUDIT:*]`
      markers.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove migrated files from `docs/unsafe-migration-allowlist.tsv`.
- [x] Lower `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
  - Evidence: `src/types/page_states/tests.rs` now uses
    `SAFETY[AUDIT:typed-page-state-test-fixtures]` for stack-local page
    metadata, non-dereferenced thread-heap identity witnesses, typestate
    transitions, abandoned ownership classification, and thread-free ownership
    claiming. The file was removed from `docs/unsafe-migration-allowlist.tsv`,
    and `ALLOWLIST_ENTRY_CAP` is now 52.

Verify: `scripts/check-unsafe-audit.py`,
`scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py`, focused migrated-module tests,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Small production wrappers still rely on file-level unsafe allowlisting

> **Comment:** `padding`, `guarded`, `callbacks`, and `random` are small enough
> to audit directly. At least one should move from broad allowlisting to
> operation-level proof markers without widening public APIs or changing hot
> allocator behavior.

- [x] Review `src/types/padding.rs`, `src/internal/guarded.rs`,
      `src/types/callbacks.rs`, and `src/internal/random.rs`.
- [x] Pick one low-risk production file and replace file-level allowlisting
      with operation-level `SAFETY[AUDIT:*]` markers.
- [x] Keep wrapper representation private and preserve existing constructor
      validation behavior.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`,
      `docs/unsafe-migration-allowlist.tsv`, and generated unsafe inventory.
- [x] Lower `ALLOWLIST_ENTRY_CAP` again.
  - Evidence: `src/internal/guarded.rs` now uses
    `SAFETY[AUDIT:guarded-config]` for the private unchecked guarded
    configuration constructor after raw option values are clamped to preserve
    `size_min <= size_max`. The file was removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now
    51.

Verify: unsafe audit scripts, focused unit tests for the migrated module,
release audit, full clippy, and full tests.

## Issue: Broad strict-type usize allowances remain after the page split

> **Comment:** The page allowance is now symbol-level, but `bitmap` and
> `internal` still use broad `bare-domain-usize` entries. The next narrowing
> should pick one broad allowance, classify live hits, and keep only true ABI or
> C-parity boundaries.

- [ ] Run `scripts/check-strict-types.py` after temporarily removing either
      `bare-domain-usize|src/bitmap.rs|*|usize` or
      `bare-domain-usize|src/internal/mod.rs|*|usize`.
- [ ] Classify each hit as raw ABI, hot C-parity helper, local typed-domain
      constructor, or migratable helper.
- [ ] Replace the broad allowance with symbol-level entries or typed scalar
      wrappers where practical.
- [ ] Keep path aliases accurate for `src/internal/bitmap/bfield.rs`,
      `src/internal/bitmap.rs`, and logical `src/bitmap.rs`.
- [ ] Lower or tighten the strict-type removable-entry guard if a broad
      allowance is removed.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`, release audit,
full clippy, and full tests.

## Issue: Release audit still has large policy and performance modules

> **Comment:** `unsafe_performance.rs` and `release_policy.rs` are both below
> the guard but still large. Another cohesive assertion cluster should move into
> a child module so future audit additions have room.

- [ ] Inventory assertion clusters in
      `tests/release_fast_path_audit/unsafe_performance.rs` and
      `tests/release_fast_path_audit/release_policy.rs`.
- [ ] Move one cohesive cluster into `tests/release_fast_path_audit/` or a
      suitable child directory.
- [ ] Keep source reads shared or pass borrowed source strings; do not duplicate
      large file-loading blocks.
- [ ] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [ ] Preserve the release audit test count and coverage intent.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Large allocator translation units still need more Rust-shaped boundaries

> **Comment:** The bitmap bit-field split helped, but `arena.rs`, `page.rs`,
> `alloc.rs`, `bitmap.rs`, and `init.rs` remain large translated review units.
> The next split should move one cohesive helper cluster with narrow imports and
> explicit re-exports only for current callers.

- [ ] Re-scan `src/allocator/arena.rs`, `src/allocator/page.rs`,
      `src/allocator/alloc.rs`, `src/internal/bitmap.rs`, and
      `src/allocator/init.rs` for low-risk helper clusters.
- [ ] Prefer non-hot, already-audited, or pure helper groups with narrow
      imports.
- [ ] Move the group into an existing or new submodule with explicit
      re-exports only for current callers.
- [ ] Update source-path assertions, strict-type aliases, no-allocation audit
      coverage, unsafe audit evidence, and generated unsafe inventory as needed.
- [ ] Preserve C ABI symbols, public Rust facade shape, and benchmark-critical
      inline behavior.

Verify: focused module tests, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Suggested implementation order

1. [x] Migrate one small test fixture off the unsafe migration allowlist.
2. [x] Migrate one small production file off the unsafe migration allowlist.
3. [ ] Narrow one remaining broad strict-type `usize` allowance.
4. [ ] Split one more release-audit assertion cluster.
5. [ ] Split one more production helper boundary.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 53 entries and the cap is lowered.
- [x] At least one additional production allowlisted file is covered by
      operation-level audit markers.
- [ ] One remaining broad strict-type `remove:` allowance is narrowed or
      eliminated.
- [ ] One release-audit module has more guard margin.
- [ ] One large production translation unit is smaller without API, ABI, or
      hot-path performance drift.
