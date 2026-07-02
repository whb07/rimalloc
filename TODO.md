# TODO: Fresh-Eyes Audit Density, Release Module, Strict-Type, and Page Module Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`eff9713`. The implemented source audit marker cap is 18, release-audit module
cap is 220, strict-type permanent reason groups remain capped at 40 entries,
and `src/allocator/arena.rs` is split below the 3,500-line production module
cap. The next mechanical-translation debt is concentrated in audit-density
ceilings and the largest remaining translated production modules.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 90 source files, 5,898 unsafe
  mentions, and 524 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 524 operation IDs with
  1,119 automated evidence rows.
- No source audit operation ID has more than 18 source markers. Three IDs sit at
  the 18-marker cap: `arena-public-reserve-boundary`,
  `os-memory-aligned-allocation-boundary`, and
  `typed-callback-invocation-test-fixtures`. The next useful source marker cap is 17
  entries.
- `strict-type-allowlist.txt` has 632 active entries: 330
  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40
  metadata-reference entries, 23 pointer-usize-roundtrip entries, 14 atomic-cast
  entries, and 5 heap-allocating-type entries. It has zero wildcard entries and
  zero removable entries.
- Compatibility snapshot: It has no retained wildcard
  entries.
- Strict-type wildcard guard: total=0; none.
- The largest strict-type allowlist file hotspots are `src/allocator/aligned.rs`
  with 50 active entries, `src/internal/mod.rs` with 49,
  `src/allocator/alloc.rs` with 46, `src/allocator/page.rs` with 45,
  `src/internal/bitmap.rs` with 37, and `src/raw/macros.rs` with 30. The next
  useful file hotspot cap is 45 active entries.
- The largest strict-type kind/file hotspots are `raw-pointer-parameter` in
  `src/allocator/alloc.rs` with 40 active entries, `raw-pointer-parameter` in
  `src/allocator/page.rs` with 40, `raw-pointer-parameter` in
  `src/internal/mod.rs` with 33, `raw-pointer-parameter` in `src/allocator/aligned.rs` with 30, `bare-domain-usize` in `src/raw/macros.rs`
  with 30, and `bare-domain-usize` in `src/internal/bitmap.rs` with 25. The
  next useful kind/file cap is 30 active entries.
- The largest raw-pointer domains are `mi_page_t` with 110 entries,
  `mi_theap_t` with 109, and `mi_heap_t` with 67.
- No strict-type permanent reason group exceeds 40 entries. The largest groups
  are internal page metadata helper boundary with 33 entries, C ABI parity or
  translated-C source shape with 33 entries, C/platform ABI forwarding boundary
  with 32 entries, aligned allocation compatibility boundary with 30 entries,
  thread-heap compatibility boundary with 24 entries, and scalar domain
  constructor boundary with 24 entries.
- The largest strict-type raw-pointer file/domain hotspots are
  `src/allocator/alloc.rs`/`mi_theap_t` with 27 entries,
  `src/internal/mod.rs`/`mi_page_t` with 25,
  `src/allocator/page.rs`/`mi_page_t` with 21,
  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20,
  `src/allocator/free.rs`/`mi_page_t` with 19, and
  `src/allocator/page.rs`/`mi_theap_t` with 19. The next useful raw-pointer
  file/domain cap is 25 entries.
