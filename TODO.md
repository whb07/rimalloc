# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`10995d2`. The last pass migrated MemId views, layout requests, page-state
witnesses, and benchmark diagnostics off the unsafe migration allowlist while
preserving C ABI shape, transparent typed wrappers, relaxed diagnostic
counters, and allocator hot-path behavior.

The next pass should keep the same discipline: retire one measurable exception
at a time, prefer local operation-level proof markers over file allowances, and
avoid refactors that change allocator ownership, synchronization, platform, or
ABI behavior.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 29 entries: 26 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 29.
- The completed pass moved `src/types/memid.rs`, `src/types/layout.rs`,
  `src/types/page_states.rs`, and `src/allocator/bench_diagnostics.rs` to local
  `SAFETY[AUDIT:*]` operation markers with evidence rows and no migration
  allowlist entries.
- `src/allocator/arena_meta.rs` now uses 16
  `SAFETY[AUDIT:arena-meta-allocator]` markers for meta memid construction,
  page/block pointer arithmetic, meta-page stack traversal and publication,
  bitmap allocation/free, arena fallback, and meta-page debug classification,
  and is off the unsafe migration allowlist.
- `src/platform/wasi.rs` now uses 42
  `SAFETY[AUDIT:wasi-primitive-boundary]` markers for C imports, `sbrk` memory
  growth, nullable primitive out-pointers, C-string stderr/env handling, no-op
  primitive range shims, and thread/TLS compatibility shims, and is off the
  unsafe migration allowlist.
- The smallest remaining allowlisted production files are
  `src/raw/posix.rs` at 329 lines with 81 `unsafe` hits and 0 local markers,
  `src/lib.rs` at 398 lines with 21 `unsafe` hits and 8 local markers,
  `src/platform/emscripten.rs` at 424 lines with 35 `unsafe` hits and 0 local
  markers, and
  `src/ffi/libc.rs` at 518 lines with 46 `unsafe` hits and 0 local markers.
- `src/ffi/abi.rs` now uses 1
  `SAFETY[AUDIT:ffi-abi-declarations]` marker for its raw C declaration block,
  preserves the existing C-header macro markers, and is off the unsafe
  migration allowlist.
- `src/types/atomic.rs` remains the next typed-wrapper domain at 755 lines with
  101 `unsafe` hits and 0 local markers.
- `strict-type-allowlist.txt` still has 75 path-wide entries with `symbol = *`,
  including 29 path-wide raw-pointer entries. `REMOVE_ENTRY_CAP` remains 0.
- The largest strict-type path clusters remain broad enough to revisit later:
  `src/allocator/page.rs` has 54 entries,
  `src/internal/bitmap.rs` has 33 entries,
  `src/allocator/arena.rs` has 24 entries,
  `src/internal/mod.rs` has 19 entries, and
  `src/allocator/aligned.rs` has 19 entries.
- The largest unsafe audit marker clusters remain broad enough to split later:
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:typed-page-state-domain]` has 36 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 34 markers, and
  `SAFETY[AUDIT:owned-allocation]` has 31 markers.
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
  `tests/release_fast_path_audit.rs` is 1,832 lines, and
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,103 lines.

## Issue: FFI ABI declarations still use a file-level unsafe allowance

> **Comment:** `src/ffi/abi.rs` is a raw C header declaration module. It already
> documents nullable pointer shape and has macro-boundary markers, but the
> `unsafe extern "C"` declaration block still keeps the whole file on the
> migration allowlist.

- [x] Add a local `SAFETY[AUDIT:ffi-abi-declarations]` marker for the raw C
      declaration block and keep it scoped to declaration-only FFI, not
      allocator implementation behavior.
- [x] Preserve the public raw C signatures exactly, including nullable pointer
      parameters, raw returns, and header macro expansion shape.
- [x] Add matching unsafe audit/evidence entries, remove `src/ffi/abi.rs` from
      the unsafe migration allowlist, regenerate unsafe inventory, and lower
      `ALLOWLIST_ENTRY_CAP`.

Verify: unsafe audit scripts, unsafe inventory regeneration/check, release
audit, full clippy, and full tests.

## Issue: Arena metadata allocator still uses a file-level unsafe allowance

> **Comment:** `src/allocator/arena_meta.rs` is small and already routes meta
> allocation identity through `MemId::<MetaMem>`. Its unsafe operations are
> localized to meta-page pointer arithmetic, bitmap allocation/free, atomic
> stack publication, arena fallback, and debug classification.

- [x] Add local `SAFETY[AUDIT:arena-meta-allocator]` markers for meta memid
      construction, page/block pointer arithmetic, meta-page stack traversal and
      publication, bitmap allocation/free, arena fallback, and meta-page debug
      classification.
- [x] Preserve `Acquire`/`AcqRel` ordering, `NonZeroU32` block-count domains,
      typed `MemId::<MetaMem>` construction, and allocation-free metadata paths.
- [x] Use existing release-audit coverage or add focused tests for meta memid
      construction, free/reuse invariants, and meta-page classification.
- [x] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: relevant arena metadata or release-audit tests, unsafe audit scripts,
unsafe inventory regeneration/check, release audit, full clippy, and full
tests.

## Issue: WASI primitive backend still uses a file-level unsafe allowance

> **Comment:** `src/platform/wasi.rs` is a bounded primitive backend, not a
> general allocator hot path. Its unsafe operations are platform imports,
> `sbrk`-style memory growth, nullable out-pointer writes, C-string environment
> lookup, stderr output, and thread-heap association shims.

- [x] Add local `SAFETY[AUDIT:wasi-primitive-boundary]` markers for WASI import
      declarations, memory growth/alignment, primitive allocation out-pointers,
      C-string stderr/env handling, and thread/TLS compatibility shims.
- [x] Preserve non-WASI fallback behavior, no-heap-allocation primitive paths,
      current errno-style return values, and existing `#[inline]` boundaries.
