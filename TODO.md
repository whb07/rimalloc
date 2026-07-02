# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master`. The
crate is in better shape: the release audit is split, the safe facade test suite
is out of the facade, and arena diagnostics are no longer embedded in the main
arena translation unit. The next issues are narrower: keep shrinking broad
exception surfaces, make the new arena diagnostics module less raw-pointer
heavy, and continue splitting large translated modules without touching hot
paths casually.

Review evidence:

- `docs/unsafe-migration-allowlist.tsv` still has 60 entries: 44 production,
  13 test, and 3 translation files.
- `strict-type-allowlist.txt` has 94 entries; 7 are still marked `remove:`.
  Those removable entries are broad `bare-domain-usize` file allowances in
  allocator, bitmap, and internal translation units.
- The largest Rust source files are still difficult review units:
  `src/allocator/arena.rs` is 4,613 lines, `src/allocator/page.rs` is 3,539
  lines, `src/internal/bitmap.rs` is 2,604 lines,
  `src/allocator/alloc.rs` is 2,545 lines, `src/allocator/init.rs` is 2,339
  lines, `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- `tests/release_fast_path_audit.rs` is split, but the split is still uneven:
  the root harness is 2,862 lines and
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,981 lines, with
  runtime gates in a 102-line child module and lifecycle-boundary audit in a
  700-line nested module.
- `src/allocator/arena/debug.rs` is a clear 504-line diagnostic module, but it
  now carries many repeated `SAFETY[AUDIT:arena-debug-render]` comments around
  direct arena/page metadata reads. That is audited, but not yet pleasant Rust.
- `scripts/check-ffi-boundary.py`, `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`, and
  `scripts/check-unsafe-inventory.py` all pass after the latest split.

## Issue: Unsafe migration allowlist still has small reviewable entries

> **Comment:** The migration allowlist stayed capped at 62 after the arena
> diagnostic split. The next reduction should again avoid hot allocation/free
> paths and instead target small typed-wrapper or test-only files where the
> safety proof is local.

- [x] Inventory the 15 test entries and the smallest production typed-wrapper
      entries still on `docs/unsafe-migration-allowlist.tsv`.
- [x] Pick at least two low-risk files whose unsafe operations can receive
      operation-level `SAFETY[AUDIT:*]` tags without assembly evidence.
- [x] Add `UNSAFE_AUDIT.md` entries and
      `docs/unsafe-audit-evidence.tsv` rows for each new operation ID.
- [x] Remove the migrated files from the allowlist and lower
      `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
- [x] Keep production, test, and translation categories visible in the audit
      output so test fixtures do not hide production debt.
  - Evidence: `src/types/page/tests.rs` now carries
    `SAFETY[AUDIT:typed-page-test-fixtures]` for its stack-local page/range
    fixture construction, and `src/types/validation/tests.rs` now carries
    `SAFETY[AUDIT:typed-free-validation]` for its stack-local guarded free
    validation fixture. Both test files were removed from
    `docs/unsafe-migration-allowlist.tsv`, the allowlist cap is now 60, and
    `scripts/check-unsafe-audit.py` reports 60 legacy files across production,
    test, and translation categories.
- Verify: `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`,
  `scripts/check-unsafe-inventory.py`, focused release audit, full clippy, and
  full tests.

## Issue: Strict-type removable entries remain broad file-level allowances

> **Comment:** The strict-type allowlist still has eight `remove:` entries.
> These broad file-level allowances are useful while translating, but they hide
> which helpers are genuinely ABI-shaped and which are just carrying raw
> `usize` out of habit.

- [x] Re-inventory the eight `remove:` entries and rank them by hot-path risk,
      expected churn, and ability to preserve code shape.
- [x] Start with `src/allocator/arena_meta.rs`, `src/bitmap.rs`, or another
      low-risk non-hot helper before touching `alloc`, `free`, `page`, or arena
      allocation paths.
- [x] Replace one broad file-level entry with typed wrappers or symbol-level
      strict-type entries.
- [x] Lower the removable-entry cap in `scripts/check-strict-types.py`.
- [x] Preserve raw `usize` at public ABI and platform boundaries where that is
      the correct contract.
  - Evidence: `src/allocator/arena_meta.rs` now converts internal meta block
    indexes and counts into `BlockIndex`/`NonZeroU32` before calling helper
    boundaries. The raw `usize` size parameters remain only on the C-shaped
    `_mi_meta_zalloc` and `_mi_meta_free` contracts, the broad
    `arena_meta.rs` strict-type allowlist entry was removed, and
    `scripts/check-strict-types.py` now caps removable entries at 7.
- Verify: `scripts/check-strict-types.py`, focused release audit, full clippy,
  full tests, and assembly capture for any touched hot path.

## Issue: Arena diagnostics still expose too many raw metadata reads

