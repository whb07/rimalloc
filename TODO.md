# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`f17fc6b`. The last pass migrated heap test fixtures and tracking hooks off
file-level unsafe migration debt, narrowed the broad `alloc` strict-type
allowance, split thread-local release-audit assertions, and moved arena prefix
layouts/accessors into `src/allocator/arena/prefix.rs`.

The next pass should keep the same shape: make small verified reductions,
preserve C ABI and allocator hot-path behavior, and replace broad translated
exceptions with typed Rust boundaries or operation-level audit evidence.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 45 entries: 39 production, 3 test,
  and 3 translation files. `ALLOWLIST_ENTRY_CAP` is 45.
- The smallest remaining allowlisted test fixtures are reviewable:
  `src/types/atomic/tests.rs` is 287 lines,
  `src/types/scalar/tests.rs` is 412 lines, and
  `src/types/ptr/tests.rs` is 618 lines.
- `src/types/memid/tests.rs` now uses
  `SAFETY[AUDIT:typed-memid-test-fixtures]` markers for stack-local OS, meta,
  arena, and raw memid fixtures, and no longer needs a file-level unsafe
  migration allowlist entry.
- `src/types/callbacks.rs` now uses `SAFETY[AUDIT:callback-wrappers]`
  markers for function-pointer storage and C ABI callback invocation
  boundaries, with local unit fixtures and release-audit evidence.
- The smallest remaining allowlisted production files are narrow:
  `src/internal/random.rs` is 257 lines,
  `src/types/page.rs` is 258 lines,
  `src/types/arena.rs` is 260 lines,
  `src/types/page_queues.rs` is 304 lines,
  `src/types/heap.rs` is 312 lines, and
  `src/allocator/arena_meta.rs` is 316 lines.
- `strict-type-allowlist.txt` has 0 remaining `remove:` entries.
  `REMOVE_ENTRY_CAP` is 0. The broad `bare-domain-usize` allowance for
  `src/allocator/aligned.rs` is replaced by symbol-level parser/shim entries
  such as `aligned_layout_or_report`,
  `mi_theap_malloc_zero_aligned_at`, and `mi_theap_recalloc_aligned`.
- `mi_malloc_is_naturally_aligned` now accepts typed `RequestedSize` and
  `Alignment` values before calling `mi_good_size_typed`.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 4,520 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The largest unsafe audit marker clusters are broad enough to revisit:
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 34 markers,
  `SAFETY[AUDIT:owned-allocation]` has 31 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 26 markers,
  `SAFETY[AUDIT:typed-page-queue-test-fixtures]` has 22 markers, and
  `SAFETY[AUDIT:page-count-accessors]` has 22 markers.
- The release audit still has large review units, with the size-class/bin
  assertions now split into a child module:
  `tests/release_fast_path_audit.rs` is 2,889 lines,
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,564 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines,
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 660 lines,
  `tests/release_fast_path_audit/unsafe_performance/bitmap_boundaries.rs` is
  206 lines,
  `tests/release_fast_path_audit/release_policy/security.rs` is 181 lines, and
  `tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs` is
  160 lines. The new size-class child module,
  `tests/release_fast_path_audit/unsafe_performance/size_class_boundaries.rs`
  is 132 lines.

## Issue: Memid test fixtures still hide unsafe migration debt

> **Comment:** `src/types/memid/tests.rs` is the smallest remaining allowlisted
> test file. It should be local proof code for memid views and constructors, not
> a file-level unsafe exception.

- [x] Migrate `src/types/memid/tests.rs` from file-level unsafe allowlisting to
      local `SAFETY[AUDIT:typed-memid-test-fixtures]` markers.
