# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`2f705ab`. The last pass migrated `src/raw/posix.rs`, `src/lib.rs`,
`src/ffi/libc.rs`, `src/allocator/threadlocal.rs`, and
`src/allocator/heap.rs` off the unsafe migration allowlist while preserving
POSIX/libc compatibility behavior, crate-root public API shape,
thread-local synchronization, heap lifecycle ownership, and allocator
hot-path constraints.

The next pass should keep the same discipline: retire one measurable exception
at a time, prefer local operation-level proof markers over file allowances, and
avoid refactors that change allocator ownership, synchronization, platform ABI,
or allocation fast paths.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 22 entries: 19 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 22.
- The completed pass moved `src/raw/posix.rs`, `src/lib.rs`,
  `src/ffi/libc.rs`, `src/allocator/threadlocal.rs`, and
  `src/allocator/heap.rs` to local `SAFETY[AUDIT:*]` operation markers with
  evidence rows and no migration allowlist entries.
- The smallest remaining allowlisted production files are
  `src/platform/macos.rs` at 630 lines with 97 `unsafe` hits and 0 local
  markers,
  `src/platform/mod.rs` at 855 lines with 75 `unsafe` hits and 0 local
  markers,
  `src/internal/atomic.rs` at 914 lines with 135 `unsafe` hits and 0 local
  markers,
  `src/allocator/page_map.rs` at 1,052 lines with 184 `unsafe` hits and
  0 local markers, and
  `src/types/scalar.rs` at 1,097 lines with 33 `unsafe` hits and 0 local
  markers.
- The next smallest allowlisted production files after those are
  `src/allocator/thread_heap.rs` at 1,267 lines with 120 `unsafe` hits,
  `src/platform/unix.rs` at 1,371 lines with 143 `unsafe` hits,
  `src/platform/os.rs` at 1,614 lines with 200 `unsafe` hits,
  `src/types/ptr.rs` at 1,617 lines with 362 `unsafe` hits, and
  `src/allocator/free.rs` at 1,621 lines with 121 `unsafe` hits.
- `strict-type-allowlist.txt` has 280 entries, including 75 path-wide
  `symbol = *` entries. The largest categories are 150 raw-pointer-parameter
  entries, 105 bare-domain `usize` entries, 13 pointer/usize round-trip
  entries, 8 metadata-reference entries, 2 atomic-cast entries, and
  2 heap-allocating-type entries. `REMOVE_ENTRY_CAP` remains 0.
- The broadest path-wide strict-type clusters are
  `src/allocator/aligned.rs`, `src/allocator/stats.rs`,
  `src/allocator/thread_heap.rs`, `src/internal/mod.rs`,
  `src/platform/unix.rs`, and `src/platform/windows.rs`, each with
  4 path-wide entries.
- The largest unsafe audit marker clusters remain broad enough to split later:
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers,
  `SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers,
  `SAFETY[AUDIT:raw-posix-compat]` has 70 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:typed-atomic-domain]` has 51 markers,
  `SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers, and
  `SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 3,915 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The release audit is still split but has large review units:
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,386 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines,
  `tests/release_fast_path_audit.rs` is 1,869 lines, and
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,108 lines.

## Issue: macOS malloc-zone override still uses a file-level unsafe allowance

> **Comment:** `src/platform/macos.rs` is a source-present macOS primitive
> boundary. Its unsafe operations cover malloc-zone ABI callbacks, mutable
> process-global zone metadata, out-pointer writes, forwarding to non-mimalloc
> zones, ownership checks through `mi_any_heap_contains`, batch allocation/free
> pointer arrays, fork hook exports, and no-op compatibility shims.

- [ ] Add local `SAFETY[AUDIT:macos-zone-override]` markers for zone metadata
      access, macOS extern declarations, default-zone shims, out-pointer writes,
      forwarded zone function calls, mimalloc ownership checks, batch
      pointer-array reads/writes, fork hooks, and no-op compatibility shims.
