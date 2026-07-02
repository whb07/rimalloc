# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`9d63307`. The last pass migrated `src/platform/macos.rs`,
`src/platform/mod.rs`, `src/internal/atomic.rs`,
`src/allocator/page_map.rs`, and `src/types/scalar.rs` off the unsafe
migration allowlist while preserving platform ABI shape, TLS fast paths,
atomic ordering, page-map lookup, scalar wrapper layout, and allocator
hot-path constraints.

The next pass should keep the same discipline: retire one measurable exception
at a time, prefer local operation-level proof markers over file allowances, and
avoid refactors that change allocator ownership, synchronization, platform ABI,
or allocation fast paths.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 17 entries: 14 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 17.
- The completed pass moved `src/platform/macos.rs`, `src/platform/mod.rs`,
  `src/internal/atomic.rs`, `src/allocator/page_map.rs`, and
  `src/types/scalar.rs` to local `SAFETY[AUDIT:*]` operation markers with
  evidence rows and no migration allowlist entries.
- `src/types/constants.rs` is the smallest remaining allowlisted file at
  603 lines with 10 `unsafe` hits and 2 local markers. The sentinel raw
  `Sync` impls are already covered by `SAFETY[AUDIT:raw-auto-trait]`, but the
  raw `unsafe extern "C" fn` callback typedef declarations still rely on the
  file-level translation allowance.
- The smallest remaining allowlisted production files with no local markers are
  `src/allocator/thread_heap.rs` at 1,267 lines with 120 `unsafe` hits,
  `src/platform/unix.rs` at 1,371 lines with 143 `unsafe` hits,
  `src/platform/os.rs` at 1,614 lines with 200 `unsafe` hits,
  `src/types/ptr.rs` at 1,617 lines with 362 `unsafe` hits, and
  `src/allocator/free.rs` at 1,621 lines with 121 `unsafe` hits.
- The next allowlisted production files after those are
  `src/allocator/options.rs` at 1,715 lines with 78 `unsafe` hits,
  `src/allocator/aligned.rs` at 1,766 lines with 215 `unsafe` hits,
  `src/allocator/stats.rs` at 2,021 lines with 196 `unsafe` hits,
  `src/raw/macros.rs` at 2,223 lines with 381 `unsafe` hits, and
  `src/allocator/init.rs` at 2,339 lines with 336 `unsafe` hits.
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
  `SAFETY[AUDIT:page-map-boundary]` has 153 markers,
  `SAFETY[AUDIT:internal-atomic-facade]` has 103 markers,
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers,
  `SAFETY[AUDIT:macos-zone-override]` has 83 markers,
  `SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers,
  `SAFETY[AUDIT:raw-posix-compat]` has 70 markers,
  `SAFETY[AUDIT:platform-tls-primitives]` has 60 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:typed-atomic-domain]` has 51 markers,
  `SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers, and
  `SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 3,915 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The release audit is still split but has large review units:
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,416 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines,
  `tests/release_fast_path_audit.rs` is 1,899 lines, and
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,126 lines.

## Issue: Raw layout callback typedefs still use a file-level unsafe allowance

> **Comment:** `src/types/constants.rs` is the raw C layout substrate. Its
> remaining unmarked unsafe operations are ABI declarations: raw
> `unsafe extern "C" fn` callback typedefs used by raw/public C compatibility
> APIs. The sentinel `Sync` impls already have local `raw-auto-trait` markers,
> but the file remains allowlisted because the callback typedef declarations are
> not locally classified.

- [ ] Add local `SAFETY[AUDIT:raw-layout-abi-typedefs]` markers for deferred
      free, output, error, heap visit, block visit, commit, claim, and forall-set
      callback typedef declarations while retaining the existing
      `SAFETY[AUDIT:raw-auto-trait]` markers for sentinel `Sync` impls.
- [ ] Preserve `repr(C)` layout definitions, raw callback typedef names and
      signatures, documented nullability/lifetime/threading/reentrancy/unwind
      contracts, raw `mi_*` aliases, and the separation between raw typedefs
      and typed callback wrappers.
