# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`46d5fdc`. The last pass migrated one test fixture and one production file off
the unsafe migration allowlist, narrowed one broad strict-type allowance, split
security-policy release-audit checks into
`tests/release_fast_path_audit/release_policy/security.rs`, and moved page flag
and owner helpers into `src/allocator/page/flags.rs`.

The next pass should keep the same discipline: make small verified reductions,
preserve C ABI and allocator hot-path shape, and prefer typed invariants over
mechanical file-level exceptions.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 49 entries: 41 production, 5 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 49.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/heap/tests.rs` is 212 lines,
  `src/types/memid/tests.rs` is 243 lines,
  `src/types/atomic/tests.rs` is 287 lines, and
  `src/types/scalar/tests.rs` is 412 lines.
- The smallest remaining allowlisted production files are narrow:
  `src/allocator/tracking.rs` is 185 lines,
  `src/types/callbacks.rs` is 255 lines,
  `src/internal/random.rs` is 257 lines,
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 2 remaining `remove:` entries:
  broad `bare-domain-usize` allowances for `alloc` and `aligned`.
  `REMOVE_ENTRY_CAP` is 2.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,628 lines,
  `src/allocator/page.rs` is 2,951 lines,
  `src/allocator/alloc.rs` is 2,545 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- Recent helper splits now have bounded review units:
  `src/internal/bitmap/bfield.rs` is 271 lines and
  `src/allocator/page/flags.rs` is 181 lines.
- The largest unsafe audit marker clusters are still broad:
  `SAFETY[AUDIT:arena-debug-render]` has 35 markers,
  `SAFETY[AUDIT:owned-allocation]` has 32 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 28 markers, and
  `SAFETY[AUDIT:page-count-accessors]` has 24 markers.
- The release audit remains close to the module guard:
  `tests/release_fast_path_audit.rs` is 2,864 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,758 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,115 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 703 lines,
  `tests/release_fast_path_audit/release_policy/security.rs` is 181 lines, and
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  160 lines.

## Issue: Remaining small test fixtures still hide unsafe migration debt

> **Comment:** `page_queues`, `heap`, and `memid` tests are fixture-heavy, but
> they are local proof code rather than allocator runtime code. Moving another
> small fixture file to operation-level markers keeps test debt from masking
> production migration progress.

- [x] Rank `src/types/page_queues/tests.rs`, `src/types/heap/tests.rs`,
      `src/types/memid/tests.rs`, and `src/types/atomic/tests.rs` by proof
      complexity.
- [x] Migrate at least one test fixture file to local `SAFETY[AUDIT:*]`
      markers.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove migrated files from `docs/unsafe-migration-allowlist.tsv`.
- [x] Lower `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
  - Evidence: `src/types/page_queues/tests.rs` now uses
    `SAFETY[AUDIT:typed-page-queue-test-fixtures]` for stack-local page and
    queue metadata covering normal, huge, full, linked, unlinked, and mismatched
    queue/page witnesses. The file was removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now 50.

Verify: `scripts/check-unsafe-audit.py`,
`scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py`, focused migrated-module tests,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Small production wrappers still rely on file-level unsafe allowlisting

> **Comment:** `padding`, `tracking`, `callbacks`, and `random` are small enough
> to audit directly. At least one should move from broad allowlisting to
> operation-level proof markers without widening public APIs or changing hot
> allocator behavior.

- [x] Review `src/types/padding.rs`, `src/allocator/tracking.rs`,
      `src/types/callbacks.rs`, and `src/internal/random.rs`.
- [x] Pick one low-risk production file and replace file-level allowlisting
      with operation-level `SAFETY[AUDIT:*]` markers.
- [x] Keep wrapper representation private and preserve existing constructor or
      callback validation behavior.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`,
      `docs/unsafe-migration-allowlist.tsv`, and generated unsafe inventory.
- [x] Lower `ALLOWLIST_ENTRY_CAP` again.
  - Evidence: `src/types/padding.rs` now uses
    `SAFETY[AUDIT:typed-padding-metadata]` for padding pointer construction,
    block-plus-usable-size pointer derivation, raw canary/delta reads, and
    typed metadata writes. The file was removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now 49.

Verify: unsafe audit scripts, focused unit tests for the migrated module,
release audit, full clippy, and full tests.

## Issue: Broad strict-type usize allowances remain in alloc, aligned, and bitmap

> **Comment:** The internal allowance is now symbol-level, but `alloc`,
> `aligned`, and `bitmap` still use broad `bare-domain-usize` entries. The next
> narrowing should pick one broad allowance, classify live hits, and keep only
> true ABI or C-parity boundaries.

- [x] Run `scripts/check-strict-types.py` after temporarily removing either
      `bare-domain-usize|src/allocator/alloc.rs|*|usize`,
      `bare-domain-usize|src/allocator/aligned.rs|*|usize`, or
      `bare-domain-usize|src/bitmap.rs|*|usize`.
- [x] Classify each hit as raw ABI, hot C-parity helper, local typed-domain
      constructor, or migratable helper.
- [x] Replace the broad allowance with symbol-level entries or typed scalar
      wrappers where practical.
- [x] Keep path aliases accurate for split helper files such as
      `src/internal/bitmap/bfield.rs` and `src/allocator/page/flags.rs`.
- [x] Lower or tighten the strict-type removable-entry guard if a broad
      allowance is removed.
  - Evidence: the broad `src/bitmap.rs` `bare-domain-usize` allowance was
    replaced with symbol-level entries for `src/internal/bitmap/bfield.rs` and
    `src/internal/bitmap.rs`, covering bitfield atomic bit indexes, chunk-map
    bookkeeping, bitmap/bbitmap range helpers, and find/claim visitor payloads.
    Representative entries include
    `bare-domain-usize|src/internal/bitmap/bfield.rs|mi_bfield_atomic_set|usize`
    and `bare-domain-usize|src/internal/bitmap.rs|mi_bitmap_setN|usize`.
    Only `alloc` and `aligned` remain as broad removable `usize` allowances, and
    `REMOVE_ENTRY_CAP` is now 2.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`, release audit,
full clippy, and full tests.

## Issue: Release audit performance module is close to the guard

> **Comment:** `unsafe_performance.rs` is still 2,758 lines after the policy
> split. Another cohesive assertion cluster should move into a child module so
> future audit additions have room.

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

## Issue: Large allocator translation units still need more Rust-shaped boundaries

> **Comment:** The page flag split helped, but `arena.rs`, `page.rs`,
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
3. [x] Narrow one remaining broad strict-type `usize` allowance.
4. [ ] Split one more release-audit assertion cluster.
5. [ ] Split one more production helper boundary.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 51 entries and the cap is lowered.
- [x] At least one additional production allowlisted file is covered by
      operation-level audit markers.
- [x] One remaining broad strict-type `remove:` allowance is narrowed or
      eliminated.
- [ ] One release-audit module has more guard margin.
- [ ] One large production translation unit is smaller without API, ABI, or
      hot-path performance drift.