> **Comment:** Moving arena diagnostics into `src/allocator/arena/debug.rs` made
> the module boundary clear, but the implementation still repeats direct prefix
> casts and close-range safety comments. A small diagnostic-only helper layer
> would make the invariants easier to review without moving allocation logic.

- [x] Identify repeated arena/page metadata reads in
      `src/allocator/arena/debug.rs`, especially `MiArenaPrefix` field access,
      page classification, and slice bitmap reads.
- [x] Add tiny diagnostic helper functions or local wrapper types that name the
      invariant being read while keeping the module diagnostic-only.
- [x] Reduce repeated `SAFETY[AUDIT:arena-debug-render]` marker noise by moving
      proofs to the smallest helper boundaries that actually perform unsafe
      reads.
- [x] Keep `mi_debug_show_arenas` and `mi_arenas_print` ABI-compatible and
      classified by `scripts/check-ffi-boundary.py`.
- [x] Keep the module out of allocation hot paths and free of Rust allocation
      helpers.
  - Evidence: `src/allocator/arena/debug.rs` now uses a local
    `ArenaDebugView` for diagnostic arena prefix and slice bitmap reads, and
    `mi_debug_page_state` owns page marker/color classification. Direct
    `MiArenaPrefix` casts no longer sit in render call sites, the
    `arena-debug-render` marker count in the module is down from 37 to 35, and
    `mi_debug_show_arenas`/`mi_arenas_print` remain the same exported C ABI
    entry points.
- Verify: `scripts/check-ffi-boundary.py`,
  `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`, release audit, full clippy, and full
  tests.

## Issue: Release fast-path audit split is still uneven

> **Comment:** Splitting the release audit helped, but
> `unsafe_performance.rs` is still 3,645 lines and the root harness remains
> 2,961 lines. This is better than the original monolith, but future review
> would be easier with smaller domain modules and stricter size guards.

- [x] Split `tests/release_fast_path_audit/unsafe_performance.rs` into smaller
      modules such as hot-path codegen, private ABI/export boundaries,
      typed-domain migration, and allocator metadata invariants.
- [x] Move root-harness tests into child modules where they have a clear domain
      owner.
- [x] Keep shared source-reading helpers in the root and avoid duplicating file
      slicing utilities.
- [x] Tighten `release_audit_harness_stays_split` so no child module can grow
      past the new review threshold.
- [x] Preserve all existing assertion coverage while moving tests.
  - Evidence: runtime gate assertions moved from the root harness into
    `tests/release_fast_path_audit/runtime_gates.rs`, and lifecycle and
    local-boundary assertions moved into
    `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`.
    The root harness is down from 2,961 to 2,862 lines, the unsafe-performance
    parent module is down from 3,646 to 2,981 lines, release audit coverage
    remains 67 tests after splitting the root runtime gates, and
    `release_audit_harness_stays_split` now caps each listed child module at
    3,200 lines, including the runtime and nested lifecycle modules.
- Verify: `cargo test --test release_fast_path_audit --all-features`,
  `git diff --check`, full clippy, and full tests.

## Issue: Page module still mixes queue, count, and allocation policy

> **Comment:** `src/allocator/page.rs` remains a 3,539-line translated unit. The
> next production split should avoid allocator fast-path churn and start with a
> small, cohesive page-queue or count-domain cluster.

- [ ] Inventory candidate clusters in `src/allocator/page.rs`, especially page
      queue count helpers, queue movement helpers, and deferred-free callback
      registration.
- [ ] Pick one low-risk cluster with clear ownership and minimal public surface.
- [ ] Add module docs that state the invariant owned by the new module.
- [ ] Preserve existing `#[inline]`/`#[inline(never)]` behavior and C helper
      boundaries.
- [ ] Update release-audit path assertions, unsafe inventory, and boundary
      scripts after the move.
- Verify: focused page tests, release audit, API inventory, ABI/symbol checks
  if exported paths move, full clippy, and full tests.

## Suggested implementation order

1. [x] Shrink the unsafe migration allowlist with small reviewable files.
2. [x] Reduce one removable strict-type file-level allowance.
3. [x] Consolidate unsafe arena diagnostic metadata reads.
4. [x] Split the uneven release-audit unsafe/performance module further.
5. [ ] Split one low-risk page module cluster.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 62 entries and the cap is lowered.
- [x] The strict-type allowlist has fewer than 8 `remove:` entries and the
      removable-entry count is capped lower.
- [x] `src/allocator/arena/debug.rs` has clearer diagnostic helper boundaries
      and fewer repeated local safety-marker comments.
- [x] Release fast-path audit modules are smaller and guarded against growing
      back into large review units.
- [ ] At least one page-module boundary is improved without API, ABI, or
      hot-path performance drift.
