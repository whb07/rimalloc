# TODO: Fresh-Eyes Audit Density, Release Module, Strict-Type, and Page Module Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`eff9713`. The implemented source audit marker cap is 17, release-audit module
cap is 200, strict-type file hotspots are capped at 45 active entries,
kind/file hotspots at 30 entries, raw-pointer file/domain hotspots at 25
entries, usize file/domain hotspots at 25 entries, and `src/allocator/page.rs`
is split below the 2,800-line production module cap. The next mechanical-translation debt is concentrated in audit-density
ceilings and the largest remaining translated production modules.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 90 source files, 5,898 unsafe
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
  The active file hotspot cap is now 45 active entries.
- The largest strict-type kind/file hotspots are `raw-pointer-parameter` in
  `src/allocator/alloc.rs` with 29 active entries, `raw-pointer-parameter` in
  `src/internal/mod.rs` with 29, `raw-pointer-parameter` in
  `src/allocator/page.rs` with 28,
  `bare-domain-usize` in `src/internal/bitmap.rs` with 25,
  `bare-domain-usize` in `src/types/scalar.rs` with 24, and
  `raw-pointer-parameter` in `src/allocator/thread_heap.rs` with 24.
  The active kind/file hotspot cap is now 30 active entries.
- The largest raw-pointer domains are `mi_page_t` with 110 entries,
  `mi_theap_t` with 109, and `mi_heap_t` with 67.
- No strict-type permanent reason group exceeds 40 entries. The largest groups
  are internal page metadata helper boundary with 33 entries, C ABI parity or
  translated-C source shape with 33 entries, C/platform ABI forwarding boundary
  with 32 entries, aligned allocation compatibility boundary with 30 entries,
  thread-heap compatibility boundary with 24 entries, and scalar domain
  constructor boundary with 24 entries.
- The largest strict-type raw-pointer file/domain hotspots are
  `src/allocator/alloc.rs`/`mi_theap_t` with 21 entries,
  `src/internal/mod.rs`/`mi_page_t` with 21,
  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20,
  `src/allocator/free.rs`/`mi_page_t` with 19, and
  `src/allocator/page.rs`/`mi_theap_t` with 19. The active raw-pointer
  file/domain cap is now 25 entries.
- Across all strict-type file/domain pairs, the remaining non-raw hotspots are
  `src/internal/bitmap.rs`/`usize` with 25,
  `src/types/scalar.rs`/`usize` with 24, and `src/raw/macros.rs`/`usize` with 23.
  The active usize file/domain cap is now 25 entries.
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
  `datatype_acceptance/spec_reconciliation.rs` at 192 lines, and
  `todo_backlog.rs` at 191 lines. No release-audit module currently exceeds
  200 lines, but nine modules still exceed the next useful 190-line cap.
- Production source module size remains dominated by translated-C-shaped files:
  `src/allocator/arena.rs` at 3,159 lines, `src/allocator/init.rs` at 2,612,
  `src/allocator/page.rs` at 2,530, `src/internal/bitmap.rs` at 2,422,
  `src/raw/macros.rs` at 2,260, and `src/allocator/alloc.rs` at 1,952.
  `src/allocator/page.rs` is now below the 2,800-line cap after extracting
  lifecycle/free-list helpers to `src/allocator/page/lifecycle.rs`.

## Issue 1: Lower the source audit marker cap to 17

The remaining 18-marker operation IDs were broad enough to hide unrelated
unsafe responsibilities. They are now split by responsibility so each audit ID
maps to a narrower local invariant family.

- [x] Split the old arena public reserve/manage boundary into
      `arena-public-reserve-entry-boundary` and
      `arena-managed-os-memory-boundary`; huge-page registration remains under
      the existing `arena-huge-reservation-boundary`.
- [x] Split the old aligned OS allocation boundary into
      `os-memory-aligned-request-boundary`,
      `os-memory-aligned-fallback-boundary`, and
      `os-memory-offset-allocation-boundary`.
- [x] Split the old callback invocation test fixture boundary into basic,
      heap/block visit, and thread-local claim callback families.
- [x] Lower the source marker release-audit cap from 18 to 17 and refresh
      `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, generated evidence, and TODO
      evidence.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 200 lines

The release-audit recursive guard has moved from 220 lines to 200 lines after
splitting the largest modules by invariant family.

- [x] Split allocation-free marker-count and typed-helper assertions into
      smaller marker-family modules.
- [x] Split arena boundary, thread stats, local boundary, benchmarking, macOS
      primitive, strict-type symbol, domain-wrapper, and memid-view assertions
      by focused responsibility.
- [x] Lower the recursive release-audit module cap from 220 to 200 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Tighten strict-type hotspot caps again

Strict-type broadness is now concentrated in a few translated-C-shaped files and
kind/file groups. Continue splitting helper families into child modules rather
than adding wildcard or broader permanent allowlist rows.

- [x] Reduce strict-type allowlist file hotspots so no file exceeds 45 active
      entries.
- [x] Reduce strict-type kind/file hotspots so no kind/file pair exceeds 30
      active entries.
- [x] Reduce raw-pointer file/domain hotspots so no pair exceeds 25 entries.
- [x] Reduce non-raw file/domain hotspots so no usize pair exceeds 25 entries.
- [x] Refresh `docs/strict-types.md`, `strict-type-allowlist.txt`,
      release-audit guards, and TODO evidence after the measured caps move.

Verify: strict-type checker, release fast-path audit, no-allocation audit, full
formatting, full clippy, and full tests.

## Issue 4: Split the next largest production module under 2,800 lines

`src/allocator/page.rs` is now the largest remaining production module above
the next useful cap. Keep page ABI and hot-path behavior stable while moving
cohesive helper families out of the translated parent file.

- [x] Extract page lifecycle, queue/bin, or validation helper families from
      `src/allocator/page.rs` where existing child modules can own them without
      broad re-exports.
- [x] Refresh source-size evidence so `src/allocator/page.rs` stays under 2,800
      lines without weakening ABI, no-allocation, unsafe-audit, or strict-type
      checks.
- [x] Keep hot-path helpers monomorphic and allocation-free; do not introduce
      dynamic dispatch, heap allocation, or broad public re-exports to make the
      split compile.

Verify: release fast-path audit, no-allocation audit, unsafe audit scripts,
strict-type checker, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [x] Lower the source audit marker cap to 17.
2. [x] Split release-audit modules over 200 lines.
3. [x] Tighten strict-type hotspot caps again.
4. [x] Split the next largest production module under 2,800 lines.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 17 source markers.
- [x] Release-audit modules are split enough to lower the recursive module cap
      to 200 lines without weakening discovery or measured TODO checks.
- [x] Strict-type file hotspots are capped at 45 active entries, kind/file
      hotspots are capped at 30 active entries, raw-pointer file/domain hotspots
      are capped at 25 entries, and usize file/domain hotspots are capped at 25
      entries.
- [x] `src/allocator/page.rs` is split below 2,800 lines while keeping raw ABI,
      no-allocation, unsafe-audit, and strict-type checks intact.
- [x] `TODO.md` and its release-audit guard reflect the current measured counts.
- [x] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