- [ ] Add or extend release-audit assertions for callback typedef marker
      coverage, raw typedef names, no typed wrapper leakage into
      `src/types/constants.rs`, sentinel-only `Sync` impls, and no migration
      allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/types/constants.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: constants/raw-layout tests, callback ABI release audit, unsafe audit
scripts, unsafe inventory regeneration/check, full clippy, and full tests.

## Issue: Thread-heap lifecycle still uses a file-level unsafe allowance

> **Comment:** `src/allocator/thread_heap.rs` owns thread-heap visitation,
> collection, default switching, initialization, creation, refcounting, free,
> delete, and random/arena-memory cleanup paths. These paths are already partly
> shaped by typed heap/thread-heap witnesses, but the file still hides all
> unsafe operations behind a dated migration entry.

- [ ] Add local `SAFETY[AUDIT:thread-heap-lifecycle]` markers for visitor
      callback invocation, area metadata initialization, fast-divisor out-pointer
      writes, page/block traversal, page validation/collection, stats merging,
      collect/default-switching shims, thread-heap init/create/free/delete,
      refcount updates, random next, and arena-memory cleanup.
- [ ] Preserve `ThreadHeapLocal`, `ThreadHeapPtr`, `HeapPtr`, `TldPtr`, and
      callback wrapper boundaries; default-thread-heap semantics; lock ordering;
      refcount acquire/release behavior; page ownership checks; and no heap
      allocation or dynamic dispatch on lifecycle hot paths.
- [ ] Add or extend release-audit assertions and focused tests for area/block
      visitation, page validation/collection, stats merge, collect/default
      switching, init/create/delete/free ordering, refcount transitions, and
      marker coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove
      `src/allocator/thread_heap.rs` from the unsafe migration allowlist,
      regenerate unsafe inventory, and lower the cap.

Verify: thread-heap/lifecycle release audit, focused thread-heap tests, unsafe
audit scripts, unsafe inventory regeneration/check, full clippy, and full tests.

## Issue: Unix primitive backend still uses a file-level unsafe allowance

> **Comment:** `src/platform/unix.rs` is the Unix primitive backend. Its unsafe
> operations cover libc/syscall imports, overcommit and transparent huge-page
> detection, `mmap`/`munmap`/`madvise`/`mprotect`, huge-page and NUMA probes,
> process info, stderr writes, environment lookup, random-byte fallback, TLS
> auto-done hooks, and scheduler yield.

- [ ] Add local `SAFETY[AUDIT:unix-primitive-backend]` markers for extern
      declarations, libc wrappers, proc/sys file reads, sysconf memory sizing,
      mmap allocation/free/commit/decommit/reset/protect paths, huge-page and
      NUMA syscalls, process-info writes, stderr output, environment scanning,
      random source fallbacks, pthread TLS destructor registration, and yield.
- [ ] Preserve Linux/macOS/BSD cfg shape, syscall parity, `MAP_FAILED` handling,
      overcommit/THP detection, aligned mmap hint behavior, large-page fallback,
      preloading safeguards, bounded environment scans, partial-write handling,
      random-byte fallback ordering, TLS destructor semantics, and no allocator
      allocation inside primitive helpers.