- Across all strict-type file/domain pairs, the remaining non-raw hotspots are
  `src/raw/macros.rs`/`usize` with 30,
  `src/internal/bitmap.rs`/`usize` with 25, and
  `src/types/scalar.rs`/`usize` with 24. The next useful usize file/domain cap
  is 25 entries.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 164 module files recursively and
  enforces a 220-line cap. The largest release-audit modules are:
  `unsafe_performance/allocation_free/marker_counts.rs` at 219 lines,
  `unsafe_performance/arena_boundaries.rs` at 217 lines,
  `unsafe_performance/thread_stats.rs` at 216 lines,
  `release_policy/benchmarking.rs` at 209 lines,
  `unsafe_performance/allocation_free/typed_helpers.rs` at 209 lines,
  `unsafe_performance/local_boundaries.rs` at 208 lines,
  `datatype_acceptance/strict_type/checker_allowlist_symbols.rs` at 205 lines,
  `unsafe_performance/platform_primitive_boundaries/macos.rs` at 204 lines,
  `datatype_acceptance/domain_wrappers.rs` at 201 lines, and
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs` at
  200 lines. No release-audit module currently exceeds 220 lines, but nine
  modules still exceed the next useful 200-line cap.
- Production source module size remains dominated by translated-C-shaped files:
  `src/allocator/arena.rs` at 3,159 lines, `src/allocator/page.rs` at 2,855,
  `src/allocator/init.rs` at 2,612, `src/internal/bitmap.rs` at 2,422,
  `src/allocator/alloc.rs` at 2,419, and `src/raw/macros.rs` at 2,330. The next
  useful production module cap is 2,800 lines, starting with `page.rs`.

## Issue 1: Lower the source audit marker cap to 17

The remaining 18-marker operation IDs are still broad enough to hide unrelated
unsafe responsibilities. Split them by responsibility so each audit ID maps to a
narrower local invariant family.

- [ ] Split `arena-public-reserve-boundary` into narrower reserve/manage and
      huge-page registration operation families where the moved child modules
      already expose separate responsibilities.
- [ ] Split `os-memory-aligned-allocation-boundary` by allocation, alignment,
      and fallback responsibility without weakening OS memid evidence.
- [ ] Split `typed-callback-invocation-test-fixtures` by callback family so test
      fixtures stay reviewable.
- [ ] Lower the source marker release-audit cap from 18 to 17 and refresh
      `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, generated evidence, and TODO
      evidence.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 200 lines

The release-audit recursive guard can now move from 220 lines to 200 lines if
the nine remaining modules above that threshold are split by invariant family.

- [ ] Split allocation-free marker-count and typed-helper assertions into
      smaller marker-family modules.
- [ ] Split arena boundary, thread stats, local boundary, benchmarking, macOS
      primitive, strict-type symbol, domain-wrapper, and memid-view assertions
      by focused responsibility.
- [ ] Lower the recursive release-audit module cap from 220 to 200 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Tighten strict-type hotspot caps again

Strict-type broadness is now concentrated in a few translated-C-shaped files and
kind/file groups. Continue splitting helper families into child modules rather
than adding wildcard or broader permanent allowlist rows.

- [ ] Reduce strict-type allowlist file hotspots so no file exceeds 45 active
      entries.
- [ ] Reduce strict-type kind/file hotspots so no kind/file pair exceeds 30
      active entries.
- [ ] Reduce raw-pointer file/domain hotspots so no pair exceeds 25 entries.
- [ ] Reduce non-raw file/domain hotspots so no usize pair exceeds 25 entries.
- [ ] Refresh `docs/strict-types.md`, `strict-type-allowlist.txt`,
      release-audit guards, and TODO evidence after the measured caps move.

Verify: strict-type checker, release fast-path audit, no-allocation audit, full
formatting, full clippy, and full tests.

## Issue 4: Split the next largest production module under 2,800 lines

`src/allocator/page.rs` is now the largest remaining production module above
the next useful cap. Keep page ABI and hot-path behavior stable while moving
cohesive helper families out of the translated parent file.

- [ ] Extract page lifecycle, queue/bin, or validation helper families from
      `src/allocator/page.rs` where existing child modules can own them without
      broad re-exports.
- [ ] Refresh source-size evidence so `src/allocator/page.rs` stays under 2,800
      lines without weakening ABI, no-allocation, unsafe-audit, or strict-type
      checks.
- [ ] Keep hot-path helpers monomorphic and allocation-free; do not introduce
      dynamic dispatch, heap allocation, or broad public re-exports to make the
      split compile.

Verify: release fast-path audit, no-allocation audit, unsafe audit scripts,
strict-type checker, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [ ] Lower the source audit marker cap to 17.
2. [ ] Split release-audit modules over 200 lines.
3. [ ] Tighten strict-type hotspot caps again.
4. [ ] Split the next largest production module under 2,800 lines.

## Definition of done for this backlog

- [ ] No source audit operation ID has more than 17 source markers.
- [ ] Release-audit modules are split enough to lower the recursive module cap
      to 200 lines without weakening discovery or measured TODO checks.
- [ ] Strict-type file hotspots are capped at 45 active entries, kind/file
      hotspots are capped at 30 active entries, raw-pointer file/domain hotspots
      are capped at 25 entries, and usize file/domain hotspots are capped at 25
      entries.
- [ ] `src/allocator/page.rs` is split below 2,800 lines while keeping raw ABI,
      no-allocation, unsafe-audit, and strict-type checks intact.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
