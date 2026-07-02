# TODO: Fresh-Eyes Source-Marker, Release Module, Strict-Type, and Arena Module Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`83d42e1`. The implemented source audit marker cap is 17, release-audit module
cap is 200, strict-type file hotspots are capped at 45 active entries,
kind/file hotspots at 30 entries, raw-pointer file/domain hotspots at 25
entries, usize file/domain hotspots at 25 entries, and `src/allocator/page.rs`
is split below the 2,800-line production module cap. The next
mechanical-translation debt is concentrated in source-marker density,
release-audit module size, strict-type hotspot groups, and the largest
remaining translated production module.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 95 source files, 5,898 unsafe
  mentions, and 529 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 529 operation IDs with
  1,129 automated evidence rows.
- No source audit operation ID has more than 17 source markers. Eleven IDs sit
  at the 17-marker cap: `aligned-global-allocation-export-boundary`,
  `arena-initialization-boundary`, `arena-page-abandon-boundary`,
  `init-platform-tls-boundary`, `init-process-boundary`,
  `init-thread-default-heap-association-boundary`,
  `init-thread-heap-main-tls-boundary`, `internal-atomic-cas-boundary`,
  `macos-zone-allocation-boundary`, `safe-heap`, and
  `typed-arena-subproc-pointer-test-fixtures`. The next useful source marker
  cap is 16 entries.
- `strict-type-allowlist.txt` has 632 active entries: 330
  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40
  metadata-reference entries, 23 pointer-usize-roundtrip entries, 14 atomic-cast
  entries, and 5 heap-allocating-type entries. It has zero wildcard entries and
  zero removable entries.
- Compatibility snapshot: It has no retained wildcard
  entries.
- Strict-type wildcard guard: total=0; none.
- The largest strict-type allowlist file hotspots are `src/internal/mod.rs`
  with 45 active entries, `src/internal/bitmap.rs` with 37,
  `src/allocator/aligned.rs` with 36, `src/allocator/alloc.rs` with 35,
  `src/allocator/page.rs` with 33, and `src/allocator/thread_heap.rs` with 25.
  The active file hotspot cap remains 45 active entries; the next useful file
  hotspot cap is 40 active entries.
- The largest strict-type kind/file hotspots are `raw-pointer-parameter` in
  `src/allocator/alloc.rs` with 29 active entries, `raw-pointer-parameter` in
  `src/internal/mod.rs` with 29, `raw-pointer-parameter` in
  `src/allocator/page.rs` with 28,
  `bare-domain-usize` in `src/internal/bitmap.rs` with 25,
  `raw-pointer-parameter` in `src/allocator/thread_heap.rs` with 24, and
  `bare-domain-usize` in `src/types/scalar.rs` with 24. The active kind/file
  hotspot cap remains 30 active entries; the next useful kind/file cap is 25
  active entries.
- The largest raw-pointer domains are `mi_page_t` with 110 entries,
  `mi_theap_t` with 109, and `mi_heap_t` with 67.
- No strict-type permanent reason group exceeds 40 entries. The largest groups
  are internal page metadata helper boundary with 33 entries, C ABI parity or
  translated-C source shape with 33 entries, C/platform ABI forwarding boundary
  with 32 entries, aligned allocation compatibility boundary with 30 entries,
  thread-heap compatibility boundary with 24 entries, and scalar domain
  constructor boundary with 24 entries. The next useful permanent reason cap is
  32 entries.
- The largest strict-type raw-pointer file/domain hotspots are
  `src/allocator/alloc.rs`/`mi_theap_t` with 21 entries,
  `src/internal/mod.rs`/`mi_page_t` with 21,
  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20,
  `src/allocator/page.rs`/`mi_theap_t` with 19, and
  `src/allocator/free.rs`/`mi_page_t` with 19. The active raw-pointer
  file/domain cap remains 25 entries; the next useful raw-pointer file/domain
  cap is 20 entries.
- Across all strict-type file/domain pairs, the remaining non-raw hotspots are
  `src/internal/bitmap.rs`/`usize` with 25,
  `src/types/scalar.rs`/`usize` with 24, and
  `src/raw/macros.rs`/`usize` with 23. The active usize file/domain cap remains
  25 entries; the next useful
  usize file/domain cap is 24 entries.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 174 module files recursively and
  enforces a 200-line cap. The largest release-audit modules are:
  `unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 200 lines,
  `unsafe_performance/page_lifecycle.rs` at 200 lines,
  `unsafe_performance/page_arena_behavior/arena_page_allocation.rs` at 198 lines,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view/memory_kinds.rs` at 196 lines,
  `unsafe_performance/os_memory/os_wrappers.rs` at 195 lines,
  `unsafe_performance/allocation_free/typed_helpers.rs` at 193 lines,
  `unsafe_performance/raw_facade/raw_override/runtime_heap.rs` at 193 lines,
  and `datatype_acceptance/spec_reconciliation.rs` at 192 lines. No
  release-audit module currently exceeds 200 lines, but eight modules still
  exceed the next useful 190-line cap.