- [ ] Add or extend release-audit assertions and parity checks for Unix syscall
      shape, mmap/free/commit/decommit/reset/protect behavior, NUMA and process
      info probes, environment/random helpers, TLS auto-done hooks, and marker
      coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove `src/platform/unix.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: Unix primitive release audit, Unix syscall parity script where
available, unsafe audit scripts, unsafe inventory regeneration/check, full
clippy, and full tests.

## Issue: OS memory boundary still uses a file-level unsafe allowance

> **Comment:** `src/platform/os.rs` bridges typed allocator policy to primitive
> OS memory operations. Its unsafe operations cover global memory config,
> page-size and alignment queries, aligned hint generation, guard pages, commit,
> decommit, reset, reuse, purge, protect/unprotect, free, aligned allocation,
> zeroing, huge pages, NUMA, and raw test/export shims.

- [ ] Add local `SAFETY[AUDIT:os-memory-boundary]` markers for memory config
      access, subproc stats projection, page-size/large-page/good-size queries,
      aligned-hint randomization, guard-page commit/decommit callbacks, OS
      commit/decommit/reset/reuse/purge/protect/free paths, aligned allocation
      and trimming, zero-state publication, huge-page claim/free, NUMA helpers,
      and raw public OS shims.
- [ ] Preserve memid commit/zero/pinned semantics, over-allocation trimming,
      partial-free behavior, subproc stat accounting, arena commit callback
      handling, out-pointer writes, alignment/page-size invariants, huge-page
      fallback behavior, and no allocator recursion from OS memory helpers.
- [ ] Add or extend release-audit assertions and focused tests for OS config,
      page sizes, aligned hints, guard pages, commit/decommit/reset/reuse/purge,
      aligned allocation/free, memid zeroing, huge-page/NUMA behavior, and
      marker coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove `src/platform/os.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: OS memory release audit, allocation boundary tests, unsafe audit
scripts, unsafe inventory regeneration/check, full clippy, and full tests.

## Issue: Typed pointer accessors still use a file-level unsafe allowance

> **Comment:** `src/types/ptr.rs` is a typed pointer/accessor substrate, but it
> still has one of the densest unsafe surfaces in the crate. The unsafe
> operations cover non-null wrapper construction, raw field reads/writes, atomic
> field projections, encoded free-list links, page/heap/thread-heap/TLD/arena
> and subproc accessors, page-queue accessors, exposed-address checks, and
> lint-touch fixtures.

- [ ] Add local `SAFETY[AUDIT:typed-pointer-accessors]` markers for pointer
      wrapper constructors, field reads/writes, atomic cell projections,
      encoded free-list link reads/writes, page start and contains-user
      arithmetic, heap/thread-heap/TLD/arena/subproc/page-queue accessors,
      count/index scalar narrowing, lock pointer projections, and lint-touch
      fixture function-pointer references.
- [ ] Preserve `repr(transparent)` pointer wrapper layout, `Option` niche
      behavior, typed scalar validation, provenance helper use, free-list
      encoding contracts, atomic field ordering boundaries, raw metadata
      ownership rules, no heap allocation, and no dynamic dispatch.
- [ ] Add or extend release-audit assertions and focused tests for pointer
      wrapper layout/niche behavior, representative page/block/heap/theap/TLD/
      arena/subproc/page-queue accessors, encoded-link helpers, exposed-address
      helpers, and marker coverage without a migration allowlist entry.
- [ ] Add matching unsafe audit/evidence entries, remove `src/types/ptr.rs` from
      the unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::ptr::tests --all-features`, datatype acceptance
release audit, unsafe audit scripts, unsafe inventory regeneration/check, full
clippy, and full tests.

## Suggested implementation order

1. [ ] Migrate `src/types/constants.rs` off the unsafe migration allowlist.
2. [ ] Migrate `src/allocator/thread_heap.rs` off the unsafe migration
       allowlist.
3. [ ] Migrate `src/platform/unix.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/platform/os.rs` off the unsafe migration allowlist.
5. [ ] Migrate `src/types/ptr.rs` off the unsafe migration allowlist.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file.
- [ ] `src/types/constants.rs`, `src/allocator/thread_heap.rs`,
      `src/platform/unix.rs`, `src/platform/os.rs`, and `src/types/ptr.rs` are
      covered by operation-level audit markers and evidence instead of
      file-level migration exceptions.
- [ ] Raw callback typedefs, thread-heap lifecycle behavior, Unix primitive
      behavior, OS memory semantics, pointer wrapper layout, and allocator
      hot-path performance constraints remain unchanged.
- [ ] Release-audit TODO guards track the active backlog and measurable
      evidence.