- [x] Add release-audit assertions or targeted tests for null out-pointers,
      non-WASI fallbacks, stderr/env bounds, and thread association behavior.
- [x] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: platform/runtime-gate release audit, unsafe audit scripts, unsafe
inventory regeneration/check, release audit, full clippy, and full tests.

## Issue: Emscripten primitive backend still uses a file-level unsafe allowance

> **Comment:** `src/platform/emscripten.rs` mirrors the WASI primitive backend
> but adds Emscripten allocation imports, clock/entropy calls, console output,
> pthread key lifecycle, and a fallback thread-local cell for non-Emscripten
> builds.

- [ ] Add local `SAFETY[AUDIT:emscripten-primitive-boundary]` markers for
      Emscripten import declarations, emmalloc calls, clock/entropy calls,
      console/C-string output, pthread key lifecycle, static key mutation, and
      fallback thread-local association.
- [ ] Preserve Emscripten-target behavior, non-Emscripten fallback behavior,
      errno-style returns, and no extra allocation or dynamic dispatch in
      primitive paths.
- [ ] Add release-audit assertions or targeted tests for null out-pointers,
      fallback allocation failure, clock fallback, and thread-key lifecycle.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: platform/runtime-gate release audit, unsafe audit scripts, unsafe
inventory regeneration/check, release audit, full clippy, and full tests.

## Issue: Atomic typed wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/atomic.rs` is a typed wrapper boundary for atomic
> pointer fields, thread-free words, abandoned counts, once guards, and lock
> words. It is larger than the other targets, but it is a Rust domain boundary
> and should not depend on a blanket migration exception indefinitely.

- [ ] Add local `SAFETY[AUDIT:typed-atomic-domain]` markers for atomic pointer
      field projection, abandoned-count/page-thread-id wrappers, thread-free
      head/owner transitions, once/lock guard construction, and release/acquire
      ownership handoffs.
- [ ] Preserve documented atomic orderings from `ATOMIC_ORDERING_AUDIT.md`,
      transparent wrapper layout, no allocation, and no broader synchronization
      primitives on allocator hot paths.
- [ ] Keep or extend the existing `src/types/atomic/tests.rs` and concurrency
      model coverage for racing remote pushes, ownership claims, and collect
      behavior.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::atomic::tests --lib --all-features`, atomic
concurrency model tests, unsafe audit scripts, unsafe inventory
regeneration/check, release audit, full clippy, and full tests.

## Suggested implementation order

1. [x] Migrate `src/ffi/abi.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/allocator/arena_meta.rs` off the unsafe migration allowlist.
3. [x] Migrate `src/platform/wasi.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/platform/emscripten.rs` off the unsafe migration allowlist.
5. [ ] Migrate `src/types/atomic.rs` off the unsafe migration allowlist.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file.
- [ ] `src/ffi/abi.rs`, `src/allocator/arena_meta.rs`,
      `src/platform/wasi.rs`, `src/platform/emscripten.rs`, and
      `src/types/atomic.rs` are covered by operation-level audit markers and
      evidence instead of file-level migration exceptions.
- [ ] C ABI declaration shape, platform fallback behavior, atomic ordering, and
      allocator hot-path performance constraints remain unchanged.
- [ ] Release-audit TODO guards track the active backlog and measurable
      evidence.
