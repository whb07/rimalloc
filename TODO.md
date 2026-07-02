# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the unsafe-audit migration pass on `master`
at `da44c1a`. That pass moved `src/allocator/free.rs`,
`src/allocator/options.rs`, `src/allocator/aligned.rs`,
`src/allocator/stats.rs`, and `src/raw/macros.rs` off the unsafe migration
allowlist with local operation markers, audit evidence, release-audit guards,
and full validation.

This follow-up pass has now also moved `src/allocator/init.rs` and
`src/internal/bitmap.rs` off the unsafe migration allowlist with local operation
markers, audit evidence, release-audit guards, and focused behavior tests.

The next pass should finish the same class of work: retire every remaining
file-level unsafe migration allowance, one file at a time, while preserving raw
ABI shape, allocator ownership, synchronization, platform behavior, and hot-path
code shape. Avoid broad refactors. Prefer marker-only migrations unless a local
test exposes a concrete bug.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 5 entries: 3 production and
  2 translation files. `ALLOWLIST_ENTRY_CAP` is 5.
- The just-completed pass moved `src/allocator/free.rs` to 69
  `SAFETY[AUDIT:free-path-boundary]` markers, `src/allocator/options.rs` to 76
  `SAFETY[AUDIT:options-callback-boundary]` markers,
  `src/allocator/aligned.rs` to 158
  `SAFETY[AUDIT:aligned-allocation-boundary]` markers,
  `src/allocator/stats.rs` to 161
  `SAFETY[AUDIT:stats-reporting-boundary]` markers, and
  `src/raw/macros.rs` to 178
  `SAFETY[AUDIT:raw-macro-allocation-boundary]` markers.
- The current pass moved `src/allocator/init.rs` to 277
  `SAFETY[AUDIT:init-lifecycle-boundary]` markers and
  `src/internal/bitmap.rs` to 71
  `SAFETY[AUDIT:internal-bitmap-boundary]` markers.
- Remaining production allowlist entries with no local markers are
  `src/allocator/alloc.rs` at 2,559 lines with 333 `unsafe` hits,
  `src/allocator/page.rs` at 2,801 lines with 196 `unsafe` hits, and
  `src/allocator/arena.rs` at 3,915 lines with 607 `unsafe` hits.
- Remaining translation allowlist entries with no local markers are
  `src/internal/mod.rs` at 1,533 lines with 163 `unsafe` hits, and
  `src/platform/windows.rs` at 2,191 lines with 214 `unsafe` hits.
- `strict-type-allowlist.txt` still has 280 entries: 150 raw-pointer-parameter
  entries, 105 bare-domain `usize` entries, 13 pointer/usize round-trip
  entries, 8 metadata-reference entries, 2 atomic-cast entries, and
  2 heap-allocating-type entries. `REMOVE_ENTRY_CAP` remains 0.
- The broadest path-wide strict-type clusters are `src/allocator/aligned.rs`,
  `src/internal/mod.rs`, `src/platform/unix.rs`, `src/platform/windows.rs`,
  `src/allocator/stats.rs`, and `src/allocator/thread_heap.rs`, each with
  4 path-wide entries. `src/allocator/alloc.rs` and
  `src/allocator/init.rs` each still have 3 path-wide entries; init is no
  longer on the unsafe migration allowlist.
- The largest unsafe audit marker clusters remain broad enough to split later:
  `SAFETY[AUDIT:init-lifecycle-boundary]` has 277 markers,
  `SAFETY[AUDIT:raw-macro-allocation-boundary]` has 178 markers,
  `SAFETY[AUDIT:stats-reporting-boundary]` has 161 markers,
  `SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers,
  `SAFETY[AUDIT:aligned-allocation-boundary]` has 158 markers,
  `SAFETY[AUDIT:page-map-boundary]` has 153 markers,
  `SAFETY[AUDIT:os-memory-boundary]` has 136 markers,
  `SAFETY[AUDIT:internal-atomic-facade]` has 103 markers,
  `SAFETY[AUDIT:unix-primitive-backend]` has 90 markers,
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers,
  `SAFETY[AUDIT:macos-zone-override]` has 83 markers,
  `SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers,
  `SAFETY[AUDIT:options-callback-boundary]` has 76 markers,
  `SAFETY[AUDIT:thread-heap-lifecycle]` has 71 markers,
  `SAFETY[AUDIT:internal-bitmap-boundary]` has 71 markers,
  `SAFETY[AUDIT:raw-posix-compat]` has 70 markers, and
  `SAFETY[AUDIT:free-path-boundary]` has 69 markers.
- The release audit is split but still has large review units:
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,464 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,150 lines,
  `tests/release_fast_path_audit.rs` is 1,935 lines,
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,131 lines, and
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 831 lines.

## Completed: Process initialization boundary moved off the file-level unsafe allowance

