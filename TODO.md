# TODO: Next Strict-Type and Audit Granularity Backlog

Fresh-eyes review after refreshing audit evidence on `master` at `53db80e`.
The previous backlog is complete and pushed. The next mechanical translation
debt is in remaining strict-type wildcard allowances, broad unsafe operation
IDs, and a root release-audit harness that is still close to its cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` still covers 82 source files, 5,893 unsafe
  mentions, and 96 operation IDs. The migration allowlist remains empty.
- `strict-type-allowlist.txt` has 26 wildcard entries left: 4
  raw-pointer-parameter entries, 13 bare-domain `usize` entries, 4
  pointer/usize round-trip entries, 2 metadata-reference entries, 2 atomic-cast
  entries, and 1 heap-allocating-type entry.
- No remaining path has more than one wildcard entry; the residual path-level
  entries are public/raw ABI, platform ABI, proof-fixture, or typed-domain
  constructor boundaries such as `src/allocator/heap.rs`,
  `src/platform/mod.rs`, `src/raw/macros.rs`, `src/types/layout.rs`,
  `src/safe/allocation.rs`, and `src/types/atomic.rs`.
- The largest remaining unsafe audit marker clusters are
  `allocation-entry-boundary` with 166 markers,
  `stats-reporting-boundary` with 161 markers,
  `typed-pointer-accessors` with 161 markers,
  `aligned-allocation-boundary` with 158 markers,
  `page-map-boundary` with 153 markers,
  `os-memory-boundary` with 136 markers,
  `init-thread-tls-boundary` with 129 markers, and
  `windows-primitive-boundary` with 114 markers.
- The previous raw macro aggregate is split into
  `raw-macro-libc-forwarding-boundary` with 65 markers,
  `raw-macro-cxx-boundary` with 46 markers,
  `raw-macro-override-entry-boundary` with 36 markers,
  `raw-macro-bootstrap-boundary` with 15 markers,
  `raw-macro-allocation-query-boundary` with 10 markers, and
  `raw-macro-header-forwarding-boundary` with 6 markers.
- Audit evidence rows for allocation, aligned allocation, split raw macro IDs,
  OS memory, page-map, stats, and typed pointer checks now point at focused
  release-audit modules instead of the old aggregate harness paths.
- The release-audit module cap is 1,000 lines, but the root harness is still
  1,935 lines. The largest split modules are
  `unsafe_performance/lifecycle_boundaries.rs` at 922 lines,
  `unsafe_performance/platform_primitive_boundaries.rs` at 895 lines,
  `unsafe_performance/raw_facade.rs` at 826 lines, and
  `unsafe_performance/page_arena_behavior.rs` at 724 lines.

## Issue 1: Refresh audit evidence paths after the release-audit split

> **Comment:** The release-audit assertions now live in focused modules, but
> some evidence rows still cite the old harness files. That weakens review
> precision and makes future evidence updates harder to trace.

- [x] Update `docs/unsafe-audit-evidence.tsv` rows for allocation, aligned
      allocation, raw macro allocation, OS memory, page-map, stats, and typed
      pointer checks to name the focused release-audit modules that now own the
      assertions.
- [x] Update `UNSAFE_AUDIT.md` and `docs/unsafe-audit.md` wording where it still
      describes broad release-audit harness ownership instead of focused
      modules.
- [x] Add or update release-audit assertions so evidence rows cannot drift back
      to stale root harness paths when a focused module exists.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Narrow the remaining allocator/init strict-type wildcards

> **Comment:** The checker is item-aware now, but the highest-risk allocator
> paths still retain path-level strict-type entries.

- [x] Replace wildcard strict-type entries in `src/allocator/alloc.rs`,
      `src/allocator/free.rs`, and `src/allocator/init.rs` with named helper or
      documented ABI-boundary entries wherever the checker can identify the
      responsible item.
- [x] Narrow the remaining paired wildcard entries in `src/bitmap.rs`,
      `src/allocator/page_map.rs`, `src/types/ptr.rs`,
      `src/allocator/options.rs`, and `src/types/arena.rs`.
- [x] Preserve true public ABI, platform ABI, proof-fixture, and typed-domain
      constructor boundaries as explicit documented exceptions instead of
      broad accidental fallbacks.

Verify: strict-type checker, datatype acceptance audit, release fast-path audit,
no-allocation audit, full clippy, and full tests.

## Issue 3: Split the raw macro allocation audit cluster

> **Comment:** The previous `SAFETY[AUDIT:raw-macro-allocation-boundary]`
> cluster had 178 markers in the raw allocation override and forwarding layer.
> It mixed bootstrap allocation, malloc/free/realloc shims, C header macro
> behavior, and platform forwarding contracts.

- [x] Split raw macro markers into narrower operation IDs for bootstrap
      tracking, allocator override entry points, libc/posix forwarding,
      usability/size queries, and C header macro compatibility.
- [x] Update audit docs and evidence rows with separate invariants and tests for
      each new raw macro operation ID.
- [x] Preserve exported allocator symbols, no hidden Rust allocation before init,
      downstream header macro expansion, and Unix bootstrap allocation behavior.

Verify: header macro expansion tests, raw macro unit tests, release fast-path
audit, unsafe audit scripts, unsafe evidence scripts, full clippy, and full
tests.

## Issue 4: Split allocation, aligned allocation, stats, page-map, and OS audit clusters

> **Comment:** Several production clusters are still above 130 markers and each
> mixes multiple invariants. They should be split only where the code already
> has clear boundaries so performance-sensitive paths stay stable.

- [ ] Split `allocation-entry-boundary` into allocation ABI entry, heap/thread
      heap dispatch, layout/product request, and page refill handoff IDs.
- [ ] Split `aligned-allocation-boundary` into alignment request normalization,
      padding shrink/grow, heap aligned allocation, and realloc/recalloc IDs.
- [ ] Split `stats-reporting-boundary`, `page-map-boundary`, and
      `os-memory-boundary` into review units that match their helper groups and
      update evidence for each new operation ID.

Verify: allocation boundary tests, stats/lifecycle exports, release fast-path
audit, unsafe audit scripts, unsafe evidence scripts, full clippy, and full
tests.

## Issue 5: Split typed pointer accessor audit coverage

> **Comment:** `SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers in
> `src/types/ptr.rs`. The module is doing useful work, but page, block, heap,
> thread-heap, TLD, arena, subproc, page-queue, and provenance helpers are
> separate invariants.