- Production source module size remains dominated by translated-C-shaped files:
  `src/allocator/arena.rs` at 3,159 lines, `src/allocator/init.rs` at 2,612,
  `src/allocator/page.rs` at 2,530, `src/internal/bitmap.rs` at 2,422,
  `src/platform/windows.rs` at 2,305, `src/raw/macros.rs` at 2,260,
  `src/allocator/stats.rs` at 2,182, and `src/allocator/alloc.rs` at 1,952.
  `src/allocator/arena.rs` is the only remaining production module above the
  next useful 3,000-line cap.

## Issue 1: Lower the source audit marker cap to 16

The remaining 17-marker operation IDs still group responsibilities broadly
enough to hide unrelated unsafe invariants. Split them by ownership, platform,
or fixture family until every source audit ID has at most 16 source markers.

- [ ] Split the 17-marker allocation, arena, init, internal atomic, macOS,
      safe-heap, and typed fixture operation IDs into narrower invariant
      families without adding aggregate fallback markers.
- [ ] Refresh `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`,
      `docs/unsafe-audit-evidence.tsv`, release-audit marker expectations, and
      TODO evidence after the source marker cap moves.
- [ ] Lower the source marker release-audit cap from 17 to 16.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 190 lines

The release-audit recursive guard can move from 200 lines to 190 lines after
splitting the largest assertion modules by invariant family.

- [ ] Split `unsafe_performance/lifecycle_boundaries/thread_tls.rs` and
      `unsafe_performance/page_lifecycle.rs` below 190 lines.
- [ ] Split the remaining modules above 190 lines by focused responsibility:
      arena page allocation, memid memory kinds, OS wrappers, allocation typed
      helpers, raw runtime heap overrides, and spec reconciliation.
- [ ] Lower the recursive release-audit module cap from 200 to 190 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Tighten strict-type and permanent-reason hotspot caps

Strict-type broadness is now concentrated in a smaller set of translated-C
files, kind/file groups, file/domain pairs, and two broad permanent reason
groups. Continue moving cohesive helper families into child modules or narrower
reason buckets instead of adding wildcard or broader permanent allowlist rows.

- [ ] Reduce strict-type allowlist file hotspots so no file exceeds 40 active
      entries.
- [ ] Reduce strict-type kind/file hotspots so no kind/file pair exceeds 25
      active entries.
- [ ] Reduce raw-pointer file/domain hotspots so no pair exceeds 20 entries.
- [ ] Reduce non-raw file/domain hotspots so no usize pair exceeds 24 entries.
- [ ] Reduce permanent reason groups so no group exceeds 32 entries without
      hiding genuinely distinct compatibility boundaries under vague labels.
- [ ] Refresh `docs/strict-types.md`, `strict-type-allowlist.txt`,
      release-audit guards, and TODO evidence after the measured caps move.

Verify: strict-type checker, release fast-path audit, no-allocation audit, full
formatting, full clippy, and full tests.

## Issue 4: Split the next largest production module under 3,000 lines

`src/allocator/arena.rs` is the only production module above the next useful
3,000-line cap. Keep arena ABI and hot-path behavior stable while moving
cohesive helper families out of the translated parent file.

- [ ] Extract arena slice geometry, page table lookup, registry iteration, or
      page allocation/free helper families where existing arena child modules
      can own them without broad re-exports.
- [ ] Refresh source-size evidence so `src/allocator/arena.rs` stays under
      3,000 lines without weakening ABI, no-allocation, unsafe-audit, or
      strict-type checks.
- [ ] Keep hot-path helpers monomorphic and allocation-free; do not introduce
      dynamic dispatch, heap allocation, or broad public re-exports to make the
      split compile.

Verify: release fast-path audit, no-allocation audit, unsafe audit scripts,
strict-type checker, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [ ] Lower the source audit marker cap to 16.
2. [ ] Split release-audit modules over 190 lines.
3. [ ] Tighten strict-type and permanent-reason hotspot caps.
4. [ ] Split the next largest production module under 3,000 lines.

## Definition of done for this backlog

- [ ] No source audit operation ID has more than 16 source markers.
- [ ] Release-audit modules are split enough to lower the recursive module cap
      to 190 lines without weakening discovery or measured TODO checks.
- [ ] Strict-type file hotspots are capped at 40 active entries, kind/file
      hotspots are capped at 25 active entries, raw-pointer file/domain hotspots
      are capped at 20 entries, usize file/domain hotspots are capped at 24
      entries, and permanent reason groups are capped at 32 entries.
- [ ] `src/allocator/arena.rs` is split below 3,000 lines while keeping raw ABI,
      no-allocation, unsafe-audit, and strict-type checks intact.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
