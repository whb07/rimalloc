# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master`. The
crate is in better shape: the release audit is split, the safe facade test suite
is out of the facade, and arena diagnostics are no longer embedded in the main
arena translation unit. The next issues are narrower: keep shrinking broad
exception surfaces, make the new arena diagnostics module less raw-pointer
heavy, and continue splitting large translated modules without touching hot
paths casually.

Review evidence:

- `docs/unsafe-migration-allowlist.tsv` still has 62 entries: 44 production,
  15 test, and 3 translation files.
- `strict-type-allowlist.txt` has 95 entries; 8 are still marked `remove:`.
  Those removable entries are broad `bare-domain-usize` file allowances in
  allocator, bitmap, and internal translation units.
- The largest Rust source files are still difficult review units:
  `src/allocator/arena.rs` is 4,613 lines, `src/allocator/page.rs` is 3,539
  lines, `src/internal/bitmap.rs` is 2,604 lines,
  `src/allocator/alloc.rs` is 2,545 lines, `src/allocator/init.rs` is 2,339
  lines, `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- `tests/release_fast_path_audit.rs` is split, but the split is still uneven:
  the root harness is 2,961 lines and
  `tests/release_fast_path_audit/unsafe_performance.rs` is 3,645 lines.
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

- [ ] Inventory the 15 test entries and the smallest production typed-wrapper
      entries still on `docs/unsafe-migration-allowlist.tsv`.
- [ ] Pick at least two low-risk files whose unsafe operations can receive
      operation-level `SAFETY[AUDIT:*]` tags without assembly evidence.
- [ ] Add `UNSAFE_AUDIT.md` entries and
      `docs/unsafe-audit-evidence.tsv` rows for each new operation ID.
- [ ] Remove the migrated files from the allowlist and lower
      `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
- [ ] Keep production, test, and translation categories visible in the audit
      output so test fixtures do not hide production debt.
- Verify: `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`,
  `scripts/check-unsafe-inventory.py`, focused release audit, full clippy, and
  full tests.

## Issue: Strict-type removable entries remain broad file-level allowances

> **Comment:** The strict-type allowlist still has eight `remove:` entries.
> These broad file-level allowances are useful while translating, but they hide
> which helpers are genuinely ABI-shaped and which are just carrying raw
> `usize` out of habit.

- [ ] Re-inventory the eight `remove:` entries and rank them by hot-path risk,
      expected churn, and ability to preserve code shape.
- [ ] Start with `src/allocator/arena_meta.rs`, `src/bitmap.rs`, or another
      low-risk non-hot helper before touching `alloc`, `free`, `page`, or arena
      allocation paths.
- [ ] Replace one broad file-level entry with typed wrappers or symbol-level
      strict-type entries.
- [ ] Lower the removable-entry cap in `scripts/check-strict-types.py`.
- [ ] Preserve raw `usize` at public ABI and platform boundaries where that is
      the correct contract.
- Verify: `scripts/check-strict-types.py`, focused release audit, full clippy,
  full tests, and assembly capture for any touched hot path.

## Issue: Arena diagnostics still expose too many raw metadata reads

> **Comment:** Moving arena diagnostics into `src/allocator/arena/debug.rs` made
> the module boundary clear, but the implementation still repeats direct prefix
> casts and close-range safety comments. A small diagnostic-only helper layer
> would make the invariants easier to review without moving allocation logic.

- [ ] Identify repeated arena/page metadata reads in
      `src/allocator/arena/debug.rs`, especially `MiArenaPrefix` field access,
      page classification, and slice bitmap reads.
- [ ] Add tiny diagnostic helper functions or local wrapper types that name the
      invariant being read while keeping the module diagnostic-only.
- [ ] Reduce repeated `SAFETY[AUDIT:arena-debug-render]` marker noise by moving
      proofs to the smallest helper boundaries that actually perform unsafe
      reads.
- [ ] Keep `mi_debug_show_arenas` and `mi_arenas_print` ABI-compatible and
      classified by `scripts/check-ffi-boundary.py`.
- [ ] Keep the module out of allocation hot paths and free of Rust allocation
      helpers.
- Verify: `scripts/check-ffi-boundary.py`,
  `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`, release audit, full clippy, and full
  tests.

## Issue: Release fast-path audit split is still uneven

> **Comment:** Splitting the release audit helped, but
> `unsafe_performance.rs` is still 3,645 lines and the root harness remains
> 2,961 lines. This is better than the original monolith, but future review
> would be easier with smaller domain modules and stricter size guards.

- [ ] Split `tests/release_fast_path_audit/unsafe_performance.rs` into smaller
      modules such as hot-path codegen, private ABI/export boundaries,
      typed-domain migration, and allocator metadata invariants.
- [ ] Move root-harness tests into child modules where they have a clear domain
      owner.
- [ ] Keep shared source-reading helpers in the root and avoid duplicating file
      slicing utilities.
- [ ] Tighten `release_audit_harness_stays_split` so no child module can grow
      past the new review threshold.
- [ ] Preserve all existing assertion coverage while moving tests.
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

1. [ ] Shrink the unsafe migration allowlist with small reviewable files.
2. [ ] Reduce one removable strict-type file-level allowance.
3. [ ] Consolidate unsafe arena diagnostic metadata reads.
4. [ ] Split the uneven release-audit unsafe/performance module further.
5. [ ] Split one low-risk page module cluster.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist is below 62 entries and the cap is lowered.
- [ ] The strict-type allowlist has fewer than 8 `remove:` entries and the
      removable-entry count is capped lower.
- [ ] `src/allocator/arena/debug.rs` has clearer diagnostic helper boundaries
      and fewer repeated local safety-marker comments.
- [ ] Release fast-path audit modules are smaller and guarded against growing
      back into large review units.
- [ ] At least one page-module boundary is improved without API, ABI, or
      hot-path performance drift.