> **Comment:** `src/allocator/init.rs` owns process and thread initialization,
> teardown, preloading state, subproc identity, default heap association,
> at-exit/constructor hooks, random reinitialization, CPU feature setup, and
> option/stat lifecycle entry points. It now has local operation markers,
> evidence rows, and lifecycle release-audit guards instead of a file-level
> migration allowance.

- [x] Add local `SAFETY[AUDIT:init-lifecycle-boundary]` markers for process
      init/done, thread init/done, subproc main/current/id conversion, preloading
      state, default thread-heap association, allocator constructor/destructor
      hooks, random and CPU feature initialization, and raw lifecycle exports.
- [x] Preserve idempotent process initialization, thread-local main-heap TLS,
      subproc identity mapping, weak random reinitialization, preloading gates,
      option/stat init order, abort/no-unwind behavior, and no allocator
      allocation before initialization is safe.
- [x] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, lifecycle helper shape,
      preloading behavior, default heap TLS behavior, constructor/destructor
      cfg shape, and random/CPU initialization order.
- [x] Add matching unsafe audit/evidence entries, remove
      `src/allocator/init.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: lifecycle release audit, runtime-gate tests, original lifecycle/override
tests, unsafe audit scripts, unsafe inventory regeneration/check, full clippy,
and full tests.

## Completed: Internal bitmap boundary moved off the file-level unsafe allowance

> **Comment:** `src/internal/bitmap.rs` owns translated bitmap field access,
> bit scanning, claim/clear loops, and arena bitmap traversal helpers. It now
> has local operation markers, evidence rows, release-audit guards, and focused
> tests instead of a file-level migration allowance.

- [x] Add local `SAFETY[AUDIT:internal-bitmap-boundary]` markers for bitmap
      field pointer projection, bit index arithmetic, claim/clear mutation,
      visited-range callbacks, arena bitmap helper entry points, and local test
      fixtures.
- [x] Preserve bit ordering, field width, bounds checks, first-fit search
      behavior, arena callback contracts, empty/full handling, and no hidden
      allocation or dynamic dispatch in bitmap search paths.
- [x] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, bit claim/clear behavior,
      out-of-range rejection, arena callback traversal, and strict-type bitmap
      aliases.
- [x] Add matching unsafe audit/evidence entries, remove
      `src/internal/bitmap.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: bitmap unit tests, bitmap release-audit module, strict-type checker,
unsafe audit scripts, unsafe inventory regeneration/check, full clippy, and full
tests.

## Issue: Hot allocation entry boundary still uses a file-level unsafe allowance

> **Comment:** `src/allocator/alloc.rs` owns malloc/calloc/realloc/product
> allocation paths, heap and thread-heap dispatch, zeroing behavior, allocation
> request construction, padding/provenance decisions, and raw allocation exports.
> It is a hot path, so migration must keep code-shape and no-allocation evidence.

- [ ] Add local `SAFETY[AUDIT:allocation-entry-boundary]` markers for malloc,
      calloc, realloc, recalloc, reallocf/reallocn, heap allocation, thread-heap
      allocation, product overflow, zero-size behavior, padding/provenance
      recovery, new/new-nothrow shims, and raw allocation exports.
- [ ] Preserve null/zero-size behavior, overflow detection, allocation failure
      semantics, realloc copy bounds, old-pointer validation, guarded and
      aligned handoff behavior, heap ownership, no hidden Rust allocation, and
      no dynamic dispatch on hot allocation paths.
- [ ] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, product overflow, realloc
      preservation, raw export signatures, no-allocation critical paths, and
      assembly/benchmark evidence for fast allocation helpers.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/allocator/alloc.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: allocation boundary tests, property/fuzz allocation sequences,
no-allocation critical-path tests, release fast-path audit, unsafe audit scripts,
unsafe inventory regeneration/check, full clippy, and full tests.

## Issue: Page allocation boundary still uses a file-level unsafe allowance

> **Comment:** `src/allocator/page.rs` owns page-local allocation, generic
> malloc refill, page queues, free-list use, stats gates, direct/normal/huge bin
> selection, retire/extend paths, and typed page helper integration. It is hot
> and should be migrated after allocation entry points.

- [ ] Add local `SAFETY[AUDIT:page-allocation-boundary]` markers for page-local
      malloc, generic refill, queue lookup, free-list pop/push, bin selection,
      retire/extend transitions, page stats updates, huge/small page handling,
      and raw page allocation exports.
- [ ] Preserve direct and normal bin classification, release-gated stats writes,
      page queue invariants, used/capacity counters, free-list ownership, retire
      and extend behavior, no hidden allocation, and force-inline/noinline
      performance annotations already guarded by release-audit tests.
- [ ] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, generic malloc helper shape,
      page stats gates, bin classification, page queue invariants, and
      no-allocation/performance evidence.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/allocator/page.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: page/bin unit tests, runtime-gate tests, release fast-path audit,
no-allocation critical-path tests, unsafe audit scripts, unsafe inventory
regeneration/check, full clippy, and full tests.

## Issue: Arena region boundary still uses a file-level unsafe allowance

