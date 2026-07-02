# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous Rust idiom and release-readiness
backlog. The crate now has stronger audit gates, but the next visible issues
are still translation residue, broad exception surfaces, and oversized review
units.

Review evidence:

- `docs/unsafe-migration-allowlist.tsv` still has 62 entries: 44 production,
  15 test, and 3 translation files.
- `strict-type-allowlist.txt` has 95 entries; 8 are still marked `remove:`.
  Those removable entries are broad `bare-domain-usize` file allowances.
- The largest source files remain hard to review as Rust modules:
  `src/allocator/arena.rs` is 5,071 lines, `src/allocator/page.rs` is 3,539
  lines, `src/internal/bitmap.rs` is 2,604 lines,
  `src/allocator/alloc.rs` is 2,545 lines, `src/allocator/init.rs` is 2,339
  lines, and `src/platform/windows.rs` is 2,191 lines.
- `tests/release_fast_path_audit.rs` is 8,845 lines and mixes API inventory,
  release policy, performance, unsafe, platform, and TODO-backlog assertions in
  one integration-test crate.
- `src/safe/mod.rs` is 1,194 lines because it contains the facade module plus a
  large in-file test suite with process-global callback and failure-injection
  fixtures.
- Small production modules still carry crate-level allowances such as
  `#![allow(dead_code)]` or broad C-name allowances, including
  `src/internal/provenance.rs`, `src/allocator/static_alloc.rs`,
  `src/allocator/tracking.rs`, and `src/internal/bits.rs`.

## Issue: Unsafe migration allowlist should keep shrinking

> **Comment:** The allowlist is now categorized and capped, but 64 entries is
> still too high. The next reduction should target small typed-wrapper modules
> where safety invariants are local and reviewable, not hot allocator paths that
> need assembly evidence.

- [x] Pick the smallest production typed-wrapper files still on
      `docs/unsafe-migration-allowlist.tsv`.
- [x] Add specific `SAFETY[AUDIT:*]` tags and `UNSAFE_AUDIT.md` entries for
      each remaining unsafe operation in those files.
