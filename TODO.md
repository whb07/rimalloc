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

- `docs/unsafe-migration-allowlist.tsv` has 47 entries: 40 production, 4 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 47.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/memid/tests.rs` is 243 lines,
  `src/types/atomic/tests.rs` is 287 lines, and
  `src/types/scalar/tests.rs` is 412 lines.
- The smallest remaining allowlisted production files are narrow:
  `src/types/callbacks.rs` is 255 lines,
  `src/internal/random.rs` is 257 lines,
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines,
  `src/types/heap.rs` is 312 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 1 remaining `remove:` entry:
  the broad `bare-domain-usize` allowance for `aligned`.
  `REMOVE_ENTRY_CAP` is 1.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,628 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
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
  `tests/release_fast_path_audit.rs` is 2,866 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,653 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,115 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 660 lines,
  `tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is
  206 lines,
  `tests/release_fast_path_audit/release_policy/security.rs` is 181 lines, and
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  160 lines. The new
  `tests/release_fast_path_audit/unsafe_performance/threadlocal_boundaries.rs`
  module is 46 lines.

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

- [x] Replace the file-level allowlist entry for `src/allocator/tracking.rs`
      with operation-level `SAFETY[AUDIT:tracking-hooks]` markers.
- [x] Keep the default no-tracking path allocation-free and keep shim calls
      behind the existing `track-valgrind` and `track-asan` cfg gates.
- [x] Add audit evidence that points at tracking parity scripts and release
      audit coverage.
- [x] Remove the file from `docs/unsafe-migration-allowlist.tsv`, update
      generated unsafe inventory, and lower `ALLOWLIST_ENTRY_CAP` again.
  - Evidence: `src/allocator/tracking.rs` now uses
    `SAFETY[AUDIT:tracking-hooks]` for the tracking shim declarations,
    feature-gated backend calls, default no-backend reporting path, and
    allocator usable-size queries. The file was removed from
    `docs/unsafe-migration-allowlist.tsv`, and `ALLOWLIST_ENTRY_CAP` is now 47.

Verify: unsafe audit scripts, `scripts/check-valgrind-tracking-parity.py`,
`scripts/check-asan-tracking-parity.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Broad strict-type usize allowance remains in aligned after alloc narrowing

> **Comment:** Bitmap is no longer covered by a broad removable allowance.
> `alloc` has been narrowed to symbol-level parser entries, and `aligned` is
> now the only remaining broad removable `usize` allowance. The completed alloc
> pass classifies size/count/alignment parameters as raw ABI, typed request
> constructors, product-size helpers, or migratable local helpers.

- [x] Temporarily remove
      `bare-domain-usize|src/allocator/alloc.rs|*|usize` and run
      `scripts/check-strict-types.py` to collect the live hits.
- [x] Classify each hit as raw ABI, hot C-parity helper, typed request
      constructor, product-size helper, or migratable local helper.
- [x] Replace the broad `alloc` entry with symbol-level entries and typed
      scalar wrappers where practical.
- [x] Keep `REMOVE_ENTRY_CAP` accurate if the broad removable entry is removed.
- [x] Preserve allocation hot-path shape and avoid adding heap allocation or
      trait-object dispatch to allocator fast paths.
  - Evidence: the broad `src/allocator/alloc.rs` `bare-domain-usize`
    allowance was replaced with symbol-level entries for
    `maybe_alignment_or_report`, `alignment_or_report`,
    `malloc_layout_or_report`, `calloc_request_or_report`, and
    `alloc_product_or_report`. Internal lazy malloc, recalloc, product-size,
    and C++ aligned-new helpers now carry `RequestedSize`, `ItemCount`,
    `CallocRequest`, or `AllocProduct` where practical. Only the broad
    `aligned` removable entry remains, and `REMOVE_ENTRY_CAP` is now 1.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Release audit modules still have broad assertion clusters

> **Comment:** The bitmap split helped, but `unsafe_performance.rs` remains
> 2,651 lines and `unsafe_performance/lifecycle_boundaries.rs` is a 703-line
> single-test cluster. Another cohesive group should move into a child module so
> future audit additions have room.

- [x] Inventory assertion sections in `unsafe_performance.rs` and
      `unsafe_performance/lifecycle_boundaries.rs`.
- [x] Move one cohesive cluster into a new or existing file under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [x] Pass borrowed source strings rather than duplicating large file reads.
- [x] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [x] Preserve the release audit test count and coverage intent.
  - Evidence: dynamic thread-local boundary assertions moved from
    `unsafe_performance/lifecycle_boundaries.rs` into
    `unsafe_performance/threadlocal_boundaries.rs`. The parent test still reads
    `src/allocator/threadlocal.rs` once and passes a borrowed source string to
    the helper. `lifecycle_boundaries.rs` dropped from 703 to 660 lines, the new
    module is 46 lines, and `cargo test --test release_fast_path_audit
    --all-features` still runs 67 tests.

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
2. [x] Migrate `src/allocator/tracking.rs` off the unsafe migration allowlist.
3. [x] Narrow the broad `alloc` strict-type `usize` allowance.
4. [x] Split one more release-audit assertion cluster.
5. [ ] Split one cohesive helper group out of `src/allocator/arena.rs`.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 49 entries and the cap is lowered.
- [x] One additional production allowlisted file is covered by operation-level
      audit markers and evidence.
- [x] One remaining broad strict-type `remove:` allowance is narrowed or
      eliminated.
- [x] One release-audit module has more review margin.
- [ ] `src/allocator/arena.rs` is smaller without API, ABI, or hot-path
      performance drift.