- [x] Keep fixtures stack-local and avoid widening helper visibility.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries.
- [x] Remove the file from `docs/unsafe-migration-allowlist.tsv`.
- [x] Regenerate unsafe inventory and lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::memid::tests --lib --all-features`,
`scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
`scripts/check-unsafe-inventory.py --write && scripts/check-unsafe-inventory.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Callback wrappers remain file-level unsafe allowlisted

> **Comment:** `src/types/callbacks.rs` was the smallest allowlisted production
> module. The unsafe operations are callback invocation boundaries and
> raw-layout wrappers, which are good candidates for
> operation-level audit markers.

- [x] Replace the file-level allowlist entry for `src/types/callbacks.rs` with
      operation-level `SAFETY[AUDIT:callback-wrappers]` markers.
- [x] Keep callback wrappers `repr(transparent)`/layout-compatible and avoid
      adding allocation or dynamic dispatch.
- [x] Add release-audit evidence for callback wrapper layout and invocation
      boundaries.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit-evidence.tsv`, generated
      unsafe inventory, and lower `ALLOWLIST_ENTRY_CAP` again.

Verify: callback unit tests, unsafe audit scripts,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Aligned still has the last broad strict-type usize allowance

> **Comment:** `src/allocator/aligned.rs` was the only remaining broad
> removable `bare-domain-usize` allowance. The file mixes raw C ABI shims,
> parser helpers, product-size paths, alignment arithmetic, and realloc helpers.

- [x] Temporarily remove
      `bare-domain-usize|src/allocator/aligned.rs|*|usize` and run
      `scripts/check-strict-types.py` to collect the live hits.
- [x] Classify each hit as raw ABI, alignment parser, product-size helper,
      pointer-provenance arithmetic, or migratable local helper.
- [x] Replace the broad `aligned` entry with symbol-level entries and typed
      scalar/layout wrappers where practical.
- [x] Lower `REMOVE_ENTRY_CAP` to 0 if the broad removable entry is eliminated.
- [x] Preserve allocation/reallocation hot-path shape and avoid heap allocation,
      trait-object dispatch, or public API drift.

Verify: `scripts/check-strict-types.py`, `scripts/check-ffi-boundary.py`,
`scripts/check-api-inventory.py`, `scripts/check-public-abi.py`,
`cargo test --test release_fast_path_audit --all-features`, full clippy, and
full tests.

## Issue: Release audit unsafe_performance.rs remains oversized

> **Comment:** `unsafe_performance.rs` is now 2,564 lines after the bitmap,
> thread-local, and size-class/bin splits. Another cohesive assertion group can
> move into a child module later if future audit additions need room.

- [x] Inventory assertion sections in `unsafe_performance.rs`, prioritizing
      size-class/bin, atomic-wrapper, or arena-prefix assertion clusters.
- [x] Move one cohesive cluster into a new or existing file under
      `tests/release_fast_path_audit/unsafe_performance/`.
- [x] Pass borrowed source strings rather than duplicating large file reads.
- [x] Update `RELEASE_AUDIT_MODULES` and this TODO evidence.
- [x] Preserve the release audit test count and coverage intent.

Verify: `cargo test --test release_fast_path_audit --all-features`,
`git diff --check`, full clippy, and full tests.

## Issue: Arena remains the largest translated production review unit

> **Comment:** The prefix split reduced `src/allocator/arena.rs` from 4,628 to
> 4,520 lines, but it is still the largest production review unit. The next
> split should target a cohesive helper group such as heap visit/delete helpers,
> arena id/memid helpers, or purge helpers if imports stay narrow.

- [ ] Re-scan `src/allocator/arena.rs` for a low-risk cohesive helper group,
      preferring heap visit/delete, arena id/memid, or purge helpers.
- [ ] Move the group into an `src/allocator/arena/` child module with explicit
      visibility only for current callers.
- [ ] Keep data layout private and avoid exposing translated prefix structs or
      arena internals outside the allocator arena module boundary.
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

1. [x] Migrate `src/types/memid/tests.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/types/callbacks.rs` off the unsafe migration allowlist.
3. [x] Narrow or eliminate the broad `aligned` strict-type `usize` allowance.
4. [x] Split one more release-audit assertion cluster.
5. [ ] Split one more cohesive helper group out of `src/allocator/arena.rs`.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is below 47 entries and the cap is lowered.
- [x] One additional production allowlisted file is covered by operation-level
      audit markers and evidence.
- [x] The final broad strict-type `remove:` allowance is narrowed or eliminated.
- [x] One release-audit module has more review margin.
- [ ] `src/allocator/arena.rs` is smaller again without API, ABI, or hot-path
      performance drift.
