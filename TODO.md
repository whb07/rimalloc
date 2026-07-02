# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`7d64cc0`. The last pass migrated `src/ffi/abi.rs`,
`src/allocator/arena_meta.rs`, `src/platform/wasi.rs`,
`src/platform/emscripten.rs`, and `src/types/atomic.rs` off the unsafe
migration allowlist while preserving ABI shape, platform fallback behavior,
atomic ordering, and allocator hot-path constraints.

The next pass should keep the same discipline: retire one measurable exception
at a time, prefer local operation-level proof markers over file allowances, and
avoid refactors that change allocator ownership, synchronization, platform ABI,
or allocation fast paths.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 22 entries: 19 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 22.
- The completed pass moved `src/ffi/abi.rs`, `src/allocator/arena_meta.rs`,
  `src/platform/wasi.rs`, `src/platform/emscripten.rs`, and
  `src/types/atomic.rs` to local `SAFETY[AUDIT:*]` operation markers with
  evidence rows and no migration allowlist entries.
- `src/raw/posix.rs` now uses 70 `SAFETY[AUDIT:raw-posix-compat]` markers for
  errno publication, allocation facade forwarding, POSIX out-pointer writes,
  `reallocarr` pointer-slot updates, wide-string duplication, environment
  duplication, and aligned/product allocation shims, and is off the unsafe
  migration allowlist.
- `src/lib.rs` now uses 13 crate-root `SAFETY[AUDIT:global-alloc]` and
  `SAFETY[AUDIT:panic-boundary]` markers for `GlobalAlloc` trait methods,
  allocation forwarding blocks, the no-std panic handler, the abort extern, and
  the personality export, and is off the unsafe migration allowlist.
- `src/ffi/libc.rs` now uses 50 `SAFETY[AUDIT:ffi-libc-helpers]` markers for
  bounded C-string reads, destination writes, `vsnprintf` forwarding,
  fixed-buffer formatting, number-output reversal, environment forwarding, and
  helper edge-case tests, and is off the unsafe migration allowlist.
- `src/allocator/threadlocal.rs` now uses 50
  `SAFETY[AUDIT:threadlocal-key-manager]` markers for Linux TLS shim access,
  x86_64 assembly, sentinel `Sync`, static lock access, slot growth, bitmap
  copy/free/publication, keep-set writes, versioned key claim/free, and helper
  tests, and is off the unsafe migration allowlist.
- `src/allocator/heap.rs` now uses 80
  `SAFETY[AUDIT:heap-lifecycle-boundary]` markers for raw heap shims, heap
  creation and subproc registration, thread-heap lazy initialization, heap
  delete/destroy, stats merge, ownership lookup, under-utilized page
  inspection, and lock-protected teardown, and is off the unsafe migration
  allowlist.
- The smallest remaining allowlisted production files are
  `src/platform/macos.rs` at 630 lines with 97 `unsafe` hits and 0 local
  markers,
  `src/platform/mod.rs` at 855 lines with 75 `unsafe` hits,
  `src/internal/atomic.rs` at 914 lines with 135 `unsafe` hits, and
  `src/allocator/page_map.rs` at 1,052 lines with 184 `unsafe` hits.
- The next smallest remaining allowlisted production files after those are
  `src/types/scalar.rs` at 1,097 lines with 33 `unsafe` hits.
- `strict-type-allowlist.txt` still has 75 path-wide entries with `symbol = *`,
  including 29 path-wide raw-pointer entries, 21 path-wide bare-domain `usize`
  entries, 13 path-wide pointer/usize round-trip entries, 8 path-wide metadata
  reference entries, 2 path-wide atomic-cast entries, and 2 path-wide
  heap-allocating-type entries. `REMOVE_ENTRY_CAP` remains 0.
- The largest strict-type path clusters remain broad enough to revisit later:
  `src/allocator/page.rs` has 54 entries,
  `src/internal/bitmap.rs` has 33 entries,
  `src/allocator/arena.rs` has 24 entries,
  `src/internal/mod.rs` has 19 entries, and
  `src/allocator/aligned.rs` has 19 entries.