> **Comment:** `src/allocator/arena.rs` is the largest remaining production
> allowlist entry. It owns arena registry state, abandoned page claims, slice
> mapping, arena allocation/free, purge traversal, page alloc/free/abandon
> shims, and subproc arena tables. It should be migrated after the smaller hot
> alloc/page boundaries are classified.

- [ ] Add local `SAFETY[AUDIT:arena-region-boundary]` markers for arena table
      access, arena metadata initialization, slice index/range projection,
      abandoned-page bitmap traversal, purge visitors, arena alloc/free,
      page alloc/free/abandon/unabandon/reabandon shims, and raw arena exports.
- [ ] Preserve arena index bounds, NUMA fields, memid ownership, abandoned-page
      claim/reclaim semantics, purge ordering, subproc arena table locking,
      page ownership transfer, OS memory interaction, and C ABI callback
      signatures.
- [ ] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, arena id/memid helpers, bitmap
      visitor shape, heap page visitor integration, page lifecycle shims, and
      arena purge/allocation behavior.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/allocator/arena.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: arena/page lifecycle release audits, arena metadata tests, original
arena/static override tests, unsafe audit scripts, unsafe inventory
regeneration/check, full clippy, and full tests.

## Issue: Internal translation boundary still uses a file-level unsafe allowance

> **Comment:** `src/internal/mod.rs` is a translation compatibility layer for
> internal helper macros, layout helpers, block/page pointer helpers, tracking
> gates, and packed-pointer/provenance conversions. It is not as hot as alloc or
> page, but it still hides translated C helper unsafety behind a file allowance.

- [ ] Add local `SAFETY[AUDIT:internal-translation-boundary]` markers for raw
      block/page helper access, packed pointer conversion, metadata references,
      internal layout and tracking helpers, debug/runtime gates, and translated
      compatibility shims.
- [ ] Preserve packed-pointer encoding, page/block helper contracts, debug assert
      gating, huge-allocation stats gates, valgrind/ASan tracking behavior,
      provenance helper use, and strict-type boundaries.
- [ ] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, runtime gate behavior,
      strict-type pointer/metadata aliases, packed-pointer round trips, and
      no broad lint/unsafe allowances.
- [ ] Add matching unsafe audit/evidence entries, remove `src/internal/mod.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: runtime-gate tests, datatype acceptance tests, strict-type checker,
unsafe audit scripts, unsafe inventory regeneration/check, full clippy, and full
tests.

## Issue: Windows primitive translation boundary still uses a file-level unsafe allowance

> **Comment:** `src/platform/windows.rs` is the last platform translation
> allowance. It owns Windows virtual memory, process information, timing,
> randomness, environment, dynamic library, huge-page, and thread/TLS primitive
> bindings. It should be migrated last because validation is more cfg-sensitive
> on non-Windows hosts.

- [ ] Add local `SAFETY[AUDIT:windows-primitive-boundary]` markers for Windows
      API declarations, virtual memory allocation/free/protect, commit/reset,
      process info, timing, randomness, environment lookup, dynamic library
      probes, huge pages, console/stderr output, and thread/TLS primitives.
- [ ] Preserve Windows ABI signatures, null/error conventions, page-size and
      allocation granularity semantics, overcommit/virtual-reserve behavior,
      huge-page feature gates, dynamic symbol lookup behavior, and non-Windows
      cfg isolation.
- [ ] Add or extend release-audit assertions and cfg-focused tests for marker
      coverage, no migration allowlist entry, Windows primitive symbol shape,
      platform documentation, and non-Windows build stability.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/platform/windows.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap to zero.

Verify: platform primitive release audits, cfg/build checks available on the
host, unsafe audit scripts, unsafe inventory regeneration/check, full clippy,
and full tests.

## Suggested implementation order

1. [x] Migrate `src/allocator/init.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/internal/bitmap.rs` off the unsafe migration allowlist.
3. [ ] Migrate `src/allocator/alloc.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/allocator/page.rs` off the unsafe migration allowlist.
5. [ ] Migrate `src/allocator/arena.rs` off the unsafe migration allowlist.
6. [ ] Migrate `src/internal/mod.rs` off the unsafe migration allowlist.
7. [ ] Migrate `src/platform/windows.rs` off the unsafe migration allowlist.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file and
      reaches zero when the final entry is removed.
- [ ] The completed init and bitmap migrations plus all 5 remaining
      allowlisted files are covered by operation-level audit markers, source
      audit entries, evidence rows, release-audit guards, and focused behavior
      tests instead of file-level migration exceptions.
- [ ] Allocator initialization, bitmap traversal, allocation hot paths, page
      allocation, arena ownership, internal translation helpers, and Windows
      platform primitives preserve their raw ABI contracts and performance
      constraints.
- [ ] After the final item, rescan the codebase with fresh eyes for the next
      class of mechanical-translation debt, rewrite `TODO.md`, and set the next
      goal before closing the loop.