- [x] Remove the migrated files from the unsafe migration allowlist and lower
      `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
- [x] Keep test allowlist entries separate from production entries so test
      fixture unsafe does not hide production debt.
- [x] Avoid hot allocator files until the change has behavior, assembly, and
      benchmark evidence.
  - Evidence: `src/types/freelist.rs` and `src/types/validation.rs` now carry
    operation-level `SAFETY[AUDIT:typed-freelist]` and
    `SAFETY[AUDIT:typed-free-validation]` tags with matching
    `UNSAFE_AUDIT.md` and `docs/unsafe-audit-evidence.tsv` entries. Both files
    were removed from `docs/unsafe-migration-allowlist.tsv`, and the unsafe
    migration cap is now 62 entries. No hot allocator file was touched.
- Verify: `scripts/check-unsafe-audit.py`,
  `scripts/check-unsafe-audit-evidence.py`,
  `scripts/check-unsafe-inventory.py`, focused release audit, full clippy, and
  full tests.

## Issue: Strict-type allowlist still has removable file-level entries

> **Comment:** `strict-type-allowlist.txt` still permits nine broad
> `bare-domain-usize` file entries with `remove:` dispositions. These are the
> next places where the translation still feels C-shaped: raw byte counts,
> indexes, and alignment values flow through non-ABI helpers longer than they
> should.

- [x] Inventory all nine `remove:` entries and rank them by risk, hot-path
      sensitivity, and expected code churn.
- [x] Start with low-risk internal helpers such as `src/allocator/arena_meta.rs`
      or `src/allocator/threadlocal.rs` before touching `arena`, `page`, or
      allocation hot paths.
- [x] Replace broad file-level entries with symbol-level entries, typed scalar
      wrappers, or checked constructor boundaries where that improves clarity
      without changing codegen on hot paths.
- [x] Add a strict-type audit cap for removable entries so the count cannot
      grow silently.
- [x] Preserve permanent ABI/platform entries where raw `usize` is the correct
      public or external contract.
  - Evidence: `src/allocator/threadlocal.rs` now uses local transparent
    `ThreadLocalSlotIndex`, `ThreadLocalSlotCount`, and
    `ThreadLocalKeyVersion` wrappers for dynamic TLS table/key helpers. The
    broad `bare-domain-usize` remove entry for that file was deleted, the
    checker now caps removable entries at 8, and the OS memid helper takes
    `ReservedBytes` directly at its single call site.
- Verify: `scripts/check-strict-types.py`, release fast-path audit, full
  clippy, full tests, and assembly capture for any touched hot path.

## Issue: Production module-level allowances hide local intent

> **Comment:** Some small production modules still use broad crate-level
> allowances for dead code or C-style names. Several are compatibility shims,
> but broad allowances make it difficult to distinguish intentionally retained
> C parity from leftovers.

- [ ] Inventory production crate-level allowances and split small modules into
      permanent compatibility shims versus removable translated residue.
- [ ] Localize `dead_code` allowances to individual constants or functions in
      `src/internal/provenance.rs`, `src/allocator/static_alloc.rs`,
      `src/allocator/tracking.rs`, and `src/internal/bits.rs` where possible.
- [ ] Prefer `cfg` gates or actual call-site ownership over suppressing unused
      code.
- [ ] Add or tighten an audit test that caps broad production module-level
      allowances and documents permanent exceptions.
- [ ] Leave large translated allocator modules alone unless the allowance
      change is obviously local and behavior-neutral.
- Verify: focused allowance audit, `cargo clippy --workspace --all-targets
  --all-features`, and full tests.

## Issue: Safe facade tests are mixed into the facade module

> **Comment:** `src/safe/mod.rs` should read like the public facade. The
> current in-file test module is useful, but it makes the facade harder to
> scan and keeps process-global fixtures next to production re-exports.

- [ ] Move the safe facade test module into `src/safe/tests.rs` or focused
      test submodules without changing the public safe facade.
- [ ] Keep `failure_injection` private to tests and preserve the serialized
      locks for process-global options and callback registration.
- [ ] Keep safe facade source files free of new backend imports and local
      Clippy allowances.
- [ ] Update `scripts/check-safe-boundary.py` and release audit checks only if
      the test-file move changes their inventory assumptions.
- [ ] Confirm callback, heap, allocation, options, stats, and process-info
      tests still run under the expected feature sets.
- Verify: `scripts/check-safe-boundary.py`, `scripts/check-layer-boundaries.py`,
  `scripts/check-safe-api-features.sh`, focused safe tests, full clippy, and
  full tests.

## Issue: Release fast-path audit is too large and brittle

> **Comment:** The release audit is valuable, but an 8,845-line integration
> test makes review and future edits harder than necessary. It also hardcodes
> the TODO backlog shape, which creates churn every time the backlog is
> rewritten.

- [ ] Split `tests/release_fast_path_audit.rs` into domain-focused modules
      such as API shape, docs/release policy, unsafe/performance, platform, and
      TODO backlog checks.
- [ ] Keep shared helpers in one place and avoid duplicating file-read and
      source-slicing utilities across modules.
- [ ] Make the TODO backlog audit check the current title, issue headings, and
      measurable evidence without asserting stale completed backlog details.
- [ ] Add a line-count or module-count guard so the monolithic test file does
      not grow back.
- [ ] Preserve existing assertion coverage while moving tests.
- Verify: `cargo test --test release_fast_path_audit --all-features`, full
  clippy, and full tests.

## Issue: Large translated modules still need smaller review boundaries

> **Comment:** The previous split improved `memid`, but several huge modules
> still read like translated C translation units. The next split should be
> conservative: move a small invariant cluster first, preserve inlining, and
> prove no API/ABI drift.

- [ ] Inventory candidate clusters in `src/allocator/arena.rs`,
      `src/allocator/page.rs`, `src/internal/bitmap.rs`, and
      `src/allocator/init.rs`.
- [ ] Pick one low-risk cluster with clear ownership, such as arena metadata
      helpers, page-map/page-queue glue, or initialization state helpers.
- [ ] Add module docs that state the invariant owned by the new module.
- [ ] Preserve hot-path `#[inline]` behavior and translated C parity where it
      matters.
- [ ] Update audit scripts, path aliases, and generated evidence paths after
      any move.
- Verify: module inventory checks, API inventory, ABI/symbol checks if public
  paths are touched, release assembly capture for hot paths, full clippy, and
  full tests.

## Suggested implementation order

1. [x] Shrink small typed-wrapper unsafe allowlist entries.
2. [x] Cap and reduce removable strict-type allowlist entries.
3. [ ] Localize small production module-level allowances.
4. [ ] Move safe facade tests out of `src/safe/mod.rs`.
5. [ ] Split the release fast-path audit into domain modules.
6. [ ] Split one additional low-risk translated module cluster.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 64 entries and the cap is lowered.
- [x] The strict-type allowlist has fewer than 9 `remove:` entries and the
      removable-entry count is capped.
- [ ] Small production module-level allowances are localized or documented as
      permanent compatibility shims.
- [ ] `src/safe/mod.rs` contains the facade and test wiring only, not the full
      safe facade test suite.
- [ ] The release fast-path audit is split into reviewable modules without
      losing coverage.
- [ ] At least one more translated module boundary is improved without API,
      ABI, or hot-path performance drift.