- The largest unsafe audit marker clusters remain broad enough to split later:
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers,
  `SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers,
  `SAFETY[AUDIT:raw-posix-compat]` has 70 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:typed-atomic-domain]` has 51 markers,
  `SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers,
  `SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers, and
  `SAFETY[AUDIT:emscripten-primitive-boundary]` has 48 markers.
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

## Issue: Raw POSIX compatibility facade still uses a file-level unsafe allowance

> **Comment:** `src/raw/posix.rs` is a C/POSIX compatibility facade. Its unsafe
> operations are narrow but varied: errno writes, environment lookup,
> allocation-family forwarding, POSIX out-pointer writes, `reallocarr` pointer
> slot updates, wide-string scans/copies, and environment duplication.

- [x] Add local `SAFETY[AUDIT:raw-posix-compat]` markers for errno access,
      allocation facade forwarding, POSIX out-pointer writes, `reallocarr`
      pointer-slot updates, wide-string duplication, and environment string
      duplication.
- [x] Preserve `EINVAL`/`ENOMEM`/`EOVERFLOW` behavior, null-pointer handling,
      alignment checks, overflow checks, exported symbol shape, and existing
      allocation/free ownership contracts.
- [x] Add or extend release-audit assertions and focused tests for invalid
      `mi_posix_memalign`, overflowed `mi_reallocarray`, zero-product
      `mi_reallocarr`, wide-string duplication, and `mi_dupenv_s` out-pointer
      behavior.
- [x] Add matching unsafe audit/evidence entries, remove `src/raw/posix.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test --test compat_exports --all-features`, header/release
audit, unsafe audit scripts, unsafe inventory regeneration/check, full clippy,
and full tests.

## Issue: Crate root still uses a file-level unsafe allowance

> **Comment:** `src/lib.rs` had local markers for `GlobalAlloc` and the no-std
> panic boundary, but still needed crate-root unsafe-attribute and extern
> coverage without widening the public Rust API or adding new compatibility
> aliases.

- [x] Cover every crate-root unsafe operation with the existing
      `SAFETY[AUDIT:global-alloc]` or `SAFETY[AUDIT:panic-boundary]` operation
      IDs, adding a new ID only if a distinct crate-root boundary is truly
      needed.
- [x] Preserve the curated public facade, crate-private translated aliases,
      `GlobalAlloc` null-on-failure behavior, over-alignment routing, and
      no-std Linux abort behavior.
- [x] Keep release-audit checks that implementation modules stay private, raw
      calls remain unsafe to downstream users, and the panic/no-unwind policy is
      unchanged.
- [x] Add matching unsafe audit/evidence entries if needed, remove `src/lib.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: rustdoc/compile-fail release audit coverage, unsafe audit scripts,
unsafe inventory regeneration/check, release audit, full clippy, and full tests.

## Issue: libc string and formatting helpers still use a file-level unsafe allowance

> **Comment:** `src/ffi/libc.rs` owns libc-like helper routines. Unsafe is
> limited to a Unix `vsnprintf` declaration/call, bounded C-string scanning,
> `strlcpy`/`strlcat` copies, fixed-buffer formatting writes, and environment
> primitive forwarding.

- [x] Add local `SAFETY[AUDIT:ffi-libc-helpers]` markers for C-string reads,
      bounded destination writes, `vsnprintf` forwarding, fixed-buffer format
      writes, pointer-distance reversal, and environment lookup forwarding.
- [x] Preserve no-heap-allocation helper behavior, nul-termination guarantees,
      existing `#[inline]`/`#[inline(never)]` boundaries, used symbol keepers,
      and platform fallback behavior.
- [x] Add or extend release-audit assertions or focused tests for null inputs,
      truncating copy/cat behavior, guaranteed trailing nul writes, and
      Unix/non-Unix `vsnprintf` behavior.
- [x] Add matching unsafe audit/evidence entries, remove `src/ffi/libc.rs` from
      the unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: libc helper/release-audit tests, unsafe audit scripts, unsafe inventory
regeneration/check, release audit, full clippy, and full tests.

## Issue: Dynamic thread-local key manager still uses a file-level unsafe allowance

