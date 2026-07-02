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

- `docs/unsafe-migration-allowlist.tsv` has 51 entries: 42 production, 6 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 51.
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
- `strict-type-allowlist.txt` has 3 remaining `remove:` entries:
  broad `bare-domain-usize` allowances for `alloc`, `aligned`, and `bitmap`.
  `REMOVE_ENTRY_CAP` is 3.
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

- [ ] Rank `src/types/page_queues/tests.rs`, `src/types/heap/tests.rs`,
      `src/types/memid/tests.rs`, and `src/types/atomic/tests.rs` by proof
      complexity.
- [ ] Migrate at least one test fixture file to local `SAFETY[AUDIT:*]`
      markers.
- [ ] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [ ] Remove migrated files from `docs/unsafe-migration-allowlist.tsv`.
- [ ] Lower `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.

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

- [ ] Review `src/types/padding.rs`, `src/allocator/tracking.rs`,
      `src/types/callbacks.rs`, and `src/internal/random.rs`.
- [ ] Pick one low-risk production file and replace file-level allowlisting
      with operation-level `SAFETY[AUDIT:*]` markers.
- [ ] Keep wrapper representation private and preserve existing constructor or
      callback validation behavior.
- [ ] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`,
      `docs/unsafe-migration-allowlist.tsv`, and generated unsafe inventory.
- [ ] Lower `ALLOWLIST_ENTRY_CAP` again.

Verify: unsafe audit scripts, focused unit tests for the migrated module,
release audit, full clippy, and full tests.

## Issue: Broad strict-type usize allowances remain in alloc, aligned, and bitmap

> **Comment:** The internal allowance is now symbol-level, but `alloc`,
> `aligned`, and `bitmap` still use broad `bare-domain-usize` entries. The next
> narrowing should pick one broad allowance, classify live hits, and keep only
> true ABI or C-parity boundaries.

- [ ] Run `scripts/check-strict-types.py` after temporarily removing either
      `bare-domain-usize|src/allocator/alloc.rs|*|usize`,
      `bare-domain-usize|src/allocator/aligned.rs|*|usize`, or
      `bare-domain-usize|src/bitmap.rs|*|usize`.
- [ ] Classify each hit as raw ABI, hot C-parity helper, local typed-domain
      constructor, or migratable helper.
- [ ] Replace the broad allowance with symbol-level entries or typed scalar
      wrappers where practical.
- [ ] Keep path aliases accurate for split helper files such as
      `src/internal/bitmap/bfield.rs` and `src/allocator/page/flags.rs`.
- [ ] Lower or tighten the strict-type removable-entry guard if a broad
      allowance is removed.

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

1. [ ] Migrate one small test fixture off the unsafe migration allowlist.
2. [ ] Migrate one small production file off the unsafe migration allowlist.
3. [ ] Narrow one remaining broad strict-type `usize` allowance.
4. [ ] Split one more release-audit assertion cluster.
5. [ ] Split one more production helper boundary.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist is below 51 entries and the cap is lowered.
- [ ] At least one additional production allowlisted file is covered by
      operation-level audit markers.
- [ ] One remaining broad strict-type `remove:` allowance is narrowed or
      eliminated.
- [ ] One release-audit module has more guard margin.
- [ ] One large production translation unit is smaller without API, ABI, or
      hot-path performance drift.
