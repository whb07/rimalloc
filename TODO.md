# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`bf4b6df`. The last pass migrated `src/types/page_queues/tests.rs` and
`src/types/padding.rs` off the unsafe migration allowlist, narrowed the bitmap
strict-type allowance to symbol-level entries, split bitmap release-audit checks,
and moved page validation helpers into `src/allocator/page/validation.rs`.

The next pass should keep the same shape: make small verified reductions,
preserve C ABI and allocator hot-path behavior, and replace broad translated
exceptions with typed Rust boundaries or operation-level audit evidence.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 48 entries: 41 production, 4 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 48.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/memid/tests.rs` is 243 lines,
  `src/types/atomic/tests.rs` is 287 lines, and
  `src/types/scalar/tests.rs` is 412 lines.
- The smallest remaining allowlisted production files are narrow:
  `src/allocator/tracking.rs` is 185 lines,
  `src/types/callbacks.rs` is 255 lines,
  `src/internal/random.rs` is 257 lines,
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines,
  `src/types/heap.rs` is 312 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 2 remaining `remove:` entries:
  broad `bare-domain-usize` allowances for `alloc` and `aligned`.
  `REMOVE_ENTRY_CAP` is 2.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,628 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,545 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The largest unsafe audit marker clusters are still broad:
  `SAFETY[AUDIT:arena-debug-render]` has 35 markers,
  `SAFETY[AUDIT:owned-allocation]` has 32 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 28 markers,
  `SAFETY[AUDIT:page-count-accessors]` has 24 markers, and
  `SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 23 markers.
- The release audit still has large review units:
  `tests/release_fast_path_audit.rs` is 2,864 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,651 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,115 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 703 lines,
  `tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is
  206 lines,
  `tests/release_fast_path_audit/release_policy/security.rs` is 181 lines, and
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  160 lines.

## Issue: Heap and metadata fixtures still hide unsafe migration debt

> **Comment:** The remaining small test fixtures are local proof code, not
> allocator runtime code. `src/types/heap/tests.rs` is now the smallest
> allowlisted test file and its unsafe blocks are centered on stack-local heap
> and thread-heap witnesses.

- [x] Migrate `src/types/heap/tests.rs` from file-level unsafe allowlisting to
      local `SAFETY[AUDIT:typed-heap-test-fixtures]` markers.
- [x] Keep the helper fixtures stack-local and avoid widening test-only helper
      visibility.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove the migrated file from `docs/unsafe-migration-allowlist.tsv`.
- [x] Lower `ALLOWLIST_ENTRY_CAP` in `scripts/check-unsafe-audit.py`.
  - Evidence: `src/types/heap/tests.rs` now uses
    `SAFETY[AUDIT:typed-heap-test-fixtures]` for stack-local heap,
    thread-heap, and TLD metadata plus real heap allocation/deletion witness
    checks. The file was removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now 48.

Verify: `cargo test types::heap::tests --lib --all-features`,
`scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py --write && scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Tracking hooks still rely on file-level unsafe allowlisting

> **Comment:** `src/allocator/tracking.rs` is 185 lines and mostly forwards to
> tracking shims or no-op compatibility paths. It is small enough to audit with
> operation-level markers while preserving the feature-gated Valgrind and ASan
> behavior.

- [ ] Replace the file-level allowlist entry for `src/allocator/tracking.rs`
      with operation-level `SAFETY[AUDIT:tracking-hooks]` markers.
- [ ] Keep the default no-tracking path allocation-free and keep shim calls
      behind the existing `track-valgrind` and `track-asan` cfg gates.
- [ ] Add audit evidence that points at tracking parity scripts and release
      audit coverage.
- [ ] Remove the file from `docs/unsafe-migration-allowlist.tsv`, update
      generated unsafe inventory, and lower `ALLOWLIST_ENTRY_CAP` again.

Verify: unsafe audit scripts, `scripts/check-valgrind-tracking-parity.py`,
`scripts/check-asan-tracking-parity.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Broad strict-type usize allowances remain in alloc and aligned

> **Comment:** Bitmap is no longer covered by a broad removable allowance.
> `alloc` and `aligned` still allow every `usize` parameter in the file, which
> hides whether each size/count/alignment parameter is raw ABI, C-parity hot
> path, or a migratable typed-domain boundary.

- [ ] Temporarily remove
      `bare-domain-usize|src/allocator/alloc.rs|*|usize` and run
      `scripts/check-strict-types.py` to collect the live hits.
- [ ] Classify each hit as raw ABI, hot C-parity helper, typed request
      constructor, product-size helper, or migratable local helper.
- [ ] Replace the broad `alloc` entry with symbol-level entries and typed
      scalar wrappers where practical.
- [ ] Keep `REMOVE_ENTRY_CAP` accurate if the broad removable entry is removed.
- [ ] Preserve allocation hot-path shape and avoid adding heap allocation or
      trait-object dispatch to allocator fast paths.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Release audit modules still have broad assertion clusters

> **Comment:** The bitmap split helped, but `unsafe_performance.rs` remains
> 2,651 lines and `unsafe_performance/lifecycle_boundaries.rs` is a 703-line
> single-test cluster. Another cohesive group should move into a child module so
> future audit additions have room.

- [ ] Inventory assertion sections in `unsafe_performance.rs` and
      `unsafe_performance/lifecycle_boundaries.rs`.
- [ ] Move one cohesive cluster into a new or existing file under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [ ] Pass borrowed source strings rather than duplicating large file reads.
- [ ] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [ ] Preserve the release audit test count and coverage intent.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Arena remains the largest translated production review unit

> **Comment:** `src/allocator/arena.rs` is 4,628 lines after prior page and
> bitmap splits elsewhere. The fresh scan shows several cohesive helper groups,
> including heap-visit/delete helpers near the top of the file and arena prefix
> accessors. The next split should reduce review size without touching ABI or
> benchmark-critical allocation behavior.

- [ ] Re-scan `src/allocator/arena.rs` for a low-risk cohesive helper group,
      preferring heap-visit/delete helpers or prefix accessor helpers if their
      imports stay narrow.
- [ ] Move the group into an `src/allocator/arena/` child module with explicit
      `pub(crate)` re-exports only for current callers.
- [ ] Keep data layout private and avoid exposing translated prefix structs
      outside the allocator arena module boundary.
- [ ] Update strict-type path aliases, unsafe audit evidence, release-audit
      source assertions, and generated unsafe inventory as needed.
- [ ] Preserve public C ABI symbols, public Rust facade shape, and hot-path
      inline behavior.

Verify: focused arena/page tests where applicable,
`scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Suggested implementation order

1. [x] Migrate `src/types/heap/tests.rs` off the unsafe migration allowlist.
2. [ ] Migrate `src/allocator/tracking.rs` off the unsafe migration allowlist.
3. [ ] Narrow the broad `alloc` strict-type `usize` allowance.
4. [ ] Split one more release-audit assertion cluster.
5. [ ] Split one cohesive helper group out of `src/allocator/arena.rs`.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 49 entries and the cap is lowered.
- [ ] One additional production allowlisted file is covered by operation-level
      audit markers and evidence.
- [ ] One remaining broad strict-type `remove:` allowance is narrowed or
      eliminated.
- [ ] One release-audit module has more review margin.
- [ ] `src/allocator/arena.rs` is smaller without API, ABI, or hot-path
      performance drift.