- [ ] Preserve source-present macOS ABI shape, `mi_malloc_zone` layout and name,
      non-mimalloc zone forwarding behavior, null-zone handling, batch free
      pointer clearing, fork hook symbol names, and no new heap allocation in
      zone callbacks.
- [ ] Add or extend release-audit assertions for malloc-zone struct layout,
      interpose macro targets, default-zone return paths, forwarding branches,
      batch pointer clearing, fork hooks, and marker coverage without a
      migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/platform/macos.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: macOS/source-shape release audit, unsafe audit scripts, unsafe
inventory regeneration/check, full clippy, and full tests.

## Issue: Platform TLS and primitive glue still use a file-level unsafe allowance

> **Comment:** `src/platform/mod.rs` owns cross-platform primitive dispatch and
> thread-heap TLS access. Its unsafe operations cover C TLS shim externs,
> Linux x86_64 initial-exec assembly, Apple fixed-slot TLS reads, dynamic
> pthread/Win32 TLS key access, cached/default thread-heap set/get paths, page
> associated thread-heap lookup, and process attach/detach shims.

- [ ] Add local `SAFETY[AUDIT:platform-tls-primitives]` markers for TLS externs,
      Linux assembly loads/stores, Apple fixed-slot reads, dynamic TLS key
      lookup, default/cached thread-heap accessors, page-associated lookup, and
      process attach/detach primitive shims.
- [ ] Preserve `MI_TLS_MODEL_*` selection, `MI_THEAP_INITASNULL` behavior,
      Linux C TLS shim ABI, x86_64 `@GOTTPOFF` sequences, Apple slot constants,
      cached-thread-heap fast path, empty-theap fallback, and no allocation or
      dynamic dispatch in TLS get/set helpers.
- [ ] Add or extend release-audit assertions for TLS model constants, assembly
      symbols, C shim accessors, Apple fixed-slot path, dynamic key path,
      `_mi_heap_theap` cached lookup, `_mi_heap_theap_peek`, page-associated
      lookup, and marker coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove `src/platform/mod.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: `interpose_linux_malloc_override_uses_c_tls_fast_path`, lifecycle/local
boundary release audit, unsafe audit scripts, unsafe inventory
regeneration/check, full clippy, and full tests.

## Issue: Internal atomic facade still uses a file-level unsafe allowance

> **Comment:** `src/internal/atomic.rs` is the C-compatible atomic and lock
> facade. Its unsafe operations cover pthread mutex externs, raw atomic field
> views, memory-order mapping, compare-exchange helpers, pointer-sized atomic
> wrappers, volatile loads, guard/once entry shims, and Linux release-lock
> behavior.

- [ ] Add local `SAFETY[AUDIT:internal-atomic-facade]` markers for pthread
      extern calls, raw `Atomic*` views, memory-order mapping helpers,
      load/store/RMW/CAS operations, pointer atomic helpers, volatile reads,
      atomic guard/once shims, lock acquire/release/done/init paths, and release
      Linux pthread lock behavior.
- [ ] Preserve the public C-compatible `mi_memory_order_e` domain, ordering
      translations documented by `ATOMIC_ORDERING_AUDIT.md`, Linux optimized
      pthread mutex branch, debug/test atomic fallback, typed `AtomicGuard` and
      `OnceGuard` release-on-drop handoff, and no extra allocation or dynamic
      dispatch in atomic helpers.
- [ ] Add or extend release-audit assertions and focused tests for
      `mi_memory_order_e` mapping, raw `Atomic*::from_ptr` helper boundaries,
      pointer CAS/load/store wrappers, guard/once scope construction, pthread
      lock calls, and marker coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove `src/internal/atomic.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: `cargo test types::atomic --all-features`, atomic/order release audit,
unsafe audit scripts, unsafe inventory regeneration/check, full clippy, and
full tests.

## Issue: Page-map registration and lookup still use a file-level unsafe allowance

