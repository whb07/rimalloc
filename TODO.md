# TODO: Fresh-Eyes Audit Density, Module Cap, Strict-Type Hotspot, and Production Module Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`8f8cb27`, then completing the source-audit density and release-audit module
split items in this backlog. The implemented source audit marker cap is now 18,
release-audit module cap is now 220, and strict-type permanent reason groups
remain capped at 40 entries. Strict-type file, kind/file, raw-pointer
file/domain, and usize file/domain hotspots now meet their tighter caps. The
remaining mechanical-translation debt is concentrated in the largest translated
production module.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 88 source files, 5,898 unsafe
  mentions, and 524 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 524 operation IDs with
  1,119 automated evidence rows.
- No source audit operation ID has more than 18 source markers. Three IDs sit at
  the 18-marker cap: `typed-callback-invocation-test-fixtures`,
  `os-memory-aligned-allocation-boundary`, and
  `arena-public-reserve-boundary`. The next useful source marker cap is 17
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
  `src/internal/bitmap.rs` with 37, and `src/raw/macros.rs` with 30.
- The largest strict-type kind/file hotspots are `raw-pointer-parameter` in
  `src/allocator/alloc.rs` with 40 active entries, `raw-pointer-parameter` in
  `src/allocator/page.rs` with 40, `raw-pointer-parameter` in
  `src/internal/mod.rs` with 33, `bare-domain-usize` in `src/raw/macros.rs`
  with 30, `raw-pointer-parameter` in `src/allocator/aligned.rs` with 30, and
  `bare-domain-usize` in `src/internal/bitmap.rs` with 25.
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
  `src/allocator/page.rs`/`mi_page_t` with 21, and
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
  `unsafe_performance/thread_stats.rs` at 216 lines,
  `unsafe_performance/allocation_free/typed_helpers.rs` at 209 lines,
  `release_policy/benchmarking.rs` at 209 lines,
  `unsafe_performance/local_boundaries.rs` at 208 lines,
  `unsafe_performance/arena_boundaries.rs` at 207 lines,
  `unsafe_performance/platform_primitive_boundaries/macos.rs` at 204 lines,
  `datatype_acceptance/strict_type/checker_allowlist_symbols.rs` at 203 lines,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs` at
  201 lines, and `datatype_acceptance/domain_wrappers.rs` at 201 lines. No
  release-audit module currently exceeds 220 lines, but ten modules still
  exceed the next useful 200-line cap.
- Production source module size remains dominated by translated-C-shaped files:
  `src/allocator/arena.rs` at 4,255 lines, `src/allocator/page.rs` at 2,855,
  `src/allocator/init.rs` at 2,612, `src/allocator/alloc.rs` at 2,440,
  `src/internal/bitmap.rs` at 2,422, and `src/raw/macros.rs` at 2,330. The next
  useful production module cap is 3,500 lines, starting with `arena.rs`.

## Issue 1: Lower the source audit marker cap to 18

The source audit was mechanically complete, but too many operation IDs still
covered broad families. The broad 20- and 19-marker groups were split by
responsibility so reviewers can reason about narrower unsafe boundaries.

- [x] Split the six 20-marker operation IDs into smaller source-audit families:
      `stats-json-boundary`, `raw-macro-unix-aligned-posix-boundary`,
      `raw-macro-linux-libc-boundary`,
      `internal-atomic-platform-lock-boundary`, `emscripten-memory-boundary`,
      and `allocation-string-path-boundary`.
- [x] Split the nine 19-marker operation IDs by platform, callback/test
      fixture, stats, page-map, OS commit, and bitmap-range responsibilities.
- [x] Lower the source marker release-audit cap from 20 to 18 and refresh
      `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, generated evidence, and TODO
      evidence.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 220 lines

The release-audit modules had eight files above the next useful 220-line cap.
Those assertions are now split by operation family so the recursive guard can
enforce the tighter cap without weakening release-audit discovery.

- [x] Split `unsafe_performance/os_memory/platform_helpers.rs` by allocation,
      commit/decommit, and platform-helper evidence.
- [x] Split platform primitive boundary modules for Emscripten and Windows so
      OS-specific assertions live in focused child modules.
- [x] Split page-arena purge-clock and strict-type atomic/provenance guards by
      invariant family.
- [x] Split allocation contracts and raw facade API-surface guards by contract
      family.
- [x] Split process lifecycle subproc fast-path assertions away from init/done
      marker checks.
- [x] Lower the recursive release-audit module cap from 240 to 220 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Tighten strict-type hotspot caps

The permanent reason groups are capped at 40, and the allowlist hotspots now
meet the tighter file, kind/file, raw file/domain, and usize file/domain caps.
The split kept the compatibility rows explicit by moving narrow helper families
into focused child modules instead of broadening allowlist entries.

- [x] Reduce strict-type allowlist file hotspots so no file exceeds 50 active
      entries.
- [x] Reduce strict-type kind/file hotspots so no kind/file pair exceeds 40
      active entries.
- [x] Reduce raw-pointer file/domain hotspots so no pair exceeds 30 entries.
- [x] Reduce non-raw file/domain hotspots so no usize pair exceeds 30 entries.
- [x] Refresh `docs/strict-types.md`, `strict-type-allowlist.txt`,
      release-audit guards, and TODO evidence after the measured caps move.

Verify: strict-type checker, release fast-path audit, no-allocation audit, full
formatting, full clippy, and full tests.

## Issue 4: Split the largest production module under 3,500 lines

`src/allocator/arena.rs` is still a 4,255-line translated-C-shaped module even
after earlier helper extraction. Start the production module split with the
largest file and keep the exported/raw ABI surface stable.

- [ ] Extract reserve/manage-OS-memory entrypoints and huge-page reservation
      helpers from `src/allocator/arena.rs` into focused child modules.
- [ ] Extract purge/collect scheduling helpers from `src/allocator/arena.rs`
      where doing so does not broaden visibility or hide unsafe invariants.
- [ ] Add or refresh release-audit/source-size evidence so
      `src/allocator/arena.rs` stays under 3,500 lines without weakening ABI
      compatibility checks.
- [ ] Keep hot-path helpers monomorphic and allocation-free; do not introduce
      dynamic dispatch, heap allocation, or broad public re-exports to make the
      split compile.

Verify: release fast-path audit, no-allocation audit, unsafe audit scripts,
strict-type checker, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [x] Lower the source audit marker cap to 18.
2. [x] Split release-audit modules over 220 lines.
3. [x] Tighten strict-type hotspot caps.
4. [ ] Split the largest production module under 3,500 lines.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 18 source markers.
- [x] Release-audit modules are split enough to lower the recursive module cap
      to 220 lines without weakening discovery or measured TODO checks.
- [x] Strict-type file hotspots are capped at 50 active entries, kind/file
      hotspots are capped at 40 active entries, raw-pointer file/domain hotspots
      are capped at 30 active entries, and usize file/domain hotspots are capped
      at 30 active entries.
- [ ] `src/allocator/arena.rs` is split below 3,500 lines while keeping raw ABI,
      no-allocation, unsafe-audit, and strict-type checks intact.
- [x] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