- [ ] Split typed pointer accessor markers by pointer family and provenance
      helper responsibility.
- [ ] Keep layout/niche tests and strict-type allowlist coverage aligned with
      the new operation IDs.
- [ ] Preserve transparent wrapper layout, hot-path inlining, and no allocator
      allocation in strict-type wrappers.

Verify: pointer/layout tests, datatype acceptance audit, strict-type checker,
unsafe audit scripts, unsafe evidence scripts, full clippy, and full tests.

## Issue 6: Split the release fast-path root harness and lower the root cap

> **Comment:** Module caps are now below 1,000 lines, but
> `tests/release_fast_path_audit.rs` is still 1,935 lines and mixes shared
> helpers with unrelated policy checks.

- [ ] Move root release-audit tests into focused modules for public facade
      shape, safe allocation API behavior, interposition/export checks,
      strict-type/no-allocation policy, and allowance inventories.
- [ ] Keep only shared source collection helpers and module declarations in the
      root harness.
- [ ] Lower the root line-count cap after the split while preserving the 1,000
      line module cap.

Verify: release fast-path audit, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [x] Refresh audit evidence paths after the release-audit split.
2. [x] Narrow the remaining allocator/init strict-type wildcards.
3. [x] Split the raw macro allocation audit cluster.
4. [ ] Split allocation, aligned allocation, stats, page-map, and OS audit
       clusters.
5. [ ] Split typed pointer accessor audit coverage.
6. [ ] Split the release fast-path root harness and lower the root cap.

## Definition of done for this backlog

- [x] Unsafe audit evidence rows point at the focused tests that actually guard
      each operation ID.
- [x] Remaining strict-type wildcard entries are either narrowed to named items
      or documented as true ABI, public API, fixture, or typed-constructor
      boundaries.
- [ ] The largest unsafe audit operation IDs are split into narrower review
      units with separate docs, evidence, and release-audit guards.
- [ ] The release fast-path root harness is split enough to lower its cap
      without weakening coverage.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