> **Comment:** `src/allocator/threadlocal.rs` is a synchronization and platform
> boundary. Its unsafe operations covered Linux TLS externs and x86_64 assembly,
> sentinel `Sync`, static lock access, TLS slot allocation/reallocation/free,
> bitmap publication, callback keep-set writes, and lock-protected key
> lifecycle operations.

- [x] Add local `SAFETY[AUDIT:threadlocal-key-manager]` markers for TLS
      extern/assembly access, sentinel `Sync`, static lock access, thread-local
      pointer get/set, slot growth and writes, bitmap copy/free/publication,
      callback keep-set writes, and key claim/free operations.
- [x] Preserve documented memory orderings from `docs/memory-ordering.md`, the
      Linux TLS shim ABI, direct `mi_free` thread-done behavior, lock scope,
      and no extra heap allocation on hot TLS get paths.
- [x] Add or extend release-audit assertions and focused tests for TLS shim
      shape, initialization/done locking, slot expansion, key version checks,
      bitmap claim/free behavior, and thread-done reset semantics.
- [x] Add matching unsafe audit/evidence entries, remove
      `src/allocator/threadlocal.rs` from the unsafe migration allowlist,
      regenerate unsafe inventory, and lower the cap.

Verify: threadlocal boundary release audit, `cargo test --test
thread_stats_regression --all-features`, unsafe
audit scripts, unsafe inventory regeneration/check, full clippy, and full
tests.

## Issue: Heap lifecycle helpers still use a file-level unsafe allowance

> **Comment:** `src/allocator/heap.rs` is a heap ownership boundary. It already
> uses `HeapPtr`, `BorrowedHeap`, `OwnedHeap`, `MainHeap`, `PagePtr`, and
> `ThreadHeapPtr` witnesses in several paths, but raw heap creation,
> registration, deletion, destruction, ownership inspection, stats merging, and
> lazy thread-heap initialization remained covered by a blanket allowance.

- [x] Add local `SAFETY[AUDIT:heap-lifecycle-boundary]` markers for raw heap
      shims, heap creation and subproc list insertion, thread-heap lazy
      initialization, heap deletion/destruction, stats merge, ownership lookup,
      under-utilized page inspection, and lock-protected teardown.
- [x] Preserve typed heap witnesses, main-heap rejection, thread-local slot
      lifecycle, heap/subproc list locking, stats updates, NUMA affinity
      behavior, and no new dynamic dispatch or extra allocation on lookup paths.
- [x] Add or extend release-audit assertions and focused tests for
      `mi_heap_new_in_arena`, `mi_heap_delete`, `mi_heap_destroy`,
      `mi_heap_check_owned`, under-utilized page inspection, and lazy thread
      heap initialization.
- [x] Add matching unsafe audit/evidence entries, remove
      `src/allocator/heap.rs` from the unsafe migration allowlist, regenerate
      unsafe inventory, and lower the cap.

Verify: heap lifecycle/stat export tests, no-allocation critical-path tests,
release audit, unsafe audit scripts, unsafe inventory regeneration/check, full
clippy, and full tests.

## Suggested implementation order

1. [x] Migrate `src/raw/posix.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/lib.rs` off the unsafe migration allowlist.
3. [x] Migrate `src/ffi/libc.rs` off the unsafe migration allowlist.
4. [x] Migrate `src/allocator/threadlocal.rs` off the unsafe migration
       allowlist.
5. [x] Migrate `src/allocator/heap.rs` off the unsafe migration allowlist.

## Definition of done for this backlog

- [x] The unsafe migration allowlist cap is lowered after each migrated file.
- [x] `src/raw/posix.rs`, `src/lib.rs`, `src/ffi/libc.rs`,
      `src/allocator/threadlocal.rs`, and `src/allocator/heap.rs` are covered by
      operation-level audit markers and evidence instead of file-level migration
      exceptions.
- [x] POSIX/libc compatibility behavior, crate-root public API shape,
      thread-local synchronization, heap lifecycle ownership, and allocator
      hot-path performance constraints remain unchanged.
- [x] Release-audit TODO guards track the active backlog and measurable
      evidence.