> **Comment:** `src/allocator/page_map.rs` owns allocator page registration,
> lookup, and teardown. Its unsafe operations cover process-global page-map
> state, OS reservation/commit/free, 32-bit bitmap commit tracking, 64-bit
> submap allocation and publication, lock-protected submap growth, page range
> registration/unregistration, `RegisteredPageRange` witnesses, and
> safe-pointer lookup.

- [ ] Add local `SAFETY[AUDIT:page-map-boundary]` markers for page-map globals,
      base pointer accessors, memid interpretation, page span/index helpers,
      commit tracking, OS reserve/commit/free paths, submap atomic publication,
      lock-protected submap growth, range set/clear/register/unregister, safe
      pointer lookup, and `mi_is_in_heap_region`.
- [ ] Preserve 32-bit and non-32-bit behavior, page-map max-address checks,
      acquire/release publication of committed submaps, lock scope,
      `RegisteredPageRange` construction, `PageMapIndex`/`PageMapSubIndex` and
      `SliceCount` domains, null/invalid pointer handling, and no allocation in
      safe lookup paths.
- [ ] Add or extend release-audit assertions and focused tests for page-map init,
      unsafe destroy, register/unregister/range-unregister, safe pointer lookup,
      submap publication atomics, 32-bit bitmap commit behavior, and marker
      coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/allocator/page_map.rs` from the unsafe migration allowlist,
      regenerate unsafe inventory, and lower the cap.

Verify: page-map/local-boundary release audit, pointer validation/free tests,
unsafe audit scripts, unsafe inventory regeneration/check, full clippy, and
full tests.

## Issue: Scalar domain wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/scalar.rs` is a strict-type domain module. Its unsafe
> operations are mostly unchecked newtype constructors for non-zero sizes,
> alignments, page-map indexes, bins, slice counts, arena indexes, and lint-touch
> fixtures. The file also contains a few duplicated or mechanical comments that
> make the wrapper surface feel less deliberately Rust-shaped than the newer
> typed modules.

- [ ] Add local `SAFETY[AUDIT:scalar-domain-wrappers]` markers for unchecked
      non-zero constructors, index/count/bin constructors, `SliceCount`
      const construction, lint-touch fixture construction, and `UsableOut`
      optional out-pointer writes.
- [ ] Preserve transparent layout, `Option` niche behavior, checked constructor
      domains, zero-size normalization, branchless hot-path helpers, no heap
      allocation, no dynamic dispatch, and existing scalar test coverage.
- [ ] Clean up duplicated/mechanical scalar comments only where directly tied to
      the audited wrapper surface, without broad refactoring or changing public
      crate-private names.
- [ ] Add or extend release-audit assertions and focused tests for scalar
      constructor boundaries, layout/niche expectations, `UsableOut`
      null/non-null writes, `SliceCount` overflow behavior, and marker coverage
      without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove `src/types/scalar.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: `cargo test types::scalar::tests --all-features`, datatype acceptance
release audit, unsafe audit scripts, unsafe inventory regeneration/check, full
clippy, and full tests.

## Suggested implementation order

1. [ ] Migrate `src/platform/macos.rs` off the unsafe migration allowlist.
2. [ ] Migrate `src/platform/mod.rs` off the unsafe migration allowlist.
3. [ ] Migrate `src/internal/atomic.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/allocator/page_map.rs` off the unsafe migration allowlist.
5. [ ] Migrate `src/types/scalar.rs` off the unsafe migration allowlist.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file.
- [ ] `src/platform/macos.rs`, `src/platform/mod.rs`,
      `src/internal/atomic.rs`, `src/allocator/page_map.rs`, and
      `src/types/scalar.rs` are covered by operation-level audit markers and
      evidence instead of file-level migration exceptions.
- [ ] Platform TLS/zone behavior, atomic ordering, page-map lookup, scalar
      wrapper layout, and allocator hot-path performance constraints remain
      unchanged.
- [ ] Release-audit TODO guards track the active backlog and measurable
      evidence.
